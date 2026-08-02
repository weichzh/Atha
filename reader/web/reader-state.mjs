const READER_STATE_SCHEMA = 1;
const MAX_STATE_LENGTH = 524288;
const MAX_BOOKMARKS = 200;

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

  function exact(value, expected) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const keys = Object.keys(value).sort();
    return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
  }

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
      exact(value, ["id", "label", "locator"].sort()) &&
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
      (value) => exact(value, ["preferences", "schema"].sort()) && value.schema === 1,
    );
  }

  function bookRecord() {
    return read(
      keys.book,
      (value) =>
        exact(value, ["bookmarks", "preferences", "schema"].sort()) &&
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
        exact(value, ["contentVersion", "locator", "schema"].sort()) &&
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
