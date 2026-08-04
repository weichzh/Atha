const params = new URLSearchParams(location.search);
const root = document.documentElement;
const reader = document.querySelector(".reader");
const errorBox = document.querySelector("#error");

document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });

function closeReaderTools() {
  root.removeAttribute("data-reader-tools");
  for (const panel of document.querySelectorAll(".reader-tool[open]")) panel.open = false;
}

function toggleReaderTools() {
  annotations?.dismissSelection();
  if (root.hasAttribute("data-reader-tools")) closeReaderTools();
  else root.setAttribute("data-reader-tools", "");
}

function emit(message) {
  const bridge = window.athaReaderIpc || window.ipc;
  if (bridge?.postMessage) bridge.postMessage(message);
}

function fail(code) {
  document.documentElement.dataset.status = "fail";
  document.documentElement.dataset.error = code;
  errorBox.hidden = false;
  emit(`error|${code}`);
  console.error(`Atha reader failed: ${code}`);
  throw new Error(code);
}

function assert(condition, code) {
  if (!condition) fail(code);
}

function durableStorage() {
  if (params.has("verify") && !params.has("persist")) return null;
  try {
    return localStorage;
  } catch {
    return null;
  }
}

const content = createContent({
  host: document.querySelector("#book-host"),
  readerStyleSource: document.querySelector("#reader-style-source"),
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
    fontFamily: document.querySelector("#font-family"),
    density: document.querySelector("#density"),
    tapToPaginate: document.querySelector("#tap-to-paginate"),
    swipeToPaginate: document.querySelector("#swipe-to-paginate"),
    sourceStyles: document.querySelector("#source-styles"),
    userStylesEnabled: document.querySelector("#user-styles-enabled"),
    userStylesheet: document.querySelector("#user-stylesheet"),
    applyUserStyle: document.querySelector("#apply-user-style"),
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
  assert,
  fail,
});

let annotations;
let conversations;
let bookmarks;
async function renderCachedSource() {
  await content.renderCached();
  await pagination.renderFromStart();
  await annotations?.redraw();
}

const session = createReadingSession({
  params,
  content,
  render: renderCachedSource,
  onState(state) {
    document.documentElement.dataset.sessionState = state;
  },
  assert,
  fail,
});

const locator = createLocator({ assert });
let readerState;
let syncDirectorySelection = () => {};
const keyPrefix = params.has("state-probe") ? "atha.reader.probe" : "atha.reader";
const bookKey = params.get("state");
const navigation = createNavigation({
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
    closeReaderTools();
    reader.focus({ preventScroll: true });
  },
  onOpenConversation: messageMode
    ? (conversationId, messageId) => conversations?.open(conversationId, messageId)
    : null,
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
      source: document.querySelector("#message-conversation-source"),
      close: document.querySelector("#message-conversation-close"),
      collapse: document.querySelector("#message-conversation-collapse"),
      exportButton: document.querySelector("#message-conversation-export"),
      exportAllButton: document.querySelector("#message-export-all"),
      content: document.querySelector("#message-conversation-content"),
      list: document.querySelector("#message-conversation-list"),
      form: document.querySelector("#message-composer"),
      composerContext: document.querySelector("#message-composer-context"),
      text: document.querySelector("#message-composer-text"),
      references: document.querySelector("#message-composer-references"),
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
  preferences,
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
    closeReaderTools();
    reader.focus({ preventScroll: true });
  });
  new MutationObserver(render).observe(source, { childList: true });
  syncDirectorySelection = sync;
  render();
}

document.querySelectorAll("[data-close-reader-tools]").forEach((button) => {
  button.addEventListener("click", closeReaderTools);
});
bindSettingsNavigation();
const diagnostics = createDiagnostics({
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
  bookmarks,
  search,
  annotations,
  reader,
  renderCachedSource,
  emit,
  assert,
});

pagination.initialize();

async function start() {
  await content.initialize();
  const firstStableStarted = performance.now();
  await session.open();
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
  diagnostics.recordFirstStable(firstStableStarted);

  const stateProbe = params.get("state-probe");
  const benchmarkMode = params.get("benchmark");
  if (stateProbe) {
    await readerState.verifyPersistence(stateProbe);
    await annotations.verifyPersistence(stateProbe);
  }
  else if (params.has("verify-import")) await diagnostics.verifyImport();
  else if (params.has("verify") && !benchmarkMode) await diagnostics.verify();
  if (benchmarkMode === "hot") await diagnostics.benchmark();

  diagnostics.complete();
}

start().catch((error) => {
  if (!document.documentElement.dataset.error) {
    const reason = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    const code = /^[a-z-]+$/.test(reason) ? reason : "book-load";
    fail(code);
  }
});
