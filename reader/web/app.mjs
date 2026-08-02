const params = new URLSearchParams(location.search);
const reader = document.querySelector(".reader");
const errorBox = document.querySelector("#error");

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
  root: document.documentElement,
  reader,
  content,
  controls: {
    theme: document.querySelector("#theme"),
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
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  assert,
  fail,
});

async function renderCachedSource() {
  await content.renderCached();
  await pagination.renderFromStart();
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
const navigation = createNavigation({
  session,
  pagination,
  locator,
  preferences,
  toc: document.querySelector("#toc"),
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  onFallback(reason) {
    document.documentElement.dataset.locatorFallback = reason;
  },
  onPreferences: (scope) => readerState?.savePreferences(scope),
  onStable: () => readerState?.scheduleProgress(),
  assert,
  fail,
});
readerState = createReaderState({
  storage: durableStorage(),
  keyPrefix: params.has("state-probe") ? "atha.reader.probe" : "atha.reader",
  bookKey: params.get("state"),
  session,
  navigation,
  pagination,
  preferences,
  locator,
  assert,
});
const bookmarks = createBookmarks({
  state: readerState,
  navigation,
  session,
  controls: {
    add: document.querySelector("#add-bookmark"),
    list: document.querySelector("#bookmarks"),
    go: document.querySelector("#go-bookmark"),
    remove: document.querySelector("#delete-bookmark"),
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
const interaction = createInteraction({ reader, content, navigation, assert, fail });

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
  await readerState.restore();
  readerState.bind();
  bookmarks.bind();
  search.bind();
  contentActions.bind();
  structuredActions.bind();
  interaction.bind();
  diagnostics.recordFirstStable(firstStableStarted);

  const stateProbe = params.get("state-probe");
  if (stateProbe) await readerState.verifyPersistence(stateProbe);
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
