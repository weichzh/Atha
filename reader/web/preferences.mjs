const APPLICATION_DEFAULTS = Object.freeze({
  theme: "system",
  brightness: 100,
  fontSize: 32,
  fontFamily: "book",
  density: "standard",
  tapToPaginate: true,
  swipeToPaginate: true,
});
const LEGACY_MARGIN_KEYS = ["marginTopPx", "marginRightPx", "marginBottomPx", "marginLeftPx"];
const BOOK_DEFAULTS = Object.freeze({
  sourceStyles: true,
  userStylesEnabled: true,
  userStylesheet: "",
});
const LINE_HEIGHT_RATIOS = Object.freeze({
  compact: 1.45,
  standard: 1.6,
  comfortable: 1.8,
});

export function createPreferences({ root, reader, content, controls, assert }) {
  let application = { ...APPLICATION_DEFAULTS };
  let book = { ...BOOK_DEFAULTS };
  const darkScheme = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
  const systemBars = globalThis.AthaSystemBars;

  function syncSafeAreaInsets() {
    try {
      const insets = JSON.parse(systemBars?.getSafeAreaInsets?.() ?? "null");
      const deviceScale = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? devicePixelRatio : 1;
      const readerPixels = (cssPixels) => cssPixels * deviceScale;
      for (const edge of ["top", "right", "bottom", "left"]) {
        if (Number.isFinite(insets?.[edge])) {
          root.style.setProperty(`--safe-area-${edge}`, `${insets[edge] / deviceScale}px`);
        }
      }
      // Pagination measures the reader canvas in device pixels, then scales it once for display.
      root.style.setProperty("--reader-chapter-top", `${insets.top + readerPixels(12)}px`);
      root.style.setProperty("--reader-chapter-size", `${readerPixels(14)}px`);
      root.style.setProperty("--reader-content-top", `${insets.top + readerPixels(64)}px`);
      root.style.setProperty("--reader-content-bottom", `${insets.bottom + readerPixels(48)}px`);
    } catch {
      // The native bridge is optional on desktop.
    }
  }
  syncSafeAreaInsets();
  globalThis.addEventListener?.("atha-safe-area-change", syncSafeAreaInsets);
  globalThis.addEventListener?.("resize", syncSafeAreaInsets);
  document.addEventListener?.("visibilitychange", syncSafeAreaInsets);

  function syncSystemBars() {
    systemBars?.setReadingMode?.(
      true,
      application.theme === "dark" || (application.theme === "system" && darkScheme?.matches),
    );
  }

  darkScheme?.addEventListener?.("change", () => {
    if (application.theme === "system") syncSystemBars();
  });

  function ensure(condition) {
    if (!condition) throw new Error("invalid-preference");
  }

  function exact(value, defaults) {
    return (
      value &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      Object.keys(value).length === Object.keys(defaults).length &&
      Object.keys(value).every((key) => Object.hasOwn(defaults, key))
    );
  }

  function validateApplication(value) {
    ensure(value && typeof value === "object" && !Array.isArray(value));
    const normalized = {
      brightness: 100,
      tapToPaginate: APPLICATION_DEFAULTS.tapToPaginate,
      swipeToPaginate: APPLICATION_DEFAULTS.swipeToPaginate,
      ...value,
    };
    for (const key of LEGACY_MARGIN_KEYS) delete normalized[key];
    ensure(
      exact(normalized, APPLICATION_DEFAULTS) &&
        ["system", "light", "paper", "dark"].includes(normalized.theme) &&
        Number.isInteger(normalized.brightness) &&
        normalized.brightness >= 70 &&
        normalized.brightness <= 120 &&
        [24, 32, 40].includes(normalized.fontSize) &&
        ["book", "serif", "sans"].includes(normalized.fontFamily) &&
        Object.hasOwn(LINE_HEIGHT_RATIOS, normalized.density) &&
        typeof normalized.tapToPaginate === "boolean" &&
        typeof normalized.swipeToPaginate === "boolean",
      "invalid-preference",
    );
    return normalized;
  }

  function validateBook(value) {
    ensure(
      exact(value, BOOK_DEFAULTS) &&
        typeof value.sourceStyles === "boolean" &&
        typeof value.userStylesEnabled === "boolean" &&
        typeof value.userStylesheet === "string" &&
        value.userStylesheet.length <= 32768,
      "invalid-preference",
    );
    return { ...value };
  }

  function syncControls() {
    controls.theme.value = application.theme;
    controls.brightness.value = String(application.brightness);
    controls.fontSize.value = String(application.fontSize);
    controls.fontFamily.value = application.fontFamily;
    controls.density.value = application.density;
    controls.tapToPaginate.checked = application.tapToPaginate;
    controls.swipeToPaginate.checked = application.swipeToPaginate;
    controls.sourceStyles.checked = book.sourceStyles;
    controls.userStylesEnabled.checked = book.userStylesEnabled;
    controls.userStylesheet.value = book.userStylesheet;
  }

  function apply(nextApplication = application, nextBook = book) {
    content.setStyles(nextBook);
    application = nextApplication;
    book = nextBook;
    if (application.theme === "system") {
      delete root.dataset.theme;
      delete content.book.dataset.theme;
    } else {
      root.dataset.theme = application.theme;
      content.book.dataset.theme = application.theme;
    }
    syncSystemBars();
    root.style.setProperty("--reader-brightness", String(application.brightness / 100));
    if (application.fontFamily === "book") delete content.book.dataset.fontFamily;
    else content.book.dataset.fontFamily = application.fontFamily;
    reader.style.setProperty(
      "--reader-line-height",
      `${application.fontSize * LINE_HEIGHT_RATIOS[application.density]}px`,
    );
    syncControls();
    return snapshot();
  }

  function update(scope, patch) {
    ensure(patch && typeof patch === "object" && !Array.isArray(patch));
    if (scope === "application") {
      const keys = Object.keys(patch);
      ensure(keys.every((key) => Object.hasOwn(APPLICATION_DEFAULTS, key)));
      return apply(validateApplication({ ...application, ...patch }), book);
    }
    ensure(scope === "book");
    const keys = Object.keys(patch);
    ensure(keys.every((key) => Object.hasOwn(BOOK_DEFAULTS, key)));
    return apply(application, validateBook({ ...book, ...patch }));
  }

  function reset(scope) {
    if (scope === "application") return apply({ ...APPLICATION_DEFAULTS }, book);
    ensure(scope === "book");
    return apply(application, { ...BOOK_DEFAULTS });
  }

  function restore(value) {
    ensure(value && typeof value === "object" && !Array.isArray(value));
    return apply(validateApplication(value.application), validateBook(value.book));
  }

  function snapshot() {
    return Object.freeze({
      application: Object.freeze({ ...application }),
      book: Object.freeze({ ...book }),
      effective: Object.freeze({ ...application, ...book }),
    });
  }

  function bind({ onUpdate, onReset }) {
    const report = (message, error = false) => {
      controls.status.textContent = message;
      controls.status.dataset.error = String(error);
    };
    const run = (action, message) => {
      Promise.resolve(action()).then(() => report(message)).catch((error) => {
        report(error instanceof Error ? error.message : "invalid-preference", true);
      });
    };
    for (const [control, key, convert] of [
      [controls.theme, "theme", String],
      [controls.fontFamily, "fontFamily", String],
      [controls.density, "density", String],
    ]) {
      control.addEventListener("change", () => {
        run(() => onUpdate("application", { [key]: convert(control.value) }), "已应用");
      });
    }
    controls.brightness.addEventListener("input", () => {
      root.style.setProperty("--reader-brightness", String(Number(controls.brightness.value) / 100));
    });
    controls.brightness.addEventListener("change", () => {
      run(
        () => onUpdate("application", { brightness: Number(controls.brightness.value) }),
        "已应用",
      );
    });
    controls.tapToPaginate.addEventListener("change", () => {
      run(
        () => onUpdate("application", { tapToPaginate: controls.tapToPaginate.checked }),
        "已应用",
      );
    });
    controls.swipeToPaginate.addEventListener("change", () => {
      run(
        () => onUpdate("application", { swipeToPaginate: controls.swipeToPaginate.checked }),
        "已应用",
      );
    });
    controls.sourceStyles.addEventListener("change", () => {
      run(() => onUpdate("book", { sourceStyles: controls.sourceStyles.checked }), "已应用");
    });
    controls.userStylesEnabled.addEventListener("change", () => {
      run(
        () => onUpdate("book", { userStylesEnabled: controls.userStylesEnabled.checked }),
        "已应用",
      );
    });
    controls.applyUserStyle.addEventListener("click", () => {
      run(
        () => onUpdate("book", { userStylesheet: controls.userStylesheet.value }),
        "已应用本书样式",
      );
    });
    controls.resetApplication.addEventListener("click", () => {
      run(() => onReset("application"), "已恢复应用默认");
    });
    controls.resetBook.addEventListener("click", () => {
      run(() => onReset("book"), "已恢复本书样式");
    });
  }

  for (const invalid of [
    ["application", { theme: "sepia" }],
    ["application", { brightness: 121 }],
    ["application", { fontSize: 31 }],
    ["application", { unknown: true }],
    ["book", { unknown: true }],
  ]) {
    let rejected = false;
    try {
      update(...invalid);
    } catch {
      rejected = true;
    }
    assert(rejected, "sample-boundary");
  }
  const migrated = validateApplication({
    ...APPLICATION_DEFAULTS,
    marginTopPx: 80,
    marginRightPx: 24,
    marginBottomPx: 96,
    marginLeftPx: 40,
  });
  assert(
    LEGACY_MARGIN_KEYS.every((key) => !Object.hasOwn(migrated, key)),
    "sample-boundary",
  );
  apply();
  return Object.freeze({ bind, reset, restore, snapshot, update });
}
