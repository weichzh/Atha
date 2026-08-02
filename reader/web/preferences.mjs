const APPLICATION_DEFAULTS = Object.freeze({
  theme: "system",
  fontSize: 32,
  fontFamily: "book",
  density: "standard",
});
const BOOK_DEFAULTS = Object.freeze({
  sourceStyles: true,
  userStylesEnabled: true,
  userStylesheet: "",
});
const DENSITIES = Object.freeze({
  compact: Object.freeze({ lineHeightRatio: 1.45, inline: 80, top: 80, bottom: 96 }),
  standard: Object.freeze({ lineHeightRatio: 1.6, inline: 112, top: 96, bottom: 128 }),
  comfortable: Object.freeze({ lineHeightRatio: 1.8, inline: 144, top: 112, bottom: 144 }),
});

export function createPreferences({ root, reader, content, controls, assert }) {
  let application = { ...APPLICATION_DEFAULTS };
  let book = { ...BOOK_DEFAULTS };

  function ensure(condition) {
    if (!condition) throw new Error("invalid-preference");
  }

  function validateApplication(value) {
    ensure(
      value &&
        ["system", "light", "dark"].includes(value.theme) &&
        [24, 32, 40].includes(value.fontSize) &&
        ["book", "serif", "sans"].includes(value.fontFamily) &&
        Object.hasOwn(DENSITIES, value.density),
      "invalid-preference",
    );
    return { ...value };
  }

  function validateBook(value) {
    ensure(
      value &&
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
    controls.fontSize.value = String(application.fontSize);
    controls.fontFamily.value = application.fontFamily;
    controls.density.value = application.density;
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
    if (application.fontFamily === "book") delete content.book.dataset.fontFamily;
    else content.book.dataset.fontFamily = application.fontFamily;
    const density = DENSITIES[application.density];
    reader.style.setProperty(
      "--reader-line-height",
      `${application.fontSize * density.lineHeightRatio}px`,
    );
    reader.style.setProperty("--page-inline-margin", `${density.inline}px`);
    reader.style.setProperty("--page-top-margin", `${density.top}px`);
    reader.style.setProperty("--page-bottom-margin", `${density.bottom}px`);
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
    ["application", { fontSize: 31 }],
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
  return Object.freeze({ bind, reset, snapshot, update });
}
