const APPLICATION_DEFAULTS = Object.freeze({
  theme: "system",
  brightness: 100,
  fontSize: 19,
  fontFamily: "book",
  density: "standard",
});
const LEGACY_MARGIN_KEYS = ["marginTopPx", "marginRightPx", "marginBottomPx", "marginLeftPx"];
const LEGACY_BOOK_KEYS = Object.freeze([
  "sourceStyles",
  "userStylesEnabled",
  "userStylesheet",
]);
const BOOK_DEFAULTS = Object.freeze({
  sourceStyles: true,
  userStylesEnabled: true,
  readingMode: "paged",
  pageMargin: "standard",
  paragraphIndent: "none",
  paragraphSpacing: "book",
  styleModules: Object.freeze([]),
});
const MAX_STYLE_MODULES = STYLE_MODULE_LIMITS.modules;
const MAX_STYLE_MODULE_BYTES = STYLE_MODULE_LIMITS.moduleBytes;
const MAX_COMBINED_STYLE_BYTES = STYLE_MODULE_LIMITS.combinedBytes;
const UTF8 = new TextEncoder();
const LINE_HEIGHT_RATIOS = Object.freeze({ compact: 1.55, standard: 1.8, comfortable: 2.05 });
const PAGE_MARGINS = Object.freeze({ narrow: 24, standard: 32, wide: 48 });

function fontSizePixels(value) {
  return Math.round(value * devicePixelRatio);
}

export function createPreferences({ root, reader, content, controls, assert }) {
  let application = { ...APPLICATION_DEFAULTS };
  let book = { ...BOOK_DEFAULTS };
  let selectedModuleId = null;
  const modulePackages = createStyleModulePackageCodec((css) => content.validateStylesheet(css));
  const { validateModules } = modulePackages;
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

  function ensure(condition, code = "invalid-preference") {
    if (!condition) throw new Error(code);
  }

  function exact(value, expected) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const keys = Object.keys(value).sort();
    return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
  }

  function validateApplication(value) {
    ensure(value && typeof value === "object" && !Array.isArray(value));
    const legacyControls = Object.hasOwn(value, "tapToPaginate") || Object.hasOwn(value, "swipeToPaginate");
    const normalized = {
      brightness: 100,
      ...value,
    };
    if (legacyControls) {
      normalized.fontSize = { 24: 16, 32: 19, 40: 24 }[normalized.fontSize] ?? 19;
    }
    delete normalized.tapToPaginate;
    delete normalized.swipeToPaginate;
    for (const key of LEGACY_MARGIN_KEYS) delete normalized[key];
    ensure(
      exact(normalized, Object.keys(APPLICATION_DEFAULTS).sort()) &&
        ["system", "light", "paper", "dark"].includes(normalized.theme) &&
        Number.isInteger(normalized.brightness) &&
        normalized.brightness >= 70 &&
        normalized.brightness <= 120 &&
        Number.isInteger(normalized.fontSize) &&
        normalized.fontSize >= 16 &&
        normalized.fontSize <= 40 &&
        ["book", "serif", "sans"].includes(normalized.fontFamily) &&
        Object.hasOwn(LINE_HEIGHT_RATIOS, normalized.density),
    );
    return normalized;
  }

  function validateBook(value) {
    ensure(value && typeof value === "object" && !Array.isArray(value));
    if (exact(value, [...LEGACY_BOOK_KEYS].sort())) {
      ensure(
        typeof value.sourceStyles === "boolean" &&
          typeof value.userStylesEnabled === "boolean" &&
          typeof value.userStylesheet === "string" &&
          value.userStylesheet.length <= MAX_STYLE_MODULE_BYTES,
      );
      return {
        ...BOOK_DEFAULTS,
        sourceStyles: value.sourceStyles,
        userStylesEnabled: value.userStylesEnabled,
        styleModules: value.userStylesheet
          ? validateModules(
              [
                {
                  id: "legacy-user-css",
                  name: "原有自定义样式",
                  group: "迁移",
                  enabled: UTF8.encode(value.userStylesheet).length <= MAX_COMBINED_STYLE_BYTES,
                  css: value.userStylesheet,
                },
              ],
              true,
            )
          : Object.freeze([]),
      };
    }
    const normalized = { readingMode: "paged", ...value };
    if (normalized.paragraphIndent === "book") normalized.paragraphIndent = "none";
    ensure(
      exact(normalized, Object.keys(BOOK_DEFAULTS).sort()) &&
        typeof normalized.sourceStyles === "boolean" &&
        typeof normalized.userStylesEnabled === "boolean" &&
        ["paged", "scroll"].includes(normalized.readingMode) &&
        Object.hasOwn(PAGE_MARGINS, normalized.pageMargin) &&
        ["none", "two"].includes(normalized.paragraphIndent) &&
        ["book", "compact", "comfortable"].includes(normalized.paragraphSpacing),
    );
    return { ...normalized, styleModules: validateModules(normalized.styleModules, true) };
  }

  function visualStylesheet(value) {
    const rules = [];
    if (value.paragraphIndent === "none") {
      rules.push(".book p { text-indent: 0 !important; }");
    } else if (value.paragraphIndent === "two") {
      rules.push(".book p { text-indent: 2em !important; }");
    }
    if (value.paragraphSpacing === "compact") {
      rules.push(".book p { margin-block: 0.25em !important; }");
    } else if (value.paragraphSpacing === "comfortable") {
      rules.push(".book p { margin-block: 0.9em !important; }");
    }
    return rules.join("\n");
  }

  function composedStylesheet(value) {
    const modules = value.userStylesEnabled
      ? value.styleModules.filter((module) => module.enabled).map((module) => module.css)
      : [];
    const css = [visualStylesheet(value), ...modules].filter(Boolean).join("\n");
    ensure(UTF8.encode(css).length <= MAX_COMBINED_STYLE_BYTES);
    return css;
  }

  function effectiveBook(value) {
    const stylesheet = composedStylesheet(value);
    return {
      sourceStyles: value.sourceStyles,
      userStylesEnabled: Boolean(stylesheet),
      userStylesheet: stylesheet,
    };
  }

  function visibleModules() {
    const query = controls.moduleSearch.value.trim().toLocaleLowerCase();
    const group = controls.moduleFilter.value;
    return book.styleModules.filter(
      (module) =>
        (!group || module.group === group) &&
        (!query || `${module.name}\n${module.group}`.toLocaleLowerCase().includes(query)),
    );
  }

  function selectedModule() {
    return book.styleModules.find((module) => module.id === selectedModuleId) || null;
  }

  function syncModuleControls() {
    const previousGroup = controls.moduleFilter.value;
    const groups = [...new Set(book.styleModules.map((module) => module.group).filter(Boolean))].sort(
      (left, right) => left.localeCompare(right),
    );
    controls.moduleFilter.replaceChildren(
      Object.assign(document.createElement("option"), { value: "", textContent: "全部分组" }),
      ...groups.map((group) =>
        Object.assign(document.createElement("option"), { value: group, textContent: group }),
      ),
    );
    controls.moduleFilter.value = groups.includes(previousGroup) ? previousGroup : "";
    const visible = visibleModules();
    if (!visible.some((module) => module.id === selectedModuleId)) {
      selectedModuleId = visible[0]?.id ?? null;
    }
    controls.moduleList.replaceChildren(
      ...visible.map((module) =>
        Object.assign(document.createElement("option"), {
          value: module.id,
          textContent: `${module.enabled ? "●" : "○"} ${module.group ? `[${module.group}] ` : ""}${module.name}`,
        }),
      ),
    );
    if (selectedModuleId) controls.moduleList.value = selectedModuleId;
    controls.moduleListView.replaceChildren(
      ...(visible.length
        ? visible.map((module) => {
            const button = Object.assign(document.createElement("button"), {
              type: "button",
              className: "module-list-item",
            });
            button.dataset.moduleId = module.id;
            button.setAttribute("role", "option");
            button.setAttribute("aria-selected", String(module.id === selectedModuleId));
            const indicator = Object.assign(document.createElement("span"), {
              className: "module-list-indicator",
            });
            indicator.dataset.enabled = String(module.enabled);
            const copy = Object.assign(document.createElement("span"), {
              className: "module-list-copy",
            });
            copy.append(
              Object.assign(document.createElement("strong"), { textContent: module.name }),
              Object.assign(document.createElement("small"), {
                textContent: module.group || "未分组",
              }),
            );
            const state = Object.assign(document.createElement("span"), {
              className: "module-list-state",
              textContent: module.enabled ? "开" : "关",
            });
            button.append(indicator, copy, state);
            return button;
          })
        : [
            Object.assign(document.createElement("p"), {
              className: "module-list-empty",
              textContent: "暂无 CSS 模块",
            }),
          ]),
    );
    const selected = selectedModule();
    for (const control of [
      controls.moduleName,
      controls.moduleGroupName,
      controls.moduleEnabled,
      controls.userStylesheet,
      controls.moduleSave,
      controls.moduleDelete,
    ]) {
      control.disabled = !selected;
    }
    controls.moduleName.value = selected?.name ?? "";
    controls.moduleGroupName.value = selected?.group ?? "";
    controls.moduleEnabled.checked = selected?.enabled ?? false;
    controls.userStylesheet.value = selected?.css ?? "";
    controls.userStylesheet.dispatchEvent(new Event("atha-css-editor-sync"));
    const index = selected ? book.styleModules.findIndex((module) => module.id === selected.id) : -1;
    controls.moduleUp.disabled = index <= 0;
    controls.moduleDown.disabled = index < 0 || index + 1 >= book.styleModules.length;
    controls.modulesEnable.disabled = visible.length === 0;
    controls.modulesDisable.disabled = visible.length === 0;
  }

  function syncPreferenceChoices(control) {
    for (const button of root.querySelectorAll(`[data-preference-for="${control.id}"]`)) {
      button.setAttribute("aria-checked", String(button.dataset.preferenceValue === control.value));
    }
  }

  function syncControls() {
    controls.theme.value = application.theme;
    controls.brightness.value = String(application.brightness);
    controls.fontSize.value = String(application.fontSize);
    controls.fontSizeValue.textContent = String(application.fontSize);
    controls.fontFamily.value = application.fontFamily;
    controls.density.value = application.density;
    controls.readingMode.value = book.readingMode;
    controls.sourceStyles.checked = book.sourceStyles;
    controls.userStylesEnabled.checked = book.userStylesEnabled;
    controls.pageMargin.value = book.pageMargin;
    controls.paragraphIndent.value = book.paragraphIndent;
    controls.paragraphSpacing.value = book.paragraphSpacing;
    for (const control of [
      controls.theme,
      controls.fontFamily,
      controls.density,
      controls.readingMode,
      controls.pageMargin,
      controls.paragraphIndent,
      controls.paragraphSpacing,
    ]) {
      syncPreferenceChoices(control);
    }
    syncModuleControls();
  }

  function apply(nextApplication = application, nextBook = book) {
    content.setStyles(effectiveBook(nextBook));
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
    reader.dataset.readingMode = nextBook.readingMode;
    content.book.dataset.readingMode = nextBook.readingMode;
    if (application.fontFamily === "book") delete content.book.dataset.fontFamily;
    else content.book.dataset.fontFamily = application.fontFamily;
    reader.style.setProperty(
      "--reader-line-height",
      String(LINE_HEIGHT_RATIOS[application.density]),
    );
    const margin = `${PAGE_MARGINS[book.pageMargin]}px`;
    reader.style.setProperty("--page-left-margin", margin);
    reader.style.setProperty("--page-right-margin", margin);
    syncControls();
    return snapshot();
  }

  function update(scope, patch) {
    ensure(patch && typeof patch === "object" && !Array.isArray(patch));
    if (scope === "application") {
      ensure(Object.keys(patch).every((key) => Object.hasOwn(APPLICATION_DEFAULTS, key)));
      return apply(validateApplication({ ...application, ...patch }), book);
    }
    ensure(scope === "book");
    ensure(Object.keys(patch).every((key) => Object.hasOwn(BOOK_DEFAULTS, key)));
    return apply(application, validateBook({ ...book, ...patch }));
  }

  function reset(scope) {
    if (scope === "application") return apply({ ...APPLICATION_DEFAULTS }, book);
    ensure(scope === "book");
    selectedModuleId = null;
    return apply(application, { ...BOOK_DEFAULTS });
  }

  function restore(value) {
    ensure(value && typeof value === "object" && !Array.isArray(value));
    return apply(validateApplication(value.application), validateBook(value.book));
  }

  function snapshot() {
    const frozenModules = Object.freeze(book.styleModules.map((module) => Object.freeze({ ...module })));
    const frozenBook = Object.freeze({ ...book, styleModules: frozenModules });
    return Object.freeze({
      application: Object.freeze({ ...application }),
      book: frozenBook,
      effective: Object.freeze({ ...application, ...frozenBook }),
    });
  }

  function bind({ onUpdate, onReset }) {
    let statusTimer = null;
    const report = (message, error = false) => {
      clearTimeout(statusTimer);
      controls.status.textContent = message;
      controls.status.dataset.error = String(error);
      if (message && !error) {
        statusTimer = setTimeout(() => {
          controls.status.textContent = "";
        }, 700);
      }
    };
    const readableError = (error) => {
      const code = error instanceof Error ? error.message : "invalid-preference";
      if (!Number.isInteger(error?.moduleIndex)) syncModuleControls();
      if (Number.isInteger(error?.moduleIndex)) {
        if (book.styleModules.some((module) => module.id === error.moduleId)) {
          selectedModuleId = error.moduleId;
          syncModuleControls();
        }
        const label = error.moduleName ? `“${error.moduleName}”` : `第 ${error.moduleIndex + 1} 个模块`;
        return ["css-subresource", "active-style", "invalid-user-style"].includes(code)
          ? `模块${label}无效或不安全，已保留上次有效样式`
          : `模块${label}无效，已保留上次有效样式`;
      }
      if (["css-subresource", "active-style", "invalid-user-style"].includes(code)) {
        return "CSS 无效或不安全，已保留上次有效样式";
      }
      return code === "state-write" ? "样式无法保存，已保留上次有效样式" : "设置无效";
    };
    const run = (action, message) =>
      Promise.resolve()
        .then(action)
        .then(() => report(message))
        .catch((error) => report(readableError(error), true));

    root.addEventListener("click", (event) => {
      const button = event.target.closest("button[data-preference-for]");
      if (!button) return;
      const control = document.getElementById(button.dataset.preferenceFor);
      if (!(control instanceof HTMLSelectElement)) return;
      control.value = button.dataset.preferenceValue;
      control.dispatchEvent(new Event("change", { bubbles: true }));
    });

    for (const [control, key, convert] of [
      [controls.theme, "theme", String],
      [controls.fontFamily, "fontFamily", String],
      [controls.density, "density", String],
    ]) {
      control.addEventListener("change", () => {
        void run(() => onUpdate("application", { [key]: convert(control.value) }), "已应用");
      });
    }
    for (const [control, key] of [
      [controls.pageMargin, "pageMargin"],
      [controls.paragraphIndent, "paragraphIndent"],
      [controls.paragraphSpacing, "paragraphSpacing"],
    ]) {
      control.addEventListener("change", () => {
        void run(() => onUpdate("book", { [key]: control.value }), "已实时应用");
      });
    }
    controls.brightness.addEventListener("input", () => {
      root.style.setProperty("--reader-brightness", String(Number(controls.brightness.value) / 100));
    });
    controls.fontSize.addEventListener("input", () => {
      controls.fontSizeValue.textContent = controls.fontSize.value;
    });
    controls.brightness.addEventListener("change", () => {
      void run(
        () => onUpdate("application", { brightness: Number(controls.brightness.value) }),
        "已应用",
      );
    });
    controls.readingMode.addEventListener("change", () => {
      void run(() => onUpdate("book", { readingMode: controls.readingMode.value }), "已应用");
    });
    controls.sourceStyles.addEventListener("change", () => {
      void run(() => onUpdate("book", { sourceStyles: controls.sourceStyles.checked }), "已应用");
    });
    controls.userStylesEnabled.addEventListener("change", () => {
      void run(
        () => onUpdate("book", { userStylesEnabled: controls.userStylesEnabled.checked }),
        "已应用",
      );
    });

    const previewTimers = new Map();
    const cancelPreview = (moduleId) => {
      clearTimeout(previewTimers.get(moduleId));
      previewTimers.delete(moduleId);
    };
    const cancelPreviews = () => {
      for (const timer of previewTimers.values()) clearTimeout(timer);
      previewTimers.clear();
    };
    const currentDraft = () => ({
      id: selectedModuleId,
      name: controls.moduleName.value.trim(),
      group: controls.moduleGroupName.value.trim(),
      enabled: controls.moduleEnabled.checked,
      css: controls.userStylesheet.value,
    });
    const saveCurrent = (message, draft = currentDraft()) => {
      const selected = book.styleModules.find((module) => module.id === draft.id);
      ensure(selected, "invalid-preference");
      const styleModules = book.styleModules.map((module) =>
        module.id === selected.id
          ? { ...module, ...draft, id: module.id }
          : module,
      );
      return run(() => onUpdate("book", { styleModules }), message);
    };
    const saveNow = (message) => {
      const draft = currentDraft();
      cancelPreview(draft.id);
      return saveCurrent(message, draft);
    };
    controls.userStylesheet.addEventListener("input", () => {
      const draft = { id: selectedModuleId, css: controls.userStylesheet.value };
      cancelPreview(draft.id);
      const timer = setTimeout(() => {
        previewTimers.delete(draft.id);
        void saveCurrent("", draft);
      }, 180);
      previewTimers.set(draft.id, timer);
    });
    controls.moduleSave.addEventListener("click", () => void saveNow("模块已保存"));
    controls.moduleName.addEventListener("change", () => void saveNow("模块已保存"));
    controls.moduleGroupName.addEventListener("change", () => void saveNow("模块已保存"));
    controls.moduleEnabled.addEventListener("change", () => void saveNow("模块已更新"));
    controls.moduleList.addEventListener("change", () => {
      selectedModuleId = controls.moduleList.value || null;
      syncModuleControls();
    });
    controls.moduleListView.addEventListener("click", (event) => {
      const item = event.target.closest("[data-module-id]");
      if (!item) return;
      selectedModuleId = item.dataset.moduleId;
      syncModuleControls();
    });
    controls.moduleSearch.addEventListener("input", syncModuleControls);
    controls.moduleFilter.addEventListener("change", syncModuleControls);
    controls.moduleAdd.addEventListener("click", () => {
      if (book.styleModules.length >= MAX_STYLE_MODULES) {
        report("最多 32 个模块", true);
        return;
      }
      const module = {
        id: crypto.randomUUID(),
        name: `新模块 ${book.styleModules.length + 1}`,
        group: "",
        enabled: true,
        css: ".book p {\n  \n}",
      };
      selectedModuleId = module.id;
      void run(
        () => onUpdate("book", { styleModules: [...book.styleModules, module] }),
        "模块已新增",
      );
    });
    controls.moduleDelete.addEventListener("click", () => {
      const selected = selectedModule();
      if (!selected || !globalThis.confirm(`删除“${selected.name}”？`)) return;
      cancelPreview(selected.id);
      selectedModuleId = null;
      void run(
        () => onUpdate("book", { styleModules: book.styleModules.filter((module) => module.id !== selected.id) }),
        "模块已删除",
      );
    });
    const move = (offset) => {
      const index = book.styleModules.findIndex((module) => module.id === selectedModuleId);
      const target = index + offset;
      if (index < 0 || target < 0 || target >= book.styleModules.length) return;
      const styleModules = [...book.styleModules];
      [styleModules[index], styleModules[target]] = [styleModules[target], styleModules[index]];
      void run(() => onUpdate("book", { styleModules }), "模块顺序已更新");
    };
    controls.moduleUp.addEventListener("click", () => move(-1));
    controls.moduleDown.addEventListener("click", () => move(1));
    const setVisible = (enabled) => {
      const ids = new Set(visibleModules().map((module) => module.id));
      void run(
        () => onUpdate("book", {
          styleModules: book.styleModules.map((module) =>
            ids.has(module.id) ? { ...module, enabled } : module,
          ),
        }),
        enabled ? "所列模块已启用" : "所列模块已停用",
      );
    };
    controls.modulesEnable.addEventListener("click", () => setVisible(true));
    controls.modulesDisable.addEventListener("click", () => setVisible(false));
    controls.moduleTransferOpen.addEventListener("click", () => {
      controls.moduleTransfer.value = modulePackages.stringify(book.styleModules);
      controls.moduleTransferDialog.showModal();
    });
    controls.moduleImport.addEventListener("click", () => {
      void run(() => {
        const modules = modulePackages.parse(controls.moduleTransfer.value);
        cancelPreviews();
        selectedModuleId = modules[0]?.id ?? null;
        return onUpdate("book", { styleModules: modules });
      }, "模块已导入");
    });
    controls.moduleCopy.addEventListener("click", () => {
      void run(async () => {
        const value = controls.moduleTransfer.value;
        if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(value);
        else {
          controls.moduleTransfer.select();
          ensure(document.execCommand("copy"));
        }
      }, "导出 JSON 已复制");
    });
    controls.resetApplication.addEventListener("click", () => {
      void run(() => onReset("application"), "已恢复应用默认");
    });
    controls.resetBook.addEventListener("click", () => {
      cancelPreviews();
      selectedModuleId = null;
      void run(() => onReset("book"), "已恢复本书样式");
    });
  }

  function benchmarkStyleModules(iterations = 20) {
    const saved = book;
    const padding = "x".repeat(1900);
    const styleModules = Array.from({ length: MAX_STYLE_MODULES }, (_, index) => ({
      id: `benchmark-${index}`,
      name: `Benchmark ${index}`,
      group: "benchmark",
      enabled: true,
      css: `.book p:nth-of-type(${index + 1}) { --atha-benchmark-${index}: "${padding}"; }`,
    }));
    const durations = [];
    try {
      for (let index = 0; index < iterations; index += 1) {
        const started = performance.now();
        const candidate = validateBook({
          ...BOOK_DEFAULTS,
          styleModules: styleModules.map((module, moduleIndex) =>
            moduleIndex === 0
              ? { ...module, css: `${module.css}\n.book { --atha-benchmark-run: ${index}; }` }
              : module,
          ),
        });
        content.setStyles(effectiveBook(candidate));
        durations.push(performance.now() - started);
      }
    } finally {
      apply(application, saved);
    }
    durations.sort((left, right) => left - right);
    return Object.freeze({
      modules: styleModules.length,
      bytes: styleModules.reduce((total, module) => total + UTF8.encode(module.css).length, 0),
      p95Ms: durations[Math.ceil(durations.length * 0.95) - 1],
    });
  }

  for (const invalid of [
    ["application", { theme: "sepia" }],
    ["application", { brightness: 121 }],
    ["application", { fontSize: 15 }],
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
  const migratedApplication = validateApplication({
    ...APPLICATION_DEFAULTS,
    marginTopPx: 80,
    marginRightPx: 24,
    marginBottomPx: 96,
    marginLeftPx: 40,
  });
  const migratedBook = validateBook({
    sourceStyles: true,
    userStylesEnabled: true,
    userStylesheet: ".book p { letter-spacing: 1px; }",
  });
  assert(
    LEGACY_MARGIN_KEYS.every((key) => !Object.hasOwn(migratedApplication, key)) &&
      migratedBook.styleModules.length === 1 &&
      migratedBook.styleModules[0].id === "legacy-user-css",
    "sample-boundary",
  );
  let duplicateRejected = false;
  try {
    validateModules([migratedBook.styleModules[0], migratedBook.styleModules[0]]);
  } catch {
    duplicateRejected = true;
  }
  assert(duplicateRejected, "sample-boundary");
  for (const invalidModules of [
    [{ ...migratedBook.styleModules[0], unknown: true }],
    [{ ...migratedBook.styleModules[0], id: "INVALID" }],
    [{ ...migratedBook.styleModules[0], css: "中".repeat(10923) }],
    Array.from({ length: MAX_STYLE_MODULES + 1 }, (_, index) => ({
      ...migratedBook.styleModules[0],
      id: `overflow-${index}`,
    })),
    [
      { ...migratedBook.styleModules[0], id: "combined-a", css: "x".repeat(32768) },
      { ...migratedBook.styleModules[0], id: "combined-b", css: "x".repeat(32768) },
      { ...migratedBook.styleModules[0], id: "combined-c", css: "x" },
    ],
  ]) {
    let rejected = false;
    try {
      validateModules(invalidModules);
    } catch {
      rejected = true;
    }
    assert(rejected, "sample-boundary");
  }
  apply();
  return Object.freeze({ benchmarkStyleModules, bind, reset, restore, snapshot, update });
}
