const XHTML_DOCTYPES = new Set([
  "<!DOCTYPE html>",
  '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">',
  '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">',
]);
const IMAGE_SETTLE_TIMEOUT_MS = 50;
const MAX_INTRINSIC_IMAGE_SIDE = 8192;
const MAX_INTRINSIC_IMAGE_PIXELS = 20_000_000;
const MAX_NATIVE_IMAGE_HINT_RULES = 512;
const MAX_SNAPSHOT_BOOK_CSS_BYTES = 1_048_576;
const SECTION_CACHE_LIMIT = 3;
const SECTION_CACHE_TEXT_LIMIT = 8 * 1024 * 1024;
const NATIVE_IMAGE_BASE_STYLE = ":where(img[data-atha-native-size]){height:auto}";

export function parseSafeXhtml(source) {
  const declarations = [...source.matchAll(/<!DOCTYPE\b/gu)];
  if (declarations.length === 0) {
    return new DOMParser().parseFromString(source, "application/xhtml+xml");
  }
  if (declarations.length !== 1) return null;
  const start = declarations[0].index;
  const end = source.indexOf(">", start);
  if (end < 0) return null;
  const declaration = source.slice(start, end + 1);
  const normalized = declaration.replace(/\s+/gu, " ").replace(/\s+>/u, ">");
  if (declaration.includes("[") || !XHTML_DOCTYPES.has(normalized)) {
    return null;
  }
  return new DOMParser().parseFromString(
    `${source.slice(0, start)}${source.slice(end + 1)}`.replaceAll("&nbsp;", "&#160;"),
    "application/xhtml+xml",
  );
}

export function cloneSelectedRange(book, selection) {
  if (!selection || selection.rangeCount !== 1) return null;
  const range = selection.getRangeAt(0);
  if (
    range.collapsed ||
    (range.commonAncestorContainer !== book && !book.contains(range.commonAncestorContainer))
  ) {
    return null;
  }
  return range.cloneRange();
}

export function createContent({ host, reader, readerStyleSource, onLateLayout }) {
  const shadow = host.attachShadow({ mode: "closed" });
  const nativeImageStyle = document.createElement("style");
  const bookStyle = document.createElement("style");
  const readerStyle = document.createElement("style");
  const userStyle = document.createElement("style");
  const book = document.createElement("article");
  book.className = "book";
  nativeImageStyle.textContent = NATIVE_IMAGE_BASE_STYLE;
  shadow.append(nativeImageStyle, bookStyle, readerStyle, userStyle, book);

  let bookUrl;
  let declaredResources;
  let cachedBody;
  let cachedCss;
  let cachedCbzPage = false;
  let cachedMarkdownSection = false;
  let sourceStyles = true;
  let userStylesEnabled = true;
  let userStylesheet = "";
  let inlineStyles = [];
  const validatedCss = new Map();
  const validatedSvg = new Map();
  const readySvg = new Set();
  const preparedSections = new Map();
  const pendingSectionMarkup = new Map();
  const sectionCacheOrder = new Map();
  const pendingImages = new Map();
  const pendingImageGeometry = new Map();
  const lateImages = new Map();
  let selfChecked = false;
  let deferredImageCount = 0;
  let eagerSvgCount = 0;
  let lastVisibleLoadCount = 0;
  let lastVisibleLoad = Object.freeze({
    passes: 0,
    generationChanged: false,
    batches: Object.freeze([]),
  });
  let renderGeneration = 0;
  let lateLayoutTimer = 0;
  let lateLayoutAnchor = null;
  let lateLayoutDirty = false;
  let warmPromise;
  let warmDurationMs = null;
  let pendingImageLayoutSignature = "";
  let sectionGeneration = 0;

  function reject(code) {
    throw new Error(code);
  }

  function ensure(condition, code) {
    if (!condition) reject(code);
  }

  function localBookUrl(value, base = bookUrl, optional = false) {
    let url;
    try {
      url = new URL(value, base);
    } catch {
      reject("external-resource");
    }
    ensure(
      url.protocol === bookUrl.protocol &&
        url.host === bookUrl.host &&
        !url.username &&
        !url.password,
      "external-resource",
    );
    ensure(!url.search, "external-resource");
    if (declaredResources && !declaredResources.has(url.href)) {
      if (optional) return null;
      reject("undeclared-resource");
    }
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
    if (target.protocol === bookUrl.protocol && target.host === bookUrl.host) {
      ensure(!target.search, "active-link");
      return Object.freeze({
        kind: "internal",
        href: target.href,
        sameSection: target.pathname === bookUrl.pathname,
      });
    }
    ensure(["http:", "https:"].includes(target.protocol), "active-link");
    return Object.freeze({ kind: "external", href: target.href, sameSection: false });
  }

  function validateCss(css, declarations = false) {
    const cacheKey = `${declarations ? "declaration" : "stylesheet"}:${css}`;
    if (validatedCss.has(cacheKey)) return validatedCss.get(cacheKey);
    ensure(
      !/@import|(?:url|src|image|image-set)\s*\(/i.test(css) && !css.includes("\\"),
      "css-subresource",
    );
    ensure(!/:host(?:-context)?\b|::part\b|::slotted\b/i.test(css), "active-style");
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(declarations ? `.atha-inline { ${css} }` : css);
    const inspect = (rules) => {
      for (const rule of rules) {
        ensure(
          !/(?:url|src|image|image-set)\s*\(/i.test(rule.cssText) &&
            !rule.cssText.includes("\\"),
          "css-subresource",
        );
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
    ensure(typeof css === "string" && new TextEncoder().encode(css).length <= 65536, "invalid-user-style");
    const sheet = validateCss(css);
    const uncommented = css.replace(/\/\*[\s\S]*?\*\//gu, "").trim();
    ensure(!uncommented || sheet.cssRules.length > 0, "invalid-user-style");
  }

  function snapshotReaderCss() {
    ensure(readerStyle.sheet, "invalid-message-snapshot");
    return [...readerStyle.sheet.cssRules]
      .filter(
        (rule) =>
          !/@import|(?:url|src|image|image-set)\s*\(/i.test(rule.cssText) &&
          !rule.cssText.includes("\\"),
      )
      .map((rule) => rule.cssText)
      .join("\n");
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
    pendingImageGeometry.clear();
    pendingImageLayoutSignature = "";
    bookStyle.textContent = sourceStyles ? cachedCss || "" : "";
    userStyle.textContent = userStylesEnabled ? userStylesheet : "";
    for (const [element, style] of inlineStyles) {
      if (sourceStyles) element.setAttribute("style", style);
      else element.removeAttribute("style");
    }
    applyPendingFormulaAspectRatios();
  }

  function setImageInteraction(image) {
    if (image.closest("a[href]")) {
      image.removeAttribute("role");
      image.removeAttribute("tabindex");
      return;
    }
    const formula = image.matches(".math-inline, .math-display");
    const alternative = image.getAttribute("alt")?.trim().slice(0, 160);
    image.removeAttribute("aria-hidden");
    image.removeAttribute("aria-disabled");
    image.draggable = false;
    image.setAttribute("role", "button");
    image.setAttribute("tabindex", "0");
    image.setAttribute(
      "aria-label",
      alternative
        ? `${formula ? "查看公式" : "查看图片"}：${alternative}`
        : formula
          ? "查看公式"
          : "查看图片",
    );
  }

  function setStructuredInteraction(element) {
    if (element.closest("a[href]")) {
      element.removeAttribute("tabindex");
      return;
    }
    const table = element.localName.toLowerCase() === "table";
    const caption = table ? element.querySelector(":scope > caption")?.textContent.trim() : "";
    element.removeAttribute("aria-hidden");
    element.removeAttribute("aria-disabled");
    element.setAttribute("tabindex", "0");
    element.setAttribute("aria-label", caption ? `查看表格：${caption.slice(0, 160)}` : table ? "查看表格" : "查看代码");
  }

  function wrapStructuredOverflow(element) {
    const overflow = element.ownerDocument.createElement("div");
    overflow.className = `atha-structured-overflow atha-${element.localName.toLowerCase()}-frame`;
    element.replaceWith(overflow);
    overflow.append(element);
  }

  function isControlledCbzSection(documentNode, url = bookUrl) {
    const page = documentNode?.body?.firstElementChild;
    return (
      /^\/\.atha-cbz\/page-\d{4}\.xhtml$/u.test(url?.pathname || "") &&
      documentNode.body.children.length === 1 &&
      page?.matches("main.atha-cbz-page") &&
      page.children.length === 1 &&
      page.firstElementChild?.matches("img[src]") &&
      page.textContent.trim() === ""
    );
  }

  function isControlledMarkdownSection(documentNode, url = bookUrl) {
    return (
      /^\/\.atha-text\/section-\d{4}\.xhtml$/u.test(url?.pathname || "") &&
      documentNode?.body?.classList.contains("atha-text") &&
      documentNode.body.classList.contains("atha-markdown")
    );
  }

  function replaceFailedCbzImage(image) {
    const page = image.parentElement;
    if (
      !book.classList.contains("atha-cbz-section") ||
      !page?.matches("main.atha-cbz-page") ||
      page.parentElement !== book
    ) {
      return false;
    }
    const placeholder = document.createElement("div");
    placeholder.className = "atha-cbz-page-error";
    placeholder.setAttribute("role", "img");
    placeholder.setAttribute("aria-label", "图片无法显示");
    image.replaceWith(placeholder);
    return true;
  }

  function replaceFailedImage(image) {
    if (replaceFailedCbzImage(image)) return;
    const fallbackSize =
      nativeImageSize(image) ||
      intrinsicImageSize(image) ||
      boundedImageSize(image, "width", "height");
    const width = image.offsetWidth || fallbackSize?.width || 0;
    const height = image.offsetHeight || fallbackSize?.height || 0;
    const layout = image.isConnected ? getComputedStyle(image) : null;
    const alternative = image.getAttribute("alt");
    const label = (alternative === null ? "图片无法显示" : alternative.trim()).slice(0, 160);
    const placeholder = image.ownerDocument.createElement("span");
    placeholder.className = "atha-image-error";
    placeholder.setAttribute("role", "img");
    if (label) {
      placeholder.setAttribute("aria-label", label);
      placeholder.dataset.label = label;
    } else {
      placeholder.setAttribute("aria-hidden", "true");
    }
    if (width > 0 && height > 0) {
      placeholder.style.width = `${width}px`;
      placeholder.style.height = `${height}px`;
      placeholder.style.boxSizing = "border-box";
    }
    if (layout) {
      if (["block", "none"].includes(layout.display)) placeholder.style.display = layout.display;
      for (const property of [
        "margin-top",
        "margin-right",
        "margin-bottom",
        "margin-left",
        "float",
        "clear",
        "vertical-align",
        "break-before",
        "break-after",
        "break-inside",
        "position",
        "top",
        "right",
        "bottom",
        "left",
        "transform",
        "transform-origin",
        "z-index",
        "visibility",
        "opacity",
      ]) {
        placeholder.style.setProperty(property, layout.getPropertyValue(property));
      }
    }
    image.replaceWith(placeholder);
  }

  function boundedImageSize(image, widthAttribute, heightAttribute) {
    const width = Number(image.getAttribute(widthAttribute));
    const height = Number(image.getAttribute(heightAttribute));
    if (
      !Number.isInteger(width) ||
      !Number.isInteger(height) ||
      width <= 0 ||
      height <= 0 ||
      width > MAX_INTRINSIC_IMAGE_SIDE ||
      height > MAX_INTRINSIC_IMAGE_SIDE ||
      width * height > MAX_INTRINSIC_IMAGE_PIXELS
    ) {
      return null;
    }
    return Object.freeze({ width, height });
  }

  function intrinsicImageSize(image) {
    return boundedImageSize(image, "data-atha-intrinsic-width", "data-atha-intrinsic-height");
  }

  function nativeImageSize(image) {
    return image.hasAttribute("data-atha-native-size")
      ? boundedImageSize(image, "width", "height")
      : null;
  }

  function nativeImageHintCss(root, maxBytes = Number.POSITIVE_INFINITY) {
    const sizes = new Map();
    for (const image of root.querySelectorAll("img[data-atha-native-size][width][height]")) {
      const size = nativeImageSize(image);
      if (!size) continue;
      sizes.set(`${size.width}x${size.height}`, size);
      // ponytail: uncommon extra size pairs keep stable width-driven HTML hints; raise only if a real book needs them.
      if (sizes.size >= MAX_NATIVE_IMAGE_HINT_RULES) break;
    }
    const rules = [];
    let bytes = 0;
    for (const rule of [
      NATIVE_IMAGE_BASE_STYLE,
      ...[...sizes.values()].map(
        ({ width, height }) =>
          `:where(img[data-atha-native-size][width="${width}"][height="${height}"]){contain:size;contain-intrinsic-size:${width}px ${height}px;width:auto;height:auto}`,
      ),
    ]) {
      const nextBytes = bytes + (rules.length ? 1 : 0) + rule.length;
      if (nextBytes > maxBytes) break;
      rules.push(rule);
      bytes = nextBytes;
    }
    return rules.join("\n");
  }

  function snapshotBookCss(root) {
    const sourceRules = [...(bookStyle.sheet?.cssRules || [])];
    let namespaceCount = 0;
    while (sourceRules[namespaceCount]?.constructor?.name === "CSSNamespaceRule") {
      namespaceCount += 1;
    }
    const namespaces = sourceRules.slice(0, namespaceCount).map((rule) => rule.cssText);
    const source = namespaceCount
      ? sourceRules.slice(namespaceCount).map((rule) => rule.cssText)
      : [bookStyle.textContent];
    const sourceCss = [...namespaces, ...source].filter(Boolean).join("\n");
    const available = Math.max(
      0,
      MAX_SNAPSHOT_BOOK_CSS_BYTES - new TextEncoder().encode(sourceCss).length - 1,
    );
    const hints = nativeImageHintCss(root, available);
    return [...namespaces, hints, ...source].filter(Boolean).join("\n");
  }

  function applyIntrinsicImageHint(image) {
    const size = intrinsicImageSize(image);
    if (!size) return false;
    image.style.setProperty("--atha-intrinsic-width", `${size.width}px`);
    image.style.setProperty("--atha-intrinsic-height", `${size.height}px`);
    image.style.setProperty("aspect-ratio", `${size.width} / ${size.height}`);
    image.style.setProperty("height", "auto", "important");
    return true;
  }

  function stableImageSize(image) {
    const intrinsic = intrinsicImageSize(image);
    if (intrinsic) return intrinsic;
    const width = Number(image.getAttribute("width"));
    const height = Number(image.getAttribute("height"));
    return Number.isFinite(width) && width > 0 && Number.isFinite(height) && height > 0
      ? Object.freeze({ width, height })
      : null;
  }

  function hasStableImageBox(image) {
    return Boolean(stableImageSize(image));
  }

  function applyPendingFormulaAspectRatios() {
    for (const image of book.querySelectorAll(
      "img.atha-resource-pending[data-atha-resource].math-inline, img.atha-resource-pending[data-atha-resource].math-display",
    )) {
      const size = stableImageSize(image);
      if (size) image.style.setProperty("aspect-ratio", `${size.width} / ${size.height}`);
    }
  }

  function deferredFormula(image) {
    const source = image.getAttribute("src");
    return (
      image.matches(".math-inline, .math-display") &&
      source?.toLowerCase().endsWith(".svg") &&
      hasStableImageBox(image)
    );
  }

  function isSvgImage(image) {
    return new URL(image.src).pathname.toLowerCase().endsWith(".svg");
  }

  function validateMarkup(documentNode) {
    ensure(documentNode && !documentNode.querySelector("parsererror") && !documentNode.doctype, "invalid-xhtml");
    documentNode.querySelectorAll("meta[http-equiv]").forEach((element) => element.remove());
    ensure(
      !documentNode.querySelector(
        "script, iframe, frame, object, embed, form, input, button, select, textarea, video, audio, source, track, base, meta[http-equiv], foreignObject",
      ),
      "active-content",
    );

    for (const element of documentNode.querySelectorAll("*")) {
      element.classList.remove("atha-resource-pending");
      const name = element.localName.toLowerCase();
      for (const attribute of [...element.attributes]) {
        const attributeName = attribute.name.toLowerCase();
        if (attributeName === "data-atha-resource") {
          element.removeAttribute(attribute.name);
          continue;
        }
        ensure(!attributeName.startsWith("on"), "event-handler");
        if (attributeName === "style" && rejected(() => validateCss(attribute.value, true))) {
          element.removeAttribute(attribute.name);
        }
        if (["srcset", "poster", "action", "formaction", "ping"].includes(attributeName)) {
          reject("unsupported-resource-attribute");
        }
        if (["target", "download"].includes(attributeName)) reject("active-link");
        if (attributeName === "src" && name !== "img") reject("unsupported-resource-attribute");
        if (attributeName === "href" || attributeName.endsWith(":href")) {
          if (name === "a") {
            element.setAttribute("href", describeLink(attribute.value).href);
          } else if (name === "link") {
            ensure(element.getAttribute("rel")?.trim().toLowerCase() === "stylesheet", "active-content");
            element.setAttribute("rel", "stylesheet");
            const href = localBookUrl(attribute.value, bookUrl, true);
            if (href) element.setAttribute("href", href);
            else element.remove();
          } else if (name === "image") {
            const href = localBookUrl(attribute.value, bookUrl, true);
            if (href) attribute.value = href;
            else element.remove();
          } else {
            ensure(attribute.value.startsWith("#"), "external-resource");
          }
        }
      }
      if (name === "style" && rejected(() => validateCss(element.textContent))) element.remove();
    }

    for (const image of documentNode.querySelectorAll("img[src]")) {
      const src = localBookUrl(image.getAttribute("src"), bookUrl, true);
      if (!src) {
        replaceFailedImage(image);
        continue;
      }
      image.setAttribute("src", src);
      setImageInteraction(image);
    }
    for (const element of documentNode.querySelectorAll("table, pre")) {
      setStructuredInteraction(element);
      wrapStructuredOverflow(element);
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

  async function validateSvg(url, generation) {
    const response = await fetch(url);
    ensure(response.ok, "svg-load");
    const source = await response.text();
    ensureSectionCurrent(generation);
    parseSvg(source);
  }

  function validateSvgOnce(url, generation = sectionGeneration) {
    if (!validatedSvg.has(url)) validatedSvg.set(url, validateSvg(url, generation));
    return validatedSvg.get(url);
  }

  function loadBounds(includeNextPage) {
    const viewport = reader.getBoundingClientRect();
    if (reader.dataset.readingMode === "scroll") {
      return {
        top: viewport.top,
        right: viewport.right,
        bottom: viewport.bottom + (includeNextPage ? viewport.height : 0),
        left: viewport.left,
      };
    }
    const style = getComputedStyle(book);
    const scale = viewport.width / reader.clientWidth;
    const pageStep = (parseFloat(style.width) + parseFloat(style.columnGap)) * scale;
    return {
      top: viewport.top,
      right: viewport.right + (includeNextPage ? pageStep : 0),
      bottom: viewport.bottom,
      left: viewport.left - (book.hasAttribute("data-swipe-settling") ? pageStep : 0),
    };
  }

  function pendingWithin(bounds) {
    const bookRect = book.getBoundingClientRect();
    const readerRect = reader.getBoundingClientRect();
    const measuredScale = reader.clientWidth > 0 ? readerRect.width / reader.clientWidth : 1;
    const scale = Number.isFinite(measuredScale) && measuredScale > 0 ? measuredScale : 1;
    const signature = [
      book.scrollWidth,
      book.scrollHeight,
      book.clientWidth,
      book.clientHeight,
      reader.clientWidth,
      reader.clientHeight,
      scale,
    ].join(":");
    if (signature !== pendingImageLayoutSignature) {
      pendingImageGeometry.clear();
      pendingImageLayoutSignature = signature;
    }
    const localBounds = {
      top: (bounds.top - bookRect.top) / scale,
      right: (bounds.right - bookRect.left) / scale,
      bottom: (bounds.bottom - bookRect.top) / scale,
      left: (bounds.left - bookRect.left) / scale,
    };
    return [...pendingImages.keys()].filter((image) => {
      const stableBox = hasStableImageBox(image);
      let rect = stableBox ? pendingImageGeometry.get(image) : null;
      if (!rect) {
        const clientRect = image.getBoundingClientRect();
        rect = {
          width: clientRect.width / scale,
          height: clientRect.height / scale,
          top: (clientRect.top - bookRect.top) / scale,
          right: (clientRect.right - bookRect.left) / scale,
          bottom: (clientRect.bottom - bookRect.top) / scale,
          left: (clientRect.left - bookRect.left) / scale,
        };
        if (stableBox) pendingImageGeometry.set(image, rect);
      }
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        rect.right > localBounds.left &&
        rect.left < localBounds.right &&
        rect.bottom > localBounds.top &&
        rect.top < localBounds.bottom
      );
    });
  }

  function forgetPendingImage(image) {
    pendingImages.delete(image);
    pendingImageGeometry.delete(image);
  }

  function normalizeLayoutAnchor(value) {
    if (Number.isFinite(value)) return { offset: value, pageIndex: null };
    if (!Number.isFinite(value?.offset)) return null;
    return {
      offset: value.offset,
      pageIndex: Number.isInteger(value.pageIndex) && value.pageIndex >= 0 ? value.pageIndex : null,
      scrollTop: Number.isFinite(value.scrollTop) && value.scrollTop >= 0 ? value.scrollTop : null,
    };
  }

  function scheduleLateLayout(anchor) {
    if (!onLateLayout) return;
    pendingImageGeometry.clear();
    pendingImageLayoutSignature = "";
    lateLayoutAnchor = normalizeLayoutAnchor(anchor) ?? lateLayoutAnchor;
    lateLayoutDirty = true;
    clearTimeout(lateLayoutTimer);
    lateLayoutTimer = setTimeout(() => {
      lateLayoutTimer = 0;
      if (!lateLayoutDirty || !lateLayoutAnchor) return;
      const anchorValue = lateLayoutAnchor;
      lateLayoutAnchor = null;
      lateLayoutDirty = false;
      onLateLayout(anchorValue);
    }, IMAGE_SETTLE_TIMEOUT_MS);
  }

  function resetLateImages() {
    lateImages.clear();
    lateLayoutAnchor = null;
    lateLayoutDirty = false;
    clearTimeout(lateLayoutTimer);
    lateLayoutTimer = 0;
  }

  function waitForImage(image, onLateSettle, timeoutMs = IMAGE_SETTLE_TIMEOUT_MS, signal) {
    return new Promise((resolve) => {
      let returned = false;
      let settled = false;
      let timer;
      const finish = (outcome) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        image.removeEventListener("load", loaded);
        image.removeEventListener("error", failed);
        signal?.removeEventListener("abort", aborted);
        if (returned) onLateSettle?.(outcome);
        else {
          returned = true;
          resolve(outcome);
        }
      };
      const loaded = () => finish("loaded");
      const failed = () => finish("failed");
      const aborted = () => finish("aborted");
      image.addEventListener("load", loaded, { once: true });
      image.addEventListener("error", failed, { once: true });
      signal?.addEventListener("abort", aborted, { once: true });
      if (signal?.aborted) {
        queueMicrotask(aborted);
        return;
      }
      if (image.complete && image.naturalWidth > 0) queueMicrotask(loaded);
      if (Number.isFinite(timeoutMs)) {
        timer = setTimeout(() => {
          if (returned) return;
          if (image.complete) {
            finish(image.naturalWidth > 0 ? "loaded" : "failed");
            return;
          }
          returned = true;
          resolve("timed-out");
        }, timeoutMs);
      }
    });
  }

  function imageLayoutSignature(images) {
    const values = [book.scrollWidth, book.scrollHeight];
    const elements = new Set(images.flatMap((image) => [image, image.parentElement]).filter(Boolean));
    for (const element of elements) {
      const rect = element.getBoundingClientRect();
      values.push(
        rect.left.toFixed(2),
        rect.top.toFixed(2),
        rect.right.toFixed(2),
        rect.bottom.toFixed(2),
      );
    }
    return values.join(":");
  }

  async function materializePreviewImage(source, preview, signal) {
    const url = pendingImages.get(source);
    if (!url || signal?.aborted) return false;
    const generation = renderGeneration;
    try {
      if (signal) {
        let abort;
        const aborted = new Promise((resolve) => {
          abort = () => resolve(false);
          signal.addEventListener("abort", abort, { once: true });
          if (signal.aborted) abort();
        });
        try {
          const validated = await Promise.race([validateSvgOnce(url).then(() => true), aborted]);
          if (!validated) return false;
        } finally {
          signal.removeEventListener("abort", abort);
        }
      } else await validateSvgOnce(url);
    } catch {
      if (signal?.aborted || generation !== renderGeneration || !preview.isConnected) return false;
      replaceFailedImage(preview);
      return false;
    }
    if (generation !== renderGeneration || signal?.aborted) return false;
    preview.src = url;
    const settle = (outcome) => {
      if (!preview.isConnected) return;
      if (outcome === "loaded") {
        preview.classList.remove("atha-preview-pending");
        preview.removeAttribute("aria-busy");
      } else if (outcome === "failed") {
        replaceFailedImage(preview);
      }
    };
    const outcome = await waitForImage(preview, settle, null, signal);
    if (outcome !== "timed-out") settle(outcome);
    if (outcome === "aborted") preview.removeAttribute("src");
    return !["aborted", "failed"].includes(outcome);
  }

  async function loadImages(images, generation, beforeLayoutChange, batch) {
    const beforeLayout = imageLayoutSignature(images);
    let layoutChanged = false;
    if (images.length > 0) beforeLayoutChange?.();
    const replaceFailed = (image, count = true) => {
      layoutChanged = true;
      replaceFailedImage(image);
      forgetPendingImage(image);
      if (count && batch) batch.failure += 1;
    };
    await Promise.all(
      images.map(async (image) => {
        const url = pendingImages.get(image);
        if (!url) return;
        try {
          await validateSvgOnce(url);
        } catch {
          if (generation !== renderGeneration || pendingImages.get(image) !== url) return;
          replaceFailed(image);
          return;
        }
        if (generation !== renderGeneration || pendingImages.get(image) !== url) return;
        image.src = url;
        const reveal = () => {
          image.classList.remove("atha-resource-pending");
          image.removeAttribute("aria-busy");
          delete image.dataset.athaResource;
        };
        const outcome = await waitForImage(image, (lateOutcome) => {
          const late = lateImages.get(image);
          lateImages.delete(image);
          if (generation !== renderGeneration || image.src !== url || !book.contains(image)) return;
          if (lateOutcome === "loaded") {
            readySvg.add(url);
            reveal();
          }
          else replaceFailed(image, false);
          if (
            lateOutcome === "failed" ||
            (lateOutcome === "loaded" && late?.layout !== imageLayoutSignature([image]))
          ) {
            scheduleLateLayout(late?.anchor);
          }
        });
        if (generation !== renderGeneration || pendingImages.get(image) !== url) return;
        if (outcome === "failed") {
          replaceFailed(image);
          return;
        }
        if (outcome === "loaded") readySvg.add(url);
        else {
          lateImages.set(image, {
            anchor: normalizeLayoutAnchor(beforeLayoutChange?.()) ?? {
              offset: 0,
              pageIndex: 0,
              scrollTop: 0,
            },
            generation,
            layout: imageLayoutSignature([image]),
          });
        }
        if (outcome === "loaded") reveal();
        forgetPendingImage(image);
        if (outcome === "loaded" && batch) batch.success += 1;
      }),
    );
    return layoutChanged || beforeLayout !== imageLayoutSignature(images);
  }

  async function loadVisible(includeNextPage = false, beforeLayoutChange) {
    const generation = renderGeneration;
    let loaded = 0;
    let layoutChanged = false;
    let layoutChangeNotified = false;
    let layoutChangeAnchor;
    const batches = [];
    const notifyBeforeLayoutChange = () => {
      if (!layoutChangeNotified) {
        layoutChangeNotified = true;
        layoutChangeAnchor = beforeLayoutChange?.();
      }
      return layoutChangeAnchor;
    };
    if ((lateImages.size > 0 || lateLayoutDirty) && beforeLayoutChange) {
      const anchor = normalizeLayoutAnchor(notifyBeforeLayoutChange());
      if (anchor) {
        for (const [image, late] of lateImages) {
          if (late.generation === generation && book.contains(image)) late.anchor = anchor;
        }
        if (lateLayoutDirty) {
          clearTimeout(lateLayoutTimer);
          lateLayoutTimer = 0;
          lateLayoutAnchor = null;
          lateLayoutDirty = false;
          layoutChanged = true;
        }
      }
    }
    for (let pass = 0; pass < 4; pass += 1) {
      const images = pendingWithin(loadBounds(includeNextPage));
      if (images.length === 0) break;
      loaded += images.length;
      const batch = { selected: images.length, success: 0, failure: 0, layoutChanged: false };
      const passChangedLayout = await loadImages(
        images,
        generation,
        notifyBeforeLayoutChange,
        batch,
      );
      batch.layoutChanged = passChangedLayout;
      batches.push(Object.freeze(batch));
      layoutChanged ||= passChangedLayout;
      if (passChangedLayout) {
        pendingImageGeometry.clear();
        pendingImageLayoutSignature = "";
      }
      if (generation !== renderGeneration) break;
    }
    lastVisibleLoadCount = loaded;
    lastVisibleLoad = Object.freeze({
      passes: batches.length,
      generationChanged: generation !== renderGeneration,
      batches: Object.freeze(batches),
    });
    return Object.freeze({ loaded, layoutChanged });
  }

  function idleTurn() {
    return new Promise((resolve) => {
      if (globalThis.requestIdleCallback) globalThis.requestIdleCallback(resolve, { timeout: 50 });
      else setTimeout(resolve, 0);
    });
  }

  function warmRemaining() {
    if (warmPromise) return warmPromise;
    const generation = renderGeneration;
    warmPromise = (async () => {
      const started = performance.now();
      while (generation === renderGeneration && pendingImages.size > 0) {
        await idleTurn();
        if (generation !== renderGeneration) break;
        // ponytail: fixed small batches yield between formula groups; make adaptive only if traces show jank.
        await loadImages([...pendingImages.keys()].slice(0, 16), generation);
      }
      if (generation === renderGeneration) {
        warmDurationMs = performance.now() - started;
      }
      return warmDurationMs;
    })();
    return warmPromise;
  }

  function resourceSnapshot() {
    return Object.freeze({
      pending: pendingImages.size,
      late: lateImages.size,
      currentPending: pendingWithin(loadBounds(false)).length,
      currentOrNextPending: pendingWithin(loadBounds(true)).length,
      deferred: deferredImageCount,
      eagerSvg: eagerSvgCount,
      lastVisible: lastVisibleLoadCount,
      visibleLoad: lastVisibleLoad,
      validatedSvg: validatedSvg.size,
      warming: Boolean(warmPromise) && pendingImages.size > 0,
      warmDurationMs,
    });
  }

  function rejected(action) {
    try {
      action();
      return false;
    } catch {
      return true;
    }
  }

  async function validatorSelfCheck() {
    for (const markup of [
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><script>1</script></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><p onclick='x()'>x</p></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><img src='https://example.com/x.png'/></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><svg><image href='https://example.com/x.png'/></svg></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><form/></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='javascript:alert(1)'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='mailto:x@example.com'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='data:text/plain,x'>x</a></body></html>",
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><a href='blob:https://example.com/x.xhtml'>x</a></body></html>",
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
    const passiveMetadata = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><head><meta http-equiv='Content-Type' content='text/html'/></head><body>safe</body></html>",
      "application/xhtml+xml",
    );
    validateMarkup(passiveMetadata);
    ensure(!passiveMetadata.querySelector("meta[http-equiv]"), "active-content");
    const xhtml11 = `<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd"><html xmlns="http://www.w3.org/1999/xhtml"><body>safe&nbsp;space</body></html>`;
    const xhtml11Document = parseSafeXhtml(xhtml11);
    validateMarkup(xhtml11Document);
    ensure(xhtml11Document.body.textContent === "safe\u00a0space", "invalid-xhtml");
    const internalEntity = `<!DOCTYPE html [<!ENTITY unsafe "expanded">]><html xmlns="http://www.w3.org/1999/xhtml"><body>&unsafe;</body></html>`;
    ensure(rejected(() => validateMarkup(parseSafeXhtml(internalEntity))), "active-content");
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
    const missingResource = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><head><link rel='stylesheet' href='missing.css'/></head><body><img src='missing.png' alt='缺失插图'/></body></html>",
      "application/xhtml+xml",
    );
    const previousResources = declaredResources;
    declaredResources = new Set();
    try {
      validateMarkup(missingResource);
    } finally {
      declaredResources = previousResources;
    }
    const missingPlaceholder = missingResource.querySelector(".atha-image-error");
    ensure(
      !missingResource.querySelector("link, img") &&
        missingPlaceholder?.dataset.label === "缺失插图" &&
        missingPlaceholder.getAttribute("aria-label") === "缺失插图" &&
        missingResource.body.textContent === "",
      "undeclared-resource",
    );
    const imageMarkup = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><img alt='插图' aria-hidden='true' aria-disabled='true'/><img class='math-inline' alt='x + y'/><a href='#x'><img role='button' tabindex='0'/></a></body></html>",
      "application/xhtml+xml",
    );
    const [standaloneImage, formulaImage, linkedImage] = imageMarkup.querySelectorAll("img");
    for (const image of imageMarkup.querySelectorAll("img")) setImageInteraction(image);
    ensure(
      standaloneImage.getAttribute("role") === "button" &&
        standaloneImage.getAttribute("tabindex") === "0" &&
        standaloneImage.getAttribute("aria-label") === "查看图片：插图" &&
        !standaloneImage.hasAttribute("aria-hidden") &&
        !standaloneImage.hasAttribute("aria-disabled") &&
        formulaImage.getAttribute("aria-label") === "查看公式：x + y" &&
        !deferredFormula(formulaImage) &&
        !linkedImage.hasAttribute("role") &&
        !linkedImage.hasAttribute("tabindex"),
      "active-content",
    );
    const hintedImage = document.createElement("img");
    hintedImage.setAttribute("data-atha-intrinsic-width", "598");
    hintedImage.setAttribute("data-atha-intrinsic-height", "130");
    ensure(applyIntrinsicImageHint(hintedImage) && hasStableImageBox(hintedImage), "image-load");
    const formulaRatio = document.createElement("img");
    formulaRatio.className = "math-inline";
    formulaRatio.setAttribute("width", "47.5");
    formulaRatio.setAttribute("height", "17");
    ensure(hasStableImageBox(formulaRatio), "image-load");
    formulaRatio.style.cssText = "height:1.02em;width:auto";
    formulaRatio.classList.add("atha-resource-pending");
    formulaRatio.dataset.athaResource = "formula.svg";
    book.append(formulaRatio);
    const previousInlineStyles = inlineStyles;
    const previousSourceStyles = sourceStyles;
    inlineStyles = [[formulaRatio, formulaRatio.getAttribute("style")]];
    try {
      setStyles({ sourceStyles: false, userStylesEnabled, userStylesheet });
      ensure(
        formulaRatio.style.aspectRatio === "47.5 / 17" && !formulaRatio.style.height,
        "image-load",
      );
      setStyles({ sourceStyles: true, userStylesEnabled, userStylesheet });
      ensure(
        formulaRatio.style.aspectRatio === "47.5 / 17" && formulaRatio.style.height === "1.02em",
        "image-load",
      );
    } finally {
      formulaRatio.remove();
      inlineStyles = previousInlineStyles;
      setStyles({ sourceStyles: previousSourceStyles, userStylesEnabled, userStylesheet });
    }
    const previousUserStyle = userStyle.textContent;
    userStyle.textContent =
      "img{display:block;width:50px;height:300px!important}";
    book.append(hintedImage);
    const hintedStyle = getComputedStyle(hintedImage);
    ensure(
      Math.abs(parseFloat(hintedStyle.width) - 50) < 0.1 &&
        Math.abs(parseFloat(hintedStyle.height) - (50 * 130) / 598) < 0.2 &&
        hintedImage.style.getPropertyPriority("height") === "important",
      "image-load",
    );
    hintedImage.remove();
    userStyle.textContent = "";
    const nativeImage = document.createElement("img");
    nativeImage.width = 598;
    nativeImage.height = 130;
    nativeImage.dataset.athaNativeSize = "";
    book.append(nativeImage);
    nativeImageStyle.textContent = nativeImageHintCss(book);
    const nativeDefaultStyle = getComputedStyle(nativeImage);
    const nativeDefaultWidth = parseFloat(nativeDefaultStyle.width);
    ensure(
      nativeDefaultWidth > 0 &&
        nativeDefaultWidth <= 598 &&
        Math.abs(parseFloat(nativeDefaultStyle.height) - (nativeDefaultWidth * 130) / 598) < 0.2,
      "image-load",
    );
    userStyle.textContent = "img{display:block;width:50px}";
    const nativeStyle = getComputedStyle(nativeImage);
    ensure(
      Math.abs(parseFloat(nativeStyle.width) - 50) < 0.1 &&
        Math.abs(parseFloat(nativeStyle.height) - (50 * 130) / 598) < 0.2,
      "image-load",
    );
    userStyle.textContent = "img{display:block;height:30px}";
    const heightSizedStyle = getComputedStyle(nativeImage);
    ensure(
      Math.abs(parseFloat(heightSizedStyle.width) - (30 * 598) / 130) < 0.2 &&
        Math.abs(parseFloat(heightSizedStyle.height) - 30) < 0.1,
      "image-load",
    );
    userStyle.textContent =
      "img{display:block;height:30px}@supports (not-a-real-property:x){img{width:50px}}";
    ensure(
      Math.abs(parseFloat(getComputedStyle(nativeImage).width) - (30 * 598) / 130) < 0.2,
      "image-load",
    );
    userStyle.textContent = "img{display:block;width:50px;height:30px}";
    ensure(
      Math.abs(parseFloat(getComputedStyle(nativeImage).height) - 30) < 0.1,
      "image-load",
    );
    const previousBookStyle = bookStyle.textContent;
    bookStyle.textContent = '@namespace epub "urn:atha:test"; epub|img{height:30px}';
    const snapshotCss = snapshotBookCss(book);
    const snapshotSheet = new CSSStyleSheet();
    snapshotSheet.replaceSync(snapshotCss);
    ensure(
      snapshotSheet.cssRules[0]?.constructor?.name === "CSSNamespaceRule" &&
        [...snapshotSheet.cssRules].some((rule) => rule.selectorText === "epub|img") &&
        new TextEncoder().encode(snapshotCss).length <= MAX_SNAPSHOT_BOOK_CSS_BYTES,
      "invalid-message-snapshot",
    );
    bookStyle.textContent = previousBookStyle;
    nativeImage.remove();
    nativeImageStyle.textContent = NATIVE_IMAGE_BASE_STYLE;
    userStyle.textContent = previousUserStyle;
    const structuredMarkup = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><table aria-hidden='true'><caption>数据</caption></table><pre aria-disabled='true'>code</pre><a href='#x'><pre tabindex='0'>linked</pre></a></body></html>",
      "application/xhtml+xml",
    );
    const [table, code, linkedCode] = structuredMarkup.querySelectorAll("table, pre");
    for (const element of structuredMarkup.querySelectorAll("table, pre")) {
      setStructuredInteraction(element);
      wrapStructuredOverflow(element);
    }
    ensure(
      table.getAttribute("tabindex") === "0" &&
        table.getAttribute("aria-label") === "查看表格：数据" &&
        !table.hasAttribute("aria-hidden") &&
        code.getAttribute("tabindex") === "0" &&
        code.getAttribute("aria-label") === "查看代码" &&
        !code.hasAttribute("aria-disabled") &&
        !linkedCode.hasAttribute("tabindex") &&
        table.parentElement?.classList.contains("atha-table-frame") &&
        [code, linkedCode].every((element) =>
          element.parentElement?.classList.contains("atha-pre-frame"),
        ),
      "active-content",
    );
    const cbzPage = document.createElement("main");
    cbzPage.className = "atha-cbz-page";
    const cbzImage = document.createElement("img");
    cbzPage.append(cbzImage);
    book.append(cbzPage);
    ensure(
      isControlledCbzSection(
        new DOMParser().parseFromString(
          "<html xmlns='http://www.w3.org/1999/xhtml'><body><main class='atha-cbz-page'><img src='images/page-0001.png'/></main></body></html>",
          "application/xhtml+xml",
        ),
        new URL("https://atha-book.localhost/.atha-cbz/page-0001.xhtml"),
      ),
      "image-load",
    );
    book.classList.add("atha-cbz-section");
    ensure(replaceFailedCbzImage(cbzImage), "image-load");
    const cbzPlaceholder = cbzPage.firstElementChild;
    ensure(
      cbzPlaceholder?.classList.contains("atha-cbz-page-error") &&
        cbzPlaceholder.getAttribute("role") === "img" &&
        cbzPlaceholder.getAttribute("aria-label") === "图片无法显示" &&
        cbzPlaceholder.textContent === "",
      "image-load",
    );
    cbzPage.remove();
    book.classList.remove("atha-cbz-section");
    const ordinaryImage = document.createElement("img");
    ensure(!replaceFailedCbzImage(ordinaryImage), "image-load");
    ordinaryImage.alt = "缺失插图";
    ordinaryImage.style.cssText =
      "display:block;width:320px;height:180px;margin:12px auto 18px";
    book.append(ordinaryImage);
    const ordinaryBox = [ordinaryImage.offsetWidth, ordinaryImage.offsetHeight];
    replaceFailedImage(ordinaryImage);
    const ordinaryPlaceholder = book.querySelector(".atha-image-error");
    ensure(
      ordinaryPlaceholder?.dataset.label === "缺失插图" &&
        ordinaryPlaceholder.getAttribute("aria-label") === "缺失插图" &&
        ordinaryPlaceholder.offsetWidth === ordinaryBox[0] &&
        ordinaryPlaceholder.offsetHeight === ordinaryBox[1] &&
        getComputedStyle(ordinaryPlaceholder).display === "block" &&
        getComputedStyle(ordinaryPlaceholder).marginTop === "12px" &&
        getComputedStyle(ordinaryPlaceholder).marginBottom === "18px" &&
        book.textContent === "",
      "image-load",
    );
    const absoluteImage = document.createElement("img");
    absoluteImage.alt = "absolute";
    absoluteImage.style.cssText =
      "position:absolute;top:7px;left:9px;width:20px;height:10px;transform:translateX(2px);z-index:2";
    book.append(absoluteImage);
    replaceFailedImage(absoluteImage);
    const absolutePlaceholder = book.querySelector(".atha-image-error[data-label='absolute']");
    const absoluteStyle = getComputedStyle(absolutePlaceholder);
    ensure(
      absoluteStyle.position === "absolute" &&
        absoluteStyle.top === "7px" &&
        absoluteStyle.left === "9px" &&
        absoluteStyle.transform !== "none" &&
        absoluteStyle.zIndex === "2",
      "image-load",
    );
    const hiddenImage = document.createElement("img");
    hiddenImage.alt = "hidden";
    hiddenImage.style.display = "none";
    book.append(hiddenImage);
    replaceFailedImage(hiddenImage);
    ensure(
      getComputedStyle(book.querySelector(".atha-image-error[data-label='hidden']")).display ===
        "none",
      "image-load",
    );
    book.replaceChildren();
    const markdownDocument = new DOMParser().parseFromString(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body class='atha-text atha-markdown'><pre>code</pre></body></html>",
      "application/xhtml+xml",
    );
    ensure(
      isControlledMarkdownSection(
        markdownDocument,
        new URL("https://atha-book.localhost/.atha-text/section-0001.xhtml"),
      ) &&
        !isControlledMarkdownSection(
          markdownDocument,
          new URL("https://atha-book.localhost/chapter.xhtml"),
        ),
      "active-content",
    );
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
    ensure(isSvgImage({ src: new URL("probe.SVG", bookUrl).href }), "invalid-svg");
    const validProbe = document.createElement("img");
    const validProbeUrl =
      "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='1'%20height='1'/%3E";
    Object.defineProperties(validProbe, {
      complete: { get: () => true },
      naturalWidth: { get: () => 1 },
    });
    const invalidProbe = document.createElement("img");
    invalidProbe.alt = "缺失公式";
    const invalidProbeUrl = "https://atha-book.localhost/invalid-probe.svg";
    book.append(validProbe, invalidProbe);
    pendingImages.set(validProbe, validProbeUrl);
    pendingImages.set(invalidProbe, invalidProbeUrl);
    validatedSvg.set(validProbeUrl, Promise.resolve());
    validatedSvg.set(invalidProbeUrl, Promise.reject(new Error("invalid-svg")));
    let layoutCallbacks = 0;
    try {
      const layoutChanged = await loadImages([validProbe, invalidProbe], renderGeneration, () => {
        layoutCallbacks += 1;
        ensure(book.contains(validProbe) && book.contains(invalidProbe), "invalid-svg");
      });
      const placeholder = book.querySelector(".atha-image-error");
      ensure(
        layoutChanged &&
          layoutCallbacks === 1 &&
          book.contains(validProbe) &&
          !book.contains(invalidProbe) &&
          !pendingImages.has(validProbe) &&
          !pendingImages.has(invalidProbe) &&
          placeholder?.dataset.label === "缺失公式" &&
          placeholder.getAttribute("aria-label") === "缺失公式" &&
          book.textContent === "",
        "invalid-svg",
      );
    } finally {
      pendingImages.delete(validProbe);
      pendingImages.delete(invalidProbe);
      validatedSvg.delete(validProbeUrl);
      validatedSvg.delete(invalidProbeUrl);
      readySvg.delete(validProbeUrl);
    }
    book.replaceChildren();
    const previewSource = document.createElement("img");
    const previewImage = document.createElement("img");
    Object.defineProperties(previewImage, {
      complete: { get: () => true },
      naturalWidth: { get: () => 1 },
    });
    previewImage.className = "atha-preview-pending";
    book.append(previewImage);
    pendingImages.set(previewSource, validProbeUrl);
    validatedSvg.set(validProbeUrl, Promise.resolve());
    try {
      ensure(
        (await materializePreviewImage(previewSource, previewImage)) &&
          previewImage.src === validProbeUrl &&
          !previewImage.classList.contains("atha-preview-pending"),
        "image-load",
      );
      const abortedPreview = document.createElement("img");
      const controller = new AbortController();
      controller.abort();
      ensure(
        !(await materializePreviewImage(previewSource, abortedPreview, controller.signal)) &&
          !abortedPreview.hasAttribute("src"),
        "image-load",
      );
    } finally {
      pendingImages.delete(previewSource);
      validatedSvg.delete(validProbeUrl);
    }
    book.replaceChildren();
    const firstVisible = document.createElement("img");
    const resizedVisible = document.createElement("img");
    const lateVisible = document.createElement("img");
    const firstVisibleUrl = `${validProbeUrl}#first-visible`;
    const resizedVisibleUrl = `${validProbeUrl}#resized-visible`;
    const lateVisibleUrl = `${validProbeUrl}#late-visible`;
    const bounds = loadBounds(false);
    const inside = () => ({
      width: 1,
      height: 1,
      top: bounds.top + 1,
      right: bounds.left + 2,
      bottom: bounds.top + 2,
      left: bounds.left + 1,
    });
    let lateInside = false;
    Object.defineProperty(firstVisible, "getBoundingClientRect", { value: inside });
    Object.defineProperty(resizedVisible, "getBoundingClientRect", {
      value: () => {
        const rect = inside();
        return resizedVisible.hasAttribute("src")
          ? { ...rect, right: rect.right + 1, bottom: rect.bottom + 1 }
          : rect;
      },
    });
    Object.defineProperty(lateVisible, "getBoundingClientRect", {
      value: () =>
        lateInside
          ? inside()
          : {
              width: 1,
              height: 1,
              top: bounds.top + 1,
              right: bounds.right + 2,
              bottom: bounds.top + 2,
              left: bounds.right + 1,
            },
    });
    Object.defineProperties(firstVisible, {
      complete: { get: () => true },
      naturalWidth: {
        get: () => {
          lateInside = true;
          return 1;
        },
      },
    });
    Object.defineProperties(resizedVisible, {
      complete: { get: () => true },
      naturalWidth: { get: () => 1 },
    });
    let lateLoad;
    let lateDecodeCalls = 0;
    Object.defineProperties(lateVisible, {
      addEventListener: {
        value: (type, listener) => {
          if (type === "load") lateLoad = listener;
        },
      },
      complete: { get: () => false },
      decode: {
        value: () => {
          lateDecodeCalls += 1;
          return new Promise(() => {});
        },
      },
      removeEventListener: { value: () => {} },
    });
    lateVisible.classList.add("atha-resource-pending");
    lateVisible.setAttribute("aria-busy", "true");
    lateVisible.dataset.athaResource = lateVisibleUrl;
    book.append(firstVisible, resizedVisible, lateVisible);
    pendingImages.set(firstVisible, firstVisibleUrl);
    pendingImages.set(resizedVisible, resizedVisibleUrl);
    pendingImages.set(lateVisible, lateVisibleUrl);
    validatedSvg.set(firstVisibleUrl, Promise.resolve());
    validatedSvg.set(resizedVisibleUrl, Promise.resolve());
    validatedSvg.set(lateVisibleUrl, Promise.resolve());
    try {
      const loaded = await loadVisible(false, () => 37);
      ensure(
        loaded.loaded === 3 &&
          loaded.layoutChanged &&
          !pendingImages.has(firstVisible) &&
          !pendingImages.has(resizedVisible) &&
          !pendingImages.has(lateVisible) &&
          lastVisibleLoad.passes === 2 &&
          lastVisibleLoad.batches[0].layoutChanged &&
          !lastVisibleLoad.batches[1].layoutChanged &&
          lastVisibleLoad.batches[1].success === 0 &&
          lateImages.get(lateVisible)?.anchor.offset === 37 &&
          lateVisible.classList.contains("atha-resource-pending") &&
          lateVisible.getAttribute("aria-busy") === "true" &&
          lateVisible.dataset.athaResource === lateVisibleUrl &&
          lateDecodeCalls === 0 &&
          typeof lateLoad === "function",
        "image-load",
      );
      lateLoad();
      ensure(
        book.contains(lateVisible) &&
          !lateVisible.classList.contains("atha-resource-pending") &&
          !lateVisible.hasAttribute("aria-busy") &&
          !lateVisible.dataset.athaResource &&
          !lateImages.has(lateVisible),
        "image-load",
      );
    } finally {
      pendingImages.delete(firstVisible);
      pendingImages.delete(resizedVisible);
      pendingImages.delete(lateVisible);
      validatedSvg.delete(firstVisibleUrl);
      validatedSvg.delete(resizedVisibleUrl);
      validatedSvg.delete(lateVisibleUrl);
      readySvg.delete(firstVisibleUrl);
      readySvg.delete(resizedVisibleUrl);
      readySvg.delete(lateVisibleUrl);
      resetLateImages();
    }
    book.replaceChildren();
  }

  async function initialize() {
    const response = await fetch(readerStyleSource.href);
    ensure(response.ok, "reader-style-load");
    readerStyle.textContent = await response.text();
  }

  function remember(cache, key, value, characters = sectionCacheOrder.get(key) || 0) {
    cache.delete(key);
    cache.set(key, value);
    sectionCacheOrder.delete(key);
    sectionCacheOrder.set(key, characters);
    let cachedCharacters = [...sectionCacheOrder.values()].reduce(
      (total, count) => total + count,
      0,
    );
    while (
      sectionCacheOrder.size > SECTION_CACHE_LIMIT ||
      (cachedCharacters > SECTION_CACHE_TEXT_LIMIT && sectionCacheOrder.size > 1)
    ) {
      const oldest = sectionCacheOrder.keys().next().value;
      cachedCharacters -= sectionCacheOrder.get(oldest) || 0;
      sectionCacheOrder.delete(oldest);
      preparedSections.delete(oldest);
      pendingSectionMarkup.delete(oldest);
    }
    return value;
  }

  function activateSection(section) {
    bookUrl = section.url;
    declaredResources = section.resources;
    cachedBody = section.body;
    cachedCss = section.css;
    cachedCbzPage = section.cbz;
    cachedMarkdownSection = section.markdown;
    eagerSvgCount = section.eagerSvgCount;
  }

  function sectionMarkup(url) {
    const key = url.href;
    const cached = pendingSectionMarkup.get(key);
    if (cached) return remember(pendingSectionMarkup, key, cached);
    const generation = sectionGeneration;
    const pending = fetch(url).then(async (response) => {
      if (!response.ok) throw new Error("section-load");
      const markup = await response.text();
      if (generation === sectionGeneration && pendingSectionMarkup.get(key) === pending) {
        remember(pendingSectionMarkup, key, pending, markup.length);
      }
      return markup;
    });
    return remember(pendingSectionMarkup, key, pending);
  }

  function forgetPendingSection(key, expected) {
    if (pendingSectionMarkup.get(key) !== expected) return;
    pendingSectionMarkup.delete(key);
    if (!preparedSections.has(key)) sectionCacheOrder.delete(key);
  }

  function prefetchSection(url) {
    const target = new URL(url);
    if (preparedSections.has(target.href) || pendingSectionMarkup.has(target.href)) return;
    const pending = sectionMarkup(target);
    void pending.catch(() => forgetPendingSection(target.href, pending));
  }

  function ensureSectionCurrent(generation) {
    if (generation !== sectionGeneration) throw new Error("section-superseded");
  }

  async function prepareSectionEntry(targetUrl, resources, loadError, generation) {
    if (!selfChecked) {
      const activeUrl = bookUrl;
      const activeResources = declaredResources;
      bookUrl = targetUrl;
      declaredResources = resources;
      try {
        await validatorSelfCheck();
        ensureSectionCurrent(generation);
        selfChecked = true;
      } finally {
        bookUrl = activeUrl;
        declaredResources = activeResources;
      }
    }
    const key = targetUrl.href;
    const prepared = preparedSections.get(key);
    if (prepared) {
      return remember(preparedSections, key, prepared, prepared.characters);
    }
    const pending = sectionMarkup(targetUrl);
    try {
      let markup;
      try {
        markup = await pending;
      } catch {
        reject(loadError);
      }
      ensureSectionCurrent(generation);
      const source = parseSafeXhtml(markup);
      const activeUrl = bookUrl;
      const activeResources = declaredResources;
      bookUrl = targetUrl;
      declaredResources = resources;
      let styleSources;
      try {
        validateMarkup(source);
        styleSources = detachSourceStyles(source);
      } finally {
        bookUrl = activeUrl;
        declaredResources = activeResources;
      }
      const stylesheets = await Promise.all(
        styleSources.map(async ({ css, url }) => {
          if (css === undefined) {
            const styleResponse = await fetch(url);
            if (!styleResponse.ok) return "";
            css = await styleResponse.text();
          }
          ensureSectionCurrent(generation);
          return rejected(() => validateCss(css)) ? "" : css;
        }),
      );
      ensureSectionCurrent(generation);
      const svgImages = new Map();
      for (const image of source.querySelectorAll("img[src]")) {
        if (!isSvgImage(image) || deferredFormula(image)) continue;
        const images = svgImages.get(image.src);
        if (images) images.push(image);
        else svgImages.set(image.src, [image]);
      }
      const sectionEagerSvgCount = svgImages.size;
      await Promise.all(
        [...svgImages].map(async ([url, images]) => {
          try {
            await validateSvgOnce(url, generation);
          } catch {
            for (const image of images) replaceFailedImage(image);
          }
        }),
      );
      ensureSectionCurrent(generation);
      for (const image of source.querySelectorAll("img")) {
        if (!deferredFormula(image)) continue;
        image.dataset.athaResource = image.src;
        image.removeAttribute("src");
        image.classList.add("atha-resource-pending");
        image.setAttribute("aria-busy", "true");
      }
      pendingSectionMarkup.delete(key);
      const section = Object.freeze({
        url: targetUrl,
        resources,
        body: source.body,
        css: stylesheets.join("\n"),
        cbz: isControlledCbzSection(source, targetUrl),
        markdown: isControlledMarkdownSection(source, targetUrl),
        eagerSvgCount: sectionEagerSvgCount,
        characters: markup.length,
      });
      return remember(preparedSections, key, section, section.characters);
    } catch (error) {
      forgetPendingSection(key, pending);
      throw error;
    }
  }

  async function prepareSection(url, resources, loadError) {
    const targetUrl = new URL(url);
    const generation = sectionGeneration;
    return prepareSectionEntry(targetUrl, resources, loadError, generation);
  }

  async function renderCached() {
    ensure(cachedBody, "section-load");
    renderGeneration += 1;
    resetLateImages();
    lastVisibleLoad = Object.freeze({
      passes: 0,
      generationChanged: false,
      batches: Object.freeze([]),
    });
    warmPromise = undefined;
    warmDurationMs = null;
    bookStyle.textContent = sourceStyles ? cachedCss : "";
    userStyle.textContent = userStylesEnabled ? userStylesheet : "";
    book.classList.toggle("atha-cbz-section", cachedCbzPage);
    book.classList.toggle("atha-markdown-section", cachedMarkdownSection);
    const imported = document.importNode(cachedBody, true);
    pendingImages.clear();
    pendingImageGeometry.clear();
    pendingImageLayoutSignature = "";
    const deferredImages = imported.querySelectorAll(
      "img.atha-resource-pending[data-atha-resource]",
    );
    deferredImageCount = deferredImages.length;
    for (const image of deferredImages) {
      const url = image.dataset.athaResource;
      if (readySvg.has(url)) {
        image.src = url;
        image.classList.remove("atha-resource-pending");
        image.removeAttribute("aria-busy");
        delete image.dataset.athaResource;
        continue;
      }
      pendingImages.set(image, url);
    }
    book.replaceChildren(...imported.childNodes);
    nativeImageStyle.textContent = nativeImageHintCss(book);
    inlineStyles = [...book.querySelectorAll("[style]")].map((element) => [
      element,
      element.getAttribute("style"),
    ]);
    setStyles({ sourceStyles, userStylesEnabled, userStylesheet });
    for (const image of book.querySelectorAll(
      "img[data-atha-intrinsic-width][data-atha-intrinsic-height]",
    )) {
      applyIntrinsicImageHint(image);
    }
    const generation = renderGeneration;
    for (const image of [...book.querySelectorAll("img")].filter(
      (candidate) => !pendingImages.has(candidate),
    )) {
      const source = image.getAttribute("src");
      if (!source || (image.complete && image.naturalWidth === 0)) {
        replaceFailedImage(image);
        continue;
      }
      if (image.complete) continue;
      const url = image.src;
      const settle = (outcome) => {
        const late = lateImages.get(image);
        lateImages.delete(image);
        if (generation !== renderGeneration || image.src !== url || !book.contains(image)) return;
        if (outcome === "failed") replaceFailedImage(image);
        if (outcome === "failed" || late?.dynamic) scheduleLateLayout(late?.anchor);
      };
      lateImages.set(image, {
        anchor: { offset: 0, pageIndex: 0, scrollTop: 0 },
        generation,
        dynamic: !hasStableImageBox(image),
      });
      void waitForImage(image, settle).then((outcome) => {
        if (outcome !== "timed-out") settle(outcome);
      });
    }
    await document.fonts.ready;
  }

  function close() {
    sectionGeneration += 1;
    renderGeneration += 1;
    resetLateImages();
    book.replaceChildren();
    nativeImageStyle.textContent = NATIVE_IMAGE_BASE_STYLE;
    bookStyle.textContent = "";
    userStyle.textContent = "";
    bookUrl = undefined;
    declaredResources = undefined;
    cachedBody = undefined;
    cachedCss = undefined;
    cachedCbzPage = false;
    cachedMarkdownSection = false;
    book.classList.remove("atha-cbz-section", "atha-markdown-section");
    inlineStyles = [];
    deferredImageCount = 0;
    eagerSvgCount = 0;
    lastVisibleLoadCount = 0;
    lastVisibleLoad = Object.freeze({
      passes: 0,
      generationChanged: false,
      batches: Object.freeze([]),
    });
    warmPromise = undefined;
    warmDurationMs = null;
    pendingImages.clear();
    pendingImageGeometry.clear();
    pendingImageLayoutSignature = "";
    validatedCss.clear();
    validatedSvg.clear();
    readySvg.clear();
    preparedSections.clear();
    pendingSectionMarkup.clear();
    sectionCacheOrder.clear();
  }

  function styleSnapshot() {
    return Object.freeze({
      sourceStyles,
      userStylesEnabled,
      bookStyleApplied: bookStyle.textContent.length > 0,
      userStyleApplied: userStyle.textContent.length > 0,
    });
  }

  function selectionRange() {
    return cloneSelectedRange(book, shadow.getSelection?.());
  }

  async function captureRange(range, presentation) {
    ensure(range instanceof Range && book.contains(range.commonAncestorContainer), "annotation-selection");
    const wrapper = document.createElement("div");
    wrapper.append(range.cloneContents());
    wrapper
      .querySelectorAll(
        "script,iframe,object,embed,form,input,button,select,textarea,video,audio,source,track,style,link,meta,base",
      )
      .forEach((element) => element.remove());
    for (const element of wrapper.querySelectorAll("*")) {
      for (const attribute of [...element.attributes]) {
        const name = attribute.name.toLowerCase();
        if (
          name.startsWith("on") ||
          ["style", "srcset"].includes(name) ||
          (name === "href" && element.localName.toLowerCase() !== "image")
        ) {
          element.removeAttribute(attribute.name);
        }
      }
    }
    const resources = new Map();
    for (const element of wrapper.querySelectorAll("img[src], image[href]")) {
      const attribute = element.localName.toLowerCase() === "img" ? "src" : "href";
      const url = new URL(localBookUrl(element.getAttribute(attribute)));
      const path = url.pathname.slice(1);
      element.setAttribute(attribute, path);
      if (resources.has(path)) continue;
      const response = await fetch(url);
      ensure(response.ok, "image-load");
      const mediaType = (response.headers.get("content-type") || "")
        .split(";", 1)[0]
        .trim()
        .toLowerCase();
      ensure(mediaType.startsWith("image/"), "image-load");
      resources.set(path, {
        path,
        mediaType,
        bytes: [...new Uint8Array(await response.arrayBuffer())],
      });
    }
    const capturedTheme =
      presentation.theme === "system"
        ? globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light"
        : presentation.theme;
    return Object.freeze({
      fragmentHtml: wrapper.innerHTML,
      readerCss: snapshotReaderCss(),
      bookCss: snapshotBookCss(wrapper),
      userCss: userStyle.textContent,
      presentationJson: JSON.stringify({
        schema: 1,
        theme: capturedTheme,
        brightness: presentation.brightness,
        fontSize: presentation.fontSize,
        fontFamily: presentation.fontFamily,
        density: presentation.density,
      }),
      resources: Object.freeze([...resources.values()].map(Object.freeze)),
    });
  }

  return Object.freeze({
    book,
    activateSection,
    captureRange,
    close,
    initialize,
    prepareSection,
    loadVisible,
    materializePreviewImage,
    prefetchSection,
    renderCached,
    resourceSnapshot,
    selectionRange,
    describeLink,
    setStyles,
    styleSnapshot,
    validateStylesheet: validateUserStylesheet,
    warmRemaining,
  });
}
