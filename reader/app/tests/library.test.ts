import assert from "node:assert/strict";
import test from "node:test";

import {
  filterLibraryBooks,
  groupLibraryBooksByProgress,
  readStartedBookIds,
  removeBooksSerially,
  type LibraryBook,
} from "../src/library.ts";

const ids = ["a".repeat(64), "b".repeat(64), "c".repeat(64)];
const books: LibraryBook[] = [
  { id: ids[1], title: "Zeta", authors: ["Beta"], hasCover: true, importedAt: 30 },
  { id: ids[0], title: "Alpha", authors: ["Gamma"], hasCover: false, importedAt: 20 },
  { id: ids[2], title: "Middle", authors: [], hasCover: false, importedAt: 10 },
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
