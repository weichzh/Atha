import { parseReadingFact, readProgressLocator, readRecentBooks, type LibraryBook } from "./library.ts";

export interface ShelfReading {
  status: "unread" | "reading" | "finished" | "unknown";
  percent: number | null;
  lastReadAt: number | null;
  lastReadDate: string | null;
}

type StorageReader = Pick<Storage, "getItem">;

export function readShelfReading(
  books: LibraryBook[],
  storage?: StorageReader | null,
): Map<string, ShelfReading> {
  let target: StorageReader | null;
  try {
    target = storage === undefined ? (globalThis.localStorage ?? null) : storage;
  } catch {
    target = null;
  }
  const recent = target ? readRecentBooks(books, target) : null;
  const legacyDates = new Map(recent?.map(({ book, lastReadDate }) => [book.id, lastReadDate]));
  const result = new Map<string, ShelfReading>();
  for (const { id } of books) {
    const legacyDate = legacyDates.get(id) ?? null;
    const unknown: ShelfReading = {
      status: legacyDate ? "reading" : "unknown",
      percent: null,
      lastReadAt: null,
      lastReadDate: legacyDate,
    };
    let value = unknown;
    if (target && /^[a-f0-9]{64}$/.test(id)) {
      try {
        const progress = target.getItem(`atha.reader.progress.${id.slice(0, 16)}.v1`);
        const rawFact = target.getItem(`atha.reader.reading.${id}.v1`);
        const locator = readProgressLocator(progress, id);
        const fact = parseReadingFact(rawFact, id);
        // A recent event survives a missing/moved Locator, but its percentage
        // describes only the corresponding saved position and this exact edition.
        const percent = locator !== null && locator === fact?.locator ? fact.percent : null;
        const preciseDateIsLatest = fact !== null && (!legacyDate || fact.lastReadDate >= legacyDate);
        value = {
          status: fact?.completed ? "finished"
            : locator || fact || legacyDate ? "reading"
            : progress !== null || rawFact !== null || recent === null ? "unknown"
            : "unread",
          percent,
          lastReadAt: preciseDateIsLatest ? fact.lastReadAt : null,
          lastReadDate: preciseDateIsLatest ? fact.lastReadDate : legacyDate,
        };
      } catch {
        // An unreadable book record cannot turn the rest of the shelf into unknown.
      }
    }
    result.set(id, value);
  }
  return result;
}
