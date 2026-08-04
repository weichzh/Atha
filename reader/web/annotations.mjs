const ANNOTATION_MAX_SELECTED_LENGTH = 4096;
const ANNOTATION_MAX_NOTE_LENGTH = 2000;
const ANNOTATION_CONTEXT_LENGTH = 32;
const HIGHLIGHT_NAME = "atha-annotations";

export function createAnnotations({
  store,
  content,
  session,
  navigation,
  locator,
  controls,
  onNavigate,
  assert,
}) {
  let lastError = null;
  let overlayCount = 0;
  let reanchorFailures = 0;
  let pendingSelection = null;

  function textNodes() {
    const nodes = [];
    const walker = document.createTreeWalker(content.book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  function pointForOffset(offset) {
    if (!Number.isInteger(offset) || offset < 0) return null;
    let remaining = offset;
    const nodes = textNodes();
    for (const node of nodes) {
      if (remaining <= node.data.length) return { node, offset: remaining };
      remaining -= node.data.length;
    }
    return null;
  }

  function rangeForOffsets(start, end) {
    const first = pointForOffset(start);
    const last = pointForOffset(end);
    if (!first || !last || end <= start) return null;
    try {
      const range = document.createRange();
      range.setStart(first.node, first.offset);
      range.setEnd(last.node, last.offset);
      return range;
    } catch {
      return null;
    }
  }

  function offsetForBoundary(node, offset) {
    if (!(node === content.book || content.book.contains(node))) return null;
    try {
      const before = document.createRange();
      before.selectNodeContents(content.book);
      before.setEnd(node, offset);
      return before.toString().length;
    } catch {
      return null;
    }
  }

  async function sourceAnchorForRange(range) {
    const start = offsetForBoundary(range.startContainer, range.startOffset);
    const end = offsetForBoundary(range.endContainer, range.endOffset);
    const text = content.book.textContent || "";
    if (start === null || end === null || end <= start) throw new Error("annotation-selection");
    const selectedText = text.slice(start, end);
    if (
      !selectedText.trim() ||
      selectedText.length > ANNOTATION_MAX_SELECTED_LENGTH ||
      selectedText !== range.toString()
    ) {
      throw new Error("annotation-selection");
    }
    const description = session.describe();
    const section = session.snapshot().currentSection;
    return Object.freeze({
      schema: 1,
      canonicalLocator: locator.serialize(
        description,
        locator.range(
          description,
          { section, offset: start },
          { section, offset: end },
        ),
      ),
      selectedText,
      prefixText: text.slice(Math.max(0, start - ANNOTATION_CONTEXT_LENGTH), start),
      suffixText: text.slice(end, end + ANNOTATION_CONTEXT_LENGTH),
      contentHash: await annotationTextHash(selectedText),
    });
  }

  function currentSelection() {
    const selection = content.book.getRootNode().getSelection?.();
    if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) return null;
    const range = selection.getRangeAt(0);
    return content.book.contains(range.commonAncestorContainer) ||
      range.commonAncestorContainer === content.book
      ? range.cloneRange()
      : null;
  }

  function errorCode() {
    return lastError || store.snapshot().lastError;
  }

  function statusText(code) {
    return {
      "annotation-storage": "标注存储不可用",
      "annotation-corrupt": "标注数据损坏，已禁止覆盖",
      "annotation-write": "标注保存失败",
      "annotation-selection": "请先选择 1 至 4096 个正文字符",
      "annotation-limit": "标注数量已达上限",
      "annotation-missing": "标注已不存在",
      "annotation-anchor": "标注位置无法重锚",
      "annotation-overlay": "当前浏览器不支持高亮",
      "annotation-note": "笔记最多 2000 个字符",
      "annotation-copy": "复制失败，请使用系统复制命令",
    }[code];
  }

  function sync() {
    const active = store.active();
    controls.list.replaceChildren(
      ...active.map((item) => {
        const button = document.createElement("button");
        const kind = document.createElement("span");
        const quote = document.createElement("span");
        button.type = "button";
        button.className = "annotation-item";
        button.dataset.annotationId = item.id;
        kind.className = "annotation-item-kind";
        kind.textContent = item.type === "note" ? "笔记" : "标注";
        quote.className = "annotation-item-quote";
        quote.textContent = item.sourceAnchor.selectedText.replace(/\s+/gu, " ").trim().slice(0, 100);
        button.append(kind, quote);
        if (item.note) {
          const note = document.createElement("span");
          note.className = "annotation-item-note";
          note.textContent = item.note.replace(/\s+/gu, " ").trim().slice(0, 140);
          button.append(note);
        }
        return button;
      }),
    );
    const code = errorCode();
    controls.status.dataset.error = String(Boolean(code));
    controls.status.textContent = statusText(code) || `${active.length} 条笔记与标注`;
  }

  function fail(error) {
    lastError = error;
    sync();
    return Object.freeze({ ok: false, error });
  }

  function reanchor(item) {
    const description = session.describe();
    const inspected = locator.inspect(item.sourceAnchor.canonicalLocator);
    if (!description.sections.some((section) => section.id === inspected.start.section)) return false;
    if (inspected.start.section !== session.snapshot().currentSection) return null;
    const direct = rangeForOffsets(inspected.start.offset, inspected.end.offset);
    if (
      inspected.contentVersion === description.contentVersion &&
      direct?.toString() === item.sourceAnchor.selectedText
    ) {
      return { range: direct, sourceAnchor: null };
    }
    const text = content.book.textContent || "";
    const start = text.indexOf(item.sourceAnchor.selectedText);
    if (start < 0 || text.indexOf(item.sourceAnchor.selectedText, start + 1) >= 0) return false;
    const range = rangeForOffsets(start, start + item.sourceAnchor.selectedText.length);
    if (!range) return false;
    return {
      range,
      sourceAnchor: Object.freeze({
        ...item.sourceAnchor,
        canonicalLocator: locator.serialize(
          description,
          locator.range(
            description,
            { section: inspected.start.section, offset: start },
            { section: inspected.start.section, offset: start + item.sourceAnchor.selectedText.length },
          ),
        ),
      }),
    };
  }

  function resolve(item) {
    const candidate = reanchor(item);
    if (!candidate || candidate === false || !candidate.sourceAnchor) {
      return candidate && candidate.range;
    }
    const replaced = store.replaceAnchor(item.id, candidate.sourceAnchor);
    if (!replaced.ok) lastError = replaced.error;
    return replaced.ok ? candidate.range : "write";
  }

  async function redraw() {
    dismissSelection();
    if (!globalThis.CSS?.highlights || typeof globalThis.Highlight !== "function") {
      lastError = "annotation-overlay";
      sync();
      return false;
    }
    const ranges = [];
    reanchorFailures = 0;
    for (const item of store.active()) {
      const range = resolve(item);
      if (range === false) reanchorFailures += 1;
      else if (range !== "write" && range) ranges.push(range);
    }
    CSS.highlights.delete(HIGHLIGHT_NAME);
    if (ranges.length) CSS.highlights.set(HIGHLIGHT_NAME, new Highlight(...ranges));
    overlayCount = ranges.length;
    if (reanchorFailures && lastError !== "annotation-write") lastError = "annotation-anchor";
    else if (lastError === "annotation-anchor") lastError = null;
    sync();
    return reanchorFailures === 0;
  }

  async function addRange(range, note) {
    let sourceAnchor;
    try {
      sourceAnchor = await sourceAnchorForRange(range);
    } catch {
      return fail("annotation-selection");
    }
    const result = await store.add(sourceAnchor, note);
    if (!result.ok) return fail(result.error);
    lastError = null;
    pendingSelection = null;
    content.book.getRootNode().getSelection?.().removeAllRanges();
    await redraw();
    return result;
  }

  async function addSelection(note) {
    const range = currentSelection() || pendingSelection;
    return range ? addRange(range, note) : fail("annotation-selection");
  }

  async function updateNote(id, note) {
    if (String(note).trim().length > ANNOTATION_MAX_NOTE_LENGTH) return fail("annotation-note");
    const result = store.updateNote(id, note);
    if (!result.ok) return fail(result.error);
    lastError = null;
    sync();
    return result;
  }

  async function remove(id) {
    const result = store.remove(id);
    if (!result.ok) return fail(result.error);
    lastError = null;
    await redraw();
    return result;
  }

  async function go(id) {
    let item = store.item(id);
    if (!item) return fail("annotation-missing");
    const inspected = locator.inspect(item.sourceAnchor.canonicalLocator);
    const index = session.describe().sections.findIndex(
      (section) => section.id === inspected.start.section,
    );
    if (index < 0) return fail("annotation-anchor");
    if (session.snapshot().currentSection !== inspected.start.section) {
      await session.open(index);
    }
    const resolved = resolve(store.item(id));
    if (!resolved || resolved === "write") return fail(lastError || "annotation-anchor");
    item = store.item(id);
    if (!item || !(await navigation.goTo(item.sourceAnchor.canonicalLocator))) {
      return fail("annotation-anchor");
    }
    lastError = null;
    sync();
    return Object.freeze({ ok: true });
  }

  async function restore() {
    await store.restore();
    await redraw();
    return snapshot();
  }

  function hideSelectionActions() {
    controls.selectionActions.hidden = true;
  }

  function dismissSelection() {
    hideSelectionActions();
    pendingSelection = null;
  }

  function finishSelection() {
    dismissSelection();
    content.book.getRootNode().getSelection?.().removeAllRanges();
  }

  function bind() {
    const showSelectionActions = () => {
      const range = currentSelection();
      const text = range?.toString() || "";
      if (!range || !text.trim() || text.length > ANNOTATION_MAX_SELECTED_LENGTH) {
        dismissSelection();
        return;
      }
      const rect = range.getBoundingClientRect();
      if (!rect.width && !rect.height) {
        dismissSelection();
        return;
      }
      pendingSelection = range;
      controls.selectionStatus.textContent = "";
      controls.selectionActions.hidden = false;
      const toolbar = controls.selectionActions.getBoundingClientRect();
      const maxLeft = Math.max(8, innerWidth - toolbar.width - 8);
      const maxTop = Math.max(8, innerHeight - toolbar.height - 8);
      const above = rect.top - toolbar.height - 8;
      controls.selectionActions.style.left = `${Math.min(maxLeft, Math.max(8, rect.left + rect.width / 2 - toolbar.width / 2))}px`;
      controls.selectionActions.style.top = `${Math.min(maxTop, Math.max(8, above >= 8 ? above : rect.bottom + 8))}px`;
    };
    const captureSelection = () => requestAnimationFrame(showSelectionActions);
    const copySelection = () => {
      if (!pendingSelection) return fail("annotation-selection");
      const selection = content.book.getRootNode().getSelection?.();
      try {
        if (!selection) throw new Error("copy");
        selection.removeAllRanges();
        selection.addRange(pendingSelection.cloneRange());
        if (!document.execCommand("copy")) throw new Error("copy");
        lastError = null;
        controls.selectionStatus.textContent = "已复制";
        finishSelection();
        sync();
        return Object.freeze({ ok: true });
      } catch {
        controls.selectionStatus.textContent = statusText("annotation-copy");
        return fail("annotation-copy");
      }
    };

    content.book.addEventListener("pointerdown", () => {
      hideSelectionActions();
      pendingSelection = null;
    });
    content.book.addEventListener("pointerup", captureSelection);
    content.book.addEventListener("keyup", captureSelection);
    document.addEventListener("selectionchange", () => {
      requestAnimationFrame(() => {
        if (
          !controls.noteDialog.open &&
          !controls.selectionActions.contains(document.activeElement) &&
          !currentSelection()
        ) {
          dismissSelection();
        }
      });
    });
    controls.selectionActions.addEventListener("pointerdown", (event) => event.preventDefault());
    controls.copy.addEventListener("click", copySelection);
    controls.highlight.addEventListener("click", async () => {
      if ((await addSelection("")).ok) hideSelectionActions();
    });
    controls.note.addEventListener("click", () => {
      if (!pendingSelection) return fail("annotation-selection");
      hideSelectionActions();
      controls.noteInput.value = "";
      controls.noteInput.setCustomValidity("");
      controls.noteDialog.showModal();
      requestAnimationFrame(() => controls.noteInput.focus());
    });
    controls.noteInput.addEventListener("input", () => controls.noteInput.setCustomValidity(""));
    controls.noteForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const note = controls.noteInput.value.trim();
      if (!note) {
        controls.noteInput.setCustomValidity("请输入笔记内容");
        controls.noteInput.reportValidity();
        return;
      }
      const result = pendingSelection && (await addRange(pendingSelection, note));
      if (result?.ok) controls.noteDialog.close();
    });
    controls.cancelNote.addEventListener("click", () => controls.noteDialog.close());
    controls.noteDialog.addEventListener("close", finishSelection);
    controls.list.addEventListener("click", async (event) => {
      const button = event.target.closest("button[data-annotation-id]");
      if (!button) return;
      const result = await go(button.dataset.annotationId);
      if (result.ok) onNavigate?.();
    });
    addEventListener("resize", dismissSelection);
    sync();
  }

  function firstTextRange() {
    const text = content.book.textContent || "";
    const start = text.search(/\S/u);
    return rangeForOffsets(start, Math.min(text.length, start + 8));
  }

  async function verifyPersistence(mode) {
    assert(store.snapshot().durable && store.snapshot().available, "state-persistence");
    if (mode === "write") {
      assert(store.clear(), "state-persistence");
      const range = firstTextRange();
      const result = range && (await addRange(range, "host-persistence-probe"));
      assert(result?.ok, "state-persistence");
      return;
    }
    assert(mode === "read", "state-persistence");
    const probe = store.active().find((item) => item.note === "host-persistence-probe");
    const valid = probe && store.sourceAnchor(probe.id)?.contentHash.length === 64 && overlayCount > 0;
    assert(store.clear(), "state-persistence");
    await redraw();
    assert(valid, "state-persistence");
  }

  async function verify() {
    const memory = new Map();
    const testStore = createAnnotationStore({
      storage: Object.freeze({
        getItem: (key) => memory.get(key) ?? null,
        removeItem: (key) => memory.delete(key),
        setItem: (key, value) => memory.set(key, value),
      }),
      requireDurable: true,
      keyPrefix: "atha.reader.probe",
      bookKey: "annotations",
      locator,
    });
    await testStore.restore();
    const range = firstTextRange();
    assert(range, "sample-boundary");
    const sourceAnchor = await sourceAnchorForRange(range);
    assert(
      !testStore.validSourceAnchor({
        ...sourceAnchor,
        canonicalLocator: JSON.stringify(JSON.parse(sourceAnchor.canonicalLocator), null, 2),
      }),
      "sample-boundary",
    );
    assert(
      !(await testStore.add({ ...sourceAnchor, contentHash: "0".repeat(64) }, "")).ok,
      "sample-boundary",
    );
    const staleLocator = JSON.parse(sourceAnchor.canonicalLocator);
    staleLocator.contentVersion = staleLocator.contentVersion === null ? "a".repeat(64) : "b".repeat(64);
    const stale = {
      id: "stale",
      sourceAnchor: { ...sourceAnchor, canonicalLocator: JSON.stringify(staleLocator) },
    };
    const unique = reanchor(stale);
    assert(
      unique?.sourceAnchor &&
        locator.parse(session.describe(), unique.sourceAnchor.canonicalLocator).end,
      "sample-boundary",
    );
    const duplicateCharacter = [...(content.book.textContent || "")].find(
      (value) => value.trim() && content.book.textContent.indexOf(value) !== content.book.textContent.lastIndexOf(value),
    );
    assert(duplicateCharacter, "sample-boundary");
    stale.sourceAnchor = { ...stale.sourceAnchor, selectedText: duplicateCharacter };
    assert(reanchor(stale) === false, "sample-boundary");
    stale.sourceAnchor = { ...stale.sourceAnchor, selectedText: "atha-annotation-missing" };
    assert(reanchor(stale) === false, "sample-boundary");
    const missingSectionLocator = JSON.parse(sourceAnchor.canonicalLocator);
    missingSectionLocator.start.section = "missing-section";
    missingSectionLocator.end.section = "missing-section";
    stale.sourceAnchor = {
      ...sourceAnchor,
      canonicalLocator: JSON.stringify(missingSectionLocator),
    };
    assert(reanchor(stale) === false, "sample-boundary");
    const mutableSourceAnchor = { ...sourceAnchor };
    const created = await testStore.add(mutableSourceAnchor, "验证笔记");
    mutableSourceAnchor.selectedText = "绕过事务";
    assert(
      created.ok &&
        testStore.validSourceAnchor(created.sourceAnchor) &&
        testStore.item(created.id).sourceAnchor.selectedText === sourceAnchor.selectedText &&
        Object.isFrozen(testStore.item(created.id).sourceAnchor),
      "sample-boundary",
    );
    const corruptMemory = new Map([
      [
        "atha.reader.probe.annotations.annotations.v1",
        JSON.stringify({
          schema: 1,
          items: [
            {
              ...testStore.item(created.id),
              sourceAnchor: { ...created.sourceAnchor, contentHash: "0".repeat(64) },
            },
          ],
        }),
      ],
    ]);
    const corruptStore = createAnnotationStore({
      storage: Object.freeze({
        getItem: (key) => corruptMemory.get(key) ?? null,
        removeItem: (key) => corruptMemory.delete(key),
        setItem: (key, value) => corruptMemory.set(key, value),
      }),
      requireDurable: true,
      keyPrefix: "atha.reader.probe",
      bookKey: "annotations",
      locator,
    });
    await corruptStore.restore();
    assert(
      !corruptStore.snapshot().available &&
        corruptStore.snapshot().lastError === "annotation-corrupt" &&
        corruptMemory.size === 1,
      "sample-boundary",
    );
    assert(testStore.updateNote(created.id, "更新笔记").ok, "sample-boundary");
    assert(
      testStore.remove(created.id).ok &&
        testStore.snapshot().tombstones === 1 &&
        testStore.sourceAnchor(created.id)?.selectedText === sourceAnchor.selectedText,
      "sample-boundary",
    );
    const failedStore = createAnnotationStore({
      storage: Object.freeze({
        getItem: () => null,
        removeItem: () => {},
        setItem: () => {
          throw new Error("full");
        },
      }),
      requireDurable: true,
      keyPrefix: "atha.reader.probe",
      bookKey: "annotations",
      locator,
    });
    await failedStore.restore();
    const failedWrite = await failedStore.add(sourceAnchor, "");
    assert(!failedWrite.ok && failedStore.active().length === 0, "sample-boundary");
    const selection = content.book.getRootNode().getSelection?.();
    const verifySelectionInput = async (event) => {
      selection.removeAllRanges();
      selection.addRange(firstTextRange());
      content.book.dispatchEvent(event);
      await new Promise(requestAnimationFrame);
      assert(
        !controls.selectionActions.hidden &&
          [controls.copy, controls.highlight, controls.note].every(
            (control) => !control.disabled && control.tabIndex >= 0,
          ),
        "sample-boundary",
      );
      dismissSelection();
    };
    assert(selection, "sample-boundary");
    await verifySelectionInput(
      new PointerEvent("pointerup", { bubbles: true, pointerType: "touch" }),
    );
    await verifySelectionInput(new KeyboardEvent("keyup", { bubbles: true, key: "ArrowRight" }));
    selection.removeAllRanges();
    content.book.dispatchEvent(new KeyboardEvent("keyup", { bubbles: true, key: "ArrowRight" }));
    await new Promise(requestAnimationFrame);
    assert(controls.selectionActions.hidden, "sample-boundary");
    return Object.freeze({
      sourceAnchor: true,
      noteUpdated: true,
      reanchored: true,
      ambiguousRejected: true,
      missingRejected: true,
      missingSectionRejected: true,
      corruptHashRejected: true,
      writeFailureRejected: true,
      softDeleted: true,
      touchSelectionActions: true,
      keyboardSelectionActions: true,
      invalidSelectionDismissed: true,
    });
  }

  function snapshot() {
    return Object.freeze({
      ...store.snapshot(),
      lastError: errorCode(),
      overlayCount,
      reanchorFailures,
    });
  }

  return Object.freeze({
    addSelection,
    bind,
    dismissSelection,
    go,
    redraw,
    remove,
    restore,
    snapshot,
    sourceAnchor: store.sourceAnchor,
    updateNote,
    verify,
    verifyPersistence,
  });
}
