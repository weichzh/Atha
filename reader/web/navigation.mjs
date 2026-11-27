function findTocIndex(
  description,
  sectionIndex,
  preferred = -1,
  contentOffset = 0,
  offsetForFragment = () => null,
) {
  const href = description.sections[sectionIndex]?.href;
  if (description.toc[preferred]?.href.split("#", 1)[0] === href) return preferred;
  let best = { index: -1, offset: -1 };
  description.toc.forEach((item, index) => {
    const [itemHref, fragment] = item.href.split("#");
    if (itemHref !== href) return;
    const offset = fragment === undefined ? 0 : offsetForFragment(fragment);
    if (offset !== null && offset <= contentOffset && offset >= best.offset) {
      best = { index, offset };
    }
  });
  return best.index;
}

function encodeProgress(sectionIndex, sectionCount, pageIndex, pageCount) {
  return (sectionIndex + (pageIndex + 1) / pageCount) / sectionCount;
}

export function readingPercent(sectionIndex, sectionCount, pageIndex, pageCount) {
  if (
    ![sectionIndex, sectionCount, pageIndex, pageCount].every(Number.isInteger) ||
    sectionIndex < 0 || sectionIndex >= sectionCount ||
    pageIndex < 0 || pageIndex >= pageCount
  ) return null;
  if (sectionIndex + 1 === sectionCount && pageIndex + 1 === pageCount) return 100;
  const ratio = encodeProgress(sectionIndex, sectionCount, pageIndex, pageCount);
  return Math.min(99, Math.round(ratio * 100));
}

function decodeProgress(value, maximum, sectionCount) {
  const ratio = Math.max(0, Math.min(1, Number(value) / Number(maximum)));
  const absolute = ratio * sectionCount;
  const epsilon = Number.EPSILON * Math.max(1, absolute) * 4;
  const sectionIndex = Math.max(
    0,
    Math.min(sectionCount - 1, Math.ceil(absolute - epsilon) - 1),
  );
  return Object.freeze({ ratio, sectionIndex, localRatio: absolute - sectionIndex });
}

function decodeProgressPage(localRatio, pageCount) {
  return Math.max(0, Math.min(pageCount - 1, Math.round(localRatio * pageCount) - 1));
}

export function createNavigation({
  session,
  pagination,
  locator,
  preferences,
  toc,
  chapterLabel,
  progressChapter,
  progressBook,
  progressPosition,
  toolbarChapter,
  toolbarProgress,
  progressRange,
  previous,
  next,
  fontSizeControl,
  onFallback,
  onPreferences,
  onStable,
  assert,
  fail,
}) {
  let lastFallback = null;
  let pending = Promise.resolve();

  function run(action, readingAction = null) {
    const result = pending.then(async () => {
      const wasAtEnd = readingAction === "navigate" && currentPercent() === 100;
      const value = await action();
      const atEnd = readingAction !== null && currentPercent() === 100;
      // In a single-page book, opening alone is not a completion event.
      // An explicit forward turn at the end confirms it without inventing a page.
      const movedAt = readingAction === "scroll" ? value.movedAt : null;
      const active = readingAction === "scroll" ? movedAt !== null
        : readingAction !== null && (value === true || (readingAction === "forward" && atEnd));
      const reachedEnd = readingAction === "scroll" ? value.reachedEnd : atEnd && !wasAtEnd;
      onStable(active, active && reachedEnd, movedAt);
      return value;
    });
    pending = result.catch(() => undefined);
    return result;
  }

  function book() {
    return session.describe();
  }

  function current() {
    const state = session.snapshot();
    return locator.point(book(), state.currentSection, pagination.captureOffset());
  }

  function currentTocIndex(preferred = -1, contentOffset = pagination.captureOffset()) {
    const description = book();
    const state = session.snapshot();
    return findTocIndex(
      description,
      state.currentIndex,
      preferred,
      contentOffset,
      (fragment) => pagination.offsetForFragment(fragment),
    );
  }

  function syncControls(preferredTocIndex = -1, contentOffset = pagination.captureOffset()) {
    const state = session.snapshot();
    const page = pagination.snapshot();
    previous.disabled = state.currentIndex === 0 && page.page === 0;
    next.disabled = state.currentIndex + 1 === state.sections && page.page + 1 === page.pages;
    const index = currentTocIndex(preferredTocIndex, contentOffset);
    if (index >= 0) toc.value = String(index);
    const label = book().toc[index]?.label || `第 ${state.currentIndex + 1} 节`;
    const ratio = encodeProgress(state.currentIndex, state.sections, page.page, page.pages);
    const percent = readingPercent(state.currentIndex, state.sections, page.page, page.pages);
    chapterLabel.textContent = label;
    progressChapter.textContent = label;
    progressBook.textContent = percent === null ? "全书进度未知" : `全书约 ${percent}%`;
    progressPosition.textContent = `第 ${state.currentIndex + 1}/${state.sections} 节 · 本节 ${page.page + 1}/${page.pages} 页`;
    if (toolbarChapter) toolbarChapter.textContent = label;
    if (toolbarProgress) toolbarProgress.textContent = progressPosition.textContent;
    progressRange.value = String(ratio * Number(progressRange.max));
    progressRange.dispatchEvent(new Event("atha-range-sync"));
  }

  function currentPercent(refreshScroll = false) {
    const state = session.snapshot();
    const page = pagination.snapshot(refreshScroll);
    return readingPercent(state.currentIndex, state.sections, page.page, page.pages);
  }

  async function openSection(index) {
    const offset = pagination.captureOffset();
    if (await session.open(index)) return true;
    await pagination.showOffset(offset);
    syncControls();
    return false;
  }

  async function fallback(reason, sectionIndex = 0) {
    const state = session.snapshot();
    if (state.currentIndex !== sectionIndex && !(await openSection(sectionIndex))) return false;
    await pagination.show(0);
    lastFallback = reason;
    onFallback(reason);
    syncControls();
    return false;
  }

  async function goTo(value) {
    let target;
    try {
      target =
        typeof value === "string"
          ? locator.parse(book(), value)
          : locator.parse(book(), locator.serialize(book(), value));
    } catch (error) {
      const reason =
        error instanceof Error && /^locator-[a-z]+$/.test(error.message)
          ? error.message
          : "locator-format";
      return fallback(reason);
    }
    const sectionIndex = book().sections.findIndex(
      (section) => section.id === target.start.section,
    );
    if (session.snapshot().currentIndex !== sectionIndex && !(await openSection(sectionIndex))) {
      return false;
    }
    if (target.end && !pagination.hasOffset(target.end.offset)) {
      return fallback("locator-offset", sectionIndex);
    }
    if (!(await pagination.showOffset(target.start.offset))) {
      return fallback("locator-offset", sectionIndex);
    }
    syncControls(-1, target.start.offset);
    return true;
  }

  async function goToToc(index) {
    const item = book().toc[index];
    if (!item) return fallback("locator-section");
    toc.value = String(index);
    const [href, fragment] = item.href.split("#");
    const sectionIndex = book().sections.findIndex((section) => section.href === href);
    if (sectionIndex < 0) return fallback("locator-section");
    if (session.snapshot().currentIndex !== sectionIndex && !(await openSection(sectionIndex))) {
      return false;
    }
    if (fragment !== undefined) {
      const offset = pagination.offsetForFragment(fragment);
      if (offset === null) return fallback("locator-fragment", sectionIndex);
      await pagination.showOffset(offset);
    } else {
      await pagination.show(0);
    }
    syncControls(index);
    return true;
  }

  async function goToHref(value) {
    let target;
    try {
      target = new URL(value);
    } catch {
      return fallback("locator-section");
    }
    const sectionUrl = new URL(target.href);
    sectionUrl.hash = "";
    sectionUrl.search = "";
    // Custom WebView schemes can have an opaque URL.origin.
    const sectionIndex = book().sections.findIndex((section) => new URL(section.url).href === sectionUrl.href);
    if (sectionIndex < 0) return fallback("locator-section");
    if (session.snapshot().currentIndex !== sectionIndex && !(await openSection(sectionIndex))) {
      return false;
    }
    if (target.hash) {
      let fragment;
      try {
        fragment = decodeURIComponent(target.hash.slice(1));
      } catch {
        return fallback("locator-fragment", sectionIndex);
      }
      const offset = pagination.offsetForFragment(fragment);
      if (offset === null) return fallback("locator-fragment", sectionIndex);
      await pagination.showOffset(offset);
    } else {
      await pagination.show(0);
    }
    syncControls();
    return true;
  }

  async function previousPage() {
    if (await pagination.move(-1)) {
      syncControls();
      return true;
    }
    const index = session.snapshot().currentIndex;
    if (index === 0) return false;
    if (!(await openSection(index - 1))) return false;
    await pagination.show(pagination.snapshot().pages - 1);
    syncControls();
    return true;
  }

  async function nextPage() {
    if (await pagination.move(1)) {
      syncControls();
      return true;
    }
    const state = session.snapshot();
    if (state.currentIndex + 1 === state.sections) return false;
    if (!(await openSection(state.currentIndex + 1))) return false;
    syncControls();
    return true;
  }

  async function goToProgress(value) {
    const state = session.snapshot();
    const { sectionIndex, localRatio } = decodeProgress(
      value,
      progressRange.max,
      state.sections,
    );
    if (state.currentIndex !== sectionIndex && !(await openSection(sectionIndex))) return false;
    await pagination.show(decodeProgressPage(localRatio, pagination.snapshot().pages));
    syncControls();
    return true;
  }

  async function setFontSize(value) {
    return setPreferences("application", { fontSize: Number(value) });
  }

  async function commitPreferences(scope, change) {
    const anchor = current();
    const previous = preferences.snapshot();
    try {
      const state = change();
      await pagination.setFontSize(state.effective.fontSize, anchor.start.offset);
      syncControls();
      if (onPreferences(scope) === false) throw new Error("state-write");
      return anchor;
    } catch (error) {
      const restored = preferences.restore(previous);
      await pagination.setFontSize(restored.effective.fontSize, anchor.start.offset);
      preferences.restore(previous);
      syncControls();
      throw error;
    }
  }

  async function setPreferences(scope, patch) {
    return commitPreferences(scope, () => preferences.update(scope, patch));
  }

  async function resetPreferences(scope) {
    return commitPreferences(scope, () => preferences.reset(scope));
  }

  async function resizeViewport(
    offset = null,
    expectedSectionIndex = null,
    expectedPageIndex = null,
    expectedScrollTop = null,
  ) {
    if (
      expectedSectionIndex !== null &&
      session.snapshot().currentIndex !== expectedSectionIndex
    ) {
      return null;
    }
    const anchor = current();
    const pageChanged =
      expectedPageIndex !== null && pagination.snapshot().page !== expectedPageIndex;
    const scrollChanged =
      expectedScrollTop !== null &&
      Math.abs(pagination.scrollPosition() - expectedScrollTop) > 1;
    await pagination.resizeViewport(
      pageChanged || scrollChanged ? anchor.start.offset : (offset ?? anchor.start.offset),
    );
    syncControls();
    return anchor;
  }

  async function settleScroll() {
    const movedAt = pagination.takeReadingScroll();
    const reachedEnd = movedAt !== null && currentPercent(true) === 100;
    await pagination.settleScrolledContent();
    syncControls();
    return { movedAt, reachedEnd };
  }

  function bindControls() {
    const description = book();
    toc.replaceChildren(
      ...description.toc.map((item, index) => {
        const option = document.createElement("option");
        option.value = String(index);
        option.textContent = item.label;
        return option;
      }),
    );
    toc.closest("label").hidden = description.toc.length === 0;
    pagination.bindControls({
      onPrevious: () => run(previousPage, "navigate"),
      onNext: () => run(nextPage, "forward"),
      onFontSize: (value) => run(() => setFontSize(value)),
      onProgress: (value) => run(() => goToProgress(value), "navigate"),
      onScroll: () => run(settleScroll, "scroll"),
    });
    preferences.bind({
      onUpdate: (scope, patch) => run(() => setPreferences(scope, patch)),
      onReset: (scope) => run(() => resetPreferences(scope)),
    });
    toc.addEventListener("change", () => {
      if (toc.selectedOptions[0]?.dataset.bookmarkId) return;
      const index = Number(toc.value);
      run(() => goToToc(index), "navigate").catch((error) => {
        fail(error instanceof Error ? error.message : "section-load");
      });
    });
    fontSizeControl.value = String(pagination.snapshot().fontSize);
    fontSizeControl.dispatchEvent(new Event("atha-range-sync"));
    syncControls();
  }

  function snapshot() {
    const state = session.snapshot();
    return Object.freeze({
      current: state.currentIndex >= 0 ? locator.serialize(book(), current()) : null,
      lastFallback,
      tocIndex: Number(toc.value),
    });
  }

  assert(
    findTocIndex(
      {
        sections: [{ href: "section.xhtml" }],
        toc: [{ href: "section.xhtml#one" }, { href: "section.xhtml#two" }],
      },
      0,
      1,
    ) === 1,
    "sample-boundary",
  );
  const firstPageProgress = encodeProgress(0, 1, 0, 10);
  const firstPageTarget = decodeProgress(firstPageProgress, 1, 1);
  assert(
    firstPageTarget.sectionIndex === 0 &&
      decodeProgressPage(firstPageTarget.localRatio, 10) === 0,
    "sample-boundary",
  );
  const sectionEndProgress = encodeProgress(0, 173, 29, 30);
  const sectionEndTarget = decodeProgress(sectionEndProgress, 1, 173);
  assert(
    sectionEndTarget.sectionIndex === 0 &&
      decodeProgressPage(sectionEndTarget.localRatio, 30) === 29,
    "sample-boundary",
  );
  const laterSectionEndProgress = encodeProgress(24, 173, 29, 30);
  const laterSectionEndTarget = decodeProgress(laterSectionEndProgress, 1, 173);
  assert(
    laterSectionEndTarget.sectionIndex === 24 &&
      decodeProgressPage(laterSectionEndTarget.localRatio, 30) === 29,
    "sample-boundary",
  );
  const sectionMiddleProgress = encodeProgress(0, 173, 13, 30);
  const sectionMiddleTarget = decodeProgress(sectionMiddleProgress, 1, 173);
  assert(
    sectionMiddleTarget.sectionIndex === 0 &&
      decodeProgressPage(sectionMiddleTarget.localRatio, 30) === 13,
    "sample-boundary",
  );
  assert(
    findTocIndex(
      {
        sections: [{ href: "section.xhtml" }],
        toc: [
          { href: "section.xhtml", label: "one" },
          { href: "section.xhtml#two", label: "two" },
        ],
      },
      0,
      -1,
      75,
      (fragment) => (fragment === "two" ? 50 : null),
    ) === 1,
    "sample-boundary",
  );

  return Object.freeze({
    bindControls,
    current,
    currentPercent,
    goTo: (value) => run(() => goTo(value), "navigate"),
    goToHref: (value) => run(() => goToHref(value), "navigate"),
    goToToc: (index) => run(() => goToToc(index), "navigate"),
    idle: () => pending,
    next: () => run(nextPage, "forward"),
    previous: () => run(previousPage, "navigate"),
    resetPreferences: (scope) => run(() => resetPreferences(scope)),
    resize: (offset, expectedSectionIndex, expectedPageIndex, expectedScrollTop) =>
      run(() =>
        resizeViewport(offset, expectedSectionIndex, expectedPageIndex, expectedScrollTop),
      ),
    setFontSize: (value) => run(() => setFontSize(value)),
    setPreferences: (scope, patch) => run(() => setPreferences(scope, patch)),
    snapshot,
  });
}
