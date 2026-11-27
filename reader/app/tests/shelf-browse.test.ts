import assert from "node:assert/strict";
import test from "node:test";
import type { LibraryBook } from "../src/library.ts";
import type { ShelfReading } from "../src/shelf-reading.ts";
import {
  browseShelf, emptyFilters, indexShelf, parseOrganizationField, progressShelf,
  quickShelfFacets, recentShelf, shelfFacetOptions, shelfInitial, toggleFacet,
} from "../src/shelf-browse.ts";

const books: LibraryBook[] = [
  { title: "《海边的卡夫卡》", authors: ["村上春树"], tags: ["小说", "日本"], favorite: true, subjects: ["文学"] },
  { title: "A Room of One’s Own", authors: ["弗吉尼亚·伍尔夫"], tags: ["散文"], subjects: ["文学"] },
  { title: "百年孤独", authors: ["马尔克斯"], tags: ["小说"], series: ["世界文学"], subjects: ["文学"] },
  { title: "２４个比利", authors: ["丹尼尔·凯斯"], tags: ["传记", "心理"] },
  { title: "Éloge de l’ombre", authors: [], tags: ["散文"] },
  { title: "язык", authors: [] },
].map((fields, id) => ({
  id: String(id + 1), importedAt: id, prepared: true, hasCover: false, hasCustomCover: false, ...fields,
}));

const reading = new Map<string, ShelfReading>([
  ["1", { status: "reading", percent: null, lastReadAt: null, lastReadDate: "2026-10-04" }],
  ["2", { status: "finished", percent: 100, lastReadAt: 1, lastReadDate: "2026-10-04" }],
  ["3", { status: "unread", percent: null, lastReadAt: null, lastReadDate: null }],
]);
const entries = indexShelf(books, reading);
const ids = (result: ReturnType<typeof indexShelf>) => result.map(({ book }) => book.id);

test("全架拼音、拉丁、数字和其它文字可连续定位，旧排序仍可选", () => {
  assert.deepEqual(books.map((book) => shelfInitial(book.title)), ["H", "A", "B", "0–9", "E", "#"]);
  assert.deepEqual(["爱", "编", "沉", "大", "蛾", "冯", "高", "佳", "看", "鲁", "妈", "你", "欧", "评", "钱", "人", "散", "谈", "文", "新", "云", "做"].map(shelfInitial),
    ["A", "B", "C", "D", "E", "F", "G", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "W", "X", "Y", "Z"]);
  assert.deepEqual(ids(browseShelf(entries, "", emptyFilters(), "title")), ["2", "3", "5", "1", "4", "6"]);
  assert.deepEqual(ids(browseShelf(entries, "", emptyFilters(), "title-desc")), ["6", "4", "1", "5", "3", "2"]);
  assert.deepEqual(ids(browseShelf(entries, "", emptyFilters(), "added")), ["6", "5", "4", "3", "2", "1"]);
  assert.deepEqual(ids(browseShelf(entries, "", emptyFilters(), "author")).slice(-2), ["5", "6"]);
});

test("同维为或，跨维、收藏和关键词为且；快捷标签按全架而非结果", () => {
  let filters = toggleFacet(toggleFacet(emptyFilters(), "tags", "小说"), "tags", "散文");
  assert.deepEqual(ids(browseShelf(entries, "", filters, "title")), ["2", "3", "5", "1"]);
  filters = toggleFacet(filters, "subjects", "文学");
  assert.deepEqual(ids(browseShelf(entries, "", filters, "title")), ["2", "3", "1"]);
  assert.deepEqual(ids(browseShelf(entries, "卡夫卡 日本", { ...filters, favorite: true }, "title")), ["1"]);
  assert.deepEqual(ids(browseShelf(entries, "丹尼尔", filters, "title")), []);
  assert.deepEqual(ids(browseShelf(entries, "丹尼尔", emptyFilters(), "title")), ["4"]);
  assert.deepEqual(quickShelfFacets(books), quickShelfFacets([...books].reverse()));
  const draft = shelfFacetOptions(entries, "", filters);
  assert.equal(draft.matches, 3);
  assert.deepEqual(draft.groups.find(({ key }) => key === "tags")?.options.map(({ value, count }) => [value, count]),
    [["传记", 0], ["日本", 1], ["散文", 1], ["小说", 2], ["心理", 0]]);
});

test("快捷条件先列标签，再补来源主题；同名时保留标签身份", () => {
  const sourceOnly = books.map((book, index) => ({ ...book, tags: [], subjects: index === 3 ? ["纪实"] : book.subjects }));
  assert.deepEqual(quickShelfFacets(sourceOnly), [
    { key: "subjects", value: "文学" }, { key: "subjects", value: "纪实" },
  ]);
  sourceOnly[0].tags = ["文学"];
  assert.deepEqual(quickShelfFacets(sourceOnly), [
    { key: "tags", value: "文学" }, { key: "subjects", value: "纪实" },
  ]);
  const [selected] = quickShelfFacets(sourceOnly);
  const filtered = browseShelf(indexShelf(sourceOnly, reading), "",
    toggleFacet(emptyFilters(), selected.key, selected.value), "title");
  assert.deepEqual(ids(filtered), ["1"]);
});

test("最近阅读与进度分组保留日期精度和未知状态", () => {
  assert.deepEqual(ids(recentShelf(entries)), ["2", "1"]);
  assert.deepEqual(ids(browseShelf(entries, "", emptyFilters(), "recent")).slice(0, 2), ["2", "1"]);
  const grouped = progressShelf(entries);
  assert.deepEqual(ids(grouped.reading), ["1", "2"]);
  assert.deepEqual(ids(grouped.unread), ["3"]);
  assert.deepEqual(ids(grouped.unknown), ["4", "5", "6"]);
  assert.deepEqual(parseOrganizationField(" 小说 \n小说\r\n  \n文学、语言\n"), ["小说", "文学、语言"]);
});
