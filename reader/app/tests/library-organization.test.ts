import assert from "node:assert/strict";
import test from "node:test";
import { updateBookOrganization, type BookOrganization, type LibraryBook } from "../src/library.ts";

test("book organization uses the native library command and returns its current projection", async (t) => {
  const id = "a".repeat(64);
  const organization: BookOrganization = {
    favorite: true,
    tags: ["收藏标签"],
    subjects: [],
    series: ["长篇"],
  };
  const books: LibraryBook[] = [
    {
      id,
      title: "已整理",
      authors: [],
      hasCover: false,
      hasCustomCover: false,
      importedAt: 10,
      prepared: true,
      ...organization,
    },
  ];
  const invoke = t.mock.fn(async (_command: string, _args: unknown) => books);
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { __TAURI_INTERNALS__: { invoke } },
  });
  t.after(() => Reflect.deleteProperty(globalThis, "window"));

  assert.equal(await updateBookOrganization(id, organization), books);
  assert.equal(await updateBookOrganization(id, { favorite: false }), books);
  assert.deepEqual(invoke.mock.calls.map(({ arguments: args }) => args.slice(0, 2)), [
    ["update_library_book_organization", { id, organization }],
    ["update_library_book_organization", { id, organization: { favorite: false } }],
  ]);
});
