export function createBookmarks({ state, navigation, session, controls, assert }) {
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
    const heading = description.toc.find((item) => item.href.split("#", 1)[0] === section.href);
    return heading?.label || section.id;
  }

  function sync(preferred = controls.list.value) {
    const bookmarks = state.snapshot().bookmarks;
    if (bookmarks.length === 0) {
      const empty = document.createElement("option");
      empty.value = "";
      empty.textContent = "暂无书签";
      controls.list.replaceChildren(empty);
      controls.go.disabled = true;
      controls.remove.disabled = true;
      return;
    }
    controls.list.replaceChildren(
      ...bookmarks.map((bookmark) => {
        const option = document.createElement("option");
        option.value = bookmark.id;
        option.textContent = bookmark.currentVersion
          ? bookmark.label
          : `${bookmark.label}（旧版本）`;
        return option;
      }),
    );
    controls.list.value = bookmarks.some((bookmark) => bookmark.id === preferred)
      ? preferred
      : bookmarks.at(-1).id;
    controls.go.disabled = false;
    controls.remove.disabled = false;
  }

  function run(action) {
    const result = pending.then(action);
    pending = result.catch(() => undefined);
    result.catch((error) =>
      report(error instanceof Error ? messages[error.message] || "书签操作失败" : "书签操作失败", true),
    );
    return result;
  }

  function add() {
    const result = state.addBookmark(label());
    if (!result.ok) throw new Error(result.error);
    sync(result.id);
    report(result.created ? "已添加书签" : "当前位置已有书签");
    return result;
  }

  async function go() {
    const result = state.resolveBookmark(controls.list.value);
    if (!result.ok) throw new Error(result.error);
    if (!(await navigation.goTo(result.locator))) throw new Error("bookmark-location");
    report("已跳转到书签");
    return true;
  }

  function remove() {
    const result = state.removeBookmark(controls.list.value);
    if (!result.ok) throw new Error(result.error);
    sync();
    report("已删除书签");
    return true;
  }

  function bind() {
    controls.add.addEventListener("click", () => run(add));
    controls.go.addEventListener("click", () => run(go));
    controls.remove.addEventListener("click", () => run(remove));
    controls.list.addEventListener("change", () => sync());
    sync();
  }

  async function verify() {
    const before = navigation.current();
    controls.add.click();
    await pending;
    const created = state.snapshot().bookmarks;
    assert(created.length === 1 && controls.list.value === created[0].id, "sample-boundary");
    controls.add.click();
    await pending;
    assert(state.snapshot().bookmarks.length === 1, "sample-boundary");
    await navigation.next();
    controls.go.click();
    await pending;
    assert(locatorEqual(navigation.current(), before), "sample-boundary");
    controls.remove.click();
    await pending;
    assert(state.snapshot().bookmarks.length === 0 && controls.remove.disabled, "sample-boundary");
    return Object.freeze({ created: true, duplicatePrevented: true, jumped: true, deleted: true });
  }

  function locatorEqual(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  return Object.freeze({ bind, idle: () => pending, snapshot: () => state.snapshot().bookmarks, verify });
}
