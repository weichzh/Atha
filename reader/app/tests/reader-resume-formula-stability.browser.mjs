import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const base = "https://resume-formula-fixture.test";
const content = await readFile(new URL("../../web/content.mjs", import.meta.url));
const conversations = await readFile(new URL("../../web/conversations.mjs", import.meta.url));
const layout = await readFile(new URL("../src/styles/book-reader.css", import.meta.url));
const placeholder = await readFile(new URL("../src/assets/formula-placeholder.svg", import.meta.url));
const svg = (width, height) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">
<rect width="${width}" height="${height}"/></svg>`;
const chapter = `<html xmlns="http://www.w3.org/1999/xhtml"><body>
<style>img { visibility: visible; font-size: 19px; line-height: 29px; }</style>
<p>Before <img id="inline" class="math-inline" src="inline.svg" width="52" height="19"
  style="width: auto; height: 1em; vertical-align: -0.2em"
  alt="A long equation alternative that must not enter the column"/> <span id="after-inline">after</span></p>
<p><img id="display" class="math-display" src="display.svg" width="110" height="46"
  alt="A display equation alternative that must not enter the column"/></p>
<p><img id="automatic" class="math-inline" src="automatic.svg" width="52" height="19"
  style="width: auto; height: auto" alt="An intrinsically sized equation"/></p>
<p id="anchor">Saved position after the formulas.</p>
${"<p>Additional text in a long, paginated chapter.</p>".repeat(12)}
<p><img id="unavailable" class="math-inline" src="unavailable.svg" width="52" height="19" alt="Unavailable equation"/></p>
<p><img class="math-inline" src="broken.svg" width="52" height="19" alt="Rejected equation"/></p>
</body></html>`;
const files = {
  "/": ["text/html", "<main style='width:270px;height:210px'><div style='height:100%' id='host'></div></main>"],
  "/content.mjs": ["text/javascript", content],
  "/conversations.mjs": ["text/javascript", conversations],
  "/theme.css": ["text/css", ".book{--reader-col-w:270px;--book-line-height:1.5;column-gap:24px}"],
  "/layout.css": ["text/css", layout],
  "/placeholder.svg": ["image/svg+xml", placeholder],
  "/chapter.xhtml": ["application/xhtml+xml", chapter],
  "/inline.svg": ["image/svg+xml", svg(52, 19)],
  "/display.svg": ["image/svg+xml", svg(110, 46)],
  "/automatic.svg": ["image/svg+xml", svg(52, 19)],
  "/unavailable.svg": ["image/svg+xml", svg(52, 19)],
  "/broken.svg": ["image/svg+xml", "Not an SVG"],
};

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  const { promise: allowImage, resolve: releaseImage } = Promise.withResolvers();
  const { promise: allowFailure, resolve: releaseFailure } = Promise.withResolvers();
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.href === `${base}/inline.svg` && route.request().resourceType() === "image") {
      await allowImage;
    }
    if (url.href === `${base}/unavailable.svg` && route.request().resourceType() === "image") {
      await allowFailure;
      return route.abort();
    }
    const file = url.origin === base && files[url.pathname];
    return file ? route.fulfill({ contentType: file[0], body: file[1] }) : route.abort();
  });
  await page.goto(base);
  const start = await page.evaluate(async () => {
    const { createContent } = await import("/content.mjs");
    const view = createContent({
      reader: document.querySelector("main"),
      host: document.querySelector("#host"),
      onLateLayout() {},
      formulaPlaceholderUrl: new URL("/placeholder.svg", location.href).href,
      themeStyleSource: { href: "/theme.css" },
      readerStyleSource: { href: "/layout.css" },
    });
    await view.initialize();
    view.setStyles({ sourceStyles: true, userStylesEnabled: false, userStylesheet: "" });
    view.activateSection(await view.prepareSection(new URL("/chapter.xhtml", location.href), null, "section-load"));
    await view.renderCached();

    const inBook = (selector) => view.book.querySelector(selector);
    const formulas = [inBook("#inline"), inBook("#display"), inBook("#automatic")];
    const rect = (element) => {
      const { x, y, width, height } = element.getBoundingClientRect();
      return { x, y, width, height };
    };
    const geometry = () => ({
      formulas: formulas.map(rect),
      afterInline: rect(inBook("#after-inline")),
      anchor: rect(inBook("#anchor")),
    });
    const states = () => formulas.map((formula) => {
      const { visibility } = getComputedStyle(formula);
      return {
        pending: formula.classList.contains("atha-resource-pending"),
        hasSource: formula.hasAttribute("src"),
        visibility,
      };
    });
    const pending = states();
    const pendingSource = formulas[0].getAttribute("srcset");
    const before = geometry();
    window.finishFormulaTest = async () => {
      await formulas[0].decode();
      if (formulas[0].classList.contains("atha-resource-pending")) {
        await new Promise((resolve) => formulas[0].addEventListener("load", resolve, { once: true }));
      }
      const loaded = states();
      const after = geometry();
      await view.warmRemaining();
      return { loaded, after };
    };
    window.failureState = () => {
      const image = inBook("#unavailable");
      const failure = inBook('.atha-image-error[data-label="Unavailable equation"]');
      return {
        pending: image?.classList.contains("atha-resource-pending"),
        visible: failure && getComputedStyle(failure).visibility === "visible",
        indicator: failure && getComputedStyle(failure, "::after").content,
        rejected: Boolean(inBook('.atha-image-error[data-label="Rejected equation"]')),
      };
    };
    window.playSnapshot = async () => {
      const { renderSnapshotElement } = await import("/conversations.mjs");
      const range = document.createRange();
      range.selectNodeContents(formulas[0].parentElement);
      const snapshot = await view.captureRange(range, {
        theme: "light", brightness: 100, fontSize: 19, fontFamily: "book", density: "standard",
      });
      const player = await renderSnapshotElement({ source: { id: "fixture" }, snapshot }, {
        snapshotResource: async (_, path) => snapshot.resources.find((resource) => resource.path === path),
      });
      document.body.append(player);
      const picture = player.shadowRoot.querySelector("img");
      await picture.decode();
      return {
        visibility: getComputedStyle(picture).visibility,
        resources: snapshot.resources.map(({ path }) => path),
        runtimeAttributes: /srcset|atha-resource-pending|data-atha-resource|aria-busy/.test(snapshot.fragmentHtml),
      };
    };
    const deferredCapture = await window.playSnapshot();
    const visible = await view.loadVisible();
    return {
      pending, pendingSource, before, during: geometry(), deferredCapture, layoutChanged: visible.layoutChanged,
      waiting: states()[0],
    };
  });
  const snapshot = await page.evaluate(() => window.playSnapshot());
  releaseImage();
  assert.deepEqual(start.pending, Array(3).fill({
    pending: true, hasSource: false, visibility: "hidden",
  }), "Deferred formulas must keep the browser's native image fallback out of the page");
  assert.equal(start.pendingSource, `${base}/placeholder.svg`);
  assert.deepEqual(start.before.formulas.map(({ width, height }) => [width, height]), [[52, 19], [110, 46], [52, 19]]);
  assert.deepEqual(start.during, start.before);
  assert.equal(start.layoutChanged, false);
  assert.deepEqual(start.waiting, { pending: true, hasSource: true, visibility: "hidden" },
    "The placeholder is not a successful load of the requested formula");
  assert.deepEqual(snapshot, { visibility: "visible", resources: ["inline.svg"], runtimeAttributes: false });
  assert.deepEqual(start.deferredCapture, snapshot);
  const actual = await page.evaluate(() => window.finishFormulaTest());
  assert.equal((await page.evaluate(() => window.failureState())).pending, true);
  releaseFailure();
  await page.waitForFunction(() => window.failureState().visible);
  const failure = await page.evaluate(() => window.failureState());
  assert.deepEqual(actual.loaded, Array(3).fill({
    pending: false, hasSource: true, visibility: "visible",
  }));
  assert.deepEqual(actual.after, start.before, "Decoding must not move the text or the saved-position marker");
  assert.equal(failure.indicator, '"⚠"', "A late native error must show the reader's visible failure indicator");
  assert.equal(failure.rejected, true, "An invalid SVG must not be presented for decoding");
  console.log("Reader resume formulas: pending paint and loaded geometry verified.");
} finally {
  await browser.close();
}
