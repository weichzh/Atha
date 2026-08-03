export function createBookmarks({ state, navigation, session, locator, controls, assert }) {
  let pending = Promise.resolve();
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

  function syncCurrent() {
    const current = locator.serialize(session.describe(), navigation.current());
    const active = state
      .snapshot()
      .bookmarks.some((bookmark) => bookmark.currentVersion && bookmark.locator === current);
    controls.add.setAttribute("aria-pressed", String(active));
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
      let sectionId = null;
      try {
        sectionId = locator.parse(session.describe(), bookmark.locator).start.section;
      } catch {
        // The disabled legacy bookmark remains visible at the end of the directory.
      }
      const description = session.describe();
      const section = description.sections.find((item) => item.id === sectionId);
      let tocIndex = section
        ? description.toc.findIndex(
            (item) =>
              item.href.split("#", 1)[0] === section.href && item.label === bookmark.label,
          )
        : -1;
      if (tocIndex < 0 && section) {
        tocIndex = description.toc.findIndex(
          (item) => item.href.split("#", 1)[0] === section.href,
        );
      }
      const chapter = controls.list.querySelector(`option[value="${tocIndex}"]`);
      let previous = chapter;
      while (previous?.nextElementSibling?.dataset.bookmarkId) {
        previous = previous.nextElementSibling;
      }
      if (previous) previous.after(option);
      else controls.list.append(option);
    }
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
    const result = state.addBookmark(label());
    if (!result.ok) throw new Error(result.error);
    if (!result.created) {
      const removed = state.removeBookmark(result.id);
      if (!removed.ok) throw new Error(removed.error);
      sync();
      report("已取消书签");
      return Object.freeze({ ...result, removed: true });
    }
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

  function bind() {
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
    assert(created.length === 1 && controls.add.getAttribute("aria-pressed") === "true", "sample-boundary");
    controls.add.click();
    await pending;
    assert(
      state.snapshot().bookmarks.length === 0 &&
        controls.add.getAttribute("aria-pressed") === "false",
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
    idle: () => pending,
    snapshot: () => state.snapshot().bookmarks,
    syncCurrent,
    verify,
  });
}
