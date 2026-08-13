const params = new URLSearchParams(location.search);
const root = document.documentElement;
const reader = document.querySelector(".reader");
const readerShell = document.querySelector(".reader-shell");
const readerStartup = document.querySelector(".reader-startup");
const desktopWorkspaceMedia = matchMedia("(min-width: 1100px)");
const workspaceTools = ["directory", "search", "notes"].map((name) => ({
  name,
  panel: document.querySelector(`.reader-tool.${name}`),
}));
const workspacePanels = new Set(workspaceTools.map((tool) => tool.panel));
const readerTools = [...document.querySelectorAll(".reader-tool")];
const runtimeErrors = [];
const errorBox = document.querySelector("#error");
const errorDetails = Object.freeze({
  "locator-offset": "恢复阅读位置失败：文字偏移量没有对应的可见内容",
  "layout-cut": "分页布局失败：正文或图片越过页面边界",
  "unstable-layout": "分页布局失败：页面尺寸持续变化",
});
const sessionStages = Object.freeze({
  opening: Object.freeze({ label: "打开书籍", token: "opening" }),
  "content-loaded": Object.freeze({ label: "章节载入后的分页定位", token: "content-loaded" }),
  "layout-stable": Object.freeze({ label: "阅读状态恢复", token: "layout-stable" }),
});

document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });
window.addEventListener("error", (event) => runtimeErrors.push(String(event.error?.message || event.message)));
window.addEventListener("unhandledrejection", (event) =>
  runtimeErrors.push(String(event.reason instanceof Error ? event.reason.message : event.reason)),
);
if (params.has("verify") || params.has("gesture-probe")) {
  const consoleError = console.error.bind(console);
  console.error = (...values) => {
    runtimeErrors.push(values.map(String).join(" "));
    consoleError(...values);
  };
  Object.defineProperty(globalThis, "__athaReaderRuntimeErrors", { value: runtimeErrors });
}

function closeReaderTools() {
  const desktop = root.hasAttribute("data-desktop-workspace");
  if (!desktop) root.removeAttribute("data-reader-tools");
  for (const panel of document.querySelectorAll(".reader-tool[open]")) {
    if (!desktop || !workspacePanels.has(panel)) panel.open = false;
  }
}

function finishReaderToolNavigation(toolName) {
  closeReaderTools();
  if (!root.hasAttribute("data-desktop-workspace") || root.dataset.workspacePanel === toolName) {
    reader.focus({ preventScroll: true });
  }
}

function toggleReaderTools() {
  annotations?.dismissSelection();
  if (root.hasAttribute("data-desktop-workspace")) return;
  if (root.hasAttribute("data-reader-tools")) closeReaderTools();
  else root.setAttribute("data-reader-tools", "");
}

function bindDesktopWorkspace() {
  let lastTool = workspaceTools[0];
  let settling = false;
  let workspaceVisible = false;

  const open = (tool, focus = false) => {
    for (const panel of readerTools) panel.open = panel === tool.panel;
    lastTool = tool;
    root.dataset.workspacePanel = tool.name;
    if (focus) requestAnimationFrame(() => tool.panel.querySelector("summary")?.focus());
  };
  const resize = () => {
    const visible = root.hasAttribute("data-desktop-workspace") && root.hasAttribute("data-workspace-panel");
    if (visible === workspaceVisible) return;
    workspaceVisible = visible;
    if (root.hasAttribute("data-reader-ready")) window.dispatchEvent(new Event("resize"));
  };
  const settle = () => {
    settling = false;
    if (!desktopWorkspaceMedia.matches) return;
    const active = workspaceTools.find((tool) => tool.panel.open);
    const auxiliaryOpen = readerTools.some((panel) => panel.open && !workspacePanels.has(panel));
    if (active) {
      lastTool = active;
      root.dataset.workspacePanel = active.name;
    } else if (!auxiliaryOpen) {
      open(lastTool);
    } else {
      delete root.dataset.workspacePanel;
    }
    resize();
  };
  const scheduleSettle = () => {
    if (settling) return;
    settling = true;
    queueMicrotask(settle);
  };
  const applyViewport = () => {
    root.toggleAttribute("data-desktop-workspace", desktopWorkspaceMedia.matches);
    if (desktopWorkspaceMedia.matches) {
      open(workspaceTools.find((tool) => tool.panel.open) || lastTool);
    } else {
      for (const tool of workspaceTools) tool.panel.open = false;
      delete root.dataset.workspacePanel;
    }
    resize();
  };

  for (const [index, tool] of workspaceTools.entries()) {
    tool.panel.querySelector("summary")?.addEventListener("keydown", (event) => {
      const nextIndex =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? workspaceTools.length - 1
            : event.key === "ArrowLeft"
              ? (index + workspaceTools.length - 1) % workspaceTools.length
              : event.key === "ArrowRight"
                ? (index + 1) % workspaceTools.length
                : -1;
      if (nextIndex < 0) return;
      event.preventDefault();
      open(workspaceTools[nextIndex], true);
    });
  }
  for (const panel of readerTools) panel.addEventListener("toggle", scheduleSettle);
  document.addEventListener(
    "keydown",
    (event) => {
      if (!desktopWorkspaceMedia.matches) return;
      if (
        (event.ctrlKey || event.metaKey) &&
        !event.altKey &&
        !event.shiftKey &&
        event.key.toLowerCase() === "f"
      ) {
        event.preventDefault();
        open(workspaceTools[1]);
        requestAnimationFrame(() => document.querySelector("#search-query")?.focus());
      } else if (
        event.key === "Escape" &&
        event.target.closest?.(".reader-tool.directory, .reader-tool.search, .reader-tool.notes")
      ) {
        event.preventDefault();
        reader.focus({ preventScroll: true });
      }
    },
    true,
  );
  desktopWorkspaceMedia.addEventListener("change", applyViewport);
  applyViewport();
}

function emit(message) {
  const bridge = window.athaReaderIpc || window.ipc;
  if (bridge?.postMessage) bridge.postMessage(message);
}

function revealReader() {
  root.setAttribute("data-reader-ready", "");
  readerShell.setAttribute("aria-busy", "false");
  readerStartup.setAttribute("aria-hidden", "true");
}

function fail(code, operationStage = null) {
  document.documentElement.dataset.status = "fail";
  document.documentElement.dataset.error = code;
  const detail = errorDetails[code] || "处理书籍内容时发生错误";
  const sessionStage = sessionStages[document.documentElement.dataset.sessionState];
  const stage = operationStage || sessionStage?.label || "阅读器初始化";
  const diagnosticStage = operationStage
    ? "layout"
    : sessionStage?.token || "initialization";
  errorBox.textContent = `${detail}（错误代码：${code}；阶段：${stage}）。`;
  errorBox.hidden = false;
  revealReader();
  emit(`error|${code}|${diagnosticStage}`);
  console.error(`Atha reader failed: ${code}`);
  throw new Error(code);
}

function assert(condition, code, operationStage = null) {
  if (!condition) fail(code, operationStage);
}

function durableStorage() {
  if (params.has("verify") && !params.has("persist")) return null;
  try {
    return localStorage;
  } catch {
    return null;
  }
}

let navigation;
const content = createContent({
  host: document.querySelector("#book-host"),
  reader,
  readerStyleSource: document.querySelector("#reader-style-source"),
  onLateLayout: ({ offset, pageIndex, scrollTop }) => {
    if (!navigation) return;
    const sectionIndex = session.snapshot().currentIndex;
    void navigation.resize(offset, sectionIndex, pageIndex, scrollTop).catch(() => undefined);
  },
  fail,
});
const preferences = createPreferences({
  root,
  reader,
  content,
  controls: {
    theme: document.querySelector("#theme"),
    brightness: document.querySelector("#brightness"),
    fontSize: document.querySelector("#font-size"),
    fontSizeValue: document.querySelector("#font-size-value"),
    fontFamily: document.querySelector("#font-family"),
    density: document.querySelector("#density"),
    paragraphIndent: document.querySelector("#paragraph-indent"),
    paragraphSpacing: document.querySelector("#paragraph-spacing"),
    pageMargin: document.querySelector("#page-margin"),
    readingMode: document.querySelector("#reading-mode"),
    sourceStyles: document.querySelector("#source-styles"),
    userStylesEnabled: document.querySelector("#user-styles-enabled"),
    userStylesheet: document.querySelector("#user-stylesheet"),
    moduleSearch: document.querySelector("#style-module-search"),
    moduleFilter: document.querySelector("#style-module-filter"),
    moduleList: document.querySelector("#style-module-list"),
    moduleListView: document.querySelector("#style-module-list-view"),
    moduleName: document.querySelector("#style-module-name"),
    moduleGroupName: document.querySelector("#style-module-group"),
    moduleEnabled: document.querySelector("#style-module-enabled"),
    moduleAdd: document.querySelector("#add-style-module"),
    moduleSave: document.querySelector("#save-style-module"),
    moduleDelete: document.querySelector("#delete-style-module"),
    moduleUp: document.querySelector("#move-style-module-up"),
    moduleDown: document.querySelector("#move-style-module-down"),
    modulesEnable: document.querySelector("#enable-style-modules"),
    modulesDisable: document.querySelector("#disable-style-modules"),
    moduleTransferOpen: document.querySelector("#open-style-module-transfer"),
    moduleTransferDialog: document.querySelector("#style-module-transfer-dialog"),
    moduleTransfer: document.querySelector("#style-module-transfer"),
    moduleImport: document.querySelector("#import-style-modules"),
    moduleCopy: document.querySelector("#copy-style-modules"),
    resetApplication: document.querySelector("#reset-application-preferences"),
    resetBook: document.querySelector("#reset-book-preferences"),
    status: document.querySelector("#preferences-status"),
  },
  assert,
});
const pagination = createPagination({
  book: content.book,
  reader,
  page: document.querySelector("#page"),
  position: document.querySelector("#position"),
  progressRange: document.querySelector("#progress-range"),
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  onPageShown: (includeNextPage, beforeLayoutChange) =>
    content.loadVisible(includeNextPage, beforeLayoutChange),
  assert,
  fail,
});

let annotations;
let conversations;
let bookmarks;
let readingStatistics;
async function renderCachedSource() {
  await content.renderCached();
  await pagination.renderFromStart();
  await annotations?.redraw();
}

async function warmForVerification() {
  await content.warmRemaining();
  await pagination.resizeViewport(pagination.captureOffset());
}

const session = createReadingSession({
  params,
  content,
  render: renderCachedSource,
  onState(state) {
    document.documentElement.dataset.sessionState = state;
    readingStatistics?.setStable(state === "layout-stable");
  },
  assert,
  fail,
});

const locator = createLocator({ assert });
let readerState;
let syncDirectorySelection = () => {};
const keyPrefix = params.has("state-probe") ? "atha.reader.probe" : "atha.reader";
const bookKey = params.get("state");
navigation = createNavigation({
  session,
  pagination,
  locator,
  preferences,
  toc: document.querySelector("#toc"),
  chapterLabel: document.querySelector("#chapter-label"),
  progressChapter: document.querySelector("#progress-chapter"),
  progressBook: document.querySelector("#progress-book"),
  progressPosition: document.querySelector("#progress-position"),
  progressRange: document.querySelector("#progress-range"),
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  onFallback(reason) {
    document.documentElement.dataset.locatorFallback = reason;
  },
  onPreferences: (scope) => readerState?.savePreferences(scope),
  onStable() {
    readerState?.scheduleProgress();
    readingStatistics?.activity();
    bookmarks?.syncCurrent();
    syncDirectorySelection();
  },
  assert,
  fail,
});
readerState = createReaderState({
  storage: durableStorage(),
  keyPrefix,
  bookKey,
  session,
  navigation,
  pagination,
  preferences,
  locator,
  assert,
});
const legacyAnnotationStore = createAnnotationStore({
  storage: durableStorage(),
  requireDurable: !params.has("verify") || params.has("persist"),
  keyPrefix,
  bookKey,
  locator,
});
const messageMode = Boolean(window.athaMessages && !params.has("verify"));
const annotationStore =
  messageMode
    ? createMessageStore({
        client: window.athaMessages,
        legacy: legacyAnnotationStore,
        keyPrefix,
        bookKey,
        session,
        content,
        preferences,
        locator,
      })
    : legacyAnnotationStore;
annotations = createAnnotations({
  store: annotationStore,
  content,
  session,
  navigation,
  locator,
  controls: {
    selectionActions: document.querySelector("#selection-actions"),
    lookup: document.querySelector("#lookup-selection"),
    copy: document.querySelector("#copy-selection"),
    highlight: document.querySelector("#highlight-selection"),
    update: document.querySelector("#update-selection"),
    note: document.querySelector("#note-selection"),
    delete: document.querySelector("#delete-selection"),
    selectionStatus: document.querySelector("#selection-actions-status"),
    noteDialog: document.querySelector("#annotation-note-dialog"),
    noteForm: document.querySelector("#annotation-note-form"),
    noteHeading: document.querySelector("#annotation-note-heading"),
    noteInput: document.querySelector("#annotation-note"),
    cancelNote: document.querySelector("#cancel-annotation-note"),
    filterQuery: document.querySelector("#annotation-filter-query"),
    filterSection: document.querySelector("#annotation-filter-section"),
    list: document.querySelector("#annotations"),
    status: document.querySelector("#annotations-status"),
  },
  onNavigate() {
    finishReaderToolNavigation("notes");
  },
  onOpenConversation: messageMode
    ? (conversationId, messageId, edit = false) => conversations?.open(conversationId, messageId, edit)
    : null,
  assert,
});
bookmarks = createBookmarks({
  state: readerState,
  navigation,
  pagination,
  session,
  locator,
  controls: {
    add: document.querySelector("#add-bookmark"),
    list: document.querySelector("#toc"),
    status: document.querySelector("#bookmarks-status"),
  },
  assert,
});
const search = createSearch({
  session,
  navigation,
  pagination,
  locator,
  controls: {
    form: document.querySelector("#search-form"),
    query: document.querySelector("#search-query"),
    cancel: document.querySelector("#cancel-search"),
    results: document.querySelector("#search-results"),
    go: document.querySelector("#go-search-result"),
    status: document.querySelector("#search-status"),
  },
  assert,
  emit,
});
const contentActions = createContentActions({
  content,
  navigation,
  dialog: document.querySelector("#content-dialog"),
  title: document.querySelector("#content-dialog-title"),
  body: document.querySelector("#content-dialog-body"),
  image: document.querySelector("#content-dialog-image"),
  closeButton: document.querySelector("#close-content-dialog"),
  assert,
  fail,
});
const structuredActions = createStructuredActions({
  content,
  navigation,
  dialog: document.querySelector("#content-dialog"),
  title: document.querySelector("#content-dialog-title"),
  body: document.querySelector("#content-dialog-body"),
  image: document.querySelector("#content-dialog-image"),
  tablePreview: document.querySelector("#content-dialog-table"),
  codePreview: document.querySelector("#content-dialog-code"),
  contentActions,
  assert,
});
const interaction = createInteraction({
  reader,
  content,
  navigation,
  pagination,
  onCenter: toggleReaderTools,
  assert,
  fail,
});

document.querySelector("#reader-back").addEventListener("click", () => {
  if (history.length > 1) history.back();
  else window.close();
});

function bindSettingsNavigation() {
  const panel = document.querySelector("[data-settings-root]");
  if (!panel) return;
  const owner = panel.closest("details");
  const show = (name = "menu") => {
    panel.dataset.settingsView = name;
    for (const page of panel.querySelectorAll("[data-settings-page]")) {
      page.hidden = page.dataset.settingsPage !== name;
    }
    requestAnimationFrame(() => {
      panel.querySelector(`[data-settings-page="${name}"]:not([hidden]) h2`)?.focus();
    });
  };
  panel.addEventListener("click", (event) => {
    const target = event.target.closest("[data-settings-target], [data-settings-back]");
    if (!target) return;
    show(target.dataset.settingsTarget);
  });
  owner.addEventListener("toggle", () => {
    if (owner.open) show();
  });
}

function bindDirectoryProjection() {
  const source = document.querySelector("#toc");
  const target = document.querySelector("#directory-list");
  const sync = () => {
    for (const button of target.querySelectorAll("button")) {
      const current = button.dataset.value === source.value;
      button.classList.toggle("is-current", current);
      if (current) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    }
  };
  const render = () => {
    target.replaceChildren(
      ...[...source.options].map((option) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = option.dataset.bookmarkId ? "directory-item is-bookmark" : "directory-item";
        button.dataset.value = option.value;
        button.disabled = option.disabled;
        button.textContent = option.textContent;
        return button;
      }),
    );
    sync();
  };
  target.addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-value]");
    if (!button || button.disabled) return;
    const option = [...source.options].find((item) => item.value === button.dataset.value);
    if (!option) return;
    let navigated;
    try {
      navigated = option.dataset.bookmarkId
        ? await bookmarks.go(option.dataset.bookmarkId)
        : await navigation.goToToc(Number(option.value));
    } catch (error) {
      if (!option.dataset.bookmarkId) {
        fail(error instanceof Error ? error.message : "section-load");
      }
      return;
    }
    if (!navigated) return;
    finishReaderToolNavigation("directory");
  });
  new MutationObserver(render).observe(source, { childList: true });
  syncDirectorySelection = sync;
  render();
}

async function openReadingMemoryLink() {
  if (!messageMode || !conversations) return;
  const conversationId = params.get("memory-conversation");
  const messageId = params.get("memory-message");
  const rootMessageId = params.get("memory-root");
  if (!conversationId && !messageId && !rootMessageId) return;
  if (![conversationId, messageId, rootMessageId].every((id) => /^[a-f0-9]{32}$/.test(id || ""))) {
    return;
  }
  const navigation = await annotations.go(rootMessageId);
  root.dataset.readingMemoryNavigation = navigation.ok ? "ok" : "snapshot-only";
  await conversations.open(
    conversationId,
    messageId,
    false,
    rootMessageId,
    !navigation.ok,
  );
}

document.querySelectorAll("[data-close-reader-tools]").forEach((button) => {
  button.addEventListener("click", closeReaderTools);
});
bindSettingsNavigation();
let diagnostics;

bindDesktopWorkspace();

async function start() {
  await content.initialize();
  pagination.initialize();
  const firstStableStarted = performance.now();
  await session.open();
  readingStatistics = createReadingStatistics({
    storage: durableStorage(),
    keyPrefix,
    contentVersion: session.describe().contentVersion,
    controls: {
      today: document.querySelector("#statistics-today"),
      week: document.querySelector("#statistics-week"),
      book: document.querySelector("#statistics-book"),
      streak: document.querySelector("#statistics-streak"),
    },
  });
  readingStatistics.setStable(true);
  readingStatistics.bind();
  diagnostics = createDiagnostics({
    params,
    content,
    pagination,
    session,
    locator,
    navigation,
    preferences,
    interaction,
    contentActions,
    structuredActions,
    readerState,
    readingStatistics,
    bookmarks,
    search,
    annotations,
    reader,
    renderCachedSource,
    emit,
    assert,
  });
  if (messageMode) {
    conversations = createConversations({
      store: annotationStore,
      annotations,
      closeTools: closeReaderTools,
      editionId: session.describe().contentVersion,
      readingSurface: content.book,
      returnFocus: reader,
      controls: {
        overlay: document.querySelector("#message-conversation"),
        scopeButtons: document.querySelectorAll("[data-message-scope]"),
        orderButtons: document.querySelectorAll("[data-message-order]"),
        orderControls: document.querySelector("#message-order-controls"),
        sourceLabel: document.querySelector("#message-conversation-source-label"),
        source: document.querySelector("#message-conversation-source"),
        sourceJump: document.querySelector("#message-conversation-source-jump"),
        handle: document.querySelector("#message-conversation-handle"),
        close: document.querySelector("#message-conversation-close"),
        fullscreen: document.querySelector("#message-conversation-fullscreen"),
        exportAllButton: document.querySelector("#message-export-all"),
        content: document.querySelector("#message-conversation-content"),
        list: document.querySelector("#message-conversation-list"),
        form: document.querySelector("#message-composer"),
        composerContext: document.querySelector("#message-composer-context"),
        composerContextText: document.querySelector("#message-composer-context-text"),
        cancelEdit: document.querySelector("#message-composer-cancel"),
        status: document.querySelector("#message-conversation-status"),
        historyDialog: document.querySelector("#message-history-dialog"),
        historyTitle: document.querySelector("#message-history-title"),
        historyContent: document.querySelector("#message-history-content"),
        snapshotDialog: document.querySelector("#message-snapshot-dialog"),
        snapshotVersions: document.querySelector("#message-snapshot-versions"),
        snapshotContent: document.querySelector("#message-snapshot-content"),
      },
    });
  }
  navigation.bindControls();
  pagination.bindResize(() => navigation.resize());
  await annotations.restore();
  await readerState.restore();
  readerState.bind();
  await bookmarks.bind();
  bindDirectoryProjection();
  search.bind();
  annotations.bind();
  conversations?.bind();
  contentActions.bind();
  structuredActions.bind();
  interaction.bind();
  await openReadingMemoryLink();
  revealReader();
  diagnostics.recordFirstStable(firstStableStarted);

  const stateProbe = params.get("state-probe");
  const benchmarkMode = params.get("benchmark");
  let fullLayoutCheck = false;
  if (stateProbe) {
    await readerState.verifyPersistence(stateProbe);
    await annotations.verifyPersistence(stateProbe);
    await warmForVerification();
    fullLayoutCheck = true;
  }
  else if (params.has("verify-import")) {
    await diagnostics.verifyImport();
    await warmForVerification();
    fullLayoutCheck = true;
  }
  else if (params.has("verify") && !benchmarkMode) {
    await diagnostics.verify();
    await warmForVerification();
    fullLayoutCheck = true;
  }
  else if (params.has("style-module-probe")) {
    const result = preferences.benchmarkStyleModules();
    assert(result.modules === 32 && result.bytes <= 65536 && result.p95Ms < 50, "style-module-performance");
    root.dataset.styleModuleP95 = result.p95Ms.toFixed(3);
    root.dataset.styleModuleBytes = String(result.bytes);
  }
  if (benchmarkMode === "hot") {
    await diagnostics.benchmark();
    fullLayoutCheck = true;
  }

  diagnostics.complete(fullLayoutCheck);
}

start().catch((error) => {
  if (!document.documentElement.dataset.error) {
    const reason = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    const code = /^[a-z-]+$/.test(reason) ? reason : "book-load";
    fail(code);
  }
});
