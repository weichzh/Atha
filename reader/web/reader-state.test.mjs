import assert from "node:assert/strict";
import test from "node:test";

import { createLocator } from "./locator.mjs";
import { readingPercent } from "./navigation.mjs";
import { createReadingScrollWitness } from "./pagination.mjs";
import { createReaderState, createReadingStatistics } from "./reader-state.mjs";

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

function readingSession(storage, version, {
  keyPrefix = "atha.reader", recordShelfReading = true, bookKey = version.slice(0, 16), singleSection = false,
} = {}) {
  let offset = 0;
  let lastPage = false;
  let scroll = false;
  let pageCount = singleSection ? 2 : 5;
  let monotonic = 0;
  let wall = new Date(2026, 0, 8, 12).getTime();
  const book = { contentVersion: version, sections: [{ id: "intro" }, ...(singleSection ? [] : [{ id: "end" }])] };
  const locator = createLocator({ assert: assert.ok });
  const preferences = { effective: { fontSize: 19 }, application: {}, book: {} };
  const readingScroll = createReadingScrollWitness(() => offset, () => monotonic, () => wall);
  const state = createReaderState({
    storage, bookKey, keyPrefix, locator, recordShelfReading, wallNow: () => wall,
    session: { describe: () => book, snapshot: () => ({ state: "layout-stable" }) },
    navigation: {
      current: () => locator.point(book, lastPage ? "end" : "intro", offset),
      currentPercent: (flushScroll) =>
        readingPercent(lastPage ? 1 : 0, book.sections.length, 0, lastPage || flushScroll ? 1 : pageCount),
      goTo: (value) => {
        const point = locator.parse(book, value).start;
        offset = point.offset;
        lastPage = point.section === "end";
        state.scheduleProgress(true);
        return true;
      },
    },
    pagination: { setFontSize: () => {}, isScrolled: () => scroll, takeReadingScroll: readingScroll.take },
    preferences: { restore: () => preferences, snapshot: () => preferences },
    assert: assert.ok,
  });
  return {
    state,
    key: `atha.reader.reading.${version}.v1`,
    progressKey: `${keyPrefix}.progress.${bookKey}.v1`,
    advance: (delta) => { wall += delta; },
    move: (position, end = false) => { offset = position; lastPage = end; },
    useScroll: () => { scroll = true; },
    reflow: (pages) => { pageCount = pages; },
    scrollNow(next, { user = false, sequence = 1 } = {}) {
      if (user) readingScroll.noteInput();
      else readingScroll.reset();
      offset = next;
      monotonic += sequence;
      readingScroll.observe();
    },
  };
}

test("真实打开更新同日顺序，恢复定位和重新排版不生成阅读时间", async () => {
  const first = CONTENT_VERSION;
  const second = "b".repeat(64);
  const storage = memoryStorage();
  const a = readingSession(storage, first);
  await a.state.restore();
  assert.equal(storage.getItem(a.key), null);
  a.state.activateShelfReading();
  a.state.flushProgress();
  const initial = JSON.parse(storage.getItem(a.key));
  assert.equal(initial.percent, 10);
  assert.equal(initial.lastReadDate, "2026-01-08");
  assert.deepEqual(Object.keys(JSON.parse(storage.getItem(a.progressKey))).sort(), [
    "contentVersion", "locator", "schema",
  ]);

  const b = readingSession(storage, second);
  b.advance(1_000);
  await b.state.restore();
  b.state.activateShelfReading();
  b.state.flushProgress();
  const secondEntry = JSON.parse(storage.getItem(b.key));
  assert.equal(secondEntry.lastReadDate, initial.lastReadDate);
  assert.ok(secondEntry.lastReadAt > initial.lastReadAt);

  const reopened = readingSession(storage, first);
  reopened.advance(2_000);
  await reopened.state.restore();
  reopened.state.flushProgress();
  assert.deepEqual(JSON.parse(storage.getItem(a.key)), initial);
  reopened.state.activateShelfReading();
  reopened.state.flushProgress();
  const latest = JSON.parse(storage.getItem(a.key));
  assert.ok(latest.lastReadAt > secondEntry.lastReadAt);
  reopened.advance(15_000);
  reopened.move(42);
  reopened.state.scheduleProgress();
  reopened.state.flushProgress();
  assert.equal(JSON.parse(storage.getItem(a.key)).lastReadAt, latest.lastReadAt);

  reopened.advance(15_000);
  reopened.move(12, true);
  reopened.state.scheduleProgress(true, true);
  reopened.state.flushOnExit();
  const finished = JSON.parse(storage.getItem(a.key));
  assert.equal(finished.percent, 100);
  assert.equal(finished.completed, true);
  assert.equal(JSON.parse(finished.locator).start.offset, 12);
  assert.ok(finished.lastReadAt > latest.lastReadAt);
});

test("同一位置只重排 2→1→2 页不增删完读；单页需主动确认到底", async () => {
  const storage = memoryStorage();
  const current = readingSession(storage, CONTENT_VERSION, { singleSection: true });
  await current.state.restore();
  const reopen = () => {
    current.state.activateShelfReading();
    current.state.flushProgress();
    return JSON.parse(storage.getItem(current.key));
  };
  const reflow = (pages) => {
    current.reflow(pages);
    current.state.scheduleProgress(false);
    current.state.flushProgress();
    return JSON.parse(storage.getItem(current.key));
  };
  const opened = reopen();
  assert.equal(opened.percent, 50);
  assert.equal(opened.completed, false);
  for (const [pages, percent] of [[1, 100], [2, 50], [1, 100]]) {
    const changed = reflow(pages);
    assert.equal(changed.percent, percent);
    assert.equal(changed.completed, false);
    assert.equal(changed.locator, opened.locator);
    assert.equal(changed.lastReadAt, opened.lastReadAt);
  }
  current.state.scheduleProgress(true, true);
  current.state.flushProgress();
  const stillCompleted = reflow(2);
  assert.equal(stillCompleted.percent, 50);
  assert.equal(stillCompleted.completed, true);
  assert.equal(stillCompleted.locator, opened.locator);

  const resumed = readingSession(storage, CONTENT_VERSION, { singleSection: true });
  await resumed.state.restore();
  resumed.state.activateShelfReading();
  resumed.state.flushProgress();
  assert.equal(JSON.parse(storage.getItem(resumed.key)).completed, true);

  const firstPageStorage = memoryStorage();
  const singlePage = readingSession(firstPageStorage, CONTENT_VERSION, { singleSection: true });
  singlePage.reflow(1);
  await singlePage.state.restore();
  singlePage.state.activateShelfReading();
  singlePage.state.flushProgress();
  const observed = JSON.parse(firstPageStorage.getItem(singlePage.key));
  assert.equal(observed.percent, 100);
  assert.equal(observed.completed, false);
});

test("只把真实滚动记为跨日阅读，程序回位可同步位置但不推进日期", async () => {
  const storage = memoryStorage();
  const current = readingSession(storage, CONTENT_VERSION);
  await current.state.restore();
  current.state.activateShelfReading();
  current.state.flushProgress();
  const first = JSON.parse(storage.getItem(current.key));
  current.useScroll();
  current.advance(24 * 60 * 60 * 1_000);
  current.scrollNow(15);
  current.state.flushOnExit();
  assert.equal(JSON.parse(storage.getItem(current.key)).lastReadAt, first.lastReadAt);
  current.scrollNow(20, { user: true });
  const observed = first.lastReadAt + 24 * 60 * 60 * 1_000;
  current.advance(1_500);
  current.state.flushOnExit();
  const scrolled = JSON.parse(storage.getItem(current.key));
  assert.equal(scrolled.lastReadAt, observed);
  assert.equal(scrolled.lastReadDate, "2026-01-09");
  assert.equal(JSON.parse(scrolled.locator).start.offset, 20);
  assert.equal(scrolled.completed, false);
});

test("只在真正末页报告 100%，即时返回同步尚未 idle 的滚动位置", async () => {
  assert.equal(readingPercent(9, 10, 98, 100), 99);
  assert.equal(readingPercent(9, 10, 99, 100), 100);
  assert.equal(readingPercent(0, 0, 0, 0), null);

  const storage = memoryStorage();
  const current = readingSession(storage, CONTENT_VERSION);
  await current.state.restore();
  current.state.activateShelfReading();
  current.state.flushProgress();
  const initial = JSON.parse(storage.getItem(current.key));
  current.useScroll();
  current.move(19);
  current.state.flushOnExit();
  const settled = JSON.parse(storage.getItem(current.key));
  assert.equal(settled.lastReadAt, initial.lastReadAt);
  assert.equal(settled.percent, 50);
  assert.equal(JSON.parse(settled.locator).start.offset, 19);
  assert.equal(JSON.parse(storage.getItem(current.progressKey)).locator, settled.locator);
});

test("诊断探针与旧独立 host 不能写正式阅读事实", async () => {
  const storage = memoryStorage();
  for (const config of [
    { keyPrefix: "atha.reader.probe" },
    { recordShelfReading: false },
    { bookKey: "path-derived" },
  ]) {
    const current = readingSession(storage, CONTENT_VERSION, config);
    await current.state.restore();
    current.state.activateShelfReading();
    current.state.scheduleProgress(true);
    current.state.flushOnExit();
    assert.equal(storage.getItem(current.key), null);
    assert.ok(storage.getItem(current.progressKey));
  }
});

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
