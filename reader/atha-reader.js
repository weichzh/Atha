"use strict";

const params = new URLSearchParams(location.search);
const SOURCE_FONT_SIZE = 16;
const DISPLAY_FORMULA_MULTIPLIER = 1.5;
const BENCHMARK_SAMPLES = 10;
const state = { page: 0, pages: 0, fontSize: 32 };
const reader = document.querySelector(".reader");
const page = document.querySelector("#page");
const bookHost = document.querySelector("#book-host");
const position = document.querySelector("#position");
const errorBox = document.querySelector("#error");
const fontSizeControl = document.querySelector("#font-size");
const previous = document.querySelector("#previous");
const next = document.querySelector("#next");
const shadow = bookHost.attachShadow({ mode: "closed" });
const bookStyle = document.createElement("style");
const readerStyle = document.createElement("style");
const userStyle = document.createElement("style");
const book = document.createElement("article");
book.className = "book";
shadow.append(bookStyle, readerStyle, userStyle, book);

let bookUrl;
let bookOrigin;
let cachedXhtml;
let cachedCss;

function fitReader() {
  reader.style.setProperty(
    "--fit-scale",
    String(Math.min(window.innerWidth / 1264, window.innerHeight / 1680)),
  );
}

fitReader();
window.addEventListener("resize", fitReader);

function emit(message) {
  if (window.ipc?.postMessage) window.ipc.postMessage(message);
}

function fail(code) {
  if (!fail.suppressed) {
    document.documentElement.dataset.status = "fail";
    document.documentElement.dataset.error = code;
    errorBox.hidden = false;
    emit(`error|${code}`);
    console.error(`Atha reader failed: ${code}`);
  }
  throw new Error(code);
}
fail.suppressed = false;

function assert(condition, code) {
  if (!condition) fail(code);
}

function configuredBookUrl() {
  const override = params.get("book");
  if (override) return new URL(override, location.href);
  const entry = params.get("entry");
  assert(entry, "missing-book-url");
  return new URL(entry.replace(/^\/+/, ""), "https://atha-book.localhost/");
}

function localBookUrl(value, base = bookUrl) {
  let url;
  try {
    url = new URL(value, base);
  } catch {
    fail("external-resource");
  }
  assert(url.origin === bookOrigin && !url.username && !url.password, "external-resource");
  assert(!url.search, "external-resource");
  return url.href;
}

function validateCss(css) {
  assert(!/@import|url\s*\(/i.test(css), "css-subresource");
  assert(!/:host(?:-context)?\b|::part\b|::slotted\b/i.test(css), "active-style");
}

function validateMarkup(documentNode) {
  assert(!documentNode.querySelector("parsererror") && !documentNode.doctype, "invalid-xhtml");
  assert(
    !documentNode.querySelector(
      "script, iframe, frame, object, embed, form, input, button, select, textarea, video, audio, source, track, base, meta[http-equiv], foreignObject",
    ),
    "active-content",
  );

  for (const element of documentNode.querySelectorAll("*")) {
    const name = element.localName.toLowerCase();
    for (const attribute of element.attributes) {
      const attributeName = attribute.name.toLowerCase();
      assert(!attributeName.startsWith("on"), "event-handler");
      if (attributeName === "style") validateCss(attribute.value);
      if (["srcset", "poster", "action", "formaction", "ping"].includes(attributeName)) {
        fail("unsupported-resource-attribute");
      }
      if (["target", "download"].includes(attributeName)) fail("active-link");
      if (attributeName === "src" && name !== "img") {
        fail("unsupported-resource-attribute");
      }
      if (attributeName === "href" || attributeName.endsWith(":href")) {
        if (name === "a") {
          if (!attribute.value.startsWith("#")) {
            const target = new URL(attribute.value, bookUrl);
            assert(
              target.origin === bookOrigin &&
                target.pathname === bookUrl.pathname &&
                !target.search &&
                target.hash,
              "active-link",
            );
            element.setAttribute("href", target.hash);
          }
        } else if (name === "link") {
          assert(element.getAttribute("rel") === "stylesheet", "active-content");
          element.setAttribute("href", localBookUrl(attribute.value));
        } else {
          assert(attribute.value.startsWith("#"), "external-resource");
        }
      }
    }
    if (name === "style") validateCss(element.textContent);
  }

  for (const image of documentNode.querySelectorAll("img[src]")) {
    image.setAttribute("src", localBookUrl(image.getAttribute("src")));
  }
}

function parseSvg(svgText) {
  const svg = new DOMParser().parseFromString(svgText, "image/svg+xml");
  assert(!svg.querySelector("parsererror") && !svg.doctype, "invalid-svg");
  assert(
    !svg.querySelector("script, foreignObject, iframe, object, embed"),
    "invalid-svg",
  );
  for (const element of svg.querySelectorAll("*")) {
    for (const attribute of element.attributes) {
      const name = attribute.name.toLowerCase();
      assert(!name.startsWith("on"), "svg-event-handler");
      if (name === "href" || name.endsWith(":href")) {
        assert(attribute.value.startsWith("#"), "svg-external-resource");
      }
      if (name === "style") validateCss(attribute.value);
    }
    if (element.localName.toLowerCase() === "style") validateCss(element.textContent);
  }
  return svg;
}

async function validateSvg(url) {
  const response = await fetch(url);
  assert(response.ok, "svg-load");
  parseSvg(await response.text());
}

async function loadSource() {
  const response = await fetch(bookUrl);
  assert(response.ok, "book-load");
  cachedXhtml = await response.text();
  const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
  validateMarkup(source);
  const inlineCss = [...source.querySelectorAll("head > style")].map((style) => style.textContent);
  const stylesheetUrls = [...source.querySelectorAll("link[rel='stylesheet'][href]")].map((link) =>
    localBookUrl(link.getAttribute("href")),
  );
  assert(stylesheetUrls.length > 0, "missing-stylesheet");
  const stylesheets = await Promise.all(
    stylesheetUrls.map(async (url) => {
      const styleResponse = await fetch(url);
      assert(styleResponse.ok, "stylesheet-load");
      return styleResponse.text();
    }),
  );
  cachedCss = [...inlineCss, ...stylesheets].join("\n");
  validateCss(cachedCss);

  const svgUrls = [
    ...new Set([...source.querySelectorAll("img[src$='.svg']")].map((image) => image.src)),
  ];
  await Promise.all(svgUrls.map(validateSvg));
}

function importCachedSource() {
  const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
  validateMarkup(source);
  validateCss(cachedCss);
  bookStyle.textContent = cachedCss;
  const imported = document.importNode(source.body, true);
  book.replaceChildren(...imported.childNodes);
}

async function decodeImages() {
  await Promise.all(
    [...book.querySelectorAll("img")].map(async (image) => {
      try {
        await image.decode();
      } catch {
        fail("image-load");
      }
    }),
  );
  await document.fonts.ready;
}

function applyFormulaScale() {
  const fontScale = state.fontSize / SOURCE_FONT_SIZE;
  const contentWidth = book.clientWidth;
  for (const formula of book.querySelectorAll("img.math-inline, img.math-display")) {
    if (!formula.dataset.sourceWidth) {
      const width = Number(formula.getAttribute("width"));
      const height = Number(formula.getAttribute("height"));
      assert(Number.isFinite(width) && width > 0 && Number.isFinite(height) && height > 0, "invalid-formula-size");
      formula.dataset.sourceWidth = String(width);
      formula.dataset.sourceHeight = String(height);
      formula.dataset.sourceAlign = String(parseFloat(formula.style.verticalAlign) || 0);
    }
    const sourceWidth = Number(formula.dataset.sourceWidth);
    const sourceHeight = Number(formula.dataset.sourceHeight);
    const isDisplay = formula.classList.contains("math-display");
    const requestedScale = fontScale * (isDisplay ? DISPLAY_FORMULA_MULTIPLIER : 1);
    const scale = Math.min(requestedScale, contentWidth / sourceWidth);
    formula.style.width = `${sourceWidth * scale}px`;
    formula.style.height = `${sourceHeight * scale}px`;
    formula.style.verticalAlign = isDisplay ? "0px" : `${Number(formula.dataset.sourceAlign) * scale}px`;
  }
}

function layout() {
  book.style.fontSize = `${state.fontSize}px`;
  book.style.transform = "none";
  applyFormulaScale();
  const style = getComputedStyle(book);
  const width = parseFloat(style.width);
  const gap = parseFloat(style.columnGap);
  state.pages = Math.max(1, Math.round((book.scrollWidth + gap) / (width + gap)));
  state.page = Math.min(state.page, state.pages - 1);
  showPage();
}

function showPage() {
  const style = getComputedStyle(book);
  const step = parseFloat(style.width) + parseFloat(style.columnGap);
  book.style.transform = `translateX(${-state.page * step}px)`;
  position.textContent = `${state.page + 1} / ${state.pages}`;
  previous.disabled = state.page === 0;
  next.disabled = state.page + 1 === state.pages;
  document.title = `Atha Reader — ${state.page + 1} / ${state.pages}`;
}

function countCutRects() {
  const savedPage = state.page;
  book.style.transform = "none";
  const pageRect = page.getBoundingClientRect();
  const tolerance = 0.75;
  let cuts = 0;
  const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    if (!walker.currentNode.textContent.trim()) continue;
    const range = document.createRange();
    range.selectNodeContents(walker.currentNode);
    for (const rect of range.getClientRects()) {
      if (rect.height && (rect.top < pageRect.top - tolerance || rect.bottom > pageRect.bottom + tolerance)) {
        cuts += 1;
      }
    }
  }
  for (const atomic of book.querySelectorAll("img, table, pre, figure")) {
    const rect = atomic.getBoundingClientRect();
    if (rect.height && (rect.top < pageRect.top - tolerance || rect.bottom > pageRect.bottom + tolerance)) {
      cuts += 1;
    }
  }
  state.page = savedPage;
  showPage();
  return cuts;
}

function layoutSignature() {
  const rect = book.getBoundingClientRect();
  return [state.pages, book.scrollWidth, book.scrollHeight, rect.width.toFixed(2), rect.height.toFixed(2)].join(":");
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

async function waitForStableLayout() {
  let previousSignature = "";
  let equalFrames = 0;
  for (let frame = 0; frame < 20; frame += 1) {
    await nextFrame();
    const signature = layoutSignature();
    equalFrames = signature === previousSignature ? equalFrames + 1 : 0;
    if (equalFrames >= 2) return;
    previousSignature = signature;
  }
  fail("unstable-layout");
}

async function renderCachedSource() {
  importCachedSource();
  await decodeImages();
  state.page = 0;
  layout();
  await waitForStableLayout();
  assert(countCutRects() === 0, "layout-cut");
}

function verifyFormulaLayout() {
  const inline = [...book.querySelectorAll("img.math-inline")];
  const display = [...book.querySelectorAll("img.math-display")];
  const formulas = [...inline, ...display];
  if (formulas.length === 0) return;
  const savedTransform = book.style.transform;
  book.style.transform = "none";
  const fitScale = page.getBoundingClientRect().width / page.clientWidth;
  const bookRect = book.getBoundingClientRect();
  const style = getComputedStyle(book);
  const columnWidth = parseFloat(style.width);
  const columnStep = columnWidth + parseFloat(style.columnGap);
  try {
    for (const formula of formulas) {
      const rect = formula.getBoundingClientRect();
      const sourceWidth = Number(formula.dataset.sourceWidth);
      const sourceHeight = Number(formula.dataset.sourceHeight);
      const width = rect.width / fitScale;
      const height = rect.height / fitScale;
      const isDisplay = formula.classList.contains("math-display");
      const expectedScale = Math.min(
        (state.fontSize / SOURCE_FONT_SIZE) * (isDisplay ? DISPLAY_FORMULA_MULTIPLIER : 1),
        columnWidth / sourceWidth,
      );
      assert(Math.abs(width / sourceWidth - expectedScale) <= 0.02, "formula-selectors");
      assert(Math.abs(width / height - sourceWidth / sourceHeight) <= 0.02, "formula-selectors");
      assert(width <= columnWidth + 0.75, "layout-cut");
      if (isDisplay) {
        const logicalLeft = (rect.left - bookRect.left) / fitScale;
        const column = Math.round(logicalLeft / columnStep);
        const centerOffset = logicalLeft + width / 2 - (column * columnStep + columnWidth / 2);
        assert(Math.abs(centerOffset) <= 2, "formula-selectors");
      }
    }
  } finally {
    book.style.transform = savedTransform;
  }
}

async function verifySizes() {
  for (const size of [24, 32, 40]) {
    state.fontSize = size;
    layout();
    await waitForStableLayout();
    assert(countCutRects() === 0, "layout-cut");
    verifyFormulaLayout();
  }
  state.fontSize = 32;
  fontSizeControl.value = "32";
  layout();
  await waitForStableLayout();
}

function rejected(action) {
  fail.suppressed = true;
  try {
    action();
    return false;
  } catch {
    return true;
  } finally {
    fail.suppressed = false;
  }
}

function validatorSelfCheck() {
  for (const markup of [
    "<html xmlns='http://www.w3.org/1999/xhtml'><body><script>1</script></body></html>",
    "<html xmlns='http://www.w3.org/1999/xhtml'><body><p onclick='x()'>x</p></body></html>",
    "<html xmlns='http://www.w3.org/1999/xhtml'><body><img src='https://example.com/x.png'/></body></html>",
    "<html xmlns='http://www.w3.org/1999/xhtml'><body><form/></body></html>",
    "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='other.xhtml#x'>x</a></body></html>",
  ]) {
    assert(
      rejected(() => validateMarkup(new DOMParser().parseFromString(markup, "application/xhtml+xml"))),
      "active-content",
    );
  }
  const samePage = new DOMParser().parseFromString(
    `<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='${bookUrl.pathname}#x'>x</a></body></html>`,
    "application/xhtml+xml",
  );
  validateMarkup(samePage);
  assert(samePage.querySelector("a").getAttribute("href") === "#x", "active-link");
  for (const css of ["@import 'x.css';", "p{background:url(x)}", ":host{display:none}"]) {
    assert(rejected(() => validateCss(css)), "active-style");
  }
  for (const svg of [
    "<svg xmlns='http://www.w3.org/2000/svg'><script/></svg>",
    "<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.com/x'/></svg>",
  ]) {
    assert(rejected(() => parseSvg(svg)), "invalid-svg");
  }
}

async function securityProbe() {
  const probeUrl = params.get("probe");
  assert(probeUrl, "network-block");
  let violations = 0;
  const onViolation = () => {
    violations += 1;
  };
  document.addEventListener("securitypolicyviolation", onViolation);
  const probe = new Image();
  probe.src = probeUrl;
  document.body.append(probe);
  await new Promise((resolve) => setTimeout(resolve, 150));
  probe.remove();
  document.removeEventListener("securitypolicyviolation", onViolation);
  assert(violations > 0, "network-block");
}

async function measureHotOpen() {
  for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
    const started = performance.now();
    await renderCachedSource();
    emit(`metric|hot_open|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`);
  }
}

async function measurePageTurns() {
  assert(state.pages > 1, "layout-cut");
  for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
    const started = performance.now();
    state.page = sample % 2;
    showPage();
    await nextFrame();
    assert(countCutRects() === 0, "layout-cut");
    await nextFrame();
    emit(`metric|page_turn|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`);
  }
}

async function measureFontReflow() {
  for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
    const started = performance.now();
    state.fontSize = sample % 2 ? 40 : 24;
    layout();
    await waitForStableLayout();
    assert(countCutRects() === 0, "layout-cut");
    emit(`metric|font_reflow|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`);
  }
  state.fontSize = 32;
  fontSizeControl.value = "32";
  layout();
  await waitForStableLayout();
}

async function setFontSize(value) {
  state.fontSize = Number(value);
  layout();
  await waitForStableLayout();
  assert(countCutRects() === 0, "layout-cut");
}

previous.addEventListener("click", async () => {
  if (state.page > 0) state.page -= 1;
  showPage();
  await nextFrame();
});
next.addEventListener("click", async () => {
  if (state.page + 1 < state.pages) state.page += 1;
  showPage();
  await nextFrame();
});
fontSizeControl.addEventListener("change", () => {
  setFontSize(fontSizeControl.value).catch((error) => {
    if (!document.documentElement.dataset.error) fail(error instanceof Error ? error.message : "layout-cut");
  });
});

async function start() {
  bookUrl = configuredBookUrl();
  bookOrigin = bookUrl.origin;
  validatorSelfCheck();
  const readerStyleResponse = await fetch(document.querySelector("#reader-style-source").href);
  assert(readerStyleResponse.ok, "reader-style-load");
  readerStyle.textContent = await readerStyleResponse.text();

  const firstStableStarted = performance.now();
  await loadSource();
  await renderCachedSource();
  if (params.has("verify")) {
    await verifySizes();
    verifyFormulaLayout();
    await securityProbe();
  }

  if (params.get("benchmark") !== "hot") {
    emit(`metric|first_stable|1|${(performance.now() - firstStableStarted).toFixed(3)}|${state.fontSize}|${state.pages}`);
  }
  if (params.get("benchmark") === "hot") {
    await measureHotOpen();
    await measurePageTurns();
    await measureFontReflow();
  }

  const inline = book.querySelectorAll("img.math-inline").length;
  const display = book.querySelectorAll("img.math-display").length;
  const cuts = countCutRects();
  document.documentElement.dataset.status = "pass";
  document.documentElement.dataset.pages = String(state.pages);
  document.documentElement.dataset.inlineFormulas = String(inline);
  document.documentElement.dataset.displayFormulas = String(display);
  document.documentElement.dataset.cuts = String(cuts);
  emit(`ready|${state.pages}|${inline}|${display}|${cuts}`);
}

start().catch((error) => {
  if (!document.documentElement.dataset.error) {
    const code = error instanceof Error && /^[a-z-]+$/.test(error.message) ? error.message : "book-load";
    fail(code);
  }
});
