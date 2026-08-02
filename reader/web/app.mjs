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

const content = createContent({
  host: document.querySelector("#book-host"),
  readerStyleSource: document.querySelector("#reader-style-source"),
  fail,
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
const navigation = createNavigation({
  session,
  pagination,
  locator,
  toc: document.querySelector("#toc"),
  previous: document.querySelector("#previous"),
  next: document.querySelector("#next"),
  fontSizeControl: document.querySelector("#font-size"),
  onFallback(reason) {
    document.documentElement.dataset.locatorFallback = reason;
  },
  assert,
  fail,
});

const diagnostics = createDiagnostics({
  params,
  content,
  pagination,
  session,
  locator,
  navigation,
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
  diagnostics.recordFirstStable(firstStableStarted);

  if (params.has("verify")) await diagnostics.verify();
  if (params.get("benchmark") === "hot") await diagnostics.benchmark();

  diagnostics.complete();
}

start().catch((error) => {
  if (!document.documentElement.dataset.error) {
    const code = error instanceof Error && /^[a-z-]+$/.test(error.message) ? error.message : "book-load";
    fail(code);
  }
});
