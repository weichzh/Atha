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

interface ReaderLaunch {
  href: string;
}

export const libraryAvailable = Boolean(window.__TAURI_INTERNALS__);

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

export function coverUrl(id: string): string {
  return `https://atha-cover.localhost/${id}`;
}

export function importFailureMessage(code: string): string {
  switch (code) {
    case "epub-source-too-large":
    case "epub-archive-too-large":
      return "文件过大";
    case "encrypted-epub":
      return "暂不支持受保护的 EPUB";
    case "unsupported-epub":
      return "暂不支持这本 EPUB 的结构";
    case "invalid-epub-source":
    case "invalid-epub-archive":
    case "invalid-epub-xml":
      return "不是可读取的 EPUB 文件";
    default:
      return "导入失败";
  }
}
