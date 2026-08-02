export function createContent({ params, host, readerStyleSource, fail }) {
  const shadow = host.attachShadow({ mode: "closed" });
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
  let silentFailure = false;

  function reject(code) {
    if (silentFailure) throw new Error(code);
    fail(code);
  }

  function ensure(condition, code) {
    if (!condition) reject(code);
  }

  function configuredBookUrl() {
    const override = params.get("book");
    if (override) return new URL(override, location.href);
    const entry = params.get("entry");
    ensure(entry, "missing-book-url");
    return new URL(entry.replace(/^\/+/, ""), "https://atha-book.localhost/");
  }

  function localBookUrl(value, base = bookUrl) {
    let url;
    try {
      url = new URL(value, base);
    } catch {
      reject("external-resource");
    }
    ensure(url.origin === bookOrigin && !url.username && !url.password, "external-resource");
    ensure(!url.search, "external-resource");
    return url.href;
  }

  function validateCss(css) {
    ensure(!/@import|url\s*\(/i.test(css), "css-subresource");
    ensure(!/:host(?:-context)?\b|::part\b|::slotted\b/i.test(css), "active-style");
  }

  function validateMarkup(documentNode) {
    ensure(!documentNode.querySelector("parsererror") && !documentNode.doctype, "invalid-xhtml");
    ensure(
      !documentNode.querySelector(
        "script, iframe, frame, object, embed, form, input, button, select, textarea, video, audio, source, track, base, meta[http-equiv], foreignObject",
      ),
      "active-content",
    );

    for (const element of documentNode.querySelectorAll("*")) {
      const name = element.localName.toLowerCase();
      for (const attribute of element.attributes) {
        const attributeName = attribute.name.toLowerCase();
        ensure(!attributeName.startsWith("on"), "event-handler");
        if (attributeName === "style") validateCss(attribute.value);
        if (["srcset", "poster", "action", "formaction", "ping"].includes(attributeName)) {
          reject("unsupported-resource-attribute");
        }
        if (["target", "download"].includes(attributeName)) reject("active-link");
        if (attributeName === "src" && name !== "img") reject("unsupported-resource-attribute");
        if (attributeName === "href" || attributeName.endsWith(":href")) {
          if (name === "a") {
            if (!attribute.value.startsWith("#")) {
              const target = new URL(attribute.value, bookUrl);
              ensure(
                target.origin === bookOrigin &&
                  target.pathname === bookUrl.pathname &&
                  !target.search &&
                  target.hash,
                "active-link",
              );
              element.setAttribute("href", target.hash);
            }
          } else if (name === "link") {
            ensure(element.getAttribute("rel") === "stylesheet", "active-content");
            element.setAttribute("href", localBookUrl(attribute.value));
          } else {
            ensure(attribute.value.startsWith("#"), "external-resource");
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
    ensure(!svg.querySelector("parsererror") && !svg.doctype, "invalid-svg");
    ensure(!svg.querySelector("script, foreignObject, iframe, object, embed"), "invalid-svg");
    for (const element of svg.querySelectorAll("*")) {
      for (const attribute of element.attributes) {
        const name = attribute.name.toLowerCase();
        ensure(!name.startsWith("on"), "svg-event-handler");
        if (name === "href" || name.endsWith(":href")) {
          ensure(attribute.value.startsWith("#"), "svg-external-resource");
        }
        if (name === "style") validateCss(attribute.value);
      }
      if (element.localName.toLowerCase() === "style") validateCss(element.textContent);
    }
    return svg;
  }

  async function validateSvg(url) {
    const response = await fetch(url);
    ensure(response.ok, "svg-load");
    parseSvg(await response.text());
  }

  function rejected(action) {
    silentFailure = true;
    try {
      action();
      return false;
    } catch {
      return true;
    } finally {
      silentFailure = false;
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
      ensure(
        rejected(() =>
          validateMarkup(new DOMParser().parseFromString(markup, "application/xhtml+xml")),
        ),
        "active-content",
      );
    }
    const samePage = new DOMParser().parseFromString(
      `<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='${bookUrl.pathname}#x'>x</a></body></html>`,
      "application/xhtml+xml",
    );
    validateMarkup(samePage);
    ensure(samePage.querySelector("a").getAttribute("href") === "#x", "active-link");
    for (const css of ["@import 'x.css';", "p{background:url(x)}", ":host{display:none}"]) {
      ensure(rejected(() => validateCss(css)), "active-style");
    }
    for (const svg of [
      "<svg xmlns='http://www.w3.org/2000/svg'><script/></svg>",
      "<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.com/x'/></svg>",
    ]) {
      ensure(rejected(() => parseSvg(svg)), "invalid-svg");
    }
  }

  async function initialize() {
    bookUrl = configuredBookUrl();
    bookOrigin = bookUrl.origin;
    validatorSelfCheck();
    const response = await fetch(readerStyleSource.href);
    ensure(response.ok, "reader-style-load");
    readerStyle.textContent = await response.text();
  }

  async function load() {
    const response = await fetch(bookUrl);
    ensure(response.ok, "book-load");
    cachedXhtml = await response.text();
    const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
    validateMarkup(source);
    const inlineCss = [...source.querySelectorAll("head > style")].map(
      (style) => style.textContent,
    );
    const stylesheetUrls = [...source.querySelectorAll("link[rel='stylesheet'][href]")].map(
      (link) => localBookUrl(link.getAttribute("href")),
    );
    ensure(stylesheetUrls.length > 0, "missing-stylesheet");
    const stylesheets = await Promise.all(
      stylesheetUrls.map(async (url) => {
        const styleResponse = await fetch(url);
        ensure(styleResponse.ok, "stylesheet-load");
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

  async function renderCached() {
    const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
    validateMarkup(source);
    validateCss(cachedCss);
    bookStyle.textContent = cachedCss;
    const imported = document.importNode(source.body, true);
    book.replaceChildren(...imported.childNodes);
    await Promise.all(
      [...book.querySelectorAll("img")].map(async (image) => {
        try {
          await image.decode();
        } catch {
          reject("image-load");
        }
      }),
    );
    await document.fonts.ready;
  }

  return Object.freeze({ book, initialize, load, renderCached });
}
