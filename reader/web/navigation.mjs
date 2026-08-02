function findTocIndex(description, sectionIndex, preferred = -1) {
  const href = description.sections[sectionIndex]?.href;
  if (description.toc[preferred]?.href.split("#", 1)[0] === href) return preferred;
  return description.toc.findIndex((item) => item.href.split("#", 1)[0] === href);
}

export function createNavigation({
  session,
  pagination,
  locator,
  preferences,
  toc,
  previous,
  next,
  fontSizeControl,
  onFallback,
  assert,
  fail,
}) {
  let lastFallback = null;
  let pending = Promise.resolve();

  function run(action) {
    const result = pending.then(action);
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

  function syncControls(preferredTocIndex = -1) {
    const state = session.snapshot();
    const page = pagination.snapshot();
    previous.disabled = state.currentIndex === 0 && page.page === 0;
    next.disabled = state.currentIndex + 1 === state.sections && page.page + 1 === page.pages;
    const index = findTocIndex(book(), state.currentIndex, preferredTocIndex);
    if (index >= 0) toc.value = String(index);
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
    syncControls();
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

  async function setFontSize(value) {
    return setPreferences("application", { fontSize: Number(value) });
  }

  async function setPreferences(scope, patch) {
    const anchor = current();
    const state = preferences.update(scope, patch);
    await pagination.setFontSize(state.effective.fontSize, anchor.start.offset);
    syncControls();
    return anchor;
  }

  async function resetPreferences(scope) {
    const anchor = current();
    const state = preferences.reset(scope);
    await pagination.setFontSize(state.effective.fontSize, anchor.start.offset);
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
    });
    preferences.bind({
      onUpdate: (scope, patch) => run(() => setPreferences(scope, patch)),
      onReset: (scope) => run(() => resetPreferences(scope)),
    });
    toc.addEventListener("change", () => {
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
      tocIndex: toc.selectedIndex,
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

  return Object.freeze({
    bindControls,
    current,
    goTo: (value) => run(() => goTo(value)),
    goToHref: (value) => run(() => goToHref(value)),
    goToToc: (index) => run(() => goToToc(index)),
    next: () => run(nextPage),
    previous: () => run(previousPage),
    resetPreferences: (scope) => run(() => resetPreferences(scope)),
    setFontSize: (value) => run(() => setFontSize(value)),
    setPreferences: (scope, patch) => run(() => setPreferences(scope, patch)),
    snapshot,
  });
}
