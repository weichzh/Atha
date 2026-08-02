const BENCHMARK_SAMPLES = 10;

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
      `metric|first_stable|1|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`,
    );
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
    const anchorAt32 = await navigation.setFontSize(40);
    assert(anchorAt32.start.offset === reflowAnchor.start.offset, "sample-boundary");
    assert(pagination.isOffsetVisible(anchorAt32.start.offset), "sample-boundary");
    const anchorAt40 = await navigation.setFontSize(24);
    assert(pagination.isOffsetVisible(anchorAt40.start.offset), "sample-boundary");
    const anchorAt24 = await navigation.setFontSize(32);
    assert(pagination.isOffsetVisible(anchorAt24.start.offset), "sample-boundary");

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
    if (pagination.snapshot().pages > 1) await pagination.show(1);
    const compactAnchor = await navigation.setPreferences("application", {
      theme: "dark",
      fontSize: 24,
      fontFamily: "serif",
      density: "compact",
    });
    assert(pagination.isOffsetVisible(compactAnchor.start.offset), "sample-boundary");
    assert(document.documentElement.dataset.theme === "dark", "sample-boundary");
    assert(content.book.dataset.fontFamily === "serif", "sample-boundary");
    assert(reader.style.getPropertyValue("--page-inline-margin") === "80px", "sample-boundary");
    const compactStyle = getComputedStyle(content.book);
    assert(compactStyle.backgroundColor === "rgb(24, 34, 38)", "sample-boundary");
    assert(compactStyle.color === "rgb(233, 238, 238)", "sample-boundary");
    assert(compactStyle.fontFamily.includes("Georgia"), "sample-boundary");
    assert(Math.abs(parseFloat(compactStyle.lineHeight) - 34.8) < 0.1, "sample-boundary");
    const formula = content.book.querySelector("img.math-inline, img.math-display");
    const ordinary = content.book.querySelector("img:not(.math-inline):not(.math-display)");
    if (formula) assert(getComputedStyle(formula).filter !== "none", "sample-boundary");
    if (ordinary) assert(getComputedStyle(ordinary).filter === "none", "sample-boundary");
    assert(pagination.countCutRects() === 0, "layout-cut");

    const comfortableAnchor = await navigation.setPreferences("application", {
      theme: "light",
      fontSize: 40,
      fontFamily: "sans",
      density: "comfortable",
    });
    assert(pagination.isOffsetVisible(comfortableAnchor.start.offset), "sample-boundary");
    assert(document.documentElement.dataset.theme === "light", "sample-boundary");
    assert(content.book.dataset.fontFamily === "sans", "sample-boundary");
    assert(reader.style.getPropertyValue("--page-inline-margin") === "144px", "sample-boundary");
    const comfortableStyle = getComputedStyle(content.book);
    assert(comfortableStyle.backgroundColor === "rgb(255, 253, 248)", "sample-boundary");
    assert(comfortableStyle.color === "rgb(24, 34, 38)", "sample-boundary");
    assert(comfortableStyle.fontFamily.includes("Microsoft YaHei"), "sample-boundary");
    assert(Math.abs(parseFloat(comfortableStyle.lineHeight) - 72) < 0.1, "sample-boundary");
    if (formula) assert(getComputedStyle(formula).filter === "none", "sample-boundary");
    if (ordinary) assert(getComputedStyle(ordinary).filter === "none", "sample-boundary");
    assert(pagination.countCutRects() === 0, "layout-cut");

    const bookAnchor = await navigation.setPreferences("book", { sourceStyles: false });
    assert(pagination.isOffsetVisible(bookAnchor.start.offset), "sample-boundary");
    assert(!content.styleSnapshot().bookStyleApplied, "sample-boundary");
    const userCss = ".book { --atha-user-style-probe: applied; }";
    await navigation.setPreferences("book", { userStylesheet: userCss });
    assert(content.styleSnapshot().userStyleApplied, "sample-boundary");
    assert(
      getComputedStyle(content.book).getPropertyValue("--atha-user-style-probe") === "applied",
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
        userStylesheet: ".book { background: url(https://example.com/x); }",
      });
    } catch (error) {
      unsafeRejected = error instanceof Error && error.message === "css-subresource";
    }
    assert(unsafeRejected, "sample-boundary");
    assert(preferences.snapshot().book.userStylesheet === userCss, "sample-boundary");

    await navigation.resetPreferences("book");
    assert(content.styleSnapshot().bookStyleApplied, "sample-boundary");
    assert(!content.styleSnapshot().userStyleApplied, "sample-boundary");
    await navigation.resetPreferences("application");
    const restored = preferences.snapshot();
    assert(
      restored.application.theme === "system" &&
        restored.application.fontSize === 32 &&
        restored.application.fontFamily === "book" &&
        restored.application.density === "standard" &&
        !document.documentElement.dataset.theme &&
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
    };
  }

  async function waitFor(predicate) {
    for (let frame = 0; frame < 120; frame += 1) {
      if (predicate()) return;
      await new Promise(requestAnimationFrame);
    }
    assert(false, "sample-boundary");
  }

  async function verifyInteraction() {
    const key = (value, target = document, options = {}) => {
      target.dispatchEvent(new KeyboardEvent("keydown", { key: value, bubbles: true, ...options }));
    };
    const pointer = (type, options, target = reader) =>
      target.dispatchEvent(
        new PointerEvent(type, { bubbles: true, pointerType: "mouse", isPrimary: true, ...options }),
      );
    const rect = reader.getBoundingClientRect();

    await pagination.show(0);
    key("ArrowRight");
    await waitFor(() => pagination.snapshot().page === 1);
    key("PageUp");
    await waitFor(() => pagination.snapshot().page === 0);

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

    for (const deltaY of [20, 20, 20, 100]) {
      reader.dispatchEvent(
        new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY, deltaMode: 0 }),
      );
    }
    await waitFor(() => pagination.snapshot().page === 1);
    assert(interaction.snapshot().wheel === 1, "sample-boundary");

    await pagination.show(0);
    const right = rect.right - 20;
    pointer("pointerdown", { pointerId: 1, button: 0, clientX: right, clientY: rect.top + 40 });
    pointer("pointerup", { pointerId: 1, button: 0, clientX: right, clientY: rect.top + 40 });
    await waitFor(() => pagination.snapshot().page === 1);
    const left = rect.left + 20;
    pointer("pointerdown", { pointerId: 1, button: 0, clientX: left, clientY: rect.top + 40 });
    pointer("pointerup", { pointerId: 1, button: 0, clientX: left, clientY: rect.top + 40 });
    await waitFor(() => pagination.snapshot().page === 0);

    pointer("pointerdown", {
      pointerId: 2,
      pointerType: "touch",
      button: 0,
      clientX: rect.right - 80,
      clientY: rect.top + 80,
    });
    pointer("pointerup", {
      pointerId: 2,
      pointerType: "touch",
      button: 0,
      clientX: rect.left + 80,
      clientY: rect.top + 84,
    });
    await waitFor(() => pagination.snapshot().page === 1);

    await pagination.show(0);
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
      key("PageDown");
      await waitFor(() => session.snapshot().currentIndex === 1);
      key("PageUp");
      await waitFor(() => session.snapshot().currentIndex === 0);
    }
    await session.open(0);
    await pagination.show(0);
    const counts = interaction.snapshot();
    interactionEvidence = {
      ...counts,
      keyboardVerified: counts.keyboard >= 2,
      wheelVerified: counts.wheel === 1,
      mouseVerified: counts.mouse === 2,
      touchVerified: counts.touch === 1,
      selectionVerified: counts.selectionProtected === 1,
      controlsVerified: counts.controlProtected === 1,
      linksVerified: counts.contentProtected === 1,
      multiTouchVerified: counts.multiTouchProtected === 1,
    };
  }

  async function verify() {
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
    await verifyNavigation();
    await verifyPreferences();
    await verifyInteraction();
    contentActionEvidence = await contentActions.verify({ pagination, session });
    await pagination.verifySizes();
    pagination.verifyFormulaLayout();
    pagination.verifyDisplayGeometry();
    await securityProbe();
  }

  async function benchmark() {
    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await renderCachedSource();
      const state = pagination.snapshot();
      emit(
        `metric|hot_open|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`,
      );
    }

    assert(pagination.snapshot().pages > 1, "layout-cut");
    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await pagination.show(sample % 2);
      const state = pagination.snapshot();
      emit(
        `metric|page_turn|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`,
      );
      assert(pagination.countCutRects() === 0, "layout-cut");
    }

    for (let sample = 1; sample <= BENCHMARK_SAMPLES; sample += 1) {
      const started = performance.now();
      await pagination.setFontSize(sample % 2 ? 40 : 24);
      const state = pagination.snapshot();
      emit(
        `metric|font_reflow|${sample}|${(performance.now() - started).toFixed(3)}|${state.fontSize}|${state.pages}`,
      );
    }
    await pagination.setFontSize(32);
    document.querySelector("#font-size").value = "32";
  }

  function visualSnapshot() {
    const book = content.book;
    const formulas = [...book.querySelectorAll("img.math-inline, img.math-display")];
    const ordinary = [...book.querySelectorAll("img:not(.math-inline):not(.math-display)")];
    const ordinaryPngCount = ordinary.filter((image) =>
      new URL(image.src).pathname.toLowerCase().endsWith(".png"),
    ).length;
    return {
      status: document.documentElement.dataset.status || null,
      error: document.documentElement.dataset.error || null,
      dark: matchMedia("(prefers-color-scheme: dark)").matches,
      pages: pagination.snapshot().pages,
      formulaCount: formulas.length,
      ordinaryCount: ordinary.length,
      ordinaryPngCount,
      codeBlockCount: book.querySelectorAll("pre code").length,
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
        selectionLength: contentActions.selectionLength(),
      },
    };
  }

  function complete() {
    const book = content.book;
    const state = pagination.snapshot();
    const inline = book.querySelectorAll("img.math-inline").length;
    const display = book.querySelectorAll("img.math-display").length;
    const cuts = pagination.countCutRects();
    document.documentElement.dataset.status = "pass";
    document.documentElement.dataset.pages = String(state.pages);
    document.documentElement.dataset.inlineFormulas = String(inline);
    document.documentElement.dataset.displayFormulas = String(display);
    document.documentElement.dataset.cuts = String(cuts);
    emit(`ready|${state.pages}|${inline}|${display}|${cuts}`);
  }

  if (params.has("verify")) {
    Object.defineProperty(globalThis, "__athaReaderDiagnostics", {
      value: Object.freeze({
        armCopyProbe: contentActions.armCopyProbe,
        clearSelection: contentActions.clearSelection,
        selectionProbe: contentActions.selectionProbe,
        snapshot: visualSnapshot,
      }),
      configurable: false,
      writable: false,
    });
  }

  return Object.freeze({ benchmark, complete, recordFirstStable, verify });
}
