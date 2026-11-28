import {
  dictionaryAvailable,
  dictionaryFontScales,
  importDictionary as tauriImportDictionary,
  listDictionaries as tauriListDictionaries,
  lookupDictionary as tauriLookupDictionary,
  readDictionaryPreferences,
  removeDictionary as tauriRemoveDictionary,
  writeDictionaryPreferences,
  type DictionaryFontScale,
  type DictionaryLookup,
  type DictionaryPreferences,
  type LocalDictionary,
} from "../../dictionary.ts";
import {
  applyBookLocalStateDeletion,
  backupLocalData as tauriBackupLocalData,
  captureLocalDataState,
  commitLocalDataRestore,
  coverUrl,
  deleteBooksSerially,
  finishLocalDataRestore,
  importBooks as tauriImportBooks,
  libraryAvailable,
  listBooks as tauriListBooks,
  openReadingMemoryHit as tauriOpenReadingMemoryHit,
  prepareBookLaunch as tauriPrepareBookLaunch,
  prepareLocalDataRestore,
  prepareReadingMemoryLaunch as tauriPrepareReadingMemoryLaunch,
  readAppTheme,
  readStorageUsage as tauriReadStorageUsage,
  removeBook as tauriRemoveBook,
  removeBooksSerially,
  replaceLocalDataState,
  resetBookCover as tauriResetBookCover,
  resetReaderApplicationPreferences,
  rollbackLocalDataRestore,
  setBookCover as tauriSetBookCover,
  updateBookOrganization as tauriUpdateBookOrganization,
  writeAppTheme,
  type AppTheme,
  type BatchRemoveResult,
  type BookOrganization,
  type ImportReport,
  type LibraryBook,
  type ReadingMemoryJump,
  type StorageUsage,
} from "../../library.ts";
import {
  DEMO_BOOKS,
  DEMO_DICTIONARIES,
  DEMO_DICTIONARY_ENTRIES,
  DEMO_PRIMARY_BOOK_ID,
  generateDemoCoverSvg,
  seedDemoLocalStorageIfEmpty,
} from "./demo-seed.ts";
import { isTauriRuntime } from "./runtime.ts";

const DEMO_BOOKS_KEY = "atha.demo.books.v1";
const DEMO_DICT_KEY = "atha.demo.dictionaries.v1";
const DEMO_COVER_KEY = "atha.demo.custom_covers.v1";

function loadDemoBooks(): LibraryBook[] {
  seedDemoLocalStorageIfEmpty();
  try {
    const raw = window.localStorage.getItem(DEMO_BOOKS_KEY);
    if (!raw) {
      window.localStorage.setItem(DEMO_BOOKS_KEY, JSON.stringify(DEMO_BOOKS));
      return structuredClone(DEMO_BOOKS);
    }
    return JSON.parse(raw) as LibraryBook[];
  } catch {
    return structuredClone(DEMO_BOOKS);
  }
}

function saveDemoBooks(books: LibraryBook[]): void {
  window.localStorage.setItem(DEMO_BOOKS_KEY, JSON.stringify(books));
}

function loadDemoDictionaries(): LocalDictionary[] {
  try {
    const raw = window.localStorage.getItem(DEMO_DICT_KEY);
    if (!raw) {
      window.localStorage.setItem(DEMO_DICT_KEY, JSON.stringify(DEMO_DICTIONARIES));
      return structuredClone(DEMO_DICTIONARIES);
    }
    return JSON.parse(raw) as LocalDictionary[];
  } catch {
    return structuredClone(DEMO_DICTIONARIES);
  }
}

function saveDemoDictionaries(dicts: LocalDictionary[]): void {
  window.localStorage.setItem(DEMO_DICT_KEY, JSON.stringify(dicts));
}

function loadDemoCustomCovers(): Record<string, string> {
  try {
    const raw = window.localStorage.getItem(DEMO_COVER_KEY);
    return raw ? (JSON.parse(raw) as Record<string, string>) : {};
  } catch {
    return {};
  }
}

export function resolveBookCoverUrl(book: LibraryBook, index: number, revision = 0): string {
  if (isTauriRuntime() && libraryAvailable) {
    return coverUrl(book.id, revision);
  }
  const custom = loadDemoCustomCovers()[book.id];
  if (book.hasCustomCover && custom) return custom;
  return generateDemoCoverSvg(book.title, book.authors.join(" · "), index);
}

export async function listLibraryBooks(): Promise<LibraryBook[]> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriListBooks();
  }
  return loadDemoBooks();
}

export async function importLibraryBooks(): Promise<ImportReport | null> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriImportBooks();
  }
  const existing = loadDemoBooks();
  const sampleId = `${(Date.now().toString(16) + "f9a8").padEnd(64, "c").slice(0, 64)}`;
  const newBook: LibraryBook = {
    id: sampleId,
    title: "新入书斋：数字文献装帧随笔",
    authors: ["叶澄"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: Date.now(),
    prepared: true,
    favorite: false,
    tags: ["新导入", "排版美学"],
    subjects: ["出版设计"],
    series: [],
  };
  const updated = [newBook, ...existing];
  saveDemoBooks(updated);
  return { books: updated, failures: [] };
}

export async function updateLibraryBookOrganization(
  id: string,
  organization: Partial<BookOrganization>,
): Promise<LibraryBook[]> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriUpdateBookOrganization(id, organization);
  }
  const books = loadDemoBooks().map((book) =>
    book.id === id ? { ...book, ...organization } : book,
  );
  saveDemoBooks(books);
  return books;
}

export async function setLibraryBookCover(id: string): Promise<LibraryBook[] | null> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriSetBookCover(id);
  }
  const books = loadDemoBooks();
  const target = books.find((b) => b.id === id);
  if (!target) return null;
  const covers = loadDemoCustomCovers();
  covers[id] = generateDemoCoverSvg(target.title, target.authors.join(" · "), 5);
  window.localStorage.setItem(DEMO_COVER_KEY, JSON.stringify(covers));
  const updated = books.map((b) => (b.id === id ? { ...b, hasCover: true, hasCustomCover: true } : b));
  saveDemoBooks(updated);
  return updated;
}

export async function resetLibraryBookCover(id: string): Promise<LibraryBook[]> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriResetBookCover(id);
  }
  const covers = loadDemoCustomCovers();
  delete covers[id];
  window.localStorage.setItem(DEMO_COVER_KEY, JSON.stringify(covers));
  const updated = loadDemoBooks().map((b) => (b.id === id ? { ...b, hasCustomCover: false } : b));
  saveDemoBooks(updated);
  return updated;
}

export async function removeLibraryBook(id: string): Promise<LibraryBook[]> {
  if (isTauriRuntime() && libraryAvailable) {
    return tauriRemoveBook(id);
  }
  const updated = loadDemoBooks().filter((b) => b.id !== id);
  saveDemoBooks(updated);
  return updated;
}

export function applyShelfRemoval(ids: string[], mode: "remove" | "delete"): Promise<BatchRemoveResult> {
  if (mode === "remove") return removeBooksSerially(ids, removeLibraryBook);
  if (isTauriRuntime() && libraryAvailable) return deleteBooksSerially(ids);
  return removeBooksSerially(ids, deleteDemoBookData);
}

async function deleteDemoBookData(id: string): Promise<LibraryBook[]> {
  try {
    const state = captureLocalDataState(window.localStorage);
    applyBookLocalStateDeletion(state, id, window.localStorage);
  } catch {
    // Ignore when book has no local records yet
  }
  const updated = loadDemoBooks().filter((b) => b.id !== id);
  saveDemoBooks(updated);
  return updated;
}

function isAndroidAppShell(): boolean {
  return typeof window !== "undefined" && Boolean(window.AthaSystemBars);
}

function normalizeAppHref(href: string): string {
  return href.startsWith("/") ? href : `/${href}`;
}

function manifestFromHref(href: string, fallback = ".atha-reader.json"): string {
  const idx = href.indexOf("?");
  if (idx < 0) return fallback;
  return new URLSearchParams(href.slice(idx + 1)).get("manifest") || fallback;
}

export function opensShelfReaderInPlace(): boolean {
  return !isTauriRuntime() || !libraryAvailable || isAndroidAppShell();
}

export async function prepareShelfReaderHref(id: string): Promise<string> {
  if (isTauriRuntime() && libraryAvailable) {
    const launch = await tauriPrepareBookLaunch(id);
    return normalizeAppHref(launch.href);
  }
  return `?manifest=${encodeURIComponent("demo/manifest.json")}&version=${encodeURIComponent(id)}`;
}

export function activateShelfReader(href: string): string {
  history.replaceState({ athaReaderSpa: true }, "", href);
  return manifestFromHref(href);
}

export async function launchReadingMemoryHit(
  hit: ReadingMemoryJump,
  onSpaLaunch: (bookId: string, manifestPath: string, jump: ReadingMemoryJump) => void,
  canActivate: () => boolean,
): Promise<void> {
  if (isTauriRuntime() && libraryAvailable) {
    if (!isAndroidAppShell()) {
      await tauriOpenReadingMemoryHit(hit);
      return;
    }
    const href = await tauriPrepareReadingMemoryLaunch(hit);
    if (!canActivate()) return;
    const nextHref = normalizeAppHref(href);
    history.replaceState({ athaReaderSpa: true }, "", nextHref);
    onSpaLaunch(hit.editionId, manifestFromHref(nextHref), hit);
    return;
  }
  if (!canActivate()) return;
  history.replaceState(
    { athaReaderSpa: true },
    "",
    `?manifest=${encodeURIComponent("demo/manifest.json")}&version=${encodeURIComponent(hit.editionId)}`,
  );
  onSpaLaunch(hit.editionId, "demo/manifest.json", hit);
}

export function getAppTheme(): AppTheme {
  return readAppTheme();
}

export function setAppTheme(theme: AppTheme): void {
  writeAppTheme(theme);
  applyAppThemeToDocument(theme);
}

export function applyAppThemeToDocument(theme: AppTheme): void {
  if (typeof document === "undefined") return;
  const resolved =
    theme === "system"
      ? window.matchMedia?.("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
  document.documentElement.dataset.appTheme = resolved;
}

export function resetGlobalReaderDefaults(): boolean {
  return resetReaderApplicationPreferences();
}

export async function listOfflineDictionaries(): Promise<LocalDictionary[]> {
  if (isTauriRuntime() && dictionaryAvailable) {
    return tauriListDictionaries();
  }
  return loadDemoDictionaries();
}

export async function importOfflineDictionary(): Promise<LocalDictionary[] | null> {
  if (isTauriRuntime() && dictionaryAvailable) {
    return tauriImportDictionary();
  }
  const current = loadDemoDictionaries();
  const next: LocalDictionary = {
    id: `${Date.now().toString(16).padEnd(64, "e").slice(0, 64)}`,
    title: "韦氏高阶学习词典（演示词典）",
    format: "mdict",
    entryCount: 96400,
    resourceCount: 18,
    importedAt: Date.now(),
  };
  const updated = [...current, next];
  saveDemoDictionaries(updated);
  return updated;
}

export async function removeOfflineDictionary(dictionaryId: string): Promise<LocalDictionary[]> {
  if (isTauriRuntime() && dictionaryAvailable) {
    return tauriRemoveDictionary(dictionaryId);
  }
  const updated = loadDemoDictionaries().filter((d) => d.id !== dictionaryId);
  saveDemoDictionaries(updated);
  return updated;
}

export async function lookupOfflineWord(
  dictionaryId: string,
  query: string,
): Promise<DictionaryLookup | null> {
  const clean = query.trim();
  if (!clean) return null;
  if (isTauriRuntime() && dictionaryAvailable) {
    return tauriLookupDictionary(dictionaryId, clean);
  }
  const normalized = clean.toLowerCase().replace(/^[^a-z\u4e00-\u9fff]+|[^a-z\u4e00-\u9fff]+$/gi, "");
  const exact = DEMO_DICTIONARY_ENTRIES[normalized] ?? DEMO_DICTIONARY_ENTRIES[clean];
  if (exact) return exact;
  return {
    dictionaryId: dictionaryId || DEMO_DICTIONARIES[0].id,
    headword: clean,
    definition: `“${clean}” — 本地离线词典检索示例结果。真实桌面端会直接检索您导入的 MDX / MOBI 词典索引。`,
    definitionHtml: `<p><strong>${clean.replace(/[<>&]/g, "")}</strong></p><p>本地离线词典演示词条。在桌面版中，这里会渲染对应词典中的完整音标、释义与例句。</p>`,
  };
}

export function getDictionaryPreferences(): DictionaryPreferences {
  return readDictionaryPreferences();
}

export function saveDictionaryFontScale(scale: DictionaryFontScale): DictionaryPreferences {
  const current = readDictionaryPreferences();
  const next: DictionaryPreferences = { ...current, fontScale: scale };
  writeDictionaryPreferences(next);
  return next;
}

export { dictionaryFontScales };

export async function readLocalStorageUsage(): Promise<StorageUsage> {
  const state = captureLocalDataState();
  if (isTauriRuntime() && libraryAvailable) {
    return tauriReadStorageUsage(state);
  }
  const prefBytes = new TextEncoder().encode(JSON.stringify(state)).byteLength;
  const books = loadDemoBooks();
  const dicts = loadDemoDictionaries();
  const booksBytes = books.length * 1_450_000;
  const cacheBytes = 4_200_000;
  const messagesBytes = 384_000;
  const dictionariesBytes = dicts.length * 18_400_000;
  return {
    booksBytes,
    cacheBytes,
    messagesBytes,
    dictionariesBytes,
    preferencesBytes: Math.max(4096, prefBytes),
    totalBytes: booksBytes + cacheBytes + messagesBytes + dictionariesBytes + Math.max(4096, prefBytes),
  };
}

export async function exportFullLocalDataBackup(): Promise<{ ok: boolean; label: string }> {
  const state = captureLocalDataState();
  if (isTauriRuntime() && libraryAvailable) {
    const ok = await tauriBackupLocalData(state);
    return { ok, label: ok ? "已导出完整 .atha-data 备份" : "已取消备份" };
  }
  const payload = {
    schema: 1,
    exportedAt: new Date().toISOString(),
    browserState: state,
    books: loadDemoBooks(),
    dictionaries: loadDemoDictionaries(),
  };
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `atha-backup-${new Date().toISOString().slice(0, 10)}.atha-data.json`;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return { ok: true, label: "已下载 .atha-data.json 备份文件" };
}

export async function restoreFullLocalDataBackup(file?: File | null): Promise<{ ok: boolean; label: string }> {
  const current = captureLocalDataState();
  if (isTauriRuntime() && libraryAvailable) {
    const pending = await prepareLocalDataRestore(current);
    if (!pending) return { ok: false, label: "已取消恢复" };
    try {
      replaceLocalDataState(pending.browserState);
      const committed = await commitLocalDataRestore(pending.token);
      await finishLocalDataRestore(committed.token);
      return { ok: true, label: "已从 .atha-data 备份完整恢复" };
    } catch {
      const rolledBack = await rollbackLocalDataRestore(pending.token);
      replaceLocalDataState(rolledBack);
      return { ok: false, label: "恢复校验失败，已自动回滚" };
    }
  }
  if (!file) return { ok: false, label: "请选择备份文件" };
  try {
    const parsed = JSON.parse(await file.text());
    if (parsed?.schema !== 1 || !parsed.browserState) {
      return { ok: false, label: "无效的备份格式" };
    }
    replaceLocalDataState(parsed.browserState);
    if (Array.isArray(parsed.books)) saveDemoBooks(parsed.books);
    if (Array.isArray(parsed.dictionaries)) saveDemoDictionaries(parsed.dictionaries);
    return { ok: true, label: "已从备份恢复本地书房数据" };
  } catch {
    replaceLocalDataState(current);
    return { ok: false, label: "备份文件损坏，已保持原数据" };
  }
}

export function isPrimaryDemoBook(id: string): boolean {
  return id === DEMO_PRIMARY_BOOK_ID;
}
