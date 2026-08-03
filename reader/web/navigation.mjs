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

  function run(action) {
    const result = pending.then(action).then((value) => {
      onStable();
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
    const label = book().toc[index]?.label || book().sections[state.currentIndex]?.id || "";
    const ratio = encodeProgress(state.currentIndex, state.sections, page.page, page.pages);
    chapterLabel.textContent = label;
    progressChapter.textContent = label;
    progressBook.textContent = `全书约 ${Math.round(ratio * 100)}%`;
    progressPosition.textContent = `第 ${state.currentIndex + 1}/${state.sections} 节 · 本节 ${page.page + 1}/${page.pages} 页`;
    progressRange.value = String(ratio * Number(progressRange.max));
  }

  async function fallback(reason, sectionIndex = 0) {
    const state = session.snapshot();
    if (state.currentIndex !== sectionIndex) await session.open(sectionIndex);
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
    if (session.snapshot().currentIndex !== sectionIndex) await session.open(sectionIndex);
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
    if (session.snapshot().currentIndex !== sectionIndex) await session.open(sectionIndex);
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
    const sectionIndex = book().sections.findIndex(
      (section) => new URL(section.url).href === `${target.origin}${target.pathname}`,
    );
    if (sectionIndex < 0) return fallback("locator-section");
    if (session.snapshot().currentIndex !== sectionIndex) await session.open(sectionIndex);
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
    await session.open(index - 1);
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
    await session.open(state.currentIndex + 1);
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
    if (state.currentIndex !== sectionIndex) await session.open(sectionIndex);
    await pagination.show(decodeProgressPage(localRatio, pagination.snapshot().pages));
    syncControls();
  }

  async function setFontSize(value) {
    return setPreferences("application", { fontSize: Number(value) });
  }

  async function setPreferences(scope, patch) {
    const anchor = current();
    const state = preferences.update(scope, patch);
    await pagination.setFontSize(state.effective.fontSize, anchor.start.offset);
    syncControls();
    if (onPreferences(scope) === false) throw new Error("state-write");
    return anchor;
  }

  async function resetPreferences(scope) {
    const anchor = current();
    const state = preferences.reset(scope);
    await pagination.setFontSize(state.effective.fontSize, anchor.start.offset);
    syncControls();
    if (onPreferences(scope) === false) throw new Error("state-write");
    return anchor;
  }

  async function resizeViewport() {
    const anchor = current();
    await pagination.resizeViewport(anchor.start.offset);
    syncControls();
    return anchor;
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
      onPrevious: () => run(previousPage),
      onNext: () => run(nextPage),
      onFontSize: (value) => run(() => setFontSize(value)),
      onProgress: (value) => run(() => goToProgress(value)),
    });
    preferences.bind({
      onUpdate: (scope, patch) => run(() => setPreferences(scope, patch)),
      onReset: (scope) => run(() => resetPreferences(scope)),
    });
    toc.addEventListener("change", () => {
      if (toc.selectedOptions[0]?.dataset.bookmarkId) return;
      const index = Number(toc.value);
      run(() => goToToc(index)).catch((error) => {
        fail(error instanceof Error ? error.message : "section-load");
      });
    });
    fontSizeControl.value = String(pagination.snapshot().fontSize);
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
    goTo: (value) => run(() => goTo(value)),
    goToHref: (value) => run(() => goToHref(value)),
    goToToc: (index) => run(() => goToToc(index)),
    idle: () => pending,
    next: () => run(nextPage),
    previous: () => run(previousPage),
    resetPreferences: (scope) => run(() => resetPreferences(scope)),
    resize: () => run(resizeViewport),
    setFontSize: (value) => run(() => setFontSize(value)),
    setPreferences: (scope, patch) => run(() => setPreferences(scope, patch)),
    snapshot,
  });
}
