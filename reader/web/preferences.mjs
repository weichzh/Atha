const APPLICATION_DEFAULTS = Object.freeze({
  theme: "system",
  brightness: 100,
  fontSize: 32,
  fontFamily: "book",
  density: "standard",
  marginTopPx: 88,
  marginRightPx: 32,
  marginBottomPx: 88,
  marginLeftPx: 32,
});
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
    const normalized = {
      brightness: 100,
      marginTopPx: APPLICATION_DEFAULTS.marginTopPx,
      marginRightPx: APPLICATION_DEFAULTS.marginRightPx,
      marginBottomPx: APPLICATION_DEFAULTS.marginBottomPx,
      marginLeftPx: APPLICATION_DEFAULTS.marginLeftPx,
      ...value,
    };
    const validMargin = (margin) =>
      Number.isInteger(margin) && margin >= 0 && margin <= 288 && margin % 8 === 0;
    ensure(
      exact(normalized, APPLICATION_DEFAULTS) &&
        ["system", "light", "dark"].includes(normalized.theme) &&
        Number.isInteger(normalized.brightness) &&
        normalized.brightness >= 70 &&
        normalized.brightness <= 120 &&
        [24, 32, 40].includes(normalized.fontSize) &&
        ["book", "serif", "sans"].includes(normalized.fontFamily) &&
        Object.hasOwn(LINE_HEIGHT_RATIOS, normalized.density) &&
        validMargin(normalized.marginTopPx) &&
        validMargin(normalized.marginRightPx) &&
        validMargin(normalized.marginBottomPx) &&
        validMargin(normalized.marginLeftPx),
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
    controls.marginTop.value = String(application.marginTopPx);
    controls.marginRight.value = String(application.marginRightPx);
    controls.marginBottom.value = String(application.marginBottomPx);
    controls.marginLeft.value = String(application.marginLeftPx);
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
    root.style.setProperty("--reader-brightness", String(application.brightness / 100));
    if (application.fontFamily === "book") delete content.book.dataset.fontFamily;
    else content.book.dataset.fontFamily = application.fontFamily;
    reader.style.setProperty(
      "--reader-line-height",
      `${application.fontSize * LINE_HEIGHT_RATIOS[application.density]}px`,
    );
    reader.style.setProperty("--page-top-margin", `${application.marginTopPx}px`);
    reader.style.setProperty("--page-right-margin", `${application.marginRightPx}px`);
    reader.style.setProperty("--page-bottom-margin", `${application.marginBottomPx}px`);
    reader.style.setProperty("--page-left-margin", `${application.marginLeftPx}px`);
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
      [controls.marginTop, "marginTopPx", Number],
      [controls.marginRight, "marginRightPx", Number],
      [controls.marginBottom, "marginBottomPx", Number],
      [controls.marginLeft, "marginLeftPx", Number],
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
    ["application", { marginLeftPx: 31 }],
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
  apply();
  return Object.freeze({ bind, reset, restore, snapshot, update });
}
