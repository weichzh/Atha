const params = new URLSearchParams(location.search);
const root = document.documentElement;
const reader = document.querySelector(".reader");
const errorBox = document.querySelector("#error");

function toggleReaderTools() {
  if (root.hasAttribute("data-reader-tools")) {
    root.removeAttribute("data-reader-tools");
    for (const panel of document.querySelectorAll(".reader-tool[open]")) panel.open = false;
  } else {
    root.setAttribute("data-reader-tools", "");
  }
}

function emit(message) {
  if (window.ipc?.postMessage) window.ipc.postMessage(message);
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
  progressPosition: document.querySelector("#progress-position"),
  progressRange: document.querySelector("#progress-range"),
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  assert,
  fail,
});

let annotations;
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
const keyPrefix = params.has("state-probe") ? "atha.reader.probe" : "atha.reader";
const bookKey = params.get("state");
const navigation = createNavigation({
  session,
  pagination,
  locator,
  preferences,
  toc: document.querySelector("#toc"),
  chapterLabel: document.querySelector("#chapter-label"),
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
const annotationStore = createAnnotationStore({
  storage: durableStorage(),
  requireDurable: !params.has("verify") || params.has("persist"),
  keyPrefix,
  bookKey,
  locator,
});
annotations = createAnnotations({
  store: annotationStore,
  content,
  session,
  navigation,
  locator,
  controls: {
    add: document.querySelector("#add-annotation"),
    note: document.querySelector("#annotation-note"),
    list: document.querySelector("#annotations"),
    go: document.querySelector("#go-annotation"),
    saveNote: document.querySelector("#save-annotation-note"),
    remove: document.querySelector("#delete-annotation"),
    status: document.querySelector("#annotations-status"),
  },
  assert,
});
bookmarks = createBookmarks({
  state: readerState,
  navigation,
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
  onCenter: toggleReaderTools,
  assert,
  fail,
});

document.querySelector("#reader-back").addEventListener("click", () => {
  if (history.length > 1) history.back();
  else window.close();
});
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
  await annotations.restore();
  await readerState.restore();
  readerState.bind();
  bookmarks.bind();
  search.bind();
  annotations.bind();
  contentActions.bind();
  structuredActions.bind();
  interaction.bind();
  diagnostics.recordFirstStable(firstStableStarted);

  const stateProbe = params.get("state-probe");
  if (stateProbe) {
    await readerState.verifyPersistence(stateProbe);
    await annotations.verifyPersistence(stateProbe);
  }
  else if (params.has("verify-import")) await diagnostics.verifyImport();
  else if (params.has("verify")) await diagnostics.verify();
  if (params.get("benchmark") === "hot") await diagnostics.benchmark();

  diagnostics.complete();
}

start().catch((error) => {
  if (!document.documentElement.dataset.error) {
    const code = error instanceof Error && /^[a-z-]+$/.test(error.message) ? error.message : "book-load";
    fail(code);
  }
});
