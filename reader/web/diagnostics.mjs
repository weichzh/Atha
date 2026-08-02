const BENCHMARK_SAMPLES = 10;

export function createDiagnostics({
  params,
  content,
  pagination,
  session,
  locator,
  navigation,
  reader,
  renderCachedSource,
  emit,
  assert,
}) {
  let verifiedSections = [];
  let verifiedHeadings = [];
  let releasedSections = 0;
  let navigationEvidence = {};

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
      value: Object.freeze({ snapshot: visualSnapshot }),
      configurable: false,
      writable: false,
    });
  }

  return Object.freeze({ benchmark, complete, recordFirstStable, verify });
}
