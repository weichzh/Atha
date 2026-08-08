const READER_STATE_SCHEMA = 1;
const MAX_STATE_LENGTH = 524288;
const MAX_BOOKMARKS = 200;
const STATISTICS_SCHEMA = 1;
const STATISTICS_HEARTBEAT_MS = 15000;
const STATISTICS_MAX_INTERVAL_MS = 30000;
const STATISTICS_IDLE_MS = 300000;
const STATISTICS_STREAK_DAY_MS = 60000;
const MAX_STATISTICS_DAYS = 400;
const MAX_STATISTICS_BOOKS = 2048;

export function createReaderState({
  storage,
  keyPrefix,
  bookKey,
  session,
  navigation,
  pagination,
  preferences,
  locator,
  assert,
}) {
  if (!["atha.reader", "atha.reader.probe"].includes(keyPrefix)) {
    throw new Error("invalid-state-key");
  }
  if (!/^[a-z0-9-]{1,64}$/.test(bookKey || "")) throw new Error("invalid-state-key");
  const keys = Object.freeze({
    application: `${keyPrefix}.application.v1`,
    book: `${keyPrefix}.book.${bookKey}.v1`,
    progress: `${keyPrefix}.progress.${bookKey}.v1`,
  });
  const volatile = new Map();
  const target =
    storage ||
    Object.freeze({
      getItem: (key) => volatile.get(key) ?? null,
      removeItem: (key) => volatile.delete(key),
      setItem: (key, value) => volatile.set(key, value),
    });
  const durable = Boolean(storage);
  const defaults = preferences.snapshot();
  const writes = { application: 0, book: 0, progress: 0 };
  let bookmarks = [];
  let available = true;
  let restoring = false;
  let restored = false;
  let scheduled = false;
  let pendingProgress = null;
  let lastFallback = null;
  let bound = false;

  function discard(key) {
    try {
      target.removeItem(key);
    } catch {
      available = false;
    }
  }

  function read(key, validate) {
    let raw;
    try {
      raw = target.getItem(key);
    } catch {
      available = false;
      lastFallback = "state-read";
      return null;
    }
    if (raw === null) return null;
    try {
      if (raw.length > MAX_STATE_LENGTH) throw new Error("state-corrupt");
      const value = JSON.parse(raw);
      if (!validate(value)) throw new Error("state-corrupt");
      return value;
    } catch {
      lastFallback = "state-corrupt";
      discard(key);
      return null;
    }
  }

  function validBookmark(value) {
    if (!(
      exactObject(value, ["id", "label", "locator"].sort()) &&
      typeof value.id === "string" &&
      /^[a-z0-9-]{1,64}$/.test(value.id) &&
      typeof value.label === "string" &&
      value.label.trim().length > 0 &&
      value.label.length <= 256 &&
      typeof value.locator === "string" &&
      value.locator.length > 0 &&
      value.locator.length <= 2048
    )) return false;
    try {
      return locator.inspect(value.locator).contentVersion !== null;
    } catch {
      return false;
    }
  }

  function applicationRecord() {
    return read(
      keys.application,
      (value) => exactObject(value, ["preferences", "schema"].sort()) && value.schema === 1,
    );
  }

  function bookRecord() {
    return read(
      keys.book,
      (value) =>
        exactObject(value, ["bookmarks", "preferences", "schema"].sort()) &&
        value.schema === 1 &&
        Array.isArray(value.bookmarks) &&
        value.bookmarks.length <= MAX_BOOKMARKS &&
        value.bookmarks.every(validBookmark),
    );
  }

  function progressRecord() {
    return read(
      keys.progress,
      (value) =>
        exactObject(value, ["contentVersion", "locator", "schema"].sort()) &&
        value.schema === 1 &&
        /^[a-f0-9]{64}$/.test(value.contentVersion) &&
        typeof value.locator === "string" &&
        value.locator.length > 0 &&
        value.locator.length <= 2048 &&
        (() => {
          try {
            return locator.inspect(value.locator).contentVersion === value.contentVersion;
          } catch {
            return false;
          }
        })(),
    );
  }

  function write(key, value, kind) {
    try {
      target.setItem(key, JSON.stringify(value));
      writes[kind] += 1;
      return true;
    } catch {
      available = false;
      lastFallback = "state-write";
      return false;
    }
  }

  function writeBook() {
    return write(
      keys.book,
      {
        schema: READER_STATE_SCHEMA,
        preferences: preferences.snapshot().book,
        bookmarks,
      },
      "book",
    );
  }

  function savePreferences(scope) {
    const state = preferences.snapshot();
    if (scope === "application") {
      return write(
        keys.application,
        { schema: READER_STATE_SCHEMA, preferences: state.application },
        "application",
      );
    }
    if (scope === "book") return writeBook();
    return false;
  }

  function flushProgress() {
    scheduled = false;
    if (!pendingProgress) return true;
    const value = pendingProgress;
    pendingProgress = null;
    return write(keys.progress, value, "progress");
  }

  function scheduleProgress() {
    if (restoring || session.snapshot().state !== "layout-stable") return;
    pendingProgress = {
      schema: READER_STATE_SCHEMA,
      contentVersion: session.describe().contentVersion,
      locator: locator.serialize(session.describe(), navigation.current()),
    };
    if (scheduled) return;
    scheduled = true;
    queueMicrotask(flushProgress);
  }

  function currentBookmark(value) {
    try {
      locator.parse(session.describe(), value.locator);
      return true;
    } catch {
      return false;
    }
  }

  function bookmarkSnapshot() {
    return bookmarks.map((value) =>
      Object.freeze({ ...value, currentVersion: currentBookmark(value) }),
    );
  }

  function addBookmark(label) {
    const value = String(label).trim().slice(0, 256);
    if (!value) return Object.freeze({ ok: false, error: "bookmark-label" });
    const serialized = locator.serialize(session.describe(), navigation.current());
    const existing = bookmarks.find((bookmark) => bookmark.locator === serialized);
    if (existing) return Object.freeze({ ok: true, id: existing.id, created: false });
    if (bookmarks.length >= MAX_BOOKMARKS) {
      return Object.freeze({ ok: false, error: "bookmark-limit" });
    }
    const bookmark = Object.freeze({ id: crypto.randomUUID(), label: value, locator: serialized });
    bookmarks = [...bookmarks, bookmark];
    if (!writeBook()) {
      bookmarks = bookmarks.filter((value) => value.id !== bookmark.id);
      return Object.freeze({ ok: false, error: "state-write" });
    }
    return Object.freeze({ ok: true, id: bookmark.id, created: true });
  }

  function removeBookmark(id) {
    const index = bookmarks.findIndex((bookmark) => bookmark.id === id);
    if (index < 0) return Object.freeze({ ok: false, error: "bookmark-missing" });
    const previous = bookmarks;
    bookmarks = bookmarks.filter((bookmark) => bookmark.id !== id);
    if (!writeBook()) {
      bookmarks = previous;
      return Object.freeze({ ok: false, error: "state-write" });
    }
    return Object.freeze({ ok: true });
  }

  function resolveBookmark(id) {
    const bookmark = bookmarks.find((value) => value.id === id);
    if (!bookmark) return Object.freeze({ ok: false, error: "bookmark-missing" });
    if (!currentBookmark(bookmark)) {
      return Object.freeze({ ok: false, error: "bookmark-version" });
    }
    return Object.freeze({ ok: true, locator: bookmark.locator });
  }

  async function restore() {
    restoring = true;
    const application = applicationRecord();
    const book = bookRecord();
    let applicationPreferences = defaults.application;
    try {
      if (application) {
        preferences.restore({ application: application.preferences, book: defaults.book });
        applicationPreferences = application.preferences;
      }
    } catch {
      lastFallback = "state-corrupt";
      discard(keys.application);
    }
    let bookPreferences = defaults.book;
    bookmarks = book?.bookmarks || [];
    try {
      if (book) {
        preferences.restore({ application: applicationPreferences, book: book.preferences });
        bookPreferences = book.preferences;
      }
    } catch {
      lastFallback = "state-corrupt";
      bookPreferences = defaults.book;
      preferences.restore({ application: applicationPreferences, book: bookPreferences });
      writeBook();
    }
    const state = preferences.restore({ application: applicationPreferences, book: bookPreferences });
    await pagination.setFontSize(state.effective.fontSize);
    const progress = progressRecord();
    if (progress && progress.contentVersion !== session.describe().contentVersion) {
      lastFallback = "state-version";
      discard(keys.progress);
    } else if (progress && !(await navigation.goTo(progress.locator))) {
      lastFallback = "state-locator";
      discard(keys.progress);
    }
    restoring = false;
    restored = true;
    return snapshot();
  }

  function bind() {
    if (bound) return;
    bound = true;
    window.addEventListener("pagehide", flushProgress);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") flushProgress();
    });
  }

  function clearRecords() {
    let cleared = true;
    for (const key of Object.values(keys)) {
      try {
        target.removeItem(key);
      } catch {
        available = false;
        cleared = false;
      }
    }
    return cleared;
  }

  async function verifyPersistence(mode) {
    assert(durable && available, "state-persistence");
    if (mode === "write") {
      assert(clearRecords(), "state-persistence");
      bookmarks = [];
      const state = preferences.update("application", { theme: "dark", fontSize: 24 });
      await pagination.setFontSize(state.effective.fontSize);
      assert(savePreferences("application") && writeBook(), "state-persistence");
      assert(await navigation.next(), "state-persistence");
      const bookmark = addBookmark("host-persistence-probe");
      assert(bookmark.ok && bookmark.created, "state-persistence");
      scheduleProgress();
      assert(flushProgress(), "state-persistence");
      return;
    }
    assert(mode === "read", "state-persistence");
    const probe = bookmarks.find((bookmark) => bookmark.label === "host-persistence-probe");
    const current = locator.serialize(session.describe(), navigation.current());
    const valid =
      preferences.snapshot().application.theme === "dark" &&
      preferences.snapshot().application.fontSize === 24 &&
      probe?.locator === current;
    const cleared = clearRecords();
    assert(valid && cleared, "state-persistence");
  }

  async function verify() {
    const progressWrites = writes.progress;
    scheduleProgress();
    scheduleProgress();
    scheduleProgress();
    await Promise.resolve();
    assert(writes.progress === progressWrites + 1 && !scheduled, "sample-boundary");

    scheduleProgress();
    window.dispatchEvent(new Event("pagehide"));
    assert(writes.progress === progressWrites + 2 && !scheduled, "sample-boundary");

    const currentSerialized = locator.serialize(session.describe(), navigation.current());
    const serialized = JSON.parse(currentSerialized);
    serialized.contentVersion = serialized.contentVersion === null ? "a".repeat(64) : "b".repeat(64);
    const stale = Object.freeze({
      id: crypto.randomUUID(),
      label: "旧版本书签",
      locator: JSON.stringify(serialized),
    });
    bookmarks = [...bookmarks, stale];
    assert(resolveBookmark(stale.id).error === "bookmark-version", "sample-boundary");
    bookmarks = bookmarks.filter((bookmark) => bookmark.id !== stale.id);
    assert(
      new Set(Object.values(keys)).size === 3 &&
        validBookmark({ id: "bookmark-1", label: "位置", locator: currentSerialized }) &&
        !validBookmark({ id: "bookmark-1", label: "位置", locator: "{}" }) &&
        !validBookmark({ id: "bookmark-1", label: "", locator: "{}" }),
      "sample-boundary",
    );
    return Object.freeze({ coalesced: true, lifecycleFlushed: true, versionRejected: true });
  }

  function snapshot() {
    return Object.freeze({
      available,
      durable,
      restored,
      lastFallback,
      pending: Boolean(pendingProgress),
      writes: Object.freeze({ ...writes }),
      bookmarks: Object.freeze(bookmarkSnapshot()),
    });
  }

  return Object.freeze({
    addBookmark,
    bind,
    flushProgress,
    removeBookmark,
    resolveBookmark,
    restore,
    savePreferences,
    scheduleProgress,
    snapshot,
    verify,
    verifyPersistence,
  });
}

function exactObject(value, expected) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const keys = Object.keys(value).sort();
  return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
}

function localDateKey(milliseconds) {
  const value = new Date(milliseconds);
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function shiftLocalDate(value, days) {
  const [year, month, day] = value.split("-").map(Number);
  return localDateKey(new Date(year, month - 1, day + days).getTime());
}

function inStatisticsDayWindow(value, at) {
  const today = localDateKey(at);
  return value >= shiftLocalDate(today, 1 - MAX_STATISTICS_DAYS) && value <= today;
}

function validLocalDate(value) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(year, month - 1, day);
  return (
    date.getFullYear() === year &&
    date.getMonth() === month - 1 &&
    date.getDate() === day
  );
}

function validDuration(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function validDayDuration(value) {
  return validDuration(value) && value <= 26 * 60 * 60 * 1000;
}

function validStatisticsRecord(value) {
  if (
    !exactObject(value, ["books", "days", "schema"]) ||
    value.schema !== STATISTICS_SCHEMA ||
    !Array.isArray(value.days) ||
    value.days.length > MAX_STATISTICS_DAYS ||
    !Array.isArray(value.books) ||
    value.books.length > MAX_STATISTICS_BOOKS
  ) {
    return false;
  }
  const days = new Set();
  for (const day of value.days) {
    if (
      !exactObject(day, ["date", "durationMs"]) ||
      !validLocalDate(day.date) ||
      !validDayDuration(day.durationMs) ||
      days.has(day.date)
    ) {
      return false;
    }
    days.add(day.date);
  }
  const books = new Set();
  for (const book of value.books) {
    if (
      !exactObject(book, ["contentVersion", "durationMs", "lastReadDate"]) ||
      typeof book.contentVersion !== "string" ||
      !/^[a-f0-9]{64}$/.test(book.contentVersion) ||
      !validDuration(book.durationMs) ||
      !validLocalDate(book.lastReadDate) ||
      books.has(book.contentVersion)
    ) {
      return false;
    }
    books.add(book.contentVersion);
  }
  return true;
}

function splitLocalInterval(endMilliseconds, durationMilliseconds) {
  const parts = [];
  const end = Math.floor(endMilliseconds);
  let cursor = end - Math.floor(durationMilliseconds);
  while (cursor < end) {
    const date = new Date(cursor);
    const next = new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1).getTime();
    const boundary = Math.min(end, next);
    parts.push({ date: localDateKey(cursor), durationMs: boundary - cursor });
    cursor = boundary;
  }
  return parts;
}

function formatReadingDuration(milliseconds) {
  if (milliseconds <= 0) return "0 分钟";
  if (milliseconds < 60000) return "<1 分钟";
  const minutes = Math.floor(milliseconds / 60000);
  if (minutes < 60) return `${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  return remainder === 0 ? `${hours} 小时` : `${hours} 小时 ${remainder} 分`;
}

export function createReadingStatistics({
  storage,
  keyPrefix,
  contentVersion,
  controls = {},
  now = () => performance.now(),
  wallNow = () => Date.now(),
}) {
  if (!["atha.reader", "atha.reader.probe"].includes(keyPrefix)) {
    throw new Error("invalid-state-key");
  }
  if (!/^[a-f0-9]{64}$/.test(contentVersion || "")) throw new Error("invalid-state-key");

  const key = `${keyPrefix}.statistics.v1`;
  const volatile = new Map();
  const target =
    storage ||
    Object.freeze({
      getItem: (name) => volatile.get(name) ?? null,
      removeItem: (name) => volatile.delete(name),
      setItem: (name, value) => volatile.set(name, value),
    });
  const durable = Boolean(storage);
  let available = true;
  let lastFallback = null;
  let writes = 0;
  let record = { schema: STATISTICS_SCHEMA, days: [], books: [] };
  let stable = false;
  let reading = true;
  let visible = true;
  let focused = true;
  let bound = false;
  let lastTick = Number(now());
  let lastWall = Number(wallNow());
  let lastActivity = lastTick;

  function discard() {
    try {
      target.removeItem(key);
    } catch {
      available = false;
    }
  }

  function restore() {
    let raw;
    try {
      raw = target.getItem(key);
    } catch {
      available = false;
      lastFallback = "statistics-read";
      return;
    }
    if (raw === null) return;
    try {
      if (raw.length > MAX_STATE_LENGTH) throw new Error("statistics-corrupt");
      const value = JSON.parse(raw);
      if (!validStatisticsRecord(value)) throw new Error("statistics-corrupt");
      value.days = value.days.filter((day) => inStatisticsDayWindow(day.date, wallNow()));
      value.days.sort((left, right) => left.date.localeCompare(right.date));
      value.books.sort((left, right) =>
        left.lastReadDate.localeCompare(right.lastReadDate) ||
        left.contentVersion.localeCompare(right.contentVersion),
      );
      record = value;
    } catch {
      lastFallback = "statistics-corrupt";
      discard();
    }
  }

  function write(destination = key) {
    const productWrite = destination === key;
    record.days = record.days.filter((day) => inStatisticsDayWindow(day.date, wallNow()));
    record.days.sort((left, right) => left.date.localeCompare(right.date));
    record.days = record.days.slice(-MAX_STATISTICS_DAYS);
    record.books.sort((left, right) =>
      left.lastReadDate.localeCompare(right.lastReadDate) ||
      left.contentVersion.localeCompare(right.contentVersion),
    );
    while (record.books.length > MAX_STATISTICS_BOOKS) {
      const removable = record.books.findIndex((book) => book.contentVersion !== contentVersion);
      if (removable < 0) break;
      record.books.splice(removable, 1);
    }
    let serialized = JSON.stringify(record);
    while (serialized.length > MAX_STATE_LENGTH && record.books.length > 1) {
      const removable = record.books.findIndex((book) => book.contentVersion !== contentVersion);
      if (removable < 0) break;
      record.books.splice(removable, 1);
      serialized = JSON.stringify(record);
    }
    if (serialized.length > MAX_STATE_LENGTH) {
      if (!productWrite) throw new Error("statistics-benchmark");
      available = false;
      lastFallback = "statistics-write";
      return false;
    }
    if (productWrite && !available) return false;
    try {
      target.setItem(destination, serialized);
      if (productWrite) writes += 1;
      return true;
    } catch {
      if (!productWrite) throw new Error("statistics-benchmark");
      available = false;
      lastFallback = "statistics-write";
      return false;
    }
  }

  function projection(at = Number(wallNow())) {
    const today = localDateKey(at);
    const weekStart = shiftLocalDate(today, -6);
    const days = new Map(record.days.map((day) => [day.date, day.durationMs]));
    const todayMs = days.get(today) || 0;
    const weekMs = record.days.reduce(
      (sum, day) => sum + (day.date >= weekStart && day.date <= today ? day.durationMs : 0),
      0,
    );
    const bookMs =
      record.books.find((book) => book.contentVersion === contentVersion)?.durationMs || 0;
    let cursor = todayMs >= STATISTICS_STREAK_DAY_MS ? today : shiftLocalDate(today, -1);
    let streakDays = 0;
    while ((days.get(cursor) || 0) >= STATISTICS_STREAK_DAY_MS) {
      streakDays += 1;
      cursor = shiftLocalDate(cursor, -1);
    }
    return {
      todayMs,
      weekMs,
      bookMs,
      streakDays,
      streakCapped: streakDays === MAX_STATISTICS_DAYS,
    };
  }

  function render() {
    const value = projection();
    if (controls.today) controls.today.textContent = formatReadingDuration(value.todayMs);
    if (controls.week) controls.week.textContent = formatReadingDuration(value.weekMs);
    if (controls.book) controls.book.textContent = formatReadingDuration(value.bookMs);
    if (controls.streak) {
      controls.streak.textContent = `${value.streakDays}${value.streakCapped ? "+" : ""} 天`;
    }
  }

  function addDuration(duration, endedAt, destination = key) {
    const accepted = Math.floor(duration);
    if (accepted <= 0) return 0;
    const days = new Map(record.days.map((day) => [day.date, day.durationMs]));
    const parts = splitLocalInterval(endedAt, accepted);
    for (const part of parts) {
      days.set(
        part.date,
        Math.min(Number.MAX_SAFE_INTEGER, (days.get(part.date) || 0) + part.durationMs),
      );
    }
    record.days = [...days].map(([date, durationMs]) => ({ date, durationMs }));
    const lastReadDate = parts.at(-1).date;
    const book = record.books.find((value) => value.contentVersion === contentVersion);
    if (book) {
      book.durationMs = Math.min(Number.MAX_SAFE_INTEGER, book.durationMs + accepted);
      book.lastReadDate = lastReadDate;
    } else {
      record.books.push({ contentVersion, durationMs: accepted, lastReadDate });
    }
    write(destination);
    render();
    return accepted;
  }

  function consumeTick(monotonic, wall, destination) {
    const delta = monotonic - lastTick;
    const wallDelta = wall - lastWall;
    const started = lastTick;
    lastTick = monotonic;
    lastWall = wall;
    if (
      !Number.isFinite(delta) ||
      !Number.isFinite(wall) ||
      delta < 0 ||
      delta > STATISTICS_MAX_INTERVAL_MS ||
      !stable ||
      !reading ||
      !visible ||
      !focused
    ) {
      return 0;
    }
    const activeEnd = Math.min(monotonic, lastActivity + STATISTICS_IDLE_MS);
    const duration = activeEnd - started;
    if (duration <= 0) return 0;
    const wallStable = Math.abs(wallDelta - delta) <= 2000;
    const endedAt = wallStable ? wall - (monotonic - activeEnd) : wall;
    return addDuration(duration, endedAt, destination);
  }

  function tick(monotonic = Number(now()), wall = Number(wallNow())) {
    return consumeTick(monotonic, wall, key);
  }

  function activity(monotonic = Number(now())) {
    lastActivity = monotonic;
  }

  function setState(name, value) {
    const monotonic = Number(now());
    const wall = Number(wallNow());
    tick(monotonic, wall);
    if (name === "stable") stable = Boolean(value);
    else if (name === "reading") reading = Boolean(value);
    else if (name === "visible") visible = Boolean(value);
    else focused = Boolean(value);
    if (value) lastActivity = monotonic;
  }

  function bind() {
    if (bound || typeof document === "undefined" || typeof window === "undefined") return;
    bound = true;
    visible = document.visibilityState === "visible";
    focused = document.hasFocus();
    reading = !document.documentElement.hasAttribute("data-reader-tools");
    lastTick = Number(now());
    lastWall = Number(wallNow());
    lastActivity = lastTick;
    document.addEventListener("visibilitychange", () =>
      setState("visible", document.visibilityState === "visible"),
    );
    window.addEventListener("focus", () => setState("focused", true));
    window.addEventListener("blur", () => setState("focused", false));
    window.addEventListener("pagehide", () => setState("visible", false));
    new MutationObserver(() =>
      setState("reading", !document.documentElement.hasAttribute("data-reader-tools")),
    ).observe(document.documentElement, { attributeFilter: ["data-reader-tools"] });
    for (const event of ["keydown", "pointerdown", "wheel"]) {
      document.addEventListener(event, () => activity(), { capture: true, passive: true });
    }
    window.setInterval(() => tick(), STATISTICS_HEARTBEAT_MS);
  }

  function benchmark(samples = 20) {
    const benchmarkKey = `${key}.benchmark`;
    const timings = [];
    const saved = {
      record,
      stable,
      reading,
      visible,
      focused,
      lastTick,
      lastWall,
      lastActivity,
    };
    try {
      let monotonic = 0;
      let wall = Number(wallNow());
      const today = localDateKey(wall);
      const books = [
        { contentVersion, durationMs: STATISTICS_HEARTBEAT_MS, lastReadDate: today },
      ];
      for (let index = 0; books.length < MAX_STATISTICS_BOOKS; index += 1) {
        const candidate = index.toString(16).padStart(64, "0");
        if (candidate !== contentVersion) {
          books.push({ contentVersion: candidate, durationMs: 1, lastReadDate: today });
        }
      }
      record = {
        schema: STATISTICS_SCHEMA,
        days: Array.from({ length: MAX_STATISTICS_DAYS }, (_, index) => ({
          date: shiftLocalDate(today, index + 1 - MAX_STATISTICS_DAYS),
          durationMs: STATISTICS_STREAK_DAY_MS,
        })),
        books,
      };
      stable = true;
      reading = true;
      visible = true;
      focused = true;
      lastTick = monotonic;
      lastWall = wall;
      lastActivity = monotonic;
      for (let sample = 0; sample < samples; sample += 1) {
        monotonic += STATISTICS_HEARTBEAT_MS;
        wall += STATISTICS_HEARTBEAT_MS;
        const started = performance.now();
        consumeTick(monotonic, wall, benchmarkKey);
        timings.push(performance.now() - started);
      }
    } catch {
      return Object.freeze({ samples: 0, p95Ms: null });
    } finally {
      record = saved.record;
      stable = saved.stable;
      reading = saved.reading;
      visible = saved.visible;
      focused = saved.focused;
      lastTick = saved.lastTick;
      lastWall = saved.lastWall;
      lastActivity = saved.lastActivity;
      try {
        target.removeItem(benchmarkKey);
      } catch {
        // The benchmark never changes the product record.
      }
      render();
    }
    timings.sort((left, right) => left - right);
    return Object.freeze({
      samples: timings.length,
      p95Ms: timings[Math.max(0, Math.ceil(timings.length * 0.95) - 1)],
    });
  }

  function snapshot() {
    return Object.freeze({
      available,
      durable,
      lastFallback,
      ...projection(),
      writes,
    });
  }

  restore();
  render();

  return Object.freeze({
    activity,
    benchmark,
    bind,
    flush: tick,
    setFocused: (value) => setState("focused", value),
    setReading: (value) => setState("reading", value),
    setStable: (value) => setState("stable", value),
    setVisible: (value) => setState("visible", value),
    snapshot,
    tick,
  });
}
