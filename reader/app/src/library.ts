import { invoke } from "@tauri-apps/api/core";
import { validateUserStylesheet } from "../../web/user-stylesheet.mjs";

export interface LibraryBook {
  id: string;
  title: string;
  authors: string[];
  hasCover: boolean;
  importedAt: number;
  prepared: boolean;
}

export interface ImportFailure {
  code: string;
}

export interface ImportReport {
  books: LibraryBook[];
  failures: ImportFailure[];
}

export interface StartupImport {
  bookId: string | null;
  failed: boolean;
}

export type LibraryViewMode = "default" | "progress" | "title" | "author";

export interface BatchRemoveResult {
  books: LibraryBook[] | null;
  removedIds: string[];
  remainingIds: string[];
  pendingRecovery?: boolean;
}

export interface BrowserStateRecord {
  key: string;
  value: string;
}

export interface BrowserState {
  schema: 1;
  records: BrowserStateRecord[];
}

export interface PendingLocalDataRestore {
  token: string;
  browserState: BrowserState;
  rollback: boolean;
}

export interface StorageUsage {
  booksBytes: number;
  cacheBytes: number;
  messagesBytes: number;
  dictionariesBytes: number;
  preferencesBytes: number;
  totalBytes: number;
}

export interface RecentReading {
  book: LibraryBook;
  durationMs: number;
  lastReadDate: string;
}

export interface ReadingMemoryJump {
  messageId: string;
  rootMessageId: string;
  conversationId: string;
  editionId: string;
  canonicalLocator: string;
}

interface PendingBookDeletion {
  id: string;
}

interface ReaderLaunch {
  href: string;
}

type StorageReader = Pick<Storage, "getItem">;
type StorageAccess = Pick<Storage, "getItem" | "setItem" | "removeItem" | "key" | "length">;

const MAX_STATE_LENGTH = 524_288;
const MAX_LOCATOR_LENGTH = 2_048;
const MAX_TEXT_OFFSET = 2_147_483_647;
const MAX_BROWSER_STATE_BYTES = 16 * 1024 * 1024;
const MAX_BROWSER_RECORDS = 10_000;
const DEFAULT_BOOK_PREFERENCES = {
  sourceStyles: true,
  userStylesEnabled: true,
  readingMode: "paged",
  pageMargin: "standard",
  paragraphIndent: "none",
  paragraphSpacing: "book",
  styleModules: [],
};
const collator = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });

export const libraryAvailable =
  typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

function normalized(value: string): string {
  return value.normalize("NFKC").toLocaleLowerCase();
}

function exactKeys(value: object, expected: string[]): boolean {
  const keys = Object.keys(value).sort();
  const wanted = [...expected].sort();
  return keys.length === wanted.length && keys.every((key, index) => key === wanted[index]);
}

function validPoint(value: unknown): value is { section: string; offset: number } {
  return Boolean(
    value &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      exactKeys(value, ["offset", "section"]) &&
      typeof Reflect.get(value, "section") === "string" &&
      /^[a-z0-9][a-z0-9._-]{0,63}$/.test(Reflect.get(value, "section") as string) &&
      Number.isInteger(Reflect.get(value, "offset")) &&
      (Reflect.get(value, "offset") as number) >= 0 &&
      (Reflect.get(value, "offset") as number) <= MAX_TEXT_OFFSET,
  );
}

function parseLocator(serialized: unknown): {
  schema: 1;
  contentVersion: string | null;
  start: { section: string; offset: number };
  end?: { section: string; offset: number };
} | null {
  if (
    typeof serialized !== "string" ||
    serialized.length === 0 ||
    serialized.length > MAX_LOCATOR_LENGTH
  ) {
    return null;
  }
  try {
    const value: unknown = JSON.parse(serialized);
    if (!value || typeof value !== "object" || Array.isArray(value)) return null;
    const hasEnd = Object.hasOwn(value, "end");
    if (!exactKeys(value, hasEnd ? ["contentVersion", "end", "schema", "start"] : ["contentVersion", "schema", "start"])) {
      return null;
    }
    const contentVersion = Reflect.get(value, "contentVersion");
    if (
      Reflect.get(value, "schema") !== 1 ||
      (contentVersion !== null &&
        (typeof contentVersion !== "string" || !/^[a-f0-9]{64}$/.test(contentVersion)))
    ) {
      return null;
    }
    const rawStart = Reflect.get(value, "start");
    if (!validPoint(rawStart)) return null;
    const start = { section: rawStart.section, offset: rawStart.offset };
    if (!hasEnd) return { schema: 1, contentVersion, start };
    const rawEnd = Reflect.get(value, "end");
    if (!validPoint(rawEnd) || rawEnd.section !== start.section || rawEnd.offset < start.offset) {
      return null;
    }
    const end = { section: rawEnd.section, offset: rawEnd.offset };
    return { schema: 1, contentVersion, start, end };
  } catch {
    return null;
  }
}

function validLocator(serialized: unknown, contentVersion: string): boolean {
  return parseLocator(serialized)?.contentVersion === contentVersion;
}

function validProgress(raw: string | null, contentVersion: string): boolean {
  if (raw === null || raw.length > MAX_STATE_LENGTH) return false;
  try {
    const value: unknown = JSON.parse(raw);
    return Boolean(
      value &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        exactKeys(value, ["contentVersion", "locator", "schema"]) &&
        Reflect.get(value, "schema") === 1 &&
        Reflect.get(value, "contentVersion") === contentVersion &&
        validLocator(Reflect.get(value, "locator"), contentVersion),
    );
  } catch {
    return false;
  }
}

export function filterLibraryBooks(
  books: LibraryBook[],
  query: string,
  view: LibraryViewMode,
): LibraryBook[] {
  const term = normalized(query.trim());
  const result = term
    ? books.filter((book) =>
        normalized(`${book.title}\n${book.authors.join("\n")}`).includes(term),
      )
    : [...books];
  if (view === "title") {
    result.sort((left, right) => collator.compare(left.title, right.title));
  } else if (view === "author") {
    result.sort((left, right) => {
      const leftAuthor = left.authors.join(" / ");
      const rightAuthor = right.authors.join(" / ");
      if (!leftAuthor) return rightAuthor ? 1 : 0;
      if (!rightAuthor) return -1;
      return collator.compare(leftAuthor, rightAuthor);
    });
  }
  return result;
}

export function readStartedBookIds(
  books: LibraryBook[],
  storage?: StorageReader | null,
): Set<string> | null {
  try {
    const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
    if (!target) return null;
    const result = new Set<string>();
    for (const book of books) {
      if (!/^[a-f0-9]{64}$/.test(book.id)) continue;
      const key = `atha.reader.progress.${book.id.slice(0, 16)}.v1`;
      if (validProgress(target.getItem(key), book.id)) result.add(book.id);
    }
    return result;
  } catch {
    return null;
  }
}

export function groupLibraryBooksByProgress(books: LibraryBook[], startedIds: Set<string>) {
  return {
    reading: books.filter((book) => startedIds.has(book.id)),
    unread: books.filter((book) => !startedIds.has(book.id)),
  };
}

export function readRecentBooks(
  books: LibraryBook[],
  storage?: StorageReader | null,
): RecentReading[] | null {
  try {
    const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
    if (!target) return null;
    const raw = target.getItem("atha.reader.statistics.v1");
    if (raw === null) return [];
    if (!validStorageRecord("atha.reader.statistics.v1", raw)) return null;
    const statistics = JSON.parse(raw) as {
      books: { contentVersion: string; durationMs: number; lastReadDate: string }[];
    };
    const byId = new Map(books.map((book) => [book.id, book]));
    return statistics.books
      .flatMap((entry) => {
        const book = byId.get(entry.contentVersion);
        return book ? [{ book, durationMs: entry.durationMs, lastReadDate: entry.lastReadDate }] : [];
      })
      .sort(
        (left, right) =>
          right.lastReadDate.localeCompare(left.lastReadDate) ||
          left.book.id.localeCompare(right.book.id),
      );
  } catch {
    return null;
  }
}

export function validateLocalDataState(state: BrowserState): boolean {
  if (state.schema !== 1 || !Array.isArray(state.records) || state.records.length > MAX_BROWSER_RECORDS) {
    return false;
  }
  let previous = "";
  for (const record of state.records) {
    if (
      !record ||
      typeof record !== "object" ||
      !exactKeys(record, ["key", "value"]) ||
      typeof record.key !== "string" ||
      typeof record.value !== "string" ||
      record.key <= previous ||
      !validStorageRecord(record.key, record.value)
    ) {
      return false;
    }
    previous = record.key;
  }
  return new TextEncoder().encode(JSON.stringify(state)).byteLength <= MAX_BROWSER_STATE_BYTES;
}

export function captureLocalDataState(storage?: StorageAccess | null): BrowserState {
  const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
  if (!target) throw new Error("browser-storage-unavailable");
  const state: BrowserState = { schema: 1, records: rawLocalDataRecords(target) };
  if (!validateLocalDataState(state)) throw new Error("invalid-browser-state");
  return state;
}

export function replaceLocalDataState(
  state: BrowserState,
  storage?: StorageAccess | null,
): void {
  if (!validateLocalDataState(state)) throw new Error("invalid-browser-state");
  const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
  if (!target) throw new Error("browser-storage-unavailable");
  const previous = rawLocalDataRecords(target);
  try {
    removeProductionRecords(target);
    for (const record of state.records) target.setItem(record.key, record.value);
  } catch {
    try {
      removeProductionRecords(target);
      for (const record of previous) target.setItem(record.key, record.value);
    } catch {
      throw new Error("browser-state-rollback-failed");
    }
    throw new Error("browser-state-write-failed");
  }
}

export function withoutBookLocalState(state: BrowserState, id: string): BrowserState {
  if (!/^[a-f0-9]{64}$/.test(id) || !validateLocalDataState(state)) {
    throw new Error("invalid-browser-state");
  }
  const bookKeys = new Set([id.slice(0, 16)]);
  const keyOwners = new Map<string, string>();
  for (const record of state.records) {
    const kind = productionStorageKind(record.key);
    const value = kind ? JSON.parse(record.value) : null;
    if (kind === "progress") {
      const bookKey = record.key.split(".")[3];
      keyOwners.set(bookKey, value.contentVersion);
      if (value.contentVersion === id) bookKeys.add(bookKey);
    } else if (kind === "annotations") {
      const items = value.items as { sourceAnchor: { canonicalLocator: string } }[];
      if (items.some((item) => parseLocator(item.sourceAnchor.canonicalLocator)?.contentVersion === id)) {
        bookKeys.add(record.key.split(".")[3]);
      }
    } else if (kind === "book") {
      const bookmarks = value.bookmarks as { locator: string }[];
      if (bookmarks.some((bookmark) => parseLocator(bookmark.locator)?.contentVersion === id)) {
        bookKeys.add(record.key.split(".")[3]);
      }
    }
  }
  const records: BrowserStateRecord[] = [];
  for (const record of state.records) {
    const kind = productionStorageKind(record.key);
    const bookKey = record.key.split(".")[3];
    if (!bookKeys.has(bookKey)) {
      records.push({ ...record });
      continue;
    }
    const value = JSON.parse(record.value);
    if (kind === "progress" && value.contentVersion === id) continue;
    if (kind === "annotations") {
      const items = value.items.filter((item: { sourceAnchor: { canonicalLocator: string } }) =>
        parseLocator(item.sourceAnchor.canonicalLocator)?.contentVersion !== id);
      if (items.length !== value.items.length) {
        if (items.length === 0) continue;
        value.items = items;
        records.push({ key: record.key, value: JSON.stringify(value) });
        continue;
      }
    }
    if (kind === "book") {
      value.bookmarks = value.bookmarks.filter(
        (bookmark: { locator: string }) => parseLocator(bookmark.locator)?.contentVersion !== id,
      );
      const ownsPreferences = bookKey === id.slice(0, 16) || keyOwners.get(bookKey) === id;
      if (value.bookmarks.length === 0 && (ownsPreferences || !keyOwners.has(bookKey))) continue;
      if (ownsPreferences) value.preferences = { ...DEFAULT_BOOK_PREFERENCES, styleModules: [] };
      records.push({ key: record.key, value: JSON.stringify(value) });
      continue;
    }
    records.push({ ...record });
  }
  const statistics = records.find((record) => record.key === "atha.reader.statistics.v1");
  if (statistics) {
    const value = JSON.parse(statistics.value) as {
      schema: 1;
      days: unknown[];
      books: { contentVersion: string }[];
    };
    value.books = value.books.filter((book) => book.contentVersion !== id);
    statistics.value = JSON.stringify(value);
  }
  const next: BrowserState = { schema: 1, records };
  if (!validateLocalDataState(next)) throw new Error("invalid-browser-state");
  return next;
}

export function applyBookLocalStateDeletion(
  state: BrowserState,
  id: string,
  storage?: StorageAccess | null,
): void {
  const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
  if (!target) throw new Error("browser-storage-unavailable");
  const next = withoutBookLocalState(state, id);
  const nextByKey = new Map(next.records.map((record) => [record.key, record.value]));
  const changed = state.records.filter((record) => {
    const nextValue = nextByKey.get(record.key);
    return nextValue && nextValue !== record.value;
  });
  for (const record of changed.filter((item) => productionStorageKind(item.key) === "book")) {
    target.setItem(record.key, nextByKey.get(record.key)!);
  }
  const removed = state.records.filter((record) => !nextByKey.has(record.key));
  for (const kind of ["book", "progress"]) {
    for (const record of removed) {
      if (productionStorageKind(record.key) === kind) target.removeItem(record.key);
    }
  }
  for (const record of changed.filter((item) => productionStorageKind(item.key) === "annotations")) {
    target.setItem(record.key, nextByKey.get(record.key)!);
  }
  for (const record of removed) {
    if (productionStorageKind(record.key) === "annotations") target.removeItem(record.key);
  }
  const statistics = next.records.find((record) => record.key === "atha.reader.statistics.v1");
  if (statistics) target.setItem(statistics.key, statistics.value);
}

export async function listBooks(): Promise<LibraryBook[]> {
  if (!libraryAvailable) return [];
  return invoke<LibraryBook[]>("list_library_books");
}

export async function importBooks(): Promise<ImportReport | null> {
  if (!libraryAvailable) return null;
  return invoke<ImportReport | null>("import_library_books");
}

export async function importBookPaths(paths: string[]): Promise<ImportReport> {
  return invoke<ImportReport>("import_library_paths", { paths });
}

export async function takeStartupImport(): Promise<StartupImport | null> {
  if (!libraryAvailable) return null;
  return invoke<StartupImport | null>("take_startup_import");
}

export function backupMessages(): Promise<boolean> {
  return invoke<boolean>("backup_message_store");
}

export function restoreMessages(): Promise<boolean> {
  return invoke<boolean>("restore_message_store");
}

export function backupLocalData(browserState: BrowserState): Promise<boolean> {
  return invoke<boolean>("backup_local_data", { browserState });
}

export function prepareLocalDataRestore(
  previousBrowserState: BrowserState,
): Promise<PendingLocalDataRestore | null> {
  return invoke<PendingLocalDataRestore | null>("prepare_local_data_restore", {
    previousBrowserState,
  });
}

export function commitLocalDataRestore(token: string): Promise<PendingLocalDataRestore> {
  return invoke<PendingLocalDataRestore>("commit_local_data_restore", { token });
}

export function pendingLocalDataRestore(): Promise<PendingLocalDataRestore | null> {
  return invoke<PendingLocalDataRestore | null>("pending_local_data_restore");
}

export function finishLocalDataRestore(token: string): Promise<void> {
  return invoke<void>("finish_local_data_restore", { token });
}

export function rollbackLocalDataRestore(token: string): Promise<BrowserState> {
  return invoke<BrowserState>("rollback_local_data_restore", { token });
}

export function abortLocalDataRestore(token: string): Promise<void> {
  return invoke<void>("abort_local_data_restore", { token });
}

export function readStorageUsage(browserState: BrowserState): Promise<StorageUsage> {
  return invoke<StorageUsage>("local_data_storage_usage", { browserState });
}

export async function openBook(id: string): Promise<void> {
  const launch = await invoke<ReaderLaunch>("open_library_book", { id });
  location.assign(launch.href);
}

export async function openReadingMemoryHit(hit: ReadingMemoryJump): Promise<void> {
  if (
    !/^[a-f0-9]{64}$/.test(hit.editionId) ||
    ![hit.messageId, hit.rootMessageId, hit.conversationId].every((id) =>
      /^[a-f0-9]{32}$/.test(id)
    ) ||
    parseLocator(hit.canonicalLocator)?.contentVersion !== hit.editionId
  ) {
    throw new Error("invalid-reading-memory-hit");
  }
  const launch = await invoke<ReaderLaunch>("open_library_book", { id: hit.editionId });
  const memory = new URLSearchParams({
    "memory-conversation": hit.conversationId,
    "memory-message": hit.messageId,
    "memory-root": hit.rootMessageId,
  });
  location.assign(`${launch.href}&${memory}`);
}

export function removeBook(id: string): Promise<LibraryBook[]> {
  return invoke<LibraryBook[]>("remove_library_book", { id });
}

export function prepareBookDataDeletion(id: string): Promise<PendingBookDeletion> {
  return invoke<PendingBookDeletion>("delete_library_book_data", { id });
}

export function pendingBookDataDeletions(): Promise<PendingBookDeletion[]> {
  return invoke<PendingBookDeletion[]>("pending_library_book_deletions");
}

export function finishBookDataDeletion(id: string): Promise<LibraryBook[]> {
  return invoke<LibraryBook[]>("finish_library_book_deletion", { id });
}

export async function removeBooksSerially(
  ids: string[],
  removeOne: (id: string) => Promise<LibraryBook[]> = removeBook,
): Promise<BatchRemoveResult> {
  const removedIds: string[] = [];
  let books: LibraryBook[] | null = null;
  for (const [index, id] of ids.entries()) {
    try {
      books = await removeOne(id);
      removedIds.push(id);
    } catch {
      return { books, removedIds, remainingIds: ids.slice(index) };
    }
  }
  return { books, removedIds, remainingIds: [] };
}

export async function deleteBooksSerially(
  ids: string[],
  storage?: StorageAccess | null,
  prepareOne: (id: string) => Promise<PendingBookDeletion> = prepareBookDataDeletion,
  finishOne: (id: string) => Promise<LibraryBook[]> = finishBookDataDeletion,
): Promise<BatchRemoveResult> {
  const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
  if (!target) return { books: null, removedIds: [], remainingIds: [...ids] };
  const removedIds: string[] = [];
  let books: LibraryBook[] | null = null;
  for (const [index, id] of ids.entries()) {
    let prepareStarted = false;
    try {
      const previous = captureLocalDataState(target);
      prepareStarted = true;
      await prepareOne(id);
      applyBookLocalStateDeletion(previous, id, target);
      books = await finishOne(id);
      removedIds.push(id);
    } catch {
      return { books, removedIds, remainingIds: ids.slice(index), pendingRecovery: prepareStarted };
    }
  }
  return { books, removedIds, remainingIds: [] };
}

export async function resumeBookDataDeletions(storage?: StorageAccess | null): Promise<void> {
  const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
  if (!target) throw new Error("browser-storage-unavailable");
  for (const pending of await pendingBookDataDeletions()) {
    const previous = captureLocalDataState(target);
    applyBookLocalStateDeletion(previous, pending.id, target);
    await finishBookDataDeletion(pending.id);
  }
}

function rawLocalDataRecords(storage: StorageAccess): BrowserStateRecord[] {
  const keys = Array.from({ length: storage.length }, (_, index) => storage.key(index))
    .filter((key): key is string => key !== null && productionStorageKind(key) !== null)
    .sort();
  const records: BrowserStateRecord[] = [];
  for (const key of keys) {
    const value = storage.getItem(key);
    if (value !== null) records.push({ key, value });
  }
  return records;
}

function removeProductionRecords(storage: StorageAccess) {
  for (const record of rawLocalDataRecords(storage)) storage.removeItem(record.key);
}

function productionStorageKind(key: string): string | null {
  if (key === "atha.reader.application.v1") return "application";
  if (key === "atha.reader.statistics.v1") return "statistics";
  if (key === "atha.reader.dictionary.preferences.v1") return "dictionary";
  const match = /^atha\.reader\.(book|progress|annotations)\.([a-f0-9]{16})\.v1$/.exec(key);
  return match?.[1] ?? null;
}

function validApplicationPreferences(value: unknown): boolean {
  if (!isRecord(value)) return false;
  const normalized: Record<string, unknown> = { brightness: 100, ...value };
  const legacyControls = Object.hasOwn(value, "tapToPaginate") || Object.hasOwn(value, "swipeToPaginate");
  if (legacyControls) {
    normalized.fontSize = ({ 24: 16, 32: 19, 40: 24 } as Record<number, number>)[normalized.fontSize as number] ?? 19;
  }
  for (const key of [
    "tapToPaginate",
    "swipeToPaginate",
    "marginTopPx",
    "marginRightPx",
    "marginBottomPx",
    "marginLeftPx",
  ]) delete normalized[key];
  return (
    exactKeys(normalized, ["brightness", "density", "fontFamily", "fontSize", "theme"]) &&
    ["system", "light", "paper", "dark"].includes(normalized.theme as string) &&
    Number.isInteger(normalized.brightness) &&
    (normalized.brightness as number) >= 70 &&
    (normalized.brightness as number) <= 120 &&
    Number.isInteger(normalized.fontSize) &&
    (normalized.fontSize as number) >= 16 &&
    (normalized.fontSize as number) <= 40 &&
    ["book", "serif", "sans"].includes(normalized.fontFamily as string) &&
    ["compact", "standard", "comfortable"].includes(normalized.density as string)
  );
}

function validBookPreferences(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (exactKeys(value, ["sourceStyles", "userStylesEnabled", "userStylesheet"])) {
    const stylesheet = value.userStylesheet;
    return (
      typeof value.sourceStyles === "boolean" &&
      typeof value.userStylesEnabled === "boolean" &&
      typeof stylesheet === "string" &&
      stylesheet.length <= 32_768 &&
      (new TextEncoder().encode(stylesheet).byteLength > 65_536 || validUserCss(stylesheet))
    );
  }
  const normalized: Record<string, unknown> = { readingMode: "paged", ...value };
  if (normalized.paragraphIndent === "book") normalized.paragraphIndent = "none";
  return (
    exactKeys(normalized, [
      "pageMargin",
      "paragraphIndent",
      "paragraphSpacing",
      "readingMode",
      "sourceStyles",
      "styleModules",
      "userStylesEnabled",
    ]) &&
    typeof normalized.sourceStyles === "boolean" &&
    typeof normalized.userStylesEnabled === "boolean" &&
    ["paged", "scroll"].includes(normalized.readingMode as string) &&
    ["narrow", "standard", "wide"].includes(normalized.pageMargin as string) &&
    ["none", "two"].includes(normalized.paragraphIndent as string) &&
    ["book", "compact", "comfortable"].includes(normalized.paragraphSpacing as string) &&
    validStyleModules(normalized.styleModules)
  );
}

function validStyleModules(value: unknown): boolean {
  if (!Array.isArray(value) || value.length > 32) return false;
  const ids = new Set<string>();
  let enabledBytes = 0;
  for (const module of value) {
    if (!isRecord(module) || !exactKeys(module, ["css", "enabled", "group", "id", "name"])) {
      return false;
    }
    const bytes = typeof module.css === "string"
      ? new TextEncoder().encode(module.css).byteLength
      : Number.POSITIVE_INFINITY;
    if (
      typeof module.id !== "string" ||
      !/^[a-z0-9][a-z0-9-]{0,63}$/.test(module.id) ||
      ids.has(module.id) ||
      typeof module.name !== "string" ||
      module.name !== module.name.trim() ||
      module.name.length === 0 ||
      module.name.length > 64 ||
      /[\u0000-\u001f\u007f]/u.test(module.name) ||
      typeof module.group !== "string" ||
      module.group !== module.group.trim() ||
      module.group.length > 32 ||
      /[\u0000-\u001f\u007f]/u.test(module.group) ||
      typeof module.enabled !== "boolean" ||
      typeof module.css !== "string" ||
      (bytes > 32_768 && !(module.id === "legacy-user-css" && module.css.length <= 32_768)) ||
      (bytes > 65_536
        ? !(module.id === "legacy-user-css" && !module.enabled)
        : !validUserCss(module.css))
    ) return false;
    if (module.enabled) enabledBytes += bytes;
    if (enabledBytes > 65_536) return false;
    ids.add(module.id);
  }
  return true;
}

function validBookmark(value: unknown): boolean {
  if (!isRecord(value) || !exactKeys(value, ["id", "label", "locator"])) return false;
  const locator = parseLocator(value.locator);
  return (
    typeof value.id === "string" &&
    /^[a-z0-9-]{1,64}$/.test(value.id) &&
    typeof value.label === "string" &&
    value.label.trim().length > 0 &&
    value.label.length <= 256 &&
    locator !== null &&
    locator.contentVersion !== null
  );
}

function validUserCss(value: string): boolean {
  try {
    validateUserStylesheet(value);
    return true;
  } catch {
    return false;
  }
}

function validAnnotation(value: unknown): boolean {
  if (
    !isRecord(value) ||
    !exactKeys(value, ["createdAt", "deletedAt", "id", "note", "sourceAnchor", "type", "updatedAt"])
  ) return false;
  return (
    typeof value.id === "string" &&
    /^[a-z0-9-]{1,64}$/.test(value.id) &&
    ["highlight", "note"].includes(value.type as string) &&
    typeof value.note === "string" &&
    value.note.length <= 2_000 &&
    (value.type === "note" ? Boolean(value.note.trim()) : value.note.length === 0) &&
    Number.isSafeInteger(value.createdAt) &&
    (value.createdAt as number) >= 0 &&
    Number.isSafeInteger(value.updatedAt) &&
    (value.updatedAt as number) >= (value.createdAt as number) &&
    (value.deletedAt === null ||
      (Number.isSafeInteger(value.deletedAt) &&
        (value.deletedAt as number) >= (value.createdAt as number))) &&
    validSourceAnchor(value.sourceAnchor)
  );
}

function validSourceAnchor(value: unknown): boolean {
  if (
    !isRecord(value) ||
    !exactKeys(value, [
      "canonicalLocator",
      "contentHash",
      "prefixText",
      "schema",
      "selectedText",
      "suffixText",
    ]) ||
    value.schema !== 1 ||
    typeof value.canonicalLocator !== "string" ||
    typeof value.selectedText !== "string" ||
    !value.selectedText.trim() ||
    value.selectedText.length > 4_096 ||
    typeof value.prefixText !== "string" ||
    value.prefixText.length > 32 ||
    typeof value.suffixText !== "string" ||
    value.suffixText.length > 32 ||
    typeof value.contentHash !== "string" ||
    !/^[a-f0-9]{64}$/.test(value.contentHash)
  ) return false;
  const locator = parseLocator(value.canonicalLocator);
  // The backend rechecks SHA-256 before any archive can publish; WebCrypto is async.
  return Boolean(
    locator?.end &&
      locator.contentVersion !== null &&
      JSON.stringify(locator) === value.canonicalLocator &&
      locator.end.offset - locator.start.offset === value.selectedText.length,
  );
}

function validStorageRecord(key: string, raw: string): boolean {
  const kind = productionStorageKind(key);
  if (
    raw.length === 0 ||
    kind === null ||
    raw.length > (kind === "annotations" ? 2 * 1024 * 1024 : kind === "dictionary" ? 1_024 : MAX_STATE_LENGTH)
  ) return false;
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return false;
  }
  if (!value || typeof value !== "object" || Array.isArray(value) || Reflect.get(value, "schema") !== 1) {
    return false;
  }
  if (key === "atha.reader.application.v1") {
    return exactKeys(value, ["preferences", "schema"]) && validApplicationPreferences(Reflect.get(value, "preferences"));
  }
  if (key === "atha.reader.statistics.v1") return validStatistics(value);
  if (key === "atha.reader.dictionary.preferences.v1") {
    const dictionaryId = Reflect.get(value, "dictionaryId");
    return (
      exactKeys(value, ["dictionaryId", "fontScale", "schema"]) &&
      (dictionaryId === "" || (typeof dictionaryId === "string" && /^[a-f0-9]{64}$/.test(dictionaryId))) &&
      [0.85, 1, 1.15, 1.3, 1.5, 1.75].includes(Reflect.get(value, "fontScale") as number)
    );
  }
  if (key.startsWith("atha.reader.book.")) {
    const bookmarks = Reflect.get(value, "bookmarks");
    return (
      exactKeys(value, ["bookmarks", "preferences", "schema"]) &&
      Array.isArray(bookmarks) &&
      bookmarks.length <= 200 &&
      bookmarks.every(validBookmark) &&
      validBookPreferences(Reflect.get(value, "preferences"))
    );
  }
  if (key.startsWith("atha.reader.progress.")) {
    const contentVersion = Reflect.get(value, "contentVersion");
    return (
      typeof contentVersion === "string" &&
      validProgress(raw, contentVersion)
    );
  }
  const items = Reflect.get(value, "items");
  return (
    exactKeys(value, ["items", "schema"]) &&
    Array.isArray(items) &&
    items.length <= 1000 &&
    items.every(validAnnotation) &&
    new Set(items.map((item) => isRecord(item) ? item.id : null)).size === items.length
  );
}

function validStatistics(value: object): boolean {
  if (
    !exactKeys(value, ["books", "days", "schema"]) ||
    !Array.isArray(Reflect.get(value, "days")) ||
    !Array.isArray(Reflect.get(value, "books")) ||
    (Reflect.get(value, "days") as unknown[]).length > 400 ||
    (Reflect.get(value, "books") as unknown[]).length > 2048
  ) return false;
  const dates = new Set<string>();
  for (const day of Reflect.get(value, "days") as unknown[]) {
    if (
      !isRecord(day) ||
      !exactKeys(day, ["date", "durationMs"]) ||
      !validLocalDate(day.date) ||
      !Number.isSafeInteger(day.durationMs) ||
      (day.durationMs as number) <= 0 ||
      (day.durationMs as number) > 26 * 60 * 60 * 1_000 ||
      dates.has(day.date)
    ) return false;
    dates.add(day.date);
  }
  const ids = new Set<string>();
  for (const book of Reflect.get(value, "books") as unknown[]) {
    if (
      !isRecord(book) ||
      !exactKeys(book, ["contentVersion", "durationMs", "lastReadDate"]) ||
      typeof book.contentVersion !== "string" ||
      !/^[a-f0-9]{64}$/.test(book.contentVersion) ||
      !Number.isSafeInteger(book.durationMs) ||
      (book.durationMs as number) <= 0 ||
      !validLocalDate(book.lastReadDate) ||
      ids.has(book.contentVersion)
    ) return false;
    ids.add(book.contentVersion);
  }
  return true;
}

function validLocalDate(value: unknown): value is string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

export function coverUrl(id: string): string {
  const root = globalThis.location?.protocol === "tauri:"
    ? "atha-cover://localhost/"
    : "https://atha-cover.localhost/";
  return `${root}${id}`;
}

export function importFailureMessage(code: string): string {
  switch (code) {
    case "invalid-library-source":
      return "无法读取所选文件";
    case "epub-source-too-large":
    case "epub-archive-too-large":
    case "cbz-source-too-large":
    case "cbz-archive-too-large":
    case "markdown-source-too-large":
    case "markdown-section-too-large":
    case "txt-source-too-large":
    case "txt-section-too-large":
    case "kindle-source-too-large":
    case "kindle-text-too-large":
    case "kindle-resource-too-large":
    case "fb2-source-too-large":
    case "fbz-archive-too-large":
    case "fb2-resource-too-large":
      return "文件过大";
    case "encrypted-epub":
      return "暂不支持受保护的 EPUB";
    case "encrypted-cbz":
      return "暂不支持受保护的 CBZ";
    case "encrypted-kindle":
      return "暂不支持受保护的 Kindle 书籍";
    case "encrypted-fbz":
      return "暂不支持受保护的 FBZ";
    case "kindle-dictionary-unsupported":
      return "这是 Kindle 词典，请在查词功能中使用";
    case "unsupported-epub":
      return "暂不支持这本 EPUB 的结构";
    case "unsupported-cbz":
      return "CBZ 中没有可读取的 JPEG 或 PNG 页面";
    case "unsupported-fb2":
      return "暂不支持这本 FB2 的结构";
    case "too-many-cbz-pages":
      return "CBZ 页数过多";
    case "too-many-epub-sections":
    case "too-many-epub-toc-items":
      return "EPUB 章节或目录项过多";
    case "too-many-markdown-sections":
    case "too-many-markdown-toc-items":
      return "Markdown 章节或目录项过多";
    case "too-many-txt-sections":
      return "TXT 章节过多";
    case "too-many-kindle-sections":
    case "too-many-kindle-toc-items":
      return "Kindle 书籍的章节或目录项过多";
    case "too-many-fb2-sections":
    case "too-many-fb2-toc-items":
      return "FB2 章节或目录项过多";
    case "invalid-cbz-image":
      return "CBZ 中包含无效或尺寸过大的图片";
    case "invalid-epub-source":
    case "invalid-epub-archive":
    case "invalid-epub-xml":
    case "unsafe-epub-path":
      return "不是可读取的 EPUB 文件";
    case "invalid-cbz-source":
    case "invalid-cbz-archive":
    case "unsafe-cbz-path":
      return "不是可读取的 CBZ 文件";
    case "invalid-fb2-source":
    case "invalid-fbz-archive":
    case "unsafe-fbz-path":
    case "invalid-fb2-xml":
    case "invalid-fb2-reference":
    case "invalid-fb2-image":
      return "不是可读取的 FB2 或 FBZ 文件";
    case "invalid-markdown-source":
      return "不是可读取的 Markdown 文件";
    case "invalid-markdown-encoding":
      return "Markdown 不是有效的 UTF-8 文本";
    case "invalid-txt-source":
      return "不是可读取的 TXT 文件";
    case "invalid-txt-encoding":
      return "无法识别 TXT 编码";
    case "invalid-kindle-source":
    case "invalid-kindle-structure":
    case "invalid-kindle-encoding":
    case "invalid-kindle-markup":
    case "invalid-kindle-reference":
    case "invalid-kindle-image":
    case "unsupported-kindle":
      return "不是可读取的 MOBI、AZW 或 AZW3 文件";
    case "epub-source-changed":
    case "cbz-source-changed":
    case "markdown-source-changed":
    case "txt-source-changed":
    case "kindle-source-changed":
    case "fb2-source-changed":
      return "文件在导入时发生了变化，请重试";
    case "epub-import-write-failed":
    case "cbz-import-write-failed":
    case "markdown-import-write-failed":
    case "txt-import-write-failed":
    case "kindle-import-write-failed":
    case "fb2-import-write-failed":
      return "无法保存导入的书籍";
    default:
      return "导入失败";
  }
}

export function openFailureMessage(error: unknown): string {
  const code = typeof error === "string" ? error : error instanceof Error ? error.message : "";
  switch (code) {
    case "unknown-library-book":
      return "书籍已不在书架中";
    case "corrupt-library-record":
      return "书架记录已损坏，请重新导入";
    case "invalid-root":
    case "not-found":
    case "read-failed":
    case "library-read-failed":
    case "invalid-epub-source":
    case "invalid-cbz-source":
    case "invalid-fb2-source":
    case "invalid-kindle-source":
    case "invalid-markdown-source":
    case "invalid-txt-source":
      return "已保存的书籍内容不可读取，请重新导入";
    case "library-write-failed":
      return "无法准备书籍，请检查存储空间后重试";
    default: {
      const message = importFailureMessage(code);
      return message === "导入失败" ? "无法打开书籍" : message;
    }
  }
}
