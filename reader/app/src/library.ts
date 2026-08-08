import { invoke } from "@tauri-apps/api/core";

export interface LibraryBook {
  id: string;
  title: string;
  authors: string[];
  hasCover: boolean;
  importedAt: number;
}

export interface ImportFailure {
  name: string;
  code: string;
}

export interface ImportReport {
  books: LibraryBook[];
  failures: ImportFailure[];
}

export type LibraryViewMode = "default" | "progress" | "title" | "author";

export interface BatchRemoveResult {
  books: LibraryBook[] | null;
  removedIds: string[];
  remainingIds: string[];
}

interface ReaderLaunch {
  href: string;
}

type StorageReader = Pick<Storage, "getItem">;

const MAX_STATE_LENGTH = 524_288;
const MAX_LOCATOR_LENGTH = 2_048;
const MAX_TEXT_OFFSET = 2_147_483_647;
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

function validLocator(serialized: unknown, contentVersion: string): boolean {
  if (
    typeof serialized !== "string" ||
    serialized.length === 0 ||
    serialized.length > MAX_LOCATOR_LENGTH
  ) {
    return false;
  }
  try {
    const value: unknown = JSON.parse(serialized);
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const hasEnd = Object.hasOwn(value, "end");
    if (!exactKeys(value, hasEnd ? ["contentVersion", "end", "schema", "start"] : ["contentVersion", "schema", "start"])) {
      return false;
    }
    if (Reflect.get(value, "schema") !== 1 || Reflect.get(value, "contentVersion") !== contentVersion) {
      return false;
    }
    const start = Reflect.get(value, "start");
    if (!validPoint(start)) return false;
    if (!hasEnd) return true;
    const end = Reflect.get(value, "end");
    return validPoint(end) && end.section === start.section && end.offset >= start.offset;
  } catch {
    return false;
  }
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

export async function listBooks(): Promise<LibraryBook[]> {
  if (!libraryAvailable) return [];
  return invoke<LibraryBook[]>("list_library_books");
}

export async function importBooks(): Promise<ImportReport | null> {
  if (!libraryAvailable) return null;
  return invoke<ImportReport | null>("import_library_books");
}

export function backupMessages(): Promise<boolean> {
  return invoke<boolean>("backup_message_store");
}

export function restoreMessages(): Promise<boolean> {
  return invoke<boolean>("restore_message_store");
}

export async function openBook(id: string): Promise<void> {
  const launch = await invoke<ReaderLaunch>("open_library_book", { id });
  location.assign(launch.href);
}

export function removeBook(id: string): Promise<LibraryBook[]> {
  return invoke<LibraryBook[]>("remove_library_book", { id });
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
      return "文件过大";
    case "encrypted-epub":
      return "暂不支持受保护的 EPUB";
    case "encrypted-cbz":
      return "暂不支持受保护的 CBZ";
    case "encrypted-kindle":
      return "暂不支持受保护的 Kindle 书籍";
    case "kindle-dictionary-unsupported":
      return "这是 Kindle 词典，请在查词功能中使用";
    case "unsupported-epub":
      return "暂不支持这本 EPUB 的结构";
    case "unsupported-cbz":
      return "CBZ 中没有可读取的 JPEG 或 PNG 页面";
    case "too-many-cbz-pages":
      return "CBZ 页数过多";
    case "too-many-markdown-sections":
    case "too-many-markdown-toc-items":
      return "Markdown 章节或目录项过多";
    case "too-many-txt-sections":
      return "TXT 章节过多";
    case "too-many-kindle-sections":
    case "too-many-kindle-toc-items":
      return "Kindle 书籍的章节或目录项过多";
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
      return "文件在导入时发生了变化，请重试";
    case "epub-import-write-failed":
    case "cbz-import-write-failed":
    case "markdown-import-write-failed":
    case "txt-import-write-failed":
    case "kindle-import-write-failed":
      return "无法保存导入的书籍";
    default:
      return "导入失败";
  }
}
