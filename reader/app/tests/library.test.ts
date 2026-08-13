import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import {
  captureLocalDataState,
  applyBookLocalStateDeletion,
  deleteBooksSerially,
  filterLibraryBooks,
  groupLibraryBooksByProgress,
  openFailureMessage,
  readAppTheme,
  readRecentBooks,
  resetReaderApplicationPreferences,
  readStartedBookIds,
  replaceLocalDataState,
  removeBooksSerially,
  validateLocalDataState,
  writeAppTheme,
  withoutBookLocalState,
  type BrowserState,
  type LibraryBook,
} from "../src/library.ts";

const ids = ["a".repeat(64), "b".repeat(64), "c".repeat(64)];
const books: LibraryBook[] = [
  { id: ids[1], title: "Zeta", authors: ["Beta"], hasCover: true, importedAt: 30, prepared: true },
  { id: ids[0], title: "Alpha", authors: ["Gamma"], hasCover: false, importedAt: 20, prepared: true },
  { id: ids[2], title: "Middle", authors: [], hasCover: false, importedAt: 10, prepared: false },
];

function progress(id: string) {
  return JSON.stringify({
    schema: 1,
    contentVersion: id,
    locator: JSON.stringify({
      schema: 1,
      contentVersion: id,
      start: { section: "chapter-1", offset: 12 },
    }),
  });
}

function annotation(id: string, contentVersion: string, selectedText = "x") {
  return {
    id,
    type: "highlight",
    note: "",
    createdAt: 1,
    updatedAt: 1,
    deletedAt: null,
    sourceAnchor: {
      schema: 1,
      canonicalLocator: JSON.stringify({
        schema: 1,
        contentVersion,
        start: { section: "chapter-1", offset: 0 },
        end: { section: "chapter-1", offset: selectedText.length },
      }),
      selectedText,
      prefixText: "",
      suffixText: "",
      contentHash: createHash("sha256").update(selectedText).digest("hex"),
    },
  };
}

function application(theme = "system") {
  return JSON.stringify({
    schema: 1,
    preferences: {
      theme,
      brightness: 100,
      fontSize: 19,
      fontFamily: "book",
      density: "standard",
    },
  });
}

function bookPreferences() {
  return {
    sourceStyles: true,
    userStylesEnabled: true,
    readingMode: "paged",
    pageMargin: "standard",
    paragraphIndent: "none",
    paragraphSpacing: "book",
    styleModules: [],
  };
}

test("library projection searches locally and keeps deterministic views", () => {
  assert.deepEqual(filterLibraryBooks(books, "  beta ", "default").map((book) => book.id), [ids[1]]);
  assert.deepEqual(filterLibraryBooks(books, "alpha", "default").map((book) => book.id), [ids[0]]);
  assert.deepEqual(filterLibraryBooks(books, "", "default").map((book) => book.id), [
    ids[1],
    ids[0],
    ids[2],
  ]);
  assert.deepEqual(filterLibraryBooks(books, "", "title").map((book) => book.title), [
    "Alpha",
    "Middle",
    "Zeta",
  ]);
  assert.deepEqual(filterLibraryBooks(books, "", "author").map((book) => book.title), [
    "Zeta",
    "Alpha",
    "Middle",
  ]);

  const tied = [
    { ...books[0], id: ids[2], title: "Same", authors: ["Same"] },
    { ...books[1], id: ids[0], title: "Same", authors: ["Same"] },
  ];
  assert.deepEqual(filterLibraryBooks(tied, "", "title").map((book) => book.id), [ids[2], ids[0]]);
  assert.deepEqual(filterLibraryBooks(tied, "", "author").map((book) => book.id), [ids[2], ids[0]]);
});

test("progress view accepts only a strict same-book progress record", () => {
  const records = new Map<string, string | null>([
    [`atha.reader.progress.${ids[0].slice(0, 16)}.v1`, progress(ids[0])],
    [
      `atha.reader.progress.${ids[1].slice(0, 16)}.v1`,
      JSON.stringify({ ...JSON.parse(progress(ids[1])), extra: true }),
    ],
    [
      `atha.reader.progress.${ids[2].slice(0, 16)}.v1`,
      JSON.stringify({ ...JSON.parse(progress(ids[2])), contentVersion: ids[0] }),
    ],
  ]);
  const started = readStartedBookIds(books, { getItem: (key) => records.get(key) ?? null });
  assert.deepEqual(started, new Set([ids[0]]));
  assert.deepEqual(groupLibraryBooksByProgress(books, started!), {
    reading: [books[1]],
    unread: [books[0], books[2]],
  });

  for (const raw of [
    "x".repeat(524_289),
    JSON.stringify({ ...JSON.parse(progress(ids[0])), schema: 2 }),
    JSON.stringify({ ...JSON.parse(progress(ids[0])), locator: "{}" }),
  ]) {
    assert.deepEqual(readStartedBookIds([books[1]], { getItem: () => raw }), new Set());
  }

  assert.equal(
    readStartedBookIds(books, {
      getItem() {
        throw new Error("storage-disabled");
      },
    }),
    null,
  );
});

test("recent reading uses strict statistics and only current library books", () => {
  const storage = new TestStorage([
    [
      "atha.reader.statistics.v1",
      JSON.stringify({
        schema: 1,
        days: [{ date: "2026-08-13", durationMs: 90_000 }],
        books: [
          { contentVersion: ids[0], durationMs: 60_000, lastReadDate: "2026-08-12" },
          { contentVersion: ids[1], durationMs: 30_000, lastReadDate: "2026-08-13" },
          { contentVersion: "d".repeat(64), durationMs: 10_000, lastReadDate: "2026-08-13" },
        ],
      }),
    ],
  ]);
  assert.deepEqual(readRecentBooks(books, storage), [
    { book: books[0], durationMs: 30_000, lastReadDate: "2026-08-13" },
    { book: books[1], durationMs: 60_000, lastReadDate: "2026-08-12" },
  ]);
  assert.deepEqual(readRecentBooks(books, new TestStorage()), []);

  storage.setItem(
    "atha.reader.statistics.v1",
    JSON.stringify({ schema: 1, days: [], books: [{ contentVersion: ids[0] }] }),
  );
  assert.equal(readRecentBooks(books, storage), null);
  assert.equal(
    readRecentBooks(books, {
      getItem() {
        throw new Error("storage-disabled");
      },
    }),
    null,
  );
});

test("application appearance stays separate from reader preferences", () => {
  const storage = new TestStorage([["atha.reader.application.v1", application("paper")]]);
  assert.equal(readAppTheme(storage), "system");

  writeAppTheme("light", storage);
  assert.equal(readAppTheme(storage), "light");
  assert.equal(JSON.parse(storage.getItem("atha.reader.application.v1")!).preferences.theme, "paper");
  assert.deepEqual(JSON.parse(storage.getItem("atha.app.appearance.v1")!), {
    schema: 1,
    theme: "light",
  });
  assert.deepEqual(captureLocalDataState(storage).records.map((record) => record.key), [
    "atha.reader.application.v1",
  ]);

  storage.setItem("atha.app.appearance.v1", JSON.stringify({ schema: 1, theme: "paper" }));
  assert.equal(readAppTheme(storage), "system");
  assert.throws(() => writeAppTheme("paper" as never, storage), /invalid-app-theme/);
});

test("application settings reset reading defaults without touching app appearance", () => {
  const storage = new TestStorage([
    ["atha.app.appearance.v1", JSON.stringify({ schema: 1, theme: "dark" })],
    ["atha.reader.application.v1", application("paper")],
  ]);
  assert.equal(resetReaderApplicationPreferences(storage), true);
  assert.deepEqual(JSON.parse(storage.getItem("atha.reader.application.v1")!), {
    schema: 1,
    preferences: {
      theme: "system",
      brightness: 100,
      fontSize: 19,
      fontFamily: "book",
      density: "standard",
    },
  });
  assert.equal(readAppTheme(storage), "dark");

  storage.setItem("atha.reader.application.v1", "{}");
  assert.equal(resetReaderApplicationPreferences(storage), false);
  assert.equal(storage.getItem("atha.reader.application.v1"), "{}");
});

test("serial removal stops on failure and reports unprocessed books", async () => {
  const calls: string[] = [];
  const result = await removeBooksSerially(ids, async (id) => {
    calls.push(id);
    if (id === ids[1]) throw new Error("remove-failed");
    return books.filter((book) => book.id !== id);
  });

  assert.deepEqual(calls, [ids[0], ids[1]]);
  assert.deepEqual(result.removedIds, [ids[0]]);
  assert.deepEqual(result.remainingIds, [ids[1], ids[2]]);
  assert.deepEqual(result.books, books.filter((book) => book.id !== ids[0]));
});

test("first-open errors keep deterministic format guidance", () => {
  assert.equal(openFailureMessage("encrypted-epub"), "暂不支持受保护的 EPUB");
  assert.equal(
    openFailureMessage(new Error("invalid-epub-source")),
    "已保存的书籍内容不可读取，请重新导入",
  );
  assert.equal(openFailureMessage({ code: "encrypted-epub" }), "无法打开书籍");
});

test("local data capture is production-only, sorted, and strict", () => {
  const storage = new TestStorage([
    ["unrelated", "keep"],
    ["atha.reader.probe.application.v1", application()],
    [`atha.reader.progress.${ids[0].slice(0, 16)}.v1`, progress(ids[0])],
    ["atha.reader.application.v1", application()],
  ]);
  const state = captureLocalDataState(storage);
  assert.deepEqual(state.records.map((record) => record.key), [
    "atha.reader.application.v1",
    `atha.reader.progress.${ids[0].slice(0, 16)}.v1`,
  ]);
  assert.equal(validateLocalDataState(state), true);
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{ key: "atha.reader.probe.application.v1", value: "{}" }],
    }),
    false,
  );
  for (const [key, value] of [
    [
      "atha.reader.statistics.v1",
      { schema: 1, days: [{ date: "2026-02-30", durationMs: 1 }], books: [] },
    ],
    [
      `atha.reader.book.${ids[0].slice(0, 16)}.v1`,
      {
        schema: 1,
        bookmarks: [],
        preferences: {
          ...bookPreferences(),
          styleModules: [{ id: "unsafe", name: "Unsafe", group: "", enabled: true, css: "@import 'x';" }],
        },
      },
    ],
    [
      `atha.reader.annotations.${ids[0].slice(0, 16)}.v1`,
      { schema: 1, items: [{ id: "partial" }] },
    ],
  ] as const) {
    assert.equal(
      validateLocalDataState({ schema: 1, records: [{ key, value: JSON.stringify(value) }] }),
      false,
    );
  }

  storage.setItem("atha.reader.application.v1", "{}");
  assert.throws(() => captureLocalDataState(storage), /invalid-browser-state/);
});

test("browser state replacement rolls storage back after a write failure", () => {
  const previous = application("light");
  const next = application("dark");
  const storage = new TestStorage([
    ["atha.reader.application.v1", previous],
    ["unrelated", "keep"],
  ]);
  storage.failValue = next;
  assert.throws(
    () =>
      replaceLocalDataState(
        { schema: 1, records: [{ key: "atha.reader.application.v1", value: next }] },
        storage,
      ),
    /browser-state-write-failed/,
  );
  assert.equal(storage.getItem("atha.reader.application.v1"), previous);
  assert.equal(storage.getItem("unrelated"), "keep");
});

test("physical book deletion removes only that book's browser state", async () => {
  const statistics = {
    schema: 1,
    days: [{ date: "2026-08-13", durationMs: 60_000 }],
    books: [
      { contentVersion: ids[0], durationMs: 60_000, lastReadDate: "2026-08-13" },
      { contentVersion: ids[1], durationMs: 30_000, lastReadDate: "2026-08-12" },
    ],
  };
  const state: BrowserState = {
    schema: 1,
    records: [
      { key: "atha.reader.application.v1", value: application() },
      {
        key: `atha.reader.book.${ids[0].slice(0, 16)}.v1`,
        value: JSON.stringify({ schema: 1, bookmarks: [], preferences: bookPreferences() }),
      },
      {
        key: `atha.reader.progress.${ids[0].slice(0, 16)}.v1`,
        value: progress(ids[0]),
      },
      { key: "atha.reader.statistics.v1", value: JSON.stringify(statistics) },
    ],
  };
  const next = withoutBookLocalState(state, ids[0]);
  assert.deepEqual(next.records.map((record) => record.key), [
    "atha.reader.application.v1",
    "atha.reader.statistics.v1",
  ]);
  assert.deepEqual(JSON.parse(next.records[1].value), {
    ...statistics,
    books: [statistics.books[1]],
  });

  const storage = new TestStorage(state.records.map((record) => [record.key, record.value]));
  const calls: string[] = [];
  const result = await deleteBooksSerially(
    [ids[0]],
    storage,
    async (id) => {
      calls.push(id);
      return { id };
    },
    async (id) => {
      return books.filter((book) => book.id !== id);
    },
  );
  assert.deepEqual(calls, [ids[0]]);
  assert.deepEqual(result.removedIds, [ids[0]]);
  assert.equal(storage.getItem(`atha.reader.progress.${ids[0].slice(0, 16)}.v1`), null);
  assert.equal(storage.getItem("atha.reader.application.v1"), state.records[0].value);

  const failedStorage = new TestStorage(state.records.map((record) => [record.key, record.value]));
  const failed = await deleteBooksSerially(
    [ids[0]],
    failedStorage,
    async () => {
      throw new Error("delete-failed");
    },
  );
  assert.deepEqual(failed.removedIds, []);
  assert.deepEqual(failed.remainingIds, [ids[0]]);
  assert.equal(failed.pendingRecovery, true);
  assert.equal(
    failedStorage.getItem(`atha.reader.progress.${ids[0].slice(0, 16)}.v1`),
    state.records[2].value,
  );

  const writeFailedStorage = new TestStorage(
    state.records.map((record) => [record.key, record.value]),
  );
  writeFailedStorage.failValue = JSON.stringify({
    ...statistics,
    books: [statistics.books[1]],
  });
  let finished = false;
  const writeFailed = await deleteBooksSerially(
    [ids[0]],
    writeFailedStorage,
    async (id) => ({ id }),
    async () => {
      finished = true;
      return [];
    },
  );
  assert.deepEqual(writeFailed.remainingIds, [ids[0]]);
  assert.equal(writeFailed.pendingRecovery, true);
  assert.equal(finished, false);
  assert.equal(
    writeFailedStorage.getItem(`atha.reader.progress.${ids[0].slice(0, 16)}.v1`),
    null,
  );
});

test("book deletion handles path keys without rewriting unrelated state", () => {
  const pathKey = "0123456789abcdef";
  const otherKey = ids[1].slice(0, 16);
  const pathProgress = progress(ids[0]);
  const storage = new TestStorage([
    ["atha.reader.application.v1", application()],
    [`atha.reader.book.${pathKey}.v1`, JSON.stringify({ schema: 1, bookmarks: [], preferences: bookPreferences() })],
    [`atha.reader.progress.${pathKey}.v1`, pathProgress],
    [`atha.reader.progress.${otherKey}.v1`, progress(ids[1])],
  ]);
  const before = captureLocalDataState(storage);
  applyBookLocalStateDeletion(before, ids[0], storage);
  assert.equal(storage.getItem(`atha.reader.book.${pathKey}.v1`), null);
  assert.equal(storage.getItem(`atha.reader.progress.${pathKey}.v1`), null);
  assert.equal(storage.getItem(`atha.reader.progress.${otherKey}.v1`), progress(ids[1]));
  assert.equal(storage.getItem("atha.reader.application.v1"), application());

  applyBookLocalStateDeletion(captureLocalDataState(storage), ids[0], storage);
  assert.equal(storage.getItem(`atha.reader.progress.${otherKey}.v1`), progress(ids[1]));

  const bookmark = (id: string, contentVersion: string) => ({
    id,
    label: id,
    locator: JSON.stringify({
      schema: 1,
      contentVersion,
      start: { section: "chapter-1", offset: 1 },
    }),
  });
  const currentPreferences = { ...bookPreferences(), pageMargin: "wide" };
  const oldAnnotation = annotation("old-note", ids[0]);
  const currentAnnotation = annotation("current-note", ids[1]);
  const reusedPath = new TestStorage([
    [`atha.reader.book.${pathKey}.v1`, JSON.stringify({
      schema: 1,
      bookmarks: [bookmark("old", ids[0]), bookmark("current", ids[1])],
      preferences: currentPreferences,
    })],
    [`atha.reader.annotations.${pathKey}.v1`, JSON.stringify({
      schema: 1,
      items: [oldAnnotation, currentAnnotation],
    })],
    [`atha.reader.progress.${pathKey}.v1`, progress(ids[1])],
  ]);
  applyBookLocalStateDeletion(captureLocalDataState(reusedPath), ids[1], reusedPath);
  assert.equal(reusedPath.getItem(`atha.reader.progress.${pathKey}.v1`), null);
  assert.deepEqual(JSON.parse(reusedPath.getItem(`atha.reader.book.${pathKey}.v1`)!), {
    schema: 1,
    bookmarks: [bookmark("old", ids[0])],
    preferences: bookPreferences(),
  });
  assert.deepEqual(
    JSON.parse(reusedPath.getItem(`atha.reader.annotations.${pathKey}.v1`)!).items,
    [oldAnnotation],
  );
  applyBookLocalStateDeletion(captureLocalDataState(reusedPath), ids[0], reusedPath);
  assert.equal(reusedPath.getItem(`atha.reader.book.${pathKey}.v1`), null);
  assert.equal(reusedPath.getItem(`atha.reader.annotations.${pathKey}.v1`), null);

  const staleOnly = new TestStorage([
    [`atha.reader.book.${pathKey}.v1`, JSON.stringify({
      schema: 1,
      bookmarks: [bookmark("stale", ids[0])],
      preferences: currentPreferences,
    })],
    [`atha.reader.progress.${pathKey}.v1`, progress(ids[1])],
  ]);
  applyBookLocalStateDeletion(captureLocalDataState(staleOnly), ids[0], staleOnly);
  assert.deepEqual(JSON.parse(staleOnly.getItem(`atha.reader.book.${pathKey}.v1`)!), {
    schema: 1,
    bookmarks: [],
    preferences: currentPreferences,
  });
  assert.equal(staleOnly.getItem(`atha.reader.progress.${pathKey}.v1`), progress(ids[1]));
});

test("local data accepts owner-sized CJK annotations and rejects syntactically empty CSS", () => {
  const selectedText = "中".repeat(4_096);
  const contentHash = createHash("sha256").update(selectedText).digest("hex");
  const items = Array.from({ length: 300 }, (_, index) => ({
    id: `large-${index}`,
    type: "highlight",
    note: "",
    createdAt: index + 1,
    updatedAt: index + 1,
    deletedAt: null,
    sourceAnchor: {
      schema: 1,
      canonicalLocator: JSON.stringify({
        schema: 1,
        contentVersion: ids[0],
        start: { section: "chapter-1", offset: 0 },
        end: { section: "chapter-1", offset: 4_096 },
      }),
      selectedText,
      prefixText: "",
      suffixText: "",
      contentHash,
    },
  }));
  const annotations = JSON.stringify({ schema: 1, items });
  assert.ok(annotations.length < 2 * 1024 * 1024);
  assert.ok(new TextEncoder().encode(annotations).byteLength > 2 * 1024 * 1024);
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{ key: `atha.reader.annotations.${ids[0].slice(0, 16)}.v1`, value: annotations }],
    }),
    true,
  );
  const pathKey = "0123456789abcdef";
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{ key: `atha.reader.progress.${pathKey}.v1`, value: progress(ids[0]) }],
    }),
    true,
  );
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [
        { key: `atha.reader.annotations.${pathKey}.v1`, value: annotations },
        { key: `atha.reader.progress.${pathKey}.v1`, value: progress(ids[1]) },
      ],
    }),
    true,
  );
  const oversizedLegacyCss = `/*${"汉".repeat(22_000)}*/`;
  assert.ok(new TextEncoder().encode(oversizedLegacyCss).byteLength > 65_536);
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{
        key: `atha.reader.book.${pathKey}.v1`,
        value: JSON.stringify({
          schema: 1,
          bookmarks: [],
          preferences: {
            ...bookPreferences(),
            styleModules: [{
              id: "legacy-user-css",
              name: "原有自定义样式",
              group: "迁移",
              enabled: false,
              css: oversizedLegacyCss,
            }],
          },
        }),
      }],
    }),
    true,
  );
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{
        key: `atha.reader.book.${pathKey}.v1`,
        value: JSON.stringify({
          schema: 1,
          bookmarks: [],
          preferences: {
            sourceStyles: true,
            userStylesEnabled: true,
            userStylesheet: oversizedLegacyCss,
          },
        }),
      }],
    }),
    true,
  );
  const dictionary = JSON.stringify({ schema: 1, dictionaryId: "", fontScale: 1 }).padEnd(1_025, " ");
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{ key: "atha.reader.dictionary.preferences.v1", value: dictionary }],
    }),
    false,
  );
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{
        key: `atha.reader.book.${ids[0].slice(0, 16)}.v1`,
        value: JSON.stringify({
          schema: 1,
          bookmarks: [],
          preferences: {
            ...bookPreferences(),
            styleModules: [{ id: "invalid", name: "Invalid", group: "", enabled: true, css: "not css" }],
          },
        }),
      }],
    }),
    false,
  );
  const oversizedBook = JSON.stringify({
    schema: 1,
    bookmarks: [],
    preferences: {
      ...bookPreferences(),
      styleModules: Array.from({ length: 17 }, (_, index) => ({
        id: `module-${index}`,
        name: `Module ${index}`,
        group: "",
        enabled: false,
        css: "x".repeat(32_768),
      })),
    },
  });
  assert.ok(oversizedBook.length > 524_288);
  assert.equal(
    validateLocalDataState({
      schema: 1,
      records: [{ key: `atha.reader.book.${ids[0].slice(0, 16)}.v1`, value: oversizedBook }],
    }),
    false,
  );
});

class TestStorage {
  readonly values = new Map<string, string>();
  failValue: string | null = null;

  constructor(entries: Iterable<readonly [string, string]> = []) {
    for (const [key, value] of entries) this.values.set(key, value);
  }

  get length() {
    return this.values.size;
  }

  key(index: number) {
    return [...this.values.keys()][index] ?? null;
  }

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    if (value === this.failValue) {
      this.failValue = null;
      throw new Error("quota");
    }
    this.values.set(key, value);
  }

  removeItem(key: string) {
    this.values.delete(key);
  }
}
