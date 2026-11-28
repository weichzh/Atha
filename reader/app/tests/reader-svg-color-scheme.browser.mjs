import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const base = "https://svg-fixture.test";
const content = await readFile(new URL("../../web/content.mjs", import.meta.url));
const svg = (css) => `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30">
<style>${css}</style><rect width="40" height="30"/></svg>`;
const responsive = svg("rect{fill:white}@media (prefers-color-scheme: dark){rect{fill:black}}");
const chapter = (images) => `<html xmlns="http://www.w3.org/1999/xhtml"><body>
<style>img{background:white}@media (prefers-color-scheme: dark){img{background:black}}</style>
<p id="equation">Equation <img class="math-inline" src="adaptive.svg" width="40" height="30"/></p>
${images}</body></html>`;
const files = {
  "/": ["text/html", "<main style='width:500px;height:400px'><div id='host'></div></main>"],
  "/content.mjs": ["text/javascript", content],
  "/theme.css": ["text/css", ".book{column-gap:0;color-scheme:light}"],
  "/layout.css": ["text/css", ""],
  "/one.xhtml": ["application/xhtml+xml", chapter(
    '<img src="fixed.svg"/><img src="compound.svg"/><img src="media.svg"/>' +
    '<img src="raster.png"/><img src="unsafe.svg" alt="blocked"/>',
  )],
  "/two.xhtml": ["application/xhtml+xml", chapter("")],
  "/adaptive.svg": ["image/svg+xml", responsive],
  "/fixed.svg": ["image/svg+xml", svg("rect{fill:#182226}")],
  "/compound.svg": ["image/svg+xml", svg(
    "rect{fill:white}@media (prefers-color-scheme:light){rect{fill:white}}" +
    "@media (min-width:1px) and (prefers-color-scheme:dark){rect{fill:black}}",
  )],
  "/media.svg": ["image/svg+xml", svg(
    "@media (prefers-color-scheme:dark){rect{fill:white}}",
  ).replace("<style>", '<style media="(prefers-color-scheme:light)">')],
  "/raster.png": ["image/png", Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aH1sAAAAASUVORK5CYII=", "base64",
  )],
  "/unsafe.svg": ["image/svg+xml", responsive.replace("</svg>", "<script/></svg>")],
};

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ colorScheme: "dark" });
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    const file = url.origin === base && files[url.pathname];
    return file ? route.fulfill({ contentType: file[0], body: file[1] }) : route.abort();
  });
  await page.goto(base);

  const exercise = (enabled, scheme) => page.evaluate(async ({ projectSvgColorScheme, scheme }) => {
    const { createContent } = await import("/content.mjs");
    const root = document.querySelector("main");
    const host = document.createElement("div");
    root.append(host);
    const view = createContent({
      host, reader: root, projectSvgColorScheme,
      onLateLayout() {},
      themeStyleSource: { href: "/theme.css" }, readerStyleSource: { href: "/layout.css" },
    });
    await view.initialize();
    const style = (colorScheme) => view.setStyles({
      colorScheme, sourceStyles: true, userStylesEnabled: false, userStylesheet: "",
    });
    const open = async (path) => {
      const section = await view.prepareSection(new URL(path, location.href), null, "section-load");
      view.activateSection(section);
      await view.renderCached();
    };

    style(scheme);
    await open("/one.xhtml");
    const formula = view.book.querySelector(".math-inline");
    const preview = document.createElement("img");
    root.append(preview);
    const previewOk = await view.materializePreviewImage(formula, preview);
    await view.loadVisible();
    await formula.decode();
    const range = document.createRange();
    range.selectNodeContents(view.book.querySelector("#equation"));
    const snap = await view.captureRange(range, { theme: scheme, brightness: 100, fontSize: 19 });
    const explicit = {
      canonical: formula.src,
      variant: formula.currentSrc.startsWith("blob:"),
      preview: previewOk && preview.currentSrc === formula.currentSrc,
      background: getComputedStyle(formula).backgroundColor,
      retained: [...view.book.querySelectorAll("img:not(.math-inline)")].map((img) => img.getAttribute("srcset")),
      rejected: Boolean(view.book.querySelector(".atha-image-error[data-label=blocked]")),
      snapshot: {
        path: snap.resources[0].path,
        hasVariant: new RegExp(scheme === "dark" ? "@media all" : "@media not all")
          .test(new TextDecoder().decode(Uint8Array.from(snap.resources[0].bytes))),
        noBlob: !/srcset|blob:/.test(snap.fragmentHtml),
      },
    };
    const previous = formula.currentSrc;
    const mixedCaptures = [];
    const original = await (await fetch(formula.src)).text();
    for (const htmlFirst of [true, false]) {
      const mixed = document.createElement("p");
      const ns = "http://www.w3.org/2000/svg";
      const svg = document.createElementNS(ns, "svg");
      const image = document.createElementNS(ns, "image");
      image.setAttribute("href", formula.src);
      svg.append(image);
      const html = formula.cloneNode();
      mixed.append(...(htmlFirst ? [html, svg] : [svg, html]));
      view.book.append(mixed);
      range.selectNodeContents(mixed);
      const mixedSnap = await view.captureRange(range, { theme: scheme, brightness: 100, fontSize: 19 });
      const fragment = new DOMParser().parseFromString(mixedSnap.fragmentHtml, "text/html");
      const imgPath = fragment.querySelector("img").getAttribute("src");
      const svgPath = fragment.querySelector("svg image").getAttribute("href");
      const textAt = (path) => new TextDecoder().decode(
        Uint8Array.from(mixedSnap.resources.find((item) => item.path === path).bytes),
      );
      mixedCaptures.push({
        distinct: imgPath !== svgPath,
        keptOriginal: textAt(svgPath) === original,
        savedVariant: textAt(imgPath) !== original,
      });
      mixed.remove();
    }
    style(null);
    await formula.decode();
    const system = { original: formula.currentSrc === formula.src, background: getComputedStyle(formula).backgroundColor };
    style(scheme);
    await open("/two.xhtml");
    const next = view.book.querySelector(".math-inline");
    await next.decode();
    const nextSection = {
      variant: next.currentSrc.startsWith("blob:"),
      fresh: next.currentSrc !== previous,
    };
    view.close();
    preview.remove();
    host.remove();
    return { explicit, system, nextSection, mixedCaptures };
  }, { projectSvgColorScheme: enabled, scheme });

  const rgb = { light: "rgb(255, 255, 255)", dark: "rgb(0, 0, 0)" };
  for (const [system, chosen] of [["dark", "light"], ["light", "dark"]]) {
    await page.emulateMedia({ colorScheme: system });
    for (const enabled of [true, false]) {
      const actual = await exercise(enabled, chosen);
      assert.deepEqual(actual, {
        explicit: {
          canonical: `${base}/adaptive.svg`,
          variant: enabled,
          preview: true,
          background: rgb[chosen],
          retained: [null, null, null, null],
          rejected: true,
          snapshot: { path: "adaptive.svg", hasVariant: enabled, noBlob: true },
        },
        system: { original: true, background: rgb[system] },
        nextSection: { variant: enabled, fresh: enabled },
        mixedCaptures: Array(2).fill({ distinct: enabled, keptOriginal: true, savedVariant: enabled }),
      });
    }
  }
  console.log("SVG scheme: compatibility path, native path, previews, capture and section changes verified.");
} finally {
  await browser.close();
}
