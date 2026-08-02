export function createContentActions({
  content,
  navigation,
  dialog,
  title,
  body,
  image,
  closeButton,
  assert,
  fail,
}) {
  let trigger = null;
  let pending = Promise.resolve();
  let copyProbeArmed = false;
  const counts = {
    internal: 0,
    external: 0,
    footnote: 0,
    image: 0,
    formula: 0,
    trustedCopies: 0,
  };

  function open(anchor) {
    trigger = anchor;
    if (!dialog.open) dialog.showModal();
    closeButton.focus();
  }

  function show(anchor, heading, text) {
    title.textContent = heading;
    body.textContent = text;
    body.hidden = false;
    image.hidden = true;
    image.removeAttribute("src");
    image.className = "";
    dialog.classList.remove("media-preview");
    open(anchor);
  }

  function showImage(source) {
    const formula = source.matches(".math-inline, .math-display");
    const heading = formula ? "公式" : "图片";
    title.textContent = heading;
    body.hidden = true;
    image.src = source.src;
    image.alt = source.getAttribute("alt")?.trim().slice(0, 160) || heading;
    image.className = formula ? "formula-preview" : "";
    image.hidden = false;
    dialog.classList.add("media-preview");
    counts[formula ? "formula" : "image"] += 1;
    open(source);
  }

  function isNoteref(anchor) {
    const type =
      anchor.getAttribute("epub:type") ||
      anchor.getAttributeNS("http://www.idpf.org/2007/ops", "type") ||
      "";
    return type.split(/\s+/).includes("noteref") || anchor.getAttribute("role") === "doc-noteref";
  }

  function fragmentTarget(href) {
    const hash = new URL(href).hash;
    if (!hash) return null;
    let id;
    try {
      id = decodeURIComponent(hash.slice(1));
    } catch {
      return null;
    }
    return [...content.book.querySelectorAll("[id]")].find((element) => element.id === id) || null;
  }

  async function activate(anchor) {
    const link = content.describeLink(anchor.getAttribute("href"));
    if (link.kind === "external") {
      counts.external += 1;
      show(anchor, "外部链接已阻止", new URL(link.href).host);
      return "external";
    }
    if (link.sameSection && isNoteref(anchor)) {
      const target = fragmentTarget(link.href);
      const text = target?.textContent.trim();
      if (text) {
        counts.footnote += 1;
        show(anchor, "脚注", text);
        return "footnote";
      }
    }
    counts.internal += 1;
    await navigation.goToHref(link.href);
    return "internal";
  }

  function report(error) {
    if (document.documentElement.dataset.error) return;
    try {
      fail(error instanceof Error ? error.message : "section-load");
    } catch {
      // fail already recorded the terminal reader state.
    }
  }

  function onContentClick(event) {
    if (event.type === "auxclick" && event.button !== 1) return;
    if (!(event.target instanceof Element)) return;
    const anchor = event.target.closest("a[href]");
    if (anchor && content.book.contains(anchor)) {
      event.preventDefault();
      pending = activate(anchor);
      pending.catch(report);
      return;
    }
    const source = event.type === "click" ? event.target.closest("img[role='button']") : null;
    if (source && content.book.contains(source)) {
      event.preventDefault();
      showImage(source);
    }
  }

  function onContentKeydown(event) {
    if (!["Enter", " "].includes(event.key) || !(event.target instanceof Element)) return;
    const source = event.target.closest("img[role='button']");
    if (!source || !content.book.contains(source)) return;
    event.preventDefault();
    showImage(source);
  }

  function bind() {
    content.book.addEventListener("click", onContentClick);
    content.book.addEventListener("auxclick", onContentClick);
    content.book.addEventListener("keydown", onContentKeydown);
    content.book.addEventListener("copy", (event) => {
      if (!event.isTrusted) return;
      counts.trustedCopies += 1;
      if (!copyProbeArmed) return;
      copyProbeArmed = false;
      event.preventDefault();
    });
    dialog.addEventListener("close", () => {
      if (trigger?.isConnected) trigger.focus({ preventScroll: true });
      trigger = null;
      body.hidden = false;
      image.hidden = true;
      image.removeAttribute("src");
      image.className = "";
      dialog.classList.remove("media-preview");
    });
  }

  function selectionProbe() {
    const walker = document.createTreeWalker(content.book, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) =>
        node.data.trim().length >= 4 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP,
    });
    for (let text = walker.nextNode(); text; text = walker.nextNode()) {
      const start = text.data.search(/\S/);
      const range = document.createRange();
      range.setStart(text, start);
      range.setEnd(text, Math.min(text.length, start + 8));
      const rect = [...range.getClientRects()].find(
        (item) => item.width >= 8 && item.top >= 0 && item.bottom <= innerHeight,
      );
      if (rect) {
        return Object.freeze({
          startX: rect.left + 2,
          startY: rect.top + rect.height / 2,
          endX: rect.right - 2,
          endY: rect.top + rect.height / 2,
        });
      }
    }
    return null;
  }

  function selectionLength() {
    return content.book.getRootNode().getSelection?.().toString().length || 0;
  }

  function clearSelection() {
    content.book.getRootNode().getSelection?.().removeAllRanges();
    return selectionLength();
  }

  async function verify({ pagination, session }) {
    const waitFor = async (predicate) => {
      for (let frame = 0; frame < 120; frame += 1) {
        if (predicate()) return;
        await new Promise(requestAnimationFrame);
      }
      assert(false, "sample-boundary");
    };
    const makeLink = (href, text = "probe") => {
      const anchor = document.createElement("a");
      anchor.href = href;
      anchor.textContent = text;
      anchor.style.position = "fixed";
      anchor.style.left = "-9999px";
      content.book.append(anchor);
      return anchor;
    };
    const probeMedia = async (source, key) => {
      const formula = source.matches(".math-inline, .math-display");
      const pageBefore = pagination.snapshot().page;
      const sectionBefore = session.snapshot().currentIndex;
      const locatorBefore = JSON.stringify(navigation.current());
      source.focus({ preventScroll: true });
      if (key) {
        source.dispatchEvent(
          new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }),
        );
      } else {
        source.click();
      }
      assert(
        dialog.open &&
          !image.hidden &&
          body.hidden &&
          image.src === source.src &&
          title.textContent === (formula ? "公式" : "图片") &&
          image.alt ===
            (source.getAttribute("alt")?.trim().slice(0, 160) ||
              (formula ? "公式" : "图片")),
        "sample-boundary",
      );
      await image.decode();
      const dark =
        document.documentElement.dataset.theme === "dark" ||
        (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
      const filter = getComputedStyle(image).filter;
      assert(formula ? (dark ? filter !== "none" : filter === "none") : filter === "none", "sample-boundary");
      closeButton.click();
      await waitFor(() => !dialog.open);
      assert(
        content.book.getRootNode().activeElement === source &&
          pagination.snapshot().page === pageBefore &&
          session.snapshot().currentIndex === sectionBefore &&
          JSON.stringify(navigation.current()) === locatorBefore,
        "sample-boundary",
      );
      return filter;
    };

    await session.open(0);
    await pagination.show(0);
    const walker = document.createTreeWalker(content.book, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) =>
        node.data.trim().length >= 2 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP,
    });
    const text = walker.nextNode();
    const selection = content.book.getRootNode().getSelection?.();
    assert(text && selection, "sample-boundary");
    const range = document.createRange();
    range.setStart(text, 0);
    range.setEnd(text, 2);
    selection.removeAllRanges();
    selection.addRange(range);
    const selectedText = selection.toString();
    const copy = new ClipboardEvent("copy", { bubbles: true, cancelable: true });
    content.book.dispatchEvent(copy);
    assert(!copy.defaultPrevented && selection.toString() === selectedText, "sample-boundary");
    selection.removeAllRanges();

    const target = [...content.book.querySelectorAll("h1, h2, h3, p, li, pre")].at(-1);
    assert(target, "sample-boundary");
    const previousId = target.id;
    target.id = "atha-link-probe";
    const targetOffset = pagination.offsetForFragment(target.id);
    assert(targetOffset !== null, "sample-boundary");
    const internal = makeLink("#atha-link-probe");
    internal.dispatchEvent(
      new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }),
    );
    await pending;
    assert(
      counts.internal === 1 &&
        session.snapshot().currentIndex === 0 &&
        pagination.isOffsetVisible(targetOffset),
      "sample-boundary",
    );
    internal.remove();
    if (previousId) target.id = previousId;
    else target.removeAttribute("id");

    const tail = document.createElement("span");
    const tailWhitespace = document.createTextNode("\n  ");
    tail.id = "atha-tail-fragment-probe";
    content.book.append(tail, tailWhitespace);
    const tailOffset = pagination.offsetForFragment(tail.id);
    assert(
      tailOffset !== null &&
        (await pagination.showOffset(tailOffset)) &&
        pagination.isOffsetVisible(tailOffset) &&
        pagination.snapshot().page === pagination.snapshot().pages - 1,
      "sample-boundary",
    );
    tail.remove();
    tailWhitespace.remove();

    const missing = makeLink("#atha-missing-fragment");
    missing.click();
    await pending;
    assert(
      counts.internal === 2 &&
        document.documentElement.dataset.locatorFallback === "locator-fragment",
      "sample-boundary",
    );
    missing.remove();

    const unknown = makeLink(
      new URL("atha-missing.xhtml", session.describe().sections[0].url).href,
    );
    unknown.click();
    await pending;
    assert(
      counts.internal === 3 &&
        session.snapshot().currentIndex === 0 &&
        document.documentElement.dataset.locatorFallback === "locator-section",
      "sample-boundary",
    );
    unknown.remove();

    let crossSection = false;
    if (session.describe().sections.length > 1) {
      const cross = makeLink(session.describe().sections[1].url);
      cross.click();
      await pending;
      assert(session.snapshot().currentIndex === 1, "sample-boundary");
      cross.remove();
      crossSection = true;
      await session.open(0);
    }

    const locationBefore = location.href;
    const external = makeLink("https://atha-link-probe.invalid/reference");
    external.focus({ preventScroll: true });
    external.click();
    await pending;
    assert(dialog.open && counts.external === 1, "sample-boundary");
    assert(
      location.href === locationBefore &&
        title.textContent === "外部链接已阻止" &&
        body.textContent === "atha-link-probe.invalid",
      "sample-boundary",
    );
    closeButton.click();
    await waitFor(() => !dialog.open);
    assert(content.book.getRootNode().activeElement === external, "sample-boundary");
    external.remove();

    const note = document.createElement("aside");
    note.id = "atha-footnote-probe";
    note.hidden = true;
    note.textContent = "脚注纯文本";
    content.book.append(note);
    const noteref = makeLink("#atha-footnote-probe", "脚注");
    noteref.setAttribute("role", "doc-noteref");
    noteref.focus({ preventScroll: true });
    noteref.click();
    await pending;
    assert(dialog.open && counts.footnote === 1, "sample-boundary");
    assert(title.textContent === "脚注" && body.textContent === note.textContent, "sample-boundary");
    const dialogPage = pagination.snapshot().page;
    body.dispatchEvent(
      new KeyboardEvent("keydown", { key: "PageDown", bubbles: true, cancelable: true }),
    );
    await new Promise(requestAnimationFrame);
    assert(pagination.snapshot().page === dialogPage, "sample-boundary");
    closeButton.click();
    await waitFor(() => !dialog.open);
    assert(content.book.getRootNode().activeElement === noteref, "sample-boundary");
    noteref.remove();
    note.remove();
    await pagination.show(0);

    const ordinary = content.book.querySelector(
      "img[role='button']:not(.math-inline):not(.math-display)",
    );
    const formula = content.book.querySelector(
      "img[role='button'].math-inline, img[role='button'].math-display",
    );
    const ordinaryFilter = ordinary ? await probeMedia(ordinary) : null;
    const formulaFilter = formula ? await probeMedia(formula, "Enter") : null;

    const mediaCount = counts.image + counts.formula;
    const internalCount = counts.internal;
    const linkedImage = document.createElement("img");
    const linked = makeLink("#atha-missing-fragment");
    linked.replaceChildren(linkedImage);
    linkedImage.click();
    await pending;
    assert(
      !dialog.open &&
        counts.image + counts.formula === mediaCount &&
        counts.internal === internalCount + 1,
      "sample-boundary",
    );
    linked.remove();
    await pagination.show(0);

    return Object.freeze({
      ...counts,
      selectionCopied: true,
      sameSection: true,
      tailFragmentRecovered: true,
      missingTargetRecovered: true,
      unknownSectionRecovered: true,
      crossSection,
      auxiliaryActivation: true,
      externalBlocked: true,
      footnoteDialog: true,
      dialogInputProtected: true,
      focusRestored: true,
      ordinaryPreview: ordinary ? true : null,
      formulaPreview: formula ? true : null,
      ordinaryPreviewFilter: ordinaryFilter,
      formulaPreviewFilter: formulaFilter,
      mediaPagePreserved: true,
      linkImageProtected: true,
    });
  }

  return Object.freeze({
    activate,
    armCopyProbe: () => {
      copyProbeArmed = true;
    },
    bind,
    clearSelection,
    selectionLength,
    selectionProbe,
    snapshot: () => Object.freeze({ ...counts }),
    verify,
  });
}
