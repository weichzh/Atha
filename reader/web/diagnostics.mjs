const BENCHMARK_SAMPLES = 10;
const IMAGE_LOAD_COUNT_MAX = 10_000;

function imageLoadCount(value) {
  return Math.min(Math.max(Math.trunc(value) || 0, 0), IMAGE_LOAD_COUNT_MAX);
}

function imageLoadTerminalMessage(resources) {
  const visible = resources.visibleLoad;
  const passes = Math.min(visible.passes, 4);
  const fields = [
    "image-load",
    passes,
    imageLoadCount(resources.currentPending),
    imageLoadCount(resources.currentOrNextPending),
    visible.generationChanged ? 1 : 0,
  ];
  for (let index = 0; index < 4; index += 1) {
    const batch = index < passes ? visible.batches[index] : null;
    const selected = imageLoadCount(batch?.selected);
    const success = Math.min(imageLoadCount(batch?.success), selected);
    const failure = Math.min(imageLoadCount(batch?.failure), selected - success);
    fields.push(selected, success, failure, batch?.layoutChanged ? 1 : 0);
  }
  return fields.join("|");
}

export function createDiagnostics({
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
  readingStatistics,
  bookmarks,
  search,
  annotations,
  reader,
  renderCachedSource,
  emit,
  assert,
}) {
  let verifiedSections = [];
  let verifiedHeadings = [];
  let releasedSections = 0;
  let navigationEvidence = {};
  let preferencesEvidence = {};
  let interactionEvidence = {};
  let contentActionEvidence = {};
  let stateEvidence = {};
  let bookmarkEvidence = {};
  let searchEvidence = {};
  let annotationEvidence = {};
  let gestureFixture = null;
  let gestureOrigin = null;
  let gestureTargets = new Map();
  let activeGesture = null;
  let gestureSequence = 0;

  assert(
    imageLoadTerminalMessage({
      currentPending: 2,
      currentOrNextPending: 3,
      visibleLoad: {
        passes: 2,
        generationChanged: true,
        batches: [
          { selected: 4, success: 3, failure: 1, layoutChanged: true },
          { selected: 2, success: 1, failure: 0, layoutChanged: false },
        ],
      },
    }) === "image-load|2|2|3|1|4|3|1|1|2|1|0|0|0|0|0|0|0|0|0|0",
    "image-load",
  );

  function heading() {
    return content.book.querySelector("h1, h2, h3")?.textContent.trim() || null;
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

  function recordFirstStable(started) {
    if (params.get("benchmark") === "hot") return;
    const state = pagination.snapshot();
    emit(
      `metric|first_stable|1|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}|${reader.clientWidth}|${reader.clientHeight}`,
    );
  }

  function independentLastContentPage() {
    const savedTransform = content.book.style.transform;
    content.book.style.transform = "none";
    const style = getComputedStyle(content.book);
    const viewport = reader.getBoundingClientRect();
    const scale = viewport.width / reader.clientWidth;
    const step = (parseFloat(style.width) + parseFloat(style.columnGap)) * scale;
    const left = content.book.getBoundingClientRect().left;
    let right = left;
    const walker = document.createTreeWalker(content.book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (!walker.currentNode.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(walker.currentNode);
      for (const rect of range.getClientRects()) {
        if (rect.width && rect.height) right = Math.max(right, rect.right);
      }
    }
    for (const element of content.book.querySelectorAll("img, table, figure")) {
      const rect = element.getBoundingClientRect();
      if (rect.width && rect.height) right = Math.max(right, rect.right);
    }
    content.book.style.transform = savedTransform;
    return Math.max(0, Math.floor((right - left - 0.5) / step));
  }

  async function verifyEmptyTailColumns() {
    if (reader.dataset.readingMode === "scroll") return;
    const anchor = pagination.captureOffset();
    const probes = [document.createElement("div"), document.createElement("div")];
    for (const probe of probes) {
      probe.dataset.emptyColumnProbe = "true";
      probe.style.setProperty("height", "1px", "important");
      probe.style.setProperty("break-before", "column", "important");
      probe.style.setProperty("break-after", "column", "important");
      probe.style.setProperty("visibility", "hidden", "important");
    }
    content.book.append(...probes);
    let evidence;
    try {
      await pagination.resizeViewport(anchor);
      const style = getComputedStyle(content.book);
      const step = parseFloat(style.width) + parseFloat(style.columnGap);
      const scrollPages = Math.max(
        1,
        Math.round((content.book.scrollWidth + parseFloat(style.columnGap)) / step),
      );
      const expectedPage = independentLastContentPage();
      evidence = Object.freeze({
        scrollPages,
        expectedPages: expectedPage + 1,
        pages: pagination.snapshot().pages,
      });
      assert(scrollPages > evidence.expectedPages, "sample-boundary");
      assert(evidence.pages === evidence.expectedPages, "sample-boundary");
    } finally {
      for (const probe of probes) probe.remove();
      await pagination.resizeViewport(anchor);
    }
    return evidence;
  }

  async function verifyNavigation() {
    const description = session.describe();
    const first = navigation.current();
    const serialized = locator.serialize(description, first);
    const parsed = locator.parse(description, serialized);
    assert(locator.compare(description, first, parsed) === 0, "sample-boundary");
    const ranged = locator.range(description, first.start, {
      section: first.start.section,
      offset: first.start.offset + 1,
    });
    assert(locator.compare(description, first, ranged) < 0, "sample-boundary");

    if (pagination.snapshot().pages > 1) await pagination.show(1);
    const reflowAnchor = navigation.current();
    const anchorAt40 = await navigation.setFontSize(40);
    assert(anchorAt40.start.offset === reflowAnchor.start.offset, "sample-boundary");
    assert(pagination.isOffsetVisible(anchorAt40.start.offset), "sample-boundary");
    const anchorAt16 = await navigation.setFontSize(16);
    assert(pagination.isOffsetVisible(anchorAt16.start.offset), "sample-boundary");
    const anchorAt19 = await navigation.setFontSize(19);
    assert(pagination.isOffsetVisible(anchorAt19.start.offset), "sample-boundary");

    let tocSection = null;
    let previousSection = null;
    let nextSection = null;
    if (description.toc.length > 1) {
      await navigation.goToToc(1);
      assert(navigation.snapshot().tocIndex === 1, "sample-boundary");
      tocSection = session.snapshot().currentSection;
      await pagination.show(0);
      await navigation.previous();
      previousSection = session.snapshot().currentSection;
      const previousPage = pagination.snapshot();
      assert(
        previousPage.page === previousPage.pages - 1 &&
          previousPage.page === pagination.contentPageCount() - 1,
        "sample-boundary",
      );
      await navigation.next();
      nextSection = session.snapshot().currentSection;
      const queued = await Promise.all([navigation.goToToc(1), navigation.goToToc(0)]);
      assert(queued.every(Boolean), "sample-boundary");
      assert(session.snapshot().currentSection === description.sections[0].id, "sample-boundary");
    }

    const invalidEnd = { ...ranged, end: { ...ranged.end, offset: 2147483647 } };
    assert(!(await navigation.goTo(invalidEnd)), "sample-boundary");
    assert(navigation.snapshot().lastFallback === "locator-offset", "sample-boundary");

    const invalid = JSON.parse(serialized);
    invalid.contentVersion = description.contentVersion === null ? "b".repeat(64) : null;
    assert(!(await navigation.goTo(JSON.stringify(invalid))), "sample-boundary");
    assert(navigation.snapshot().lastFallback === "locator-version", "sample-boundary");
    await navigation.goTo(serialized);
    await pagination.show(0);

    navigationEvidence = {
      locatorRoundTrip: true,
      rangeCompared: true,
      rangeBoundsChecked: true,
      reflowRestored: true,
      navigationSerialized: true,
      tocSection,
      previousSection,
      nextSection,
      fallback: "locator-version",
    };
  }

  async function verifyPreferences() {
    const initial = preferences.snapshot();
    const migrated = preferences.restore({
      application: {
        ...initial.application,
        marginTopPx: 80,
        marginRightPx: 24,
        marginBottomPx: 96,
        marginLeftPx: 40,
      },
      book: initial.book,
    });
    assert(
      ["marginTopPx", "marginRightPx", "marginBottomPx", "marginLeftPx"].every(
        (key) => !Object.hasOwn(migrated.application, key),
      ),
      "sample-boundary",
    );
    const legacyCss = ".book { --atha-legacy-style-probe: applied; }";
    const migratedBook = preferences.restore({
      application: initial.application,
      book: {
        sourceStyles: true,
        userStylesEnabled: true,
        userStylesheet: legacyCss,
      },
    });
    assert(
      migratedBook.book.styleModules.length === 1 &&
        migratedBook.book.styleModules[0].id === "legacy-user-css" &&
        migratedBook.book.styleModules[0].css === legacyCss,
      "sample-boundary",
    );
    const legacyUnicodeCss = `/*${"汉".repeat(12000)}*/`;
    const migratedUnicodeBook = preferences.restore({
      application: initial.application,
      book: {
        sourceStyles: true,
        userStylesEnabled: true,
        userStylesheet: legacyUnicodeCss,
      },
    });
    assert(
      migratedUnicodeBook.book.styleModules[0].enabled &&
        migratedUnicodeBook.book.styleModules[0].css === legacyUnicodeCss,
      "sample-boundary",
    );
    const oversizedLegacyCss = `/*${"汉".repeat(22000)}*/`;
    const migratedOversizedBook = preferences.restore({
      application: initial.application,
      book: {
        sourceStyles: true,
        userStylesEnabled: true,
        userStylesheet: oversizedLegacyCss,
      },
    });
    assert(
      !migratedOversizedBook.book.styleModules[0].enabled &&
        migratedOversizedBook.book.styleModules[0].css === oversizedLegacyCss,
      "sample-boundary",
    );
    preferences.restore(initial);
    if (pagination.snapshot().pages > 1) await pagination.show(1);
    const compactAnchor = await navigation.setPreferences("application", {
      theme: "dark",
      brightness: 80,
      fontSize: 16,
      fontFamily: "serif",
      density: "compact",
    });
    assert(pagination.isOffsetVisible(compactAnchor.start.offset), "sample-boundary");
    assert(document.documentElement.dataset.theme === "dark", "sample-boundary");
    assert(
      document.documentElement.style.getPropertyValue("--reader-brightness") === "0.8",
      "sample-boundary",
    );
    assert(content.book.dataset.fontFamily === "serif", "sample-boundary");
    const compactStyle = getComputedStyle(content.book);
    assert(compactStyle.backgroundColor === "rgb(26, 33, 30)", "sample-boundary");
    assert(compactStyle.color === "rgb(232, 236, 232)", "sample-boundary");
    assert(compactStyle.fontFamily.includes("Georgia"), "sample-boundary");
    assert(
      Math.abs(
        parseFloat(compactStyle.lineHeight) -
          Number(reader.dataset.fontPixels) * LINE_HEIGHT_RATIOS.compact,
      ) < 0.1,
      "sample-boundary",
    );
    const formula = content.book.querySelector("img.math-inline, img.math-display");
    const ordinary = content.book.querySelector("img:not(.math-inline):not(.math-display)");
    if (formula) assert(getComputedStyle(formula).filter !== "none", "sample-boundary");
    if (ordinary) assert(getComputedStyle(ordinary).filter === "none", "sample-boundary");
    assert(pagination.countCutRects(true) === 0, "layout-cut");

    const comfortableAnchor = await navigation.setPreferences("application", {
      theme: "light",
      fontSize: 40,
      fontFamily: "sans",
      density: "comfortable",
    });
    assert(pagination.isOffsetVisible(comfortableAnchor.start.offset), "sample-boundary");
    assert(document.documentElement.dataset.theme === "light", "sample-boundary");
    assert(content.book.dataset.fontFamily === "sans", "sample-boundary");
    const comfortableStyle = getComputedStyle(content.book);
    assert(comfortableStyle.backgroundColor === "rgb(255, 255, 255)", "sample-boundary");
    assert(comfortableStyle.color === "rgb(40, 43, 41)", "sample-boundary");
    assert(comfortableStyle.fontFamily.includes("Microsoft YaHei"), "sample-boundary");
    assert(
      Math.abs(
        parseFloat(comfortableStyle.lineHeight) -
          Number(reader.dataset.fontPixels) * LINE_HEIGHT_RATIOS.comfortable,
      ) < 0.1,
      "sample-boundary",
    );
    if (formula) assert(getComputedStyle(formula).filter === "none", "sample-boundary");
    if (ordinary) assert(getComputedStyle(ordinary).filter === "none", "sample-boundary");
    assert(pagination.countCutRects(true) === 0, "layout-cut");

    const bookAnchor = await navigation.setPreferences("book", { sourceStyles: false });
    assert(pagination.isOffsetVisible(bookAnchor.start.offset), "sample-boundary");
    assert(!content.styleSnapshot().bookStyleApplied, "sample-boundary");
    const userCss = ".book { --atha-user-style-probe: applied; }";
    const module = {
      id: "diagnostic-module",
      name: "Diagnostic",
      group: "Gate",
      enabled: true,
      css: userCss,
    };
    await navigation.setPreferences("book", {
      pageMargin: "wide",
      paragraphIndent: "two",
      paragraphSpacing: "comfortable",
      styleModules: [module],
    });
    assert(content.styleSnapshot().userStyleApplied, "sample-boundary");
    assert(
      getComputedStyle(content.book).getPropertyValue("--atha-user-style-probe").trim() === "applied" &&
        reader.style.getPropertyValue("--page-left-margin") === "48px",
      "sample-boundary",
    );
    await navigation.setPreferences("book", { userStylesEnabled: false });
    assert(!content.styleSnapshot().userStyleApplied, "sample-boundary");
    assert(
      !getComputedStyle(content.book).getPropertyValue("--atha-user-style-probe"),
      "sample-boundary",
    );
    await navigation.setPreferences("book", { userStylesEnabled: true });
    assert(content.styleSnapshot().userStyleApplied, "sample-boundary");
    assert(
      getComputedStyle(content.book).getPropertyValue("--atha-user-style-probe") === "applied",
      "sample-boundary",
    );

    let unsafeRejected = false;
    try {
      await navigation.setPreferences("book", {
        styleModules: [
          { ...module, css: ".book { background: url(https://example.com/x); }" },
        ],
      });
    } catch (error) {
      unsafeRejected = error instanceof Error && error.message === "css-subresource";
    }
    assert(unsafeRejected, "sample-boundary");
    assert(preferences.snapshot().book.styleModules[0].css === userCss, "sample-boundary");

    const styleBenchmark = preferences.benchmarkStyleModules();
    assert(
      styleBenchmark.modules === 32 &&
        styleBenchmark.bytes <= 65536 &&
        styleBenchmark.p95Ms < 50,
      "style-module-performance",
    );

    await navigation.resetPreferences("book");
    assert(content.styleSnapshot().bookStyleApplied, "sample-boundary");
    assert(!content.styleSnapshot().userStyleApplied, "sample-boundary");
    await navigation.resetPreferences("application");
    const restored = preferences.snapshot();
    assert(
      restored.application.theme === "system" &&
        restored.application.brightness === 100 &&
        restored.application.fontSize === 19 &&
        restored.application.fontFamily === "book" &&
        restored.application.density === "standard" &&
        restored.book.readingMode === "paged" &&
        restored.book.paragraphIndent === "none" &&
        !document.documentElement.dataset.theme &&
        document.documentElement.style.getPropertyValue("--reader-brightness") === "1" &&
        !content.book.dataset.fontFamily,
      "sample-boundary",
    );
    preferencesEvidence = {
      scopesSeparated: true,
      locatorRestored: true,
      sourceStylesToggled: true,
      userStylesApplied: true,
      userStylesToggled: true,
      unsafeStylesRejected: true,
      legacyMarginsDropped: true,
      legacyCssMigrated: true,
      moduleRollback: true,
      styleBenchmark,
    };
  }

  async function verifyInteraction() {
    const key = (value, target = document, options = {}) => {
      target.dispatchEvent(new KeyboardEvent("keydown", { key: value, bubbles: true, ...options }));
    };
    const pointer = (type, options, target = reader) =>
      target.dispatchEvent(
        new PointerEvent(type, { bubbles: true, pointerType: "mouse", isPrimary: true, ...options }),
      );
    const touch = (type, points, target) => {
      const event = new Event(type, { bubbles: true, composed: true });
      Object.defineProperties(event, {
        touches: { value: type === "touchend" ? [] : points },
        changedTouches: { value: points },
      });
      target.dispatchEvent(event);
    };
    const rect = reader.getBoundingClientRect();
    assert(getComputedStyle(reader).touchAction === "none", "sample-boundary");

    await pagination.show(0);
    key("ArrowRight");
    await navigation.idle();
    assert(pagination.snapshot().page === 1, "sample-boundary");
    key("PageUp");
    await navigation.idle();
    assert(pagination.snapshot().page === 0, "sample-boundary");

    const beforeProtected = interaction.snapshot();
    key("ArrowRight", document.querySelector("#font-size"));
    key("ArrowRight", document, { ctrlKey: true });
    key("ArrowRight", document, { shiftKey: true });
    key(" ", document, { shiftKey: true });
    await new Promise(requestAnimationFrame);
    assert(pagination.snapshot().page === 0, "sample-boundary");
    assert(
      interaction.snapshot().controlProtected === beforeProtected.controlProtected + 1,
      "sample-boundary",
    );

    for (const deltaY of [20, 20, 20, 20]) {
      reader.dispatchEvent(
        new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY, deltaMode: 0 }),
      );
    }
    await navigation.idle();
    assert(pagination.snapshot().page === 1, "sample-boundary");
    assert(interaction.snapshot().wheel === 1, "sample-boundary");

    await pagination.show(0);
    const right = rect.right - 20;
    pointer("pointerdown", { pointerId: 1, button: 0, clientX: right, clientY: rect.top + 40 });
    pointer("pointerup", { pointerId: 1, button: 0, clientX: right, clientY: rect.top + 40 });
    await navigation.idle();
    assert(pagination.snapshot().page === 1, "sample-boundary");
    const left = rect.left + 20;
    pointer("pointerdown", { pointerId: 1, button: 0, clientX: left, clientY: rect.top + 40 });
    pointer("pointerup", { pointerId: 1, button: 0, clientX: left, clientY: rect.top + 40 });
    await navigation.idle();
    assert(pagination.snapshot().page === 0, "sample-boundary");

    pointer("pointerdown", {
      pointerId: 2,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    const nativeRequestAnimationFrame = globalThis.requestAnimationFrame;
    const nativeCancelAnimationFrame = globalThis.cancelAnimationFrame;
    globalThis.requestAnimationFrame = () => 1;
    globalThis.cancelAnimationFrame = () => undefined;
    try {
      pointer(
        "pointermove",
        {
          pointerId: 2,
          pointerType: "touch",
          button: 0,
          clientX: rect.left + 150,
          clientY: rect.top + 84,
        },
        window,
      );
      pointer("pointerup", {
        pointerId: 2,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      });
    } finally {
      globalThis.requestAnimationFrame = nativeRequestAnimationFrame;
      globalThis.cancelAnimationFrame = nativeCancelAnimationFrame;
    }
    const releaseDelta =
      (rect.left + 80 - (rect.right - 80)) * (reader.clientWidth / rect.width);
    assert(
      releaseDelta < -48 &&
        Math.abs(new DOMMatrix(content.book.style.transform).m41 - releaseDelta) <= 1,
      "sample-boundary",
    );
    await navigation.idle();
    assert(
      pagination.snapshot().page === 1 && content.book.hasAttribute("data-swipe-settling"),
      "sample-boundary",
    );
    pointer("pointerdown", {
      pointerId: 13,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointermove",
      {
        pointerId: 13,
        pointerType: "touch",
        button: 0,
        clientX: rect.right - 150,
        clientY: rect.top + 84,
      },
      window,
    );
    assert(
      content.book.hasAttribute("data-swipe-dragging") &&
        !content.book.hasAttribute("data-swipe-settling"),
      "sample-boundary",
    );
    window.dispatchEvent(
      new PointerEvent("pointercancel", {
        bubbles: true,
        pointerId: 13,
        pointerType: "touch",
        isPrimary: true,
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 260));
    assert(
      !content.book.hasAttribute("data-swipe-dragging") &&
        !reader.hasAttribute("data-swipe-dragging"),
      "sample-boundary",
    );

    await pagination.show(0);
    pointer("pointerdown", {
      pointerId: 18,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointermove",
      {
        pointerId: 18,
        pointerType: "touch",
        button: 0,
        clientX: rect.right - 150,
        clientY: rect.top + 84,
      },
      window,
    );
    window.dispatchEvent(
      new PointerEvent("pointercancel", {
        bubbles: true,
        pointerId: 18,
        pointerType: "touch",
        isPrimary: true,
      }),
    );
    touch("touchmove", [{ identifier: 18, clientX: rect.left + 80, clientY: rect.top + 84 }], reader);
    touch("touchend", [{ identifier: 18, clientX: rect.left + 80, clientY: rect.top + 84 }], reader);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await navigation.idle();
    assert(
      pagination.snapshot().page === 1 &&
        !content.book.hasAttribute("data-swipe-dragging") &&
        !reader.hasAttribute("data-swipe-dragging"),
      "sample-boundary",
    );

    await pagination.show(0);
    pointer("pointerdown", {
      pointerId: 14,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointermove",
      {
        pointerId: 14,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      },
      window,
    );
    touch("touchend", [{ identifier: 14, clientX: rect.left + 80, clientY: rect.top + 84 }], reader);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await navigation.idle();
    assert(
      pagination.snapshot().page === 1 &&
        !content.book.hasAttribute("data-swipe-dragging") &&
        !reader.hasAttribute("data-swipe-dragging"),
      "sample-boundary",
    );

    await pagination.show(0);
    pointer("pointerdown", {
      pointerId: 16,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointermove",
      {
        pointerId: 16,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      },
      window,
    );
    touch("touchend", [{ identifier: 16, clientX: rect.left + 80, clientY: rect.top + 84 }], reader);
    pointer("pointerdown", {
      pointerId: 17,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointerup",
      {
        pointerId: 16,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      },
      window,
    );
    pointer(
      "pointermove",
      {
        pointerId: 17,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      },
      window,
    );
    pointer(
      "pointerup",
      {
        pointerId: 17,
        pointerType: "touch",
        button: 0,
        clientX: rect.left + 80,
        clientY: rect.top + 84,
      },
      window,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    await navigation.idle();
    assert(
      pagination.snapshot().page === 1 &&
        !content.book.hasAttribute("data-swipe-dragging") &&
        !reader.hasAttribute("data-swipe-dragging"),
      "sample-boundary",
    );

    await pagination.show(0);
    pointer("pointerdown", {
      pointerId: 15,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer(
      "pointermove",
      {
        pointerId: 15,
        pointerType: "touch",
        button: 0,
        clientX: rect.right - 150,
        clientY: rect.top + 84,
      },
      window,
    );
    touch("touchcancel", [], reader);
    assert(
      pagination.snapshot().page === 0 &&
        !content.book.hasAttribute("data-swipe-dragging") &&
        !reader.hasAttribute("data-swipe-dragging"),
      "sample-boundary",
    );

    await navigation.setPreferences("book", { readingMode: "scroll" });
    assert(content.book.dataset.readingMode === "scroll", "sample-boundary");
    assert(getComputedStyle(reader).touchAction === "pan-y", "sample-boundary");
    await pagination.show(0);
    const scrollProbe = document.createElement("p");
    scrollProbe.textContent = "Scrolled reading mode probe. ".repeat(240);
    content.book.append(scrollProbe);
    await pagination.resizeViewport(0);
    assert(reader.scrollHeight > reader.clientHeight, "sample-boundary");
    const protectedTouch = document.createElement("a");
    protectedTouch.href = "#protected-touch";
    protectedTouch.textContent = "Protected touch target";
    content.book.append(protectedTouch);
    reader.scrollTop = reader.scrollHeight;
    const protectedSection = session.snapshot().currentIndex;
    const protectedCount = interaction.snapshot().touch;
    touch("touchstart", [{ identifier: 1, clientX: 20, clientY: 100 }], protectedTouch);
    touch("touchend", [{ identifier: 1, clientX: 20, clientY: 20 }], protectedTouch);
    await navigation.idle();
    assert(
      session.snapshot().currentIndex === protectedSection &&
        interaction.snapshot().touch === protectedCount,
      "sample-boundary",
    );
    protectedTouch.remove();
    reader.scrollTop = 0;
    key("PageDown");
    await new Promise(requestAnimationFrame);
    assert(reader.scrollTop > 0, "sample-boundary");
    reader.scrollTop = Math.min(120, reader.scrollHeight - reader.clientHeight);
    reader.dispatchEvent(new Event("scroll"));
    await new Promise(requestAnimationFrame);
    assert(reader.scrollTop > 0, "sample-boundary");
    const beforeFontPreview = navigation.current().start.offset;
    const fontControl = document.querySelector("#font-size");
    for (const value of [20, 24, 28]) {
      fontControl.value = String(value);
      fontControl.dispatchEvent(new Event("input", { bubbles: true }));
    }
    await new Promise(requestAnimationFrame);
    fontControl.dispatchEvent(new Event("change", { bubbles: true }));
    await navigation.idle();
    const afterFontPreview = navigation.current().start.offset;
    assert(
      afterFontPreview === beforeFontPreview && pagination.isOffsetVisible(beforeFontPreview),
      "sample-boundary",
    );
    await navigation.setFontSize(19);
    if (session.snapshot().sections > 1) {
      reader.scrollTop = reader.scrollHeight - reader.clientHeight;
      reader.dispatchEvent(new Event("scroll"));
      await new Promise(requestAnimationFrame);
      assert(
        pagination.snapshot().page + 1 === pagination.snapshot().pages,
        "sample-boundary",
      );
      await navigation.next();
      assert(session.snapshot().currentIndex === 1, "sample-boundary");
      await session.open(0);
    }
    scrollProbe.remove();
    reader.scrollTop = 0;
    await pagination.resizeViewport(0);
    const disabledCounts = interaction.snapshot();
    pointer("pointerdown", { pointerId: 8, button: 0, clientX: right, clientY: rect.top + 40 });
    pointer("pointerup", { pointerId: 8, button: 0, clientX: right, clientY: rect.top + 40 });
    pointer("pointerdown", {
      pointerId: 9,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer("pointerup", {
      pointerId: 9,
      pointerType: "touch",
      button: 0,
      clientX: rect.left + 80,
      clientY: rect.top + 84,
    });
    await navigation.idle();
    assert(
      pagination.snapshot().page === 0 &&
        interaction.snapshot().mouse === disabledCounts.mouse &&
        interaction.snapshot().touch === disabledCounts.touch &&
        reader.dataset.readingMode === "scroll",
      "sample-boundary",
    );
    await navigation.setPreferences("book", { readingMode: "paged" });

    await pagination.show(0);
    document.documentElement.removeAttribute("data-reader-tools");
    const center = rect.left + rect.width / 2;
    pointer("pointerdown", {
      pointerId: 7,
      pointerType: "touch",
      button: 0,
      clientX: center,
      clientY: rect.top + 80,
    });
    pointer("pointerup", {
      pointerId: 7,
      pointerType: "touch",
      button: 0,
      clientX: center,
      clientY: rect.top + 80,
    });
    assert(document.documentElement.hasAttribute("data-reader-tools"), "sample-boundary");
    document.documentElement.removeAttribute("data-reader-tools");
    pointer("pointerdown", {
      pointerId: 10,
      pointerType: "",
      button: 0,
      clientX: center,
      clientY: rect.top + 80,
    });
    pointer("pointerup", {
      pointerId: 10,
      pointerType: "",
      button: 0,
      clientX: center,
      clientY: rect.top + 80,
    });
    assert(document.documentElement.hasAttribute("data-reader-tools"), "sample-boundary");
    assert(
      document.querySelector(".reader-controls").compareDocumentPosition(document.querySelector(".reader")) &
        Node.DOCUMENT_POSITION_FOLLOWING,
      "sample-boundary",
    );
    if (globalThis.AthaSystemBars?.getSafeAreaInsets) {
      const nativeInsets = JSON.parse(globalThis.AthaSystemBars.getSafeAreaInsets());
      const pageBox = document.querySelector("#page").getBoundingClientRect();
      const toolbar = document.querySelector(".top-toolbar");
      const toolbarBox = toolbar.getBoundingClientRect();
      const chapter = document.querySelector(".chapter-label");
      const chapterStyle = getComputedStyle(chapter);
      const pageScale = Number.parseFloat(
        getComputedStyle(document.documentElement).getPropertyValue("--page-scale"),
      );
      assert(chapterStyle.visibility === "hidden", "sample-boundary");
      assert(pageBox.top >= toolbarBox.bottom + 8, "sample-boundary");
      assert(!getComputedStyle(toolbar).backgroundColor.startsWith("rgba("), "sample-boundary");
      document.documentElement.removeAttribute("data-reader-tools");
      assert(
        chapter.getBoundingClientRect().top >= nativeInsets.top / devicePixelRatio + 8 &&
          Number.parseFloat(chapterStyle.fontSize) * pageScale >= 12,
        "sample-boundary",
      );
    }
    document.documentElement.removeAttribute("data-reader-tools");
    const walker = document.createTreeWalker(content.book, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) =>
        node.data.trim().length >= 2 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP,
    });
    const text = walker.nextNode();
    assert(text, "sample-boundary");
    const selection = content.book.getRootNode().getSelection?.();
    assert(selection, "sample-boundary");
    const range = document.createRange();
    range.setStart(text, 0);
    range.setEnd(text, 2);
    selection.removeAllRanges();

    const link = document.createElement("a");
    link.href = "#interaction-probe";
    link.textContent = "probe";
    link.hidden = true;
    content.book.append(link);
    const contentProtected = interaction.snapshot().contentProtected;
    pointer("pointerdown", { pointerId: 4, button: 0, clientX: right, clientY: rect.top + 120 }, link);
    pointer("pointerup", { pointerId: 4, button: 0, clientX: right, clientY: rect.top + 120 }, link);
    link.remove();
    await new Promise(requestAnimationFrame);
    assert(
      pagination.snapshot().page === 0 &&
        interaction.snapshot().contentProtected === contentProtected + 1,
      "sample-boundary",
    );

    pointer("pointerdown", {
      pointerId: 5,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer("pointerdown", {
      pointerId: 6,
      pointerType: "touch",
      isPrimary: false,
      button: 0,
      clientX: rect.right - 60,
      clientY: rect.top + 80,
    });
    pointer("pointerup", {
      pointerId: 5,
      pointerType: "touch",
      button: 0,
      clientX: rect.left + 80,
      clientY: rect.top + 84,
    });
    await new Promise(requestAnimationFrame);
    assert(
      pagination.snapshot().page === 0 && interaction.snapshot().multiTouchProtected === 1,
      "sample-boundary",
    );
    selection.addRange(range);
    const selectionCount = interaction.snapshot().selectionProtected;
    pointer("pointerdown", { pointerId: 3, button: 0, clientX: right, clientY: rect.top + 120 });
    pointer("pointerup", { pointerId: 3, button: 0, clientX: right, clientY: rect.top + 120 });
    await new Promise(requestAnimationFrame);
    assert(
      pagination.snapshot().page === 0 &&
        interaction.snapshot().selectionProtected === selectionCount + 1,
      "sample-boundary",
    );
    selection.removeAllRanges();

    if (session.snapshot().sections > 1) {
      await session.open(0);
      await pagination.show(pagination.snapshot().pages - 1);
      pointer("pointerdown", {
        pointerId: 11,
        pointerType: "",
        button: 0,
        clientX: rect.right - 20,
        clientY: rect.top + 80,
      });
      pointer("pointerup", {
        pointerId: 11,
        pointerType: "",
        button: 0,
        clientX: rect.right - 20,
        clientY: rect.top + 80,
      });
      await navigation.idle();
      assert(session.snapshot().currentIndex === 1, "sample-boundary");
      key("PageUp");
      await navigation.idle();
      assert(session.snapshot().currentIndex === 0, "sample-boundary");

      await session.open(session.snapshot().sections - 1);
      await pagination.show(pagination.snapshot().pages - 1);
      const finalTransform = content.book.style.transform;
      pointer("pointerdown", {
        pointerId: 12,
        pointerType: "",
        button: 0,
        clientX: rect.right - 80,
        clientY: rect.top + 80,
      });
      pointer(
        "pointermove",
        {
          pointerId: 12,
          pointerType: "",
          button: 0,
          clientX: rect.left + 80,
          clientY: rect.top + 84,
        },
        window,
      );
      assert(reader.dataset.swipeDragging === "true", "sample-boundary");
      pointer(
        "pointerup",
        {
          pointerId: 12,
          pointerType: "",
          button: 0,
          clientX: rect.left + 80,
          clientY: rect.top + 84,
        },
        window,
      );
      await navigation.idle();
      await new Promise(requestAnimationFrame);
      assert(
        session.snapshot().currentIndex === session.snapshot().sections - 1 &&
          !reader.dataset.swipeDragging &&
          content.book.style.transform === finalTransform,
        "sample-boundary",
      );
    }
    await session.open(0);
    await pagination.show(0);
    const counts = interaction.snapshot();
    interactionEvidence = {
      ...counts,
      keyboardVerified: counts.keyboard >= 2,
      wheelVerified: counts.wheel === 1,
      mouseVerified: counts.mouse === 2,
      touchVerified: counts.touch === 3,
      touchCenterVerified: true,
      selectionVerified: counts.selectionProtected === 1,
      controlsVerified: counts.controlProtected === 1,
      linksVerified: counts.contentProtected === 1,
      multiTouchVerified: counts.multiTouchProtected === 1,
    };
  }

  async function verifySections() {
    const initial = session.snapshot();
    assert(initial.state === "layout-stable" && initial.sections > 0, "sample-boundary");
    verifiedSections = [initial.currentSection];
    verifiedHeadings = [heading()];
    releasedSections = 0;
    for (let index = 1; index < Math.min(initial.sections, 3); index += 1) {
      const previousNodes = [...content.book.childNodes];
      await session.open(index);
      const current = session.snapshot();
      assert(current.state === "layout-stable" && current.currentIndex === index, "sample-boundary");
      assert(
        previousNodes.length > 0 && previousNodes.every((node) => !node.isConnected),
        "sample-boundary",
      );
      verifiedSections.push(current.currentSection);
      verifiedHeadings.push(heading());
      releasedSections += 1;
    }
    session.close();
    assert(
      session.snapshot().state === "closed" && content.book.childNodes.length === 0,
      "sample-boundary",
    );
    await session.open(0);
  }

  async function verifyImport() {
    await verifySections();
    const description = session.describe();
    const controlledCbzSection = () => {
      const current = session.snapshot();
      return (
        content.book.classList.contains("atha-cbz-section") &&
        /^\.atha-cbz\/page-\d{4}\.xhtml$/u.test(
          description.sections[current.currentIndex]?.href || "",
        )
      );
    };
    const verifyCbzImage = async () => {
      const image = content.book.querySelector("main.atha-cbz-page > img");
      assert(image, "image-load");
      const response = await fetch(image.src);
      const contentType = response.headers.get("content-type");
      await response.arrayBuffer();
      assert(response.ok && contentType === "image/png", "image-load");
    };
    if (controlledCbzSection()) {
      assert(description.sections.length === 4, "sample-boundary");
      await verifyCbzImage();
      for (let index = 1; index < description.sections.length; index += 1) {
        assert(await navigation.next(), "sample-boundary");
        assert(session.snapshot().currentIndex === index && controlledCbzSection(), "sample-boundary");
        if (index === 2) {
          assert(
            content.book.querySelector(
              ".atha-cbz-page-error[role='img'][aria-label='图片无法显示']",
            ),
            "image-load",
          );
        } else {
          await verifyCbzImage();
        }
      }
    }
    await securityProbe();
  }

  async function verify() {
    await verifySections();
    await verifyNavigation();
    await verifyPreferences();
    await verifyInteraction();
    contentActionEvidence = {
      ...(await contentActions.verify({ pagination, session })),
      ...(await structuredActions.verify({ pagination, session })),
    };
    stateEvidence = await readerState.verify();
    bookmarkEvidence = await bookmarks.verify();
    searchEvidence = await search.verify();
    annotationEvidence = await annotations.verify();
    await pagination.verifySizes();
    pagination.verifyFormulaLayout();
    pagination.verifyDisplayGeometry();
    await securityProbe();
  }

  async function benchmark() {
    await content.warmRemaining();
    await pagination.resizeViewport(pagination.captureOffset());
    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await renderCachedSource();
      const state = pagination.snapshot();
      emit(
        `metric|hot_open|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}|${reader.clientWidth}|${reader.clientHeight}`,
      );
    }

    assert(pagination.snapshot().pages > 1, "layout-cut");
    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await pagination.show(sample % 2);
      const state = pagination.snapshot();
      emit(
        `metric|page_turn|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}|${reader.clientWidth}|${reader.clientHeight}`,
      );
      assert(pagination.countCutRects() === 0, "layout-cut");
    }

    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await pagination.setFontSize(sample % 2 ? 40 : 16);
      const state = pagination.snapshot();
      emit(
        `metric|font_reflow|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}|${reader.clientWidth}|${reader.clientHeight}`,
      );
    }
    await pagination.setFontSize(19);
    document.querySelector("#font-size").value = "19";
  }

  function visualSnapshot() {
    const book = content.book;
    const formulas = [...book.querySelectorAll("img.math-inline, img.math-display")];
    const ordinary = [...book.querySelectorAll("img:not(.math-inline):not(.math-display)")];
    const standaloneFormulas = formulas.filter((image) => image.getAttribute("role") === "button");
    const standaloneOrdinary = ordinary.filter((image) => image.getAttribute("role") === "button");
    const ordinaryPngCount = ordinary.filter((image) =>
      new URL(image.src).pathname.toLowerCase().endsWith(".png"),
    ).length;
    return {
      status: document.documentElement.dataset.status || null,
      error: document.documentElement.dataset.error || null,
      dark: matchMedia("(prefers-color-scheme: dark)").matches,
      pages: pagination.snapshot().pages,
      fontSize: pagination.snapshot().fontSize,
      fontPixels: getComputedStyle(book).fontSize,
      readingMode: reader.dataset.readingMode,
      columnCount: getComputedStyle(book).columnCount,
      scrollable: reader.scrollHeight > reader.clientHeight,
      swipeDragging: book.hasAttribute("data-swipe-dragging"),
      formulaCount: formulas.length,
      ordinaryCount: ordinary.length,
      standaloneFormulaCount: standaloneFormulas.length,
      standaloneOrdinaryCount: standaloneOrdinary.length,
      ordinaryPngCount,
      tableCount: book.querySelectorAll("table").length,
      codeBlockCount: book.querySelectorAll("pre").length,
      structuredLinkCount: book.querySelectorAll("table a[href], pre a[href]").length,
      foreground: getComputedStyle(book).color,
      background: getComputedStyle(reader).backgroundColor,
      formulaFilters: [...new Set(formulas.map((image) => getComputedStyle(image).filter))],
      ordinaryFilters: [...new Set(ordinary.map((image) => getComputedStyle(image).filter))],
      session: {
        ...session.snapshot(),
        verifiedSections: [...verifiedSections],
        verifiedHeadings: [...verifiedHeadings],
        releasedSections,
      },
      navigation: { ...navigation.snapshot(), ...navigationEvidence },
      preferences: {
        ...preferences.snapshot(),
        ...preferencesEvidence,
        styles: content.styleSnapshot(),
      },
      interaction: { ...interactionEvidence },
      contentActions: {
        ...contentActionEvidence,
        ...contentActions.snapshot(),
        ...structuredActions.snapshot(),
        selectionLength: contentActions.selectionLength(),
      },
      resources: content.resourceSnapshot(),
      readerState: { ...readerState.snapshot(), ...stateEvidence },
      readingStatistics: readingStatistics.snapshot(),
      bookmarks: { items: bookmarks.snapshot(), ...bookmarkEvidence },
      search: { ...search.snapshot(), ...searchEvidence },
      annotations: { ...annotations.snapshot(), ...annotationEvidence },
    };
  }

  function gesturePageState() {
    const page = pagination.snapshot();
    return Object.freeze({
      section: session.snapshot().currentIndex,
      page: page.page,
      pages: page.pages,
    });
  }

  async function openGestureSection(href, warm = true) {
    const description = session.describe();
    const sectionIndex = description.sections.findIndex((section) => section.href === href);
    assert(sectionIndex >= 0, "sample-boundary");
    await navigation.goTo(locator.point(description, description.sections[sectionIndex].id, 0));
    let formulas = [...content.book.querySelectorAll("img.math-inline, img.math-display")];
    if (warm) {
      await content.warmRemaining();
      const deadline = performance.now() + 10_000;
      while (
        formulas.some((image) => !image.complete || image.naturalWidth <= 0) &&
        performance.now() < deadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      await navigation.resize();
    }
    await pagination.nextFrame();
    formulas = [...content.book.querySelectorAll("img.math-inline, img.math-display")];
    const style = getComputedStyle(content.book);
    const step = parseFloat(style.width) + parseFloat(style.columnGap);
    return Object.freeze({
      section: sectionIndex,
      formulas: formulas.length,
      settledFormulas: formulas.filter((image) => image.complete && image.naturalWidth > 0).length,
      pages: pagination.snapshot().pages,
      nativePagedScroll: pagination.snapshot().nativePagedScroll,
      contentPages: independentLastContentPage() + 1,
      scrollPages: Math.max(
        1,
        Math.round((content.book.scrollWidth + parseFloat(style.columnGap)) / step),
      ),
    });
  }

  async function previousBoundaryProbe() {
    const emptyTail = await verifyEmptyTailColumns();
    const before = gesturePageState();
    await pagination.show(0);
    pagination.previewSwipe(80);
    pagination.finishSwipe(80);
    const moved = await navigation.previous();
    const after = gesturePageState();
    return Object.freeze({
      moved,
      sectionDelta: after.section - before.section,
      page: after.page,
      pages: after.pages,
      lastContentPage: independentLastContentPage(),
      emptyTail,
      settling: content.book.hasAttribute("data-swipe-settling"),
    });
  }

  async function scrollResourceProbe() {
    const origin = navigation.current();
    let evidence = Object.freeze({ candidate: false });
    try {
      await navigation.setPreferences("book", { readingMode: "scroll" });
      const viewport = reader.getBoundingClientRect();
      const target = [...content.book.querySelectorAll("img.atha-resource-pending")].find(
        (image) => image.getBoundingClientRect().top > viewport.bottom + 1,
      );
      if (!target) return evidence;
      const pendingBefore = content.resourceSnapshot().pending;
      const targetRect = target.getBoundingClientRect();
      reader.scrollTop += targetRect.top - viewport.top - 8;
      reader.dispatchEvent(new Event("scroll"));
      const offsetBefore = navigation.current().start.offset;
      const deadline = performance.now() + 10_000;
      while (
        (target.classList.contains("atha-resource-pending") ||
          !target.complete ||
          target.naturalWidth <= 0) &&
        performance.now() < deadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      await new Promise((resolve) => setTimeout(resolve, 60));
      await navigation.idle();
      await pagination.nextFrame();
      const settledOffset = navigation.current().start.offset;
      await navigation.resize();
      evidence = Object.freeze({
        candidate: true,
        loaded:
          !target.classList.contains("atha-resource-pending") &&
          target.complete &&
          target.naturalWidth > 0,
        pendingBefore,
        pendingAfter: content.resourceSnapshot().pending,
        offsetPreserved: settledOffset === offsetBefore,
        relayoutPreserved:
          navigation.current().start.offset === settledOffset &&
          pagination.isOffsetVisible(settledOffset),
      });
      return evidence;
    } finally {
      await navigation.setPreferences("book", { readingMode: "paged" });
      await navigation.goTo(origin);
    }
  }

  function gestureTable(wide) {
    const wrapper = document.createElement("div");
    wrapper.className = "atha-structured-overflow";
    wrapper.dataset.gestureOverflow = String(wide);
    wrapper.style.setProperty("width", "100%", "important");
    wrapper.style.setProperty("height", "112px", "important");
    wrapper.style.setProperty("overflow", "auto", "important");
    wrapper.style.setProperty("pointer-events", "auto", "important");

    const table = document.createElement("table");
    table.tabIndex = 0;
    table.setAttribute("aria-label", wide ? "Gesture overflow table" : "Gesture table");
    table.style.setProperty("width", wide ? "1200px" : "100%", "important");
    table.style.setProperty("height", "104px", "important");
    table.style.setProperty("border-collapse", "collapse", "important");
    table.style.setProperty("pointer-events", "auto", "important");
    for (let rowIndex = 0; rowIndex < 2; rowIndex += 1) {
      const row = document.createElement("tr");
      for (let columnIndex = 0; columnIndex < (wide ? 10 : 4); columnIndex += 1) {
        const cell = document.createElement("td");
        cell.textContent = String((rowIndex + 1) * (columnIndex + 1));
        cell.style.setProperty("min-width", wide ? "116px" : "auto", "important");
        cell.style.setProperty("border", "1px solid currentColor", "important");
        cell.style.setProperty("padding", "4px", "important");
        row.append(cell);
      }
      table.append(row);
    }
    wrapper.append(table);
    return Object.freeze({ target: table, wrapper });
  }

  function gestureBlock(kind, child) {
    const block = document.createElement("section");
    block.dataset.gestureTarget = kind;
    block.style.setProperty("box-sizing", "border-box", "important");
    block.style.setProperty("width", "100%", "important");
    block.style.setProperty("min-height", "136px", "important");
    block.style.setProperty("padding", "8px 0", "important");
    block.style.setProperty("break-before", "column", "important");
    block.style.setProperty("break-after", "column", "important");
    block.style.setProperty("break-inside", "avoid", "important");
    const anchor = document.createElement("span");
    anchor.textContent = "0";
    anchor.setAttribute("aria-hidden", "true");
    anchor.style.setProperty("display", "block", "important");
    anchor.style.setProperty("height", "1px", "important");
    anchor.style.setProperty("overflow", "hidden", "important");
    block.append(anchor, child);
    return block;
  }

  async function installGestureFixture() {
    if (gestureFixture?.isConnected) return;
    if (!gestureOrigin) gestureOrigin = navigation.current();

    const description = session.describe();
    let source = content.book.querySelector("img[src]");
    for (let index = 0; !source && index < description.sections.length; index += 1) {
      await navigation.goTo(locator.point(description, description.sections[index].id, 0));
      source = content.book.querySelector("img[src]");
    }
    assert(source, "sample-boundary");

    const fixture = document.createElement("section");
    fixture.dataset.gestureFixture = "true";
    fixture.style.setProperty("width", "100%", "important");

    const ordinary = document.createElement("img");
    ordinary.src = source.src;
    ordinary.alt = "";
    ordinary.draggable = false;
    ordinary.setAttribute("role", "button");
    ordinary.tabIndex = 0;
    ordinary.style.setProperty("display", "block", "important");
    ordinary.style.setProperty("width", "100%", "important");
    ordinary.style.setProperty("height", "112px", "important");
    ordinary.style.setProperty("pointer-events", "auto", "important");

    const formula = document.createElement("img");
    formula.src = source.src;
    formula.alt = "";
    formula.draggable = false;
    formula.className = "math-display";
    formula.setAttribute("role", "button");
    formula.setAttribute("width", "480");
    formula.setAttribute("height", "96");
    formula.tabIndex = 0;
    formula.style.setProperty("pointer-events", "auto", "important");

    const regularTable = gestureTable(false);
    const overflowTable = gestureTable(true);
    const records = [
      ["ordinary", ordinary, ordinary, null],
      ["formula", formula, formula, null],
      ["table", regularTable.wrapper, regularTable.target, regularTable.wrapper],
      ["overflow-table", overflowTable.wrapper, overflowTable.target, overflowTable.wrapper],
    ];
    for (const [kind, child, target, wrapper] of records) {
      const block = gestureBlock(kind, child);
      fixture.append(block);
      gestureTargets.set(kind, Object.freeze({ block, target, wrapper }));
    }
    const tail = document.createElement("p");
    tail.textContent = Array.from({ length: 600 }, () => "0").join(" ");
    tail.style.setProperty("break-before", "column", "important");
    fixture.append(tail);
    content.book.append(fixture);
    gestureFixture = fixture;
    await Promise.all([ordinary.decode(), formula.decode()]);
    await navigation.resize();
    assert(pagination.snapshot().pages > 1, "sample-boundary");
  }

  async function showGestureTarget(record) {
    const previousId = record.block.getAttribute("id");
    const probeId = `atha-gesture-${crypto.randomUUID()}`;
    record.block.id = probeId;
    const offset = pagination.offsetForFragment(probeId);
    if (previousId === null) record.block.removeAttribute("id");
    else record.block.id = previousId;
    assert(offset !== null, "sample-boundary");
    const description = session.describe();
    assert(
      await navigation.goTo(
        locator.point(description, session.snapshot().currentSection, offset),
      ),
      "sample-boundary",
    );
    await pagination.nextFrame();
    await pagination.nextFrame();
  }

  function stopGestureTrace(trace) {
    if (!trace) return;
    cancelAnimationFrame(trace.frameRequest);
    content.book.removeEventListener("pointerdown", trace.onPointerDown);
    content.book.removeEventListener("click", trace.onCompatibilityEvent);
    content.book.removeEventListener("dblclick", trace.onCompatibilityEvent);
    window.removeEventListener("pointermove", trace.onPointerMove);
    window.removeEventListener("pointerup", trace.onPointerUp);
    window.removeEventListener("pointercancel", trace.onPointerCancel);
    if (activeGesture === trace) activeGesture = null;
  }

  function gestureSinglePage(before, after, direction) {
    if (before.section === after.section) return after.page === before.page + direction;
    if (direction > 0) {
      return (
        after.section === before.section + 1 &&
        before.page === before.pages - 1 &&
        after.page === 0
      );
    }
    return (
      after.section === before.section - 1 &&
      before.page === 0 &&
      after.page === after.pages - 1
    );
  }

  function nearestRank(values, ratio) {
    if (values.length === 0) return null;
    const sorted = [...values].sort((left, right) => left - right);
    return sorted[Math.max(0, Math.ceil(sorted.length * ratio) - 1)];
  }

  async function beginGestureProbe(kind, action, overflowMode = "edge", direction = 1) {
    assert(["ordinary", "formula", "table", "overflow-table"].includes(kind), "sample-boundary");
    assert(["tap", "drag"].includes(action), "sample-boundary");
    assert(["edge", "pan", "vertical"].includes(overflowMode), "sample-boundary");
    assert(direction === -1 || direction === 1, "sample-boundary");
    stopGestureTrace(activeGesture);
    await installGestureFixture();
    const record = gestureTargets.get(kind);
    assert(record?.target?.isConnected, "sample-boundary");

    const dialog = document.querySelector("#content-dialog");
    if (dialog.open) dialog.close();
    contentActions.clearSelection();
    await showGestureTarget(record);

    const before = gesturePageState();
    const description = session.describe();
    assert(
      (direction > 0 && (before.page + 1 < before.pages || before.section + 1 < description.sections.length)) ||
        (direction < 0 && (before.page > 0 || before.section > 0)),
      "sample-boundary",
    );

    if (record.wrapper) {
      const maximum = Math.max(0, record.wrapper.scrollWidth - record.wrapper.clientWidth);
      if (kind === "overflow-table" && overflowMode === "pan") {
        record.wrapper.scrollLeft = Math.round(maximum / 2);
      } else if (kind === "overflow-table") {
        record.wrapper.scrollLeft = direction > 0 ? maximum : 0;
      } else {
        record.wrapper.scrollLeft = 0;
      }
    }
    await pagination.nextFrame();

    const viewport = reader.getBoundingClientRect();
    const targetRect = record.target.getBoundingClientRect();
    const visibleLeft = Math.max(viewport.left, targetRect.left);
    const visibleRight = Math.min(viewport.right, targetRect.right);
    const visibleTop = Math.max(viewport.top, targetRect.top);
    const visibleBottom = Math.min(viewport.bottom, targetRect.bottom);
    const startX = Math.round(viewport.left + viewport.width * (direction > 0 ? 0.8 : 0.2));
    const endX = Math.round(
      overflowMode === "vertical"
        ? startX + viewport.width * 0.05
        : overflowMode === "pan"
          ? startX + viewport.width * (direction > 0 ? -0.2 : 0.2)
        : viewport.left + viewport.width * (direction > 0 ? 0.2 : 0.8),
    );
    const candidateYs = [...record.target.querySelectorAll("*"), record.target]
      .flatMap((element) => [...element.getClientRects()])
      .filter(
        (rect) =>
          rect.height > 4 &&
          startX > rect.left + 1 &&
          startX < rect.right - 1 &&
          rect.bottom > viewport.top + 1 &&
          rect.top < viewport.bottom - 1,
      )
      .map((rect) => Math.round((Math.max(rect.top, viewport.top) + Math.min(rect.bottom, viewport.bottom)) / 2));
    const startY = candidateYs[0];
    const endY =
      overflowMode === "vertical"
        ? Math.round(startY + (startY < viewport.top + viewport.height / 2 ? 96 : -96))
        : startY;
    assert(
      visibleRight - visibleLeft > 8 &&
        visibleBottom - visibleTop > 8 &&
        startX > visibleLeft + 1 &&
        startX < visibleRight - 1 &&
        startY !== undefined,
      "sample-boundary",
    );

    const pageScroller = reader.querySelector("#page");
    assert(pageScroller, "sample-boundary");
    const trace = {
      id: ++gestureSequence,
      action,
      direction,
      record,
      before,
      baselineTransform: content.book.style.transform,
      pageScroller,
      baselinePageScrollLeft: pageScroller.scrollLeft,
      baselineScrollLeft: record.wrapper?.scrollLeft || 0,
      scrollVisualScale: reader.clientWidth > 0 ? viewport.width / reader.clientWidth : 1,
      baselinePreview: dialog.open,
      pointerId: null,
      pointerDownAt: null,
      pointerUpAt: null,
      targetHit: false,
      events: [],
      frames: [],
      compatibilityEvents: 0,
      frameRequest: 0,
    };
    const recordEvent = (event) => {
      trace.events.push({
        type: event.type,
        at: performance.now(),
        trusted: event.isTrusted,
        pointerType: event.pointerType,
      });
    };
    trace.onPointerDown = (event) => {
      if (trace.pointerId !== null) return;
      trace.pointerId = event.pointerId;
      trace.pointerDownAt = performance.now();
      trace.targetHit = event.composedPath().includes(record.target);
      recordEvent(event);
    };
    trace.onPointerMove = (event) => {
      if (event.pointerId === trace.pointerId) recordEvent(event);
    };
    trace.onPointerUp = (event) => {
      if (event.pointerId !== trace.pointerId) return;
      trace.pointerUpAt = performance.now();
      recordEvent(event);
    };
    trace.onPointerCancel = (event) => {
      if (event.pointerId !== trace.pointerId) return;
      trace.pointerUpAt = performance.now();
      recordEvent(event);
    };
    trace.onCompatibilityEvent = (event) => {
      if (event.composedPath().includes(record.target)) trace.compatibilityEvents += 1;
    };
    const sampleFrame = (at) => {
      const page = trace.action === "drag" ? trace.before : gesturePageState();
      trace.frames.push({
        at,
        transform: content.book.style.transform,
        pageScrollLeft: trace.pageScroller?.scrollLeft || 0,
        section: page.section,
        page: page.page,
        preview: dialog.open,
        scrollLeft: record.wrapper?.scrollLeft || 0,
      });
      trace.frameRequest = requestAnimationFrame(sampleFrame);
    };
    content.book.addEventListener("pointerdown", trace.onPointerDown);
    content.book.addEventListener("click", trace.onCompatibilityEvent);
    content.book.addEventListener("dblclick", trace.onCompatibilityEvent);
    window.addEventListener("pointermove", trace.onPointerMove);
    window.addEventListener("pointerup", trace.onPointerUp);
    window.addEventListener("pointercancel", trace.onPointerCancel);
    trace.frameRequest = requestAnimationFrame(sampleFrame);
    activeGesture = trace;
    return Object.freeze({
      id: trace.id,
      x: startX,
      y: startY,
      endX,
      endY,
      direction,
    });
  }

  async function finishGestureProbe(id) {
    const trace = activeGesture;
    assert(trace?.id === Number(id), "sample-boundary");
    await navigation.idle();
    const deadline = performance.now() + 1500;
    let settledFrames = 0;
    let previousVisual = null;
    while (performance.now() < deadline && settledFrames < 2) {
      await pagination.nextFrame();
      const transform = getComputedStyle(content.book).transform;
      const pageScrollLeft = trace.pageScroller?.scrollLeft || 0;
      const ready =
        !content.book.hasAttribute("data-swipe-dragging") &&
        !content.book.hasAttribute("data-swipe-settling") &&
        !reader.dataset.swipeDragging;
      const visual = `${transform}:${pageScrollLeft}`;
      settledFrames = ready && visual === previousVisual ? settledFrames + 1 : 0;
      previousVisual = visual;
    }
    const stableAt = performance.now();
    const after = gesturePageState();
    const scrollAfter = trace.record.wrapper?.scrollLeft || 0;
    const dialog = document.querySelector("#content-dialog");
    stopGestureTrace(trace);

    const pointerEvents = trace.events.filter((event) => event.type.startsWith("pointer"));
    const pointerMoves = pointerEvents.filter((event) => event.type === "pointermove");
    const firstInputAt = pointerMoves[0]?.at ?? trace.pointerDownAt;
    const relevantFrames = trace.frames.filter(
      (frame) => trace.pointerDownAt === null || frame.at >= trace.pointerDownAt,
    );
    const visual = relevantFrames.find(
      (frame) =>
        frame.transform !== trace.baselineTransform ||
        frame.pageScrollLeft !== trace.baselinePageScrollLeft ||
        frame.section !== trace.before.section ||
        frame.page !== trace.before.page ||
        frame.preview !== trace.baselinePreview ||
        frame.scrollLeft !== trace.baselineScrollLeft,
    );
    const dragFrames = relevantFrames.filter(
      (frame) => trace.pointerUpAt === null || frame.at <= trace.pointerUpAt,
    );
    const visualFrames = dragFrames.reduce((values, frame) => {
      const previous = values.at(-1);
      if (
        (!previous &&
          (frame.transform !== trace.baselineTransform ||
            frame.pageScrollLeft !== trace.baselinePageScrollLeft ||
            frame.scrollLeft !== trace.baselineScrollLeft)) ||
        (previous &&
          (frame.transform !== previous.transform ||
            frame.pageScrollLeft !== previous.pageScrollLeft ||
            frame.scrollLeft !== previous.scrollLeft))
      ) {
        values.push(frame);
      }
      return values;
    }, []);
    const frameIntervals = dragFrames
      .slice(1)
      .map((frame, index) => frame.at - dragFrames[index].at)
      .filter((value) => value >= 0);
    const distinctRafTransforms = dragFrames.reduce((values, frame) => {
      const visual = `${frame.transform}:${frame.pageScrollLeft}`;
      if (values.at(-1) !== visual) values.push(visual);
      return values;
    }, []);
    return Object.freeze({
      singlePage: gestureSinglePage(trace.before, after, trace.direction),
      samePage: trace.before.section === after.section && trace.before.page === after.page,
      targetHit: trace.targetHit,
      pointerTypes: Object.freeze([...new Set(pointerEvents.map((event) => event.pointerType))]),
      trusted:
        pointerEvents.length >= 2 && pointerEvents.every((event) => event.trusted === true),
      touch:
        pointerEvents.length >= 2 && pointerEvents.every((event) => event.pointerType === "touch"),
      pointerMoves: pointerMoves.length,
      compatibilityEvents: trace.compatibilityEvents,
      preview: dialog.open,
      scrollDelta: (scrollAfter - trace.baselineScrollLeft) * trace.scrollVisualScale,
      visualUpdateSamples: visualFrames.length,
      rafTransformSamples: Math.max(0, distinctRafTransforms.length - 1),
      settled: settledFrames >= 2,
      timing: Object.freeze({
        inputToReleaseMs:
          trace.pointerUpAt !== null && firstInputAt !== null
            ? trace.pointerUpAt - firstInputAt
            : null,
        inputToFirstVisualMs:
          visual && firstInputAt !== null ? visual.at - firstInputAt : null,
        releaseToFirstVisualMs:
          visual && trace.pointerUpAt !== null ? Math.max(0, visual.at - trace.pointerUpAt) : null,
        releaseToStableMs:
          trace.pointerUpAt !== null ? stableAt - trace.pointerUpAt : null,
        frameP95Ms: nearestRank(frameIntervals, 0.95),
        maxFrameMs: frameIntervals.length > 0 ? Math.max(...frameIntervals) : null,
      }),
    });
  }

  async function cleanupGestureProbe() {
    stopGestureTrace(activeGesture);
    const origin = gestureOrigin;
    if (origin) await navigation.goTo(origin);
    if (gestureFixture?.isConnected) {
      gestureFixture.remove();
      await navigation.resize();
      if (origin) await navigation.goTo(origin);
    }
    gestureFixture = null;
    gestureOrigin = null;
    gestureTargets = new Map();
    const dialog = document.querySelector("#content-dialog");
    if (dialog.open) dialog.close();
    return true;
  }

  function mediaSource(kind) {
    const selector = {
      formula: "img[role='button'].math-inline, img[role='button'].math-display",
      ordinary: "img[role='button']:not(.math-inline):not(.math-display)",
      table: "table",
      code: "pre",
    }[kind];
    return content.book.querySelector(selector);
  }

  async function showSource(source) {
    const previousId = source.getAttribute("id");
    const probeId = `atha-media-${crypto.randomUUID()}`;
    source.id = probeId;
    const offset = pagination.offsetForFragment(probeId);
    if (previousId === null) source.removeAttribute("id");
    else source.id = previousId;
    assert(offset !== null, "sample-boundary");
    const description = session.describe();
    assert(
      await navigation.goTo(
        locator.point(description, session.snapshot().currentSection, offset),
      ),
      "sample-boundary",
    );
  }

  async function mediaPoint(kind) {
    const source = mediaSource(kind);
    if (!source) return null;
    await showSource(source);
    const rect = source.getBoundingClientRect();
    const viewport = reader.getBoundingClientRect();
    assert(
      rect.width > 0 &&
        rect.height > 0 &&
        rect.left >= viewport.left &&
        rect.right <= viewport.right &&
        rect.top >= viewport.top &&
        rect.bottom <= viewport.bottom,
      "sample-boundary",
    );
    return Object.freeze({
      x: kind === "code" ? rect.left + 2 : rect.left + rect.width / 2,
      y: kind === "code" ? rect.top + 2 : rect.top + rect.height / 2,
    });
  }

  async function wheelProbe() {
    const pageState = () =>
      Object.freeze({
        section: session.snapshot().currentIndex,
        ...pagination.snapshot(),
      });
    const reset = async () => {
      const description = session.describe();
      await navigation.goTo(locator.point(description, description.sections[0].id, 0));
    };
    const fire = async (target, deltaY) => {
      const before = pageState();
      const started = performance.now();
      const event = new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        deltaY,
        deltaMode: 0,
      });
      target.dispatchEvent(event);
      await navigation.idle();
      const after = pageState();
      const singleStep =
        deltaY > 0
          ? (after.section === before.section && after.page === before.page + 1) ||
            (after.section === before.section + 1 &&
              before.page === before.pages - 1 &&
              after.page === 0)
          : (after.section === before.section && after.page === before.page - 1) ||
            (after.section === before.section - 1 &&
              before.page === 0 &&
              after.page === after.pages - 1);
      return Object.freeze({
        accepted: after.section !== before.section || after.page !== before.page,
        defaultPrevented: event.defaultPrevented,
        latencyMs: performance.now() - started,
        singleStep,
      });
    };
    const targets = {};
    for (const [kind, selector] of Object.entries({
      ordinary: "img[role='button']:not(.math-inline):not(.math-display)",
      formula: "img[role='button'].math-inline, img[role='button'].math-display",
      linked: "a[href] img",
    })) {
      await reset();
      let linkedProbe = null;
      let source = content.book.querySelector(selector);
      if (!source && kind === "linked") {
        const image = content.book.querySelector("img")?.cloneNode(false);
        if (image) {
          image.removeAttribute("id");
          linkedProbe = document.createElement("a");
          linkedProbe.href = "#atha-wheel-linked-probe";
          linkedProbe.style.position = "fixed";
          linkedProbe.style.left = "-9999px";
          linkedProbe.append(image);
          content.book.append(linkedProbe);
          source = image;
        }
      }
      if (!source) {
        targets[kind] = Object.freeze({ present: false });
        continue;
      }
      await showSource(source);
      await new Promise((resolve) => setTimeout(resolve, 260));
      const state = pagination.snapshot();
      const description = session.describe();
      const forward =
        state.page + 1 < state.pages || session.snapshot().currentIndex + 1 < description.sections.length;
      targets[kind] = Object.freeze({
        present: true,
        synthetic: kind === "linked" && Boolean(linkedProbe),
        ...(await fire(source, forward ? 100 : -100)),
      });
      linkedProbe?.remove();
    }

    await reset();
    await new Promise((resolve) => setTimeout(resolve, 260));
    const repeated = [];
    for (let index = 0; index < 4; index += 1) {
      const result = await fire(reader, 100);
      repeated.push(result);
      await new Promise((resolve) => setTimeout(resolve, Math.max(0, 100 - result.latencyMs)));
    }
    const acceptedLatencies = repeated
      .filter((result) => result.accepted)
      .map((result) => result.latencyMs)
      .sort((left, right) => left - right);
    const p95 = acceptedLatencies[Math.max(0, Math.ceil(acceptedLatencies.length * 0.95) - 1)] ?? null;
    await reset();
    return Object.freeze({
      targets: Object.freeze(targets),
      repeatedInputs: repeated.length,
      repeatedAccepted: repeated.filter((result) => result.accepted).length,
      repeatedDefaultPrevented: repeated.filter((result) => result.defaultPrevented).length,
      repeatedSingleStep: repeated.filter((result) => result.singleStep).length,
      inputToStableP95Ms: p95,
    });
  }

  function focusMedia(kind) {
    const source = mediaSource(kind);
    if (!source) return false;
    source.focus({ preventScroll: true });
    return content.book.getRootNode().activeElement === source;
  }

  function previewState() {
    return Object.freeze({
      open: document.querySelector("#content-dialog").open,
      focusRestored: content.book
        .getRootNode()
        .activeElement?.matches("img[role='button'], table, pre") === true,
    });
  }

  function verifyHostPolicy() {
    if (!window.__TAURI_INTERNALS__) return;
    const policy = document.permissionsPolicy?.allowsFeature
      ? document.permissionsPolicy
      : document.featurePolicy;
    if (!policy?.allowsFeature) return;
    assert(
      ["camera", "display-capture", "geolocation", "microphone"].every(
        (feature) => !policy.allowsFeature(feature),
      ),
      "permission-policy",
    );
  }

  function complete(fullLayout = false) {
    verifyHostPolicy();
    const book = content.book;
    const state = pagination.snapshot();
    const resources = content.resourceSnapshot();
    if (resources.currentOrNextPending !== 0) emit(imageLoadTerminalMessage(resources));
    assert(resources.currentOrNextPending === 0, "image-load");
    const inline = book.querySelectorAll("img.math-inline").length;
    const display = book.querySelectorAll("img.math-display").length;
    const cuts = pagination.countCutRects(!fullLayout);
    document.documentElement.dataset.status = "pass";
    document.documentElement.dataset.pages = String(state.pages);
    document.documentElement.dataset.inlineFormulas = String(inline);
    document.documentElement.dataset.displayFormulas = String(display);
    document.documentElement.dataset.cuts = String(cuts);
    document.documentElement.dataset.pendingResources = String(resources.pending);
    emit(`ready|${state.pages}|${inline}|${display}|${cuts}`);
  }

  if (
    params.has("verify") ||
    params.has("gesture-probe") ||
    params.has("search-probe") ||
    params.has("statistics-probe")
  ) {
    Object.defineProperty(globalThis, "__athaReaderDiagnostics", {
      value: Object.freeze({
        armCopyProbe: contentActions.armCopyProbe,
        clearSelection: contentActions.clearSelection,
        beginGestureProbe,
        cleanupGestureProbe,
        finishGestureProbe,
        openGestureSection,
        pendingFormulaQueueProbe: structuredActions.verifyPendingFormulaQueue,
        previousBoundaryProbe,
        scrollResourceProbe,
        focusMedia,
        mediaPoint,
        previewState,
        readingStatisticsBenchmark: readingStatistics.benchmark,
        selectionProbe: contentActions.selectionProbe,
        snapshot: visualSnapshot,
        wheelProbe,
      }),
      configurable: false,
      writable: false,
    });
  }

  return Object.freeze({ benchmark, complete, recordFirstStable, verify, verifyImport });
}
