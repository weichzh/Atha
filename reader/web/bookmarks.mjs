export function createBookmarks({ state, navigation, pagination, session, locator, controls, assert }) {
  let pending = Promise.resolve();
  const placements = new Map();
  const messages = Object.freeze({
    "bookmark-label": "书签名称无效",
    "bookmark-limit": "书签数量已达上限",
    "bookmark-location": "书签位置已失效",
    "bookmark-missing": "书签不存在",
    "bookmark-version": "书签来自其他内容版本",
    "state-write": "书签保存失败",
  });

  function report(message, error = false) {
    controls.status.textContent = message;
    controls.status.dataset.error = String(error);
  }

  function label() {
    const description = session.describe();
    const current = session.snapshot();
    const section = description.sections[current.currentIndex];
    const heading = description.toc[navigation.snapshot().tocIndex];
    return heading?.label || section.id;
  }

  function visibleBookmark() {
    const section = session.snapshot().currentSection;
    return state.snapshot().bookmarks.find((bookmark) => {
      if (!bookmark.currentVersion) return false;
      try {
        const point = locator.parse(session.describe(), bookmark.locator).start;
        return point.section === section && pagination.isOffsetVisible(point.offset);
      } catch {
        return false;
      }
    });
  }

  function syncCurrent() {
    controls.add.setAttribute("aria-pressed", String(Boolean(visibleBookmark())));
  }

  function candidates(bookmark) {
    let sectionId;
    try {
      sectionId = locator.parse(session.describe(), bookmark.locator).start.section;
    } catch {
      return [];
    }
    const description = session.describe();
    const section = description.sections.find((item) => item.id === sectionId);
    if (!section) return [];
    const sameSection = description.toc
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.href.split("#", 1)[0] === section.href);
    const sameLabel = sameSection.filter(({ item }) => item.label === bookmark.label);
    return (sameLabel.length ? sameLabel : sameSection).map(({ index }) => index);
  }

  function sync() {
    const bookmarks = state.snapshot().bookmarks;
    controls.list.querySelectorAll("option[data-bookmark-id]").forEach((option) => option.remove());
    for (const bookmark of bookmarks) {
      const option = document.createElement("option");
      option.value = `bookmark:${bookmark.id}`;
      option.dataset.bookmarkId = bookmark.id;
      option.disabled = !bookmark.currentVersion;
      option.textContent = `　书签 · ${bookmark.label}${bookmark.currentVersion ? "" : "（旧版本）"}`;
      const choices = candidates(bookmark);
      const saved = placements.get(bookmark.id);
      const tocIndex = choices.includes(saved) ? saved : choices[0] ?? -1;
      const chapter = controls.list.querySelector(`option[value="${tocIndex}"]`);
      let previous = chapter;
      while (previous?.nextElementSibling?.dataset.bookmarkId) {
        previous = previous.nextElementSibling;
      }
      if (previous) previous.after(option);
      else controls.list.append(option);
    }
    controls.list.closest("label").hidden = controls.list.options.length === 0;
    syncCurrent();
  }

  function run(action) {
    const result = pending.then(action);
    pending = result.catch(() => undefined);
    result.catch((error) =>
      report(error instanceof Error ? messages[error.message] || "书签操作失败" : "书签操作失败", true),
    );
    return result;
  }

  function toggle() {
    const visible = visibleBookmark();
    if (visible) {
      const removed = state.removeBookmark(visible.id);
      if (!removed.ok) throw new Error(removed.error);
      placements.delete(visible.id);
      sync();
      report("已取消书签");
      return Object.freeze({ ok: true, id: visible.id, created: false, removed: true });
    }
    const result = state.addBookmark(label());
    if (!result.ok) throw new Error(result.error);
    assert(result.created, "sample-boundary");
    placements.set(result.id, navigation.snapshot().tocIndex);
    sync();
    report("已添加书签");
    return result;
  }

  async function go(id) {
    const result = state.resolveBookmark(id);
    if (!result.ok) throw new Error(result.error);
    if (!(await navigation.goTo(result.locator))) throw new Error("bookmark-location");
    report("已跳转到书签");
    return true;
  }

  async function bind() {
    const ambiguous = state
      .snapshot()
      .bookmarks.filter((bookmark) => bookmark.currentVersion && candidates(bookmark).length > 1);
    if (ambiguous.length) {
      const before = locator.serialize(session.describe(), navigation.current());
      // ponytail: Resolve rare ambiguous restores one by one; batch only if startup profiles regress.
      for (const bookmark of ambiguous) {
        if (await navigation.goTo(bookmark.locator)) {
          placements.set(bookmark.id, navigation.snapshot().tocIndex);
        }
      }
      await navigation.goTo(before);
    }
    controls.add.addEventListener("click", () => run(toggle));
    controls.list.addEventListener("change", () => {
      const id = controls.list.selectedOptions[0]?.dataset.bookmarkId;
      if (id) run(() => go(id));
    });
    sync();
  }

  async function verify() {
    const before = navigation.current();
    controls.add.click();
    await pending;
    const created = state.snapshot().bookmarks;
    const inserted = controls.list.querySelector("option[data-bookmark-id]");
    const correctlyPlaced = session.describe().toc.length
      ? inserted?.previousElementSibling?.value === String(navigation.snapshot().tocIndex)
      : inserted === controls.list.firstElementChild;
    assert(
      created.length === 1 &&
        controls.add.getAttribute("aria-pressed") === "true" &&
        correctlyPlaced &&
        !controls.list.closest("label").hidden,
      "sample-boundary",
    );
    controls.add.click();
    await pending;
    assert(
      state.snapshot().bookmarks.length === 0 &&
        controls.add.getAttribute("aria-pressed") === "false" &&
        controls.list.closest("label").hidden === (session.describe().toc.length === 0),
      "sample-boundary",
    );
    controls.add.click();
    await pending;
    await navigation.next();
    const bookmark = controls.list.querySelector("option[data-bookmark-id]");
    controls.list.value = bookmark.value;
    controls.list.dispatchEvent(new Event("change", { bubbles: true }));
    await pending;
    assert(
      locatorEqual(navigation.current(), before) &&
        controls.add.getAttribute("aria-pressed") === "true",
      "sample-boundary",
    );
    controls.add.click();
    await pending;
    assert(state.snapshot().bookmarks.length === 0, "sample-boundary");
    return Object.freeze({ created: true, toggled: true, jumped: true, deleted: true });
  }

  function locatorEqual(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  return Object.freeze({
    bind,
    go: (id) => run(() => go(id)),
    idle: () => pending,
    snapshot: () => state.snapshot().bookmarks,
    syncCurrent,
    verify,
  });
}
