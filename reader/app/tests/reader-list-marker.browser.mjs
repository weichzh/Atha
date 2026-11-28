import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const base = "https://list-marker-fixture.test";
const file = (path) => readFile(new URL(path, import.meta.url), "utf8");
const [content, shell, layout, conversations] = await Promise.all([
  file("../../web/content.mjs"), file("../src/styles/app.css"), file("../src/styles/book-reader.css"),
  file("../../web/conversations.mjs"),
]);
const chapter = (nativeScroll) => `<html xmlns="http://www.w3.org/1999/xhtml"><body>
<style>
  @counter-style paired-decimal { system: extends decimal; prefix: "("; suffix: ") "; }
  @counter-style square-decimal { system: extends decimal; prefix: "["; suffix: "] "; }
  @counter-style conditional-decimal { system: extends decimal; prefix: "("; suffix: ") "; }
  @media (min-width: 200px) {
    @counter-style conditional-decimal { system: extends decimal; prefix: "["; suffix: "] "; }
  }
  .sample li { list-style-type: paired-decimal; }
  .other li { list-style-type: square-decimal; }
  .conditional li { list-style-type: conditional-decimal; }
  .sample li::marker { color: #d000d0; }
  .next-column { break-before: column; }
  ol.authored { padding-inline-start: 3em; list-style-position: inside; }
  :where(ol.low) { padding-inline-start: 1em; }
  @layer source { .layered { padding-inline-start: 1em; } }
  .low, .layered { list-style-position: inside; }
</style>
${nativeScroll ? '<p>First column.</p>' : ""}
<section class="${nativeScroll ? "next-column" : ""}">
  <ol class="sample" start="98"><li>First item</li><li>Second item</li><li>Third item</li></ol>
  <ol class="sample" dir="rtl" start="98"><li>Right-to-left item</li></ol>
  <ol class="authored" type="a"><li>Author indentation</li></ol>
  <ol class="other"><li>Another counter</li></ol>
  <ol class="conditional"><li>A conditional counter</li></ol>
  <ol class="low"><li>Low-specificity source indentation</li></ol>
  <ol class="layered"><li>Layered source indentation</li></ol>
</section>
</body></html>`;

const browser = await chromium.launch();
try {
  for (const [dpr, nativeScroll] of [[1, false], [3, false], [3, true]]) {
    const page = await browser.newPage({ viewport: { width: 360, height: 420 }, deviceScaleFactor: dpr });
    await page.route("**/*", async (route) => {
      const { pathname, origin } = new URL(route.request().url());
      const files = {
        "/": ["text/html", '<link rel="stylesheet" href="/shell.css"/><main class="reader"><div class="page"><div class="reader-content-host"></div></div></main>'],
        "/shell.css": ["text/css", shell],
        "/layout.css": ["text/css", layout],
        "/theme.css": ["text/css", ""],
        "/content.mjs": ["text/javascript", content],
        "/conversations.mjs": ["text/javascript", conversations],
        "/chapter.xhtml": ["application/xhtml+xml", chapter(nativeScroll)],
      };
      const resource = origin === base && files[pathname];
      return resource ? route.fulfill({ contentType: resource[0], body: resource[1] }) : route.abort();
    });
    await page.goto(base);
    const sizes = await page.evaluate(async ({ dpr, nativeScroll }) => {
      const { createContent, projectShadowCounterCss } = await import("/content.mjs");
      const reader = document.querySelector(".reader");
      const frame = reader.querySelector(".page");
      for (const [name, value] of Object.entries({
        "--reader-dpr": dpr, "--reader-width": `${360 * dpr}px`,
        "--reader-height": `${420 * dpr}px`, "--reader-col-w": `${296 * dpr}px`,
        "--reader-pad-x": `${32 * dpr}px`, "--reader-pad-y": `${24 * dpr}px`,
        "--reader-column-gap": `${48 * dpr}px`, "--page-scale": 1 / dpr,
      })) reader.style.setProperty(name, value);
      const view = createContent({
        reader, host: reader.querySelector(".reader-content-host"), onLateLayout() {},
        projectShadowCounters: dpr === 1,
        themeStyleSource: { href: "/theme.css" }, readerStyleSource: { href: "/layout.css" },
      });
      await view.initialize();
      view.book.style.font = `${19 * dpr}px/1.5 serif`;
      view.activateSection(await view.prepareSection(new URL("/chapter.xhtml", location.href), null, "section-load"));
      await view.renderCached();
      if (nativeScroll) {
        reader.setAttribute("data-native-page-scroll", "");
        frame.scrollLeft = 344 * dpr;
      }
      const authored = view.book.querySelector(".authored");
      const padding = (el) => parseFloat(getComputedStyle(el).paddingInlineStart) / dpr;
      const authoredStyle = getComputedStyle(authored.firstElementChild);
      const counterType = (selector) => getComputedStyle(view.book.querySelector(selector)).listStyleType;
      const counters = [".sample li", ".other li", ".conditional li"].map(counterType);
      const authorIndents = [".low", ".layered"].map((selector) => padding(view.book.querySelector(selector)));
      const range = document.createRange();
      range.selectNodeContents(view.book.querySelector("section"));
      const capture = await view.captureRange(range, {
        theme: "light", brightness: 100, fontSize: 19, fontFamily: "book", density: "standard",
      });
      const styles = (sourceStyles, userStylesheet = "") => view.setStyles({
        sourceStyles, userStylesEnabled: Boolean(userStylesheet), userStylesheet,
      });
      styles(false);
      const unstyledCounter = counterType(".sample li");
      styles(true, '@counter-style paired-decimal { system: extends decimal; prefix: "["; suffix: "] "; }');
      const counterWithUserDefinition = counterType(".sample li");
      styles(true);
      const { renderSnapshotElement } = await import("/conversations.mjs");
      const replay = async (project) => {
        const host = await renderSnapshotElement({ snapshot: capture }, {}, undefined,
          project ? projectShadowCounterCss : undefined);
        host.style.cssText = "position:absolute;inset:0 auto auto:-1000px;width:280px;font:19px serif";
        document.body.append(host);
        const root = host.shadowRoot;
        const item = root.querySelector(".sample li");
        item.style.listStylePosition = "inside";
        const span = document.createElement("span");
        span.textContent = item.textContent;
        item.replaceChildren(span);
        const measure = () => span.getBoundingClientRect().left - item.getBoundingClientRect().left;
        const offset = measure();
        item.style.listStyleType = "decimal";
        const decimal = measure();
        const layered = padding(root.querySelector(".layered")) * dpr;
        host.remove();
        return { decorated: offset > decimal + 1, layered };
      };
      const snapshot = [await replay(false), await replay(true)];
      window.showUnclippedMarkers = () => {
        if (nativeScroll) {
          view.book.style.transform = `translateX(-${frame.scrollLeft}px)`;
          frame.scrollLeft = 0;
        }
        frame.style.overflow = "visible";
      };
      return {
        start: view.book.querySelector(".sample").start,
        authoredIndent: padding(authored),
        authoredPosition: authoredStyle.listStylePosition,
        counters: [counters[0].startsWith("atha-list-") ? "projected" : counters[0], ...counters.slice(1)],
        unstyledCounter,
        authorIndents,
        counterWithUserDefinition,
        capturedAlias: /atha-list-\d/.test(capture.bookCss),
        snapshot,
      };
    }, { dpr, nativeScroll });
    const pixels = async () => {
      const png = await page.screenshot();
      return page.evaluate(async (base64) => {
        const picture = new Image();
        picture.src = `data:image/png;base64,${base64}`;
        await picture.decode();
        const canvas = document.createElement("canvas");
        canvas.width = picture.width;
        canvas.height = picture.height;
        const context = canvas.getContext("2d");
        context.drawImage(picture, 0, 0);
        const { data } = context.getImageData(0, 0, picture.width, picture.height);
        const halves = [0, 0];
        for (let i = 0; i < data.length; i += 4) {
          if (data[i] > 130 && data[i + 1] < 105 && data[i + 2] > 130) {
            halves[Number((i / 4) % picture.width >= picture.width / 2)] += 1;
          }
        }
        return halves;
      }, png.toString("base64"));
    };
    const clipped = await pixels();
    await page.evaluate(() => window.showUnclippedMarkers());
    const full = await pixels();
    assert.ok(full.every((count) => count >= 20), `fixture must paint left and right markers: ${full}`);
    for (const i of [0, 1]) {
      assert.ok(clipped[i] >= 0.98 * full[i],
        `DPR ${dpr}, native scroll ${nativeScroll}, edge ${i}: rendered ${clipped[i]} / ${full[i]} marker pixels`);
    }
    assert.deepEqual(sizes, {
      start: 98, authoredIndent: 57, authoredPosition: "inside",
      counters: [dpr === 1 ? "projected" : "paired-decimal", "square-decimal", "conditional-decimal"],
      unstyledCounter: "decimal", capturedAlias: false,
      authorIndents: [19, 19],
      counterWithUserDefinition: "paired-decimal",
      snapshot: Array(2).fill({ decorated: true, layered: 19 }),
    });
    await page.close();
  }
  console.log("Ordered list markers visible on both edges, including DPR 3 native paging; book CSS retained.");
} finally {
  await browser.close();
}
