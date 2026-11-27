import assert from "node:assert/strict";
import test from "node:test";

import {
  applyBookLocalStateDeletion,
  captureLocalDataState,
  readRecentBooks,
  replaceLocalDataState,
  type LibraryBook,
} from "../src/library.ts";
import { readShelfReading } from "../src/shelf-reading.ts";

const ids = ["a".repeat(64), "b".repeat(64), "c".repeat(64), "d".repeat(64), "e".repeat(64)];
const books: LibraryBook[] = ids.map((id) => ({
  id, title: id[0], authors: [], importedAt: 0, prepared: true, hasCover: false, hasCustomCover: false,
}));

function storage() {
  const records = new Map<string, string>();
  return {
    records,
    get length() { return records.size; },
    key: (index: number) => [...records.keys()][index] ?? null,
    setItem: (key: string, value: string) => { records.set(key, value); },
    getItem: (key: string) => records.get(key) ?? null,
    removeItem: (key: string) => { records.delete(key); },
  };
}

const progressKey = (id: string) => `atha.reader.progress.${id.slice(0, 16)}.v1`;
const factKey = (id: string) => `atha.reader.reading.${id}.v1`;
const date = "2026-01-08";
const dateAt = Date.parse(`${date}T12:00:00Z`);
const locator = (contentVersion: string, offset = 12) =>
  JSON.stringify({ schema: 1, contentVersion, start: { section: "chapter-1", offset } });
const progress = (contentVersion: string, offset = 12) =>
  JSON.stringify({ schema: 1, contentVersion, locator: locator(contentVersion, offset) });
const fact = (id: string, percent: number, lastReadAt: number, completed?: boolean) =>
  JSON.stringify({ schema: 1, locator: locator(id), percent, lastReadAt, lastReadDate: date, completed });

test("书架区分未读、在读、读完和损坏，且不借旧统计的时长猜出时间或百分比", () => {
  const target = storage();
  const [completed, started, damaged, unread, legacy] = ids;
  target.setItem(progressKey(completed), progress(completed));
  target.setItem(factKey(completed), fact(completed, 100, dateAt, true));
  target.setItem(progressKey(started), progress(started));
  target.setItem(factKey(started), "{");
  target.setItem(factKey(damaged), fact(unread, 100, dateAt));
  target.setItem("atha.reader.statistics.v1", JSON.stringify({
    schema: 1,
    days: [],
    books: [{ contentVersion: legacy, lastReadDate: date, durationMs: 10_000 }],
  }));
  target.setItem(`atha.reader.probe.reading.${unread}.v1`, fact(unread, 100, dateAt));
  target.setItem(`atha.reader.probe.progress.${unread.slice(0, 16)}.v1`, progress(unread));
  const before = [...target.records];
  assert.deepEqual([...readShelfReading(books, target).values()], [
    { status: "finished", percent: 100, lastReadAt: dateAt, lastReadDate: date },
    { status: "reading", percent: null, lastReadAt: null, lastReadDate: null },
    { status: "unknown", percent: null, lastReadAt: null, lastReadDate: null },
    { status: "unread", percent: null, lastReadAt: null, lastReadDate: null },
    { status: "reading", percent: null, lastReadAt: null, lastReadDate: date },
  ]);
  assert.deepEqual([...target.records], before);
  assert.deepEqual(readRecentBooks(books, target), [{ book: books[4], durationMs: 10_000, lastReadDate: date }]);
  assert.equal(readShelfReading(books, null).get(unread)?.status, "unknown");
});

test("同一书籍的历史日期仍可用，只有同一保存位置与完整版本的百分比可用", () => {
  const target = storage();
  const [first, second] = ids;
  target.setItem(progressKey(first), progress(first, 8));
  target.setItem(factKey(first), fact(first, 100, dateAt));
  target.setItem(progressKey(second), progress(second));
  target.setItem(factKey(second), fact(second, 39, dateAt + 1_000));
  let facts = readShelfReading(books, target);
  assert.deepEqual(facts.get(first), { status: "reading", percent: null, lastReadAt: dateAt, lastReadDate: date });
  assert.deepEqual(facts.get(second), { status: "reading", percent: 39, lastReadAt: dateAt + 1_000, lastReadDate: date });
  target.setItem("atha.reader.statistics.v1", JSON.stringify({
    schema: 1, days: [],
    books: [{ contentVersion: first, durationMs: 5_000, lastReadDate: "2026-01-09" }],
  }));
  facts = readShelfReading(books, target);
  assert.deepEqual(facts.get(first), { status: "reading", percent: null, lastReadAt: null, lastReadDate: "2026-01-09" });
  assert.deepEqual(facts.get(second), { status: "reading", percent: 39, lastReadAt: dateAt + 1_000, lastReadDate: date });
});

test("布局 100% 不等于已读完；确认读完后复读百分比也可变", () => {
  const target = storage();
  const [id] = ids;
  target.setItem(progressKey(id), progress(id));
  for (const completed of [undefined, false]) {
    target.setItem(factKey(id), fact(id, 100, dateAt, completed));
    assert.deepEqual(readShelfReading(books, target).get(id), {
      status: "reading", percent: 100, lastReadAt: dateAt, lastReadDate: date,
    });
  }
  target.setItem(factKey(id), fact(id, 50, dateAt, true));
  assert.deepEqual(readShelfReading(books, target).get(id), {
    status: "finished", percent: 50, lastReadAt: dateAt, lastReadDate: date,
  });
  target.setItem(progressKey(id), progress(id, 99));
  assert.deepEqual(readShelfReading(books, target).get(id), {
    status: "finished", percent: null, lastReadAt: dateAt, lastReadDate: date,
  });
});

test("新事实纳入备份替换和完整删除，旧进度不变、probe 仍不属于产品数据", () => {
  const target = storage();
  const [first, second] = ids;
  const probeKey = `atha.reader.probe.reading.${second}.v1`;
  target.setItem(progressKey(first), progress(first));
  target.setItem(factKey(first), fact(first, 61, dateAt));
  target.setItem(progressKey(second), progress(second));
  target.setItem(factKey(second), fact(second, 100, dateAt + 10, true));
  target.setItem(probeKey, fact(second, 1, dateAt));
  const backup = captureLocalDataState(target);
  assert.deepEqual(backup.records.map(({ key }) => key), [
    progressKey(first), progressKey(second), factKey(first), factKey(second),
  ]);

  const destination = storage();
  destination.setItem(probeKey, target.getItem(probeKey)!);
  replaceLocalDataState(backup, destination);
  assert.deepEqual(captureLocalDataState(destination), backup);
  assert.equal(readShelfReading(books, destination).get(second)?.lastReadAt, dateAt + 10);
  applyBookLocalStateDeletion(backup, second, destination);
  assert.equal(destination.getItem(progressKey(second)), null);
  assert.equal(destination.getItem(factKey(second)), null);
  assert.equal(destination.getItem(progressKey(first)), progress(first));
  assert.equal(destination.getItem(factKey(first)), fact(first, 61, dateAt));
  assert.equal(destination.getItem(probeKey), target.getItem(probeKey));
  assert.equal(readShelfReading(books, destination).get(second)?.status, "unread");

  for (const patch of [{ lastReadAt: -1 }, { lastReadDate: "2026-02-29" }, { percent: 101 }, { locator: locator(second) }, { completed: "true" }]) {
    destination.setItem(factKey(first), JSON.stringify({ ...JSON.parse(fact(first, 61, dateAt)), ...patch }));
    assert.throws(() => captureLocalDataState(destination), /invalid-browser-state/);
  }
});
