import assert from "node:assert/strict";
import test from "node:test";

import { createReadingStatistics } from "./reader-state.mjs";

const CONTENT_VERSION = "a".repeat(64);

function memoryStorage(seed = {}) {
  const values = new Map(Object.entries(seed));
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, value),
  };
}

function harness({ wall = new Date(2026, 0, 8, 12).getTime(), storage = memoryStorage() } = {}) {
  let monotonic = 0;
  let wallClock = wall;
  const controls = {
    today: { textContent: "" },
    week: { textContent: "" },
    book: { textContent: "" },
    streak: { textContent: "" },
  };
  const statistics = createReadingStatistics({
    storage,
    keyPrefix: "atha.reader.probe",
    contentVersion: CONTENT_VERSION,
    controls,
    now: () => monotonic,
    wallNow: () => wallClock,
  });
  const advance = (milliseconds, tick = true) => {
    monotonic += milliseconds;
    wallClock += milliseconds;
    if (tick) statistics.tick();
  };
  return { advance, controls, statistics, storage };
}

test("只累计稳定、沉浸、可见、聚焦且未闲置的短区间", () => {
  const { advance, statistics } = harness();
  statistics.setStable(true);
  statistics.activity();

  advance(15_000);
  advance(5_000, false);
  statistics.setFocused(false);
  advance(20_000);
  assert.equal(statistics.snapshot().bookMs, 20_000);

  statistics.setFocused(true);
  advance(15_000);
  advance(5_000, false);
  statistics.setVisible(false);
  advance(15_000);
  assert.equal(statistics.snapshot().bookMs, 40_000);

  statistics.setVisible(true);
  statistics.setReading(false);
  advance(15_000);
  assert.equal(statistics.snapshot().bookMs, 40_000);
  statistics.setReading(true);
  for (let index = 0; index < 21; index += 1) advance(15_000);
  assert.equal(statistics.snapshot().bookMs, 340_000);
  statistics.activity();
  advance(15_000);
  assert.equal(statistics.snapshot().bookMs, 355_000);

  advance(2 * 60 * 60 * 1000);
  assert.equal(statistics.snapshot().bookMs, 355_000);
});

test("跨本地午夜拆分日统计并从同一记录恢复", () => {
  const storage = memoryStorage();
  const start = new Date(2026, 0, 8, 23, 59, 50).getTime();
  const first = harness({ wall: start, storage });
  first.statistics.setStable(true);
  first.statistics.activity();
  first.advance(20_000);

  const snapshot = first.statistics.snapshot();
  assert.equal(snapshot.todayMs, 10_000);
  assert.equal(snapshot.weekMs, 20_000);
  assert.equal(snapshot.bookMs, 20_000);
  assert.equal(snapshot.streakDays, 0);

  const reopened = harness({ wall: start + 20_000, storage });
  assert.deepEqual(
    reopened.statistics.snapshot(),
    expectSnapshot({ available: true, bookMs: 20_000, todayMs: 10_000, weekMs: 20_000 }),
  );
});

test("连续阅读要求每天至少一分钟，损坏与超限记录安全回退", () => {
  const key = "atha.reader.probe.statistics.v1";
  const today = "2026-01-08";
  const storage = memoryStorage({
    [key]: JSON.stringify({
      schema: 1,
      days: [
        { date: "2026-01-06", durationMs: 59_999 },
        { date: "2026-01-07", durationMs: 60_000 },
        { date: today, durationMs: 61_000 },
      ],
      books: [{ contentVersion: CONTENT_VERSION, durationMs: 180_999, lastReadDate: today }],
    }),
  });
  const valid = harness({ storage });
  assert.equal(valid.statistics.snapshot().streakDays, 2);
  assert.equal(valid.controls.streak.textContent, "2 天");

  for (const invalid of [
    "{",
    JSON.stringify({ schema: 2, days: [], books: [] }),
    JSON.stringify({
      schema: 1,
      days: Array.from({ length: 401 }, (_, index) => ({
        date: new Date(2025, 0, index + 1).toLocaleDateString("sv-SE"),
        durationMs: 1,
      })),
      books: [],
    }),
    JSON.stringify({
      schema: 1,
      days: [],
      books: Array.from({ length: 2049 }, (_, index) => ({
        contentVersion: index.toString(16).padStart(64, "0"),
        durationMs: 1,
        lastReadDate: today,
      })),
    }),
    "x".repeat(524_289),
  ]) {
    storage.setItem(key, invalid);
    const recovered = harness({ storage });
    assert.equal(recovered.statistics.snapshot().bookMs, 0);
    assert.equal(recovered.statistics.snapshot().lastFallback, "statistics-corrupt");
    assert.equal(storage.getItem(key), null);
  }
});

test("只保留当前本地日期窗口内的最近 400 天", () => {
  const key = "atha.reader.probe.statistics.v1";
  const wall = new Date(2026, 0, 8, 12).getTime();
  const date = (days) => new Date(2026, 0, 8 + days).toLocaleDateString("sv-SE");
  const storage = memoryStorage({
    [key]: JSON.stringify({
      schema: 1,
      days: [
        { date: date(-400), durationMs: 1 },
        { date: date(-399), durationMs: 2 },
        { date: date(0), durationMs: 3 },
        { date: date(1), durationMs: 4 },
      ],
      books: [{ contentVersion: CONTENT_VERSION, durationMs: 10, lastReadDate: date(0) }],
    }),
  });
  const value = harness({ wall, storage });
  value.statistics.setStable(true);
  value.statistics.activity();
  value.advance(15_000);

  const record = JSON.parse(storage.getItem(key));
  assert.deepEqual(record.days, [
    { date: date(-399), durationMs: 2 },
    { date: date(0), durationMs: 15_003 },
  ]);
});

test("基准走接近容量上限的完整心跳且不改产品记录", () => {
  const storage = memoryStorage();
  const setItem = storage.setItem;
  let largest = { days: 0, books: 0 };
  storage.setItem = (key, serialized) => {
    if (key.endsWith(".benchmark")) {
      const value = JSON.parse(serialized);
      largest = { days: value.days.length, books: value.books.length };
    }
    setItem(key, serialized);
  };
  const value = harness({ storage });
  const before = value.statistics.snapshot();
  const benchmark = value.statistics.benchmark();

  assert.equal(benchmark.samples, 20);
  assert.ok(benchmark.p95Ms >= 0);
  assert.deepEqual(largest, { days: 400, books: 2048 });
  assert.deepEqual(value.statistics.snapshot(), before);
  assert.equal(storage.values.has("atha.reader.probe.statistics.v1.benchmark"), false);
});

test("存储写入失败不阻断内存统计", () => {
  let attempts = 0;
  const storage = {
    getItem: () => null,
    removeItem: () => {},
    setItem: () => {
      attempts += 1;
      throw new Error("quota");
    },
  };
  const value = harness({ storage });
  value.statistics.setStable(true);
  value.statistics.activity();
  value.advance(15_000);
  value.advance(15_000);
  assert.equal(value.statistics.snapshot().bookMs, 30_000);
  assert.equal(value.statistics.snapshot().available, false);
  assert.equal(value.statistics.snapshot().lastFallback, "statistics-write");
  assert.equal(attempts, 1);
  assert.equal(value.controls.book.textContent, "<1 分钟");
});

function expectSnapshot(overrides) {
  return {
    available: true,
    durable: true,
    lastFallback: null,
    todayMs: 0,
    weekMs: 0,
    bookMs: 0,
    streakDays: 0,
    streakCapped: false,
    writes: 0,
    ...overrides,
  };
}
