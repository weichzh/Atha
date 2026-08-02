export function createContent({ host, readerStyleSource, fail }) {
  const shadow = host.attachShadow({ mode: "closed" });
  const bookStyle = document.createElement("style");
  const readerStyle = document.createElement("style");
  const userStyle = document.createElement("style");
  const book = document.createElement("article");
  book.className = "book";
  shadow.append(bookStyle, readerStyle, userStyle, book);

  let bookUrl;
  let bookOrigin;
  let declaredResources;
  let cachedXhtml;
  let cachedCss;
  let sourceStyles = true;
  let userStylesEnabled = true;
  let userStylesheet = "";
  let inlineStyles = [];
  const validatedCss = new Map();
  let silentFailure = false;
  let selfChecked = false;

  function reject(code) {
    if (silentFailure) throw new Error(code);
    fail(code);
  }

  function ensure(condition, code) {
    if (!condition) reject(code);
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
    if (declaredResources) ensure(declaredResources.has(url.href), "undeclared-resource");
    return url.href;
  }

  function describeLink(value) {
    ensure(typeof value === "string" && value.length > 0 && value.length <= 2048, "active-link");
    let target;
    try {
      target = new URL(value, bookUrl);
    } catch {
      reject("active-link");
    }
    ensure(!target.username && !target.password, "active-link");
    ensure(["http:", "https:"].includes(target.protocol), "active-link");
    if (target.origin === bookOrigin) {
      ensure(!target.search && target.pathname.toLowerCase().endsWith(".xhtml"), "active-link");
      return Object.freeze({
        kind: "internal",
        href: target.href,
        sameSection: target.pathname === bookUrl.pathname,
      });
    }
    return Object.freeze({ kind: "external", href: target.href, sameSection: false });
  }

  function validateCss(css, declarations = false) {
    const cacheKey = `${declarations ? "declaration" : "stylesheet"}:${css}`;
    if (validatedCss.has(cacheKey)) return validatedCss.get(cacheKey);
    ensure(!/@import|url\s*\(/i.test(css), "css-subresource");
    ensure(!/:host(?:-context)?\b|::part\b|::slotted\b/i.test(css), "active-style");
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(declarations ? `.atha-inline { ${css} }` : css);
    const inspect = (rules) => {
      for (const rule of rules) {
        ensure(!/url\s*\(|image-set\s*\(/i.test(rule.cssText), "css-subresource");
        ensure(
          !/:host(?:-context)?\b|::part\b|::slotted\b/i.test(rule.cssText),
          "active-style",
        );
        if (rule.cssRules) inspect(rule.cssRules);
      }
    };
    inspect(sheet.cssRules);
    if (validatedCss.size >= 256) validatedCss.delete(validatedCss.keys().next().value);
    validatedCss.set(cacheKey, sheet);
    return sheet;
  }

  function validateUserStylesheet(css) {
    ensure(typeof css === "string" && css.length <= 32768, "invalid-user-style");
    const previous = silentFailure;
    silentFailure = true;
    try {
      const sheet = validateCss(css);
      ensure(!css.trim() || sheet.cssRules.length > 0, "invalid-user-style");
    } finally {
      silentFailure = previous;
    }
  }

  function setStyles(value) {
    ensure(
      value &&
        typeof value === "object" &&
        typeof value.sourceStyles === "boolean" &&
        typeof value.userStylesEnabled === "boolean",
      "invalid-user-style",
    );
    validateUserStylesheet(value.userStylesheet);
    sourceStyles = value.sourceStyles;
    userStylesEnabled = value.userStylesEnabled;
    userStylesheet = value.userStylesheet;
    bookStyle.textContent = sourceStyles ? cachedCss || "" : "";
    userStyle.textContent = userStylesEnabled ? userStylesheet : "";
    for (const [element, style] of inlineStyles) {
      if (sourceStyles) element.setAttribute("style", style);
      else element.removeAttribute("style");
    }
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
        if (attributeName === "style") validateCss(attribute.value, true);
        if (["srcset", "poster", "action", "formaction", "ping"].includes(attributeName)) {
          reject("unsupported-resource-attribute");
        }
        if (["target", "download"].includes(attributeName)) reject("active-link");
        if (attributeName === "src" && name !== "img") reject("unsupported-resource-attribute");
        if (attributeName === "href" || attributeName.endsWith(":href")) {
          if (name === "a") {
            element.setAttribute("href", describeLink(attribute.value).href);
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

  function detachSourceStyles(documentNode) {
    return [...documentNode.querySelectorAll("style, link[rel='stylesheet'][href]")].map(
      (element) => {
        const source =
          element.localName.toLowerCase() === "style"
            ? { css: element.textContent }
            : { url: localBookUrl(element.getAttribute("href")) };
        element.remove();
        return source;
      },
    );
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
        if (name === "style") validateCss(attribute.value, true);
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
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='javascript:alert(1)'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='mailto:x@example.com'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='data:text/plain,x'>x</a></body></html>",
      `<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='blob:${bookOrigin}/x.xhtml'>x</a></body></html>`,
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='#x' target='_blank'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='#x' download=''>x</a></body></html>",
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
    ensure(describeLink(samePage.querySelector("a").getAttribute("href")).kind === "internal", "active-link");
    const external = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='https://example.com/reference'>x</a></body></html>",
      "application/xhtml+xml",
    );
    validateMarkup(external);
    ensure(describeLink(external.querySelector("a").getAttribute("href")).kind === "external", "active-link");
    for (const css of [
      "@import 'x.css';",
      "p{background:url(x)}",
      "p{background:u\\72l(x)}",
      ":host{display:none}",
      ":\\68ost{display:none}",
      "@scope (:\\68ost) { :scope { display: none; } }",
    ]) {
      ensure(rejected(() => validateCss(css)), "active-style");
    }
    const embeddedStyle = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><style>p{color:red}</style><p>x</p></body></html>",
      "application/xhtml+xml",
    );
    validateMarkup(embeddedStyle);
    const detached = detachSourceStyles(embeddedStyle);
    ensure(
      detached[0].css.includes("color:red") && !embeddedStyle.querySelector("style"),
      "active-style",
    );
    for (const svg of [
      "<svg xmlns='http://www.w3.org/2000/svg'><script/></svg>",
      "<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.com/x'/></svg>",
    ]) {
      ensure(rejected(() => parseSvg(svg)), "invalid-svg");
    }
  }

  async function initialize() {
    const response = await fetch(readerStyleSource.href);
    ensure(response.ok, "reader-style-load");
    readerStyle.textContent = await response.text();
  }

  async function loadSection(url, resources, loadError) {
    bookUrl = url;
    bookOrigin = url.origin;
    declaredResources = resources;
    if (!selfChecked) {
      validatorSelfCheck();
      selfChecked = true;
    }
    const response = await fetch(bookUrl);
    ensure(response.ok, loadError);
    cachedXhtml = await response.text();
    const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
    validateMarkup(source);
    const styleSources = detachSourceStyles(source);
    ensure(styleSources.some(({ url }) => url), "missing-stylesheet");
    const stylesheets = await Promise.all(
      styleSources.map(async ({ css, url }) => {
        if (css !== undefined) return css;
        const styleResponse = await fetch(url);
        ensure(styleResponse.ok, "stylesheet-load");
        return styleResponse.text();
      }),
    );
    cachedCss = stylesheets.join("\n");
    validateCss(cachedCss);

    const svgUrls = [
      ...new Set([...source.querySelectorAll("img[src$='.svg']")].map((image) => image.src)),
    ];
    await Promise.all(svgUrls.map(validateSvg));
  }

  async function renderCached() {
    const source = new DOMParser().parseFromString(cachedXhtml, "application/xhtml+xml");
    validateMarkup(source);
    detachSourceStyles(source);
    validateCss(cachedCss);
    bookStyle.textContent = sourceStyles ? cachedCss : "";
    userStyle.textContent = userStylesEnabled ? userStylesheet : "";
    const imported = document.importNode(source.body, true);
    book.replaceChildren(...imported.childNodes);
    inlineStyles = [...book.querySelectorAll("[style]")].map((element) => [
      element,
      element.getAttribute("style"),
    ]);
    setStyles({ sourceStyles, userStylesEnabled, userStylesheet });
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

  function close() {
    book.replaceChildren();
    bookStyle.textContent = "";
    userStyle.textContent = "";
    bookUrl = undefined;
    bookOrigin = undefined;
    declaredResources = undefined;
    cachedXhtml = undefined;
    cachedCss = undefined;
    inlineStyles = [];
    validatedCss.clear();
  }

  function styleSnapshot() {
    return Object.freeze({
      sourceStyles,
      userStylesEnabled,
      bookStyleApplied: bookStyle.textContent.length > 0,
      userStyleApplied: userStyle.textContent.length > 0,
    });
  }

  return Object.freeze({
    book,
    close,
    initialize,
    loadSection,
    renderCached,
    describeLink,
    setStyles,
    styleSnapshot,
  });
}
