const ANNOTATION_MAX_SELECTED_LENGTH = 4096;
const ANNOTATION_MAX_NOTE_LENGTH = 2000;
const ANNOTATION_CONTEXT_LENGTH = 32;
const HIGHLIGHT_NAME = "atha-annotations";
const SELECTION_CHANGE_SETTLE_MS = 120;

export function dispatchDictionaryLookup(selection, target = globalThis) {
  if (!selection) return false;
  target.dispatchEvent(
    new CustomEvent("atha:dictionary-lookup", {
      detail: Object.freeze({ query: selection.toString() }),
    }),
  );
  return true;
}

export function createAnnotations({
  store,
  content,
  session,
  navigation,
  locator,
  controls,
  onNavigate,
  onOpenConversation,
  assert,
}) {
  let lastError = null;
  let overlayCount = 0;
  let reanchorFailures = 0;
  let pendingSelection = null;
  let selectedAnnotationId = null;
  let editingNoteId = null;
  let rangeEditingId = null;
  let renderedRanges = new Map();
  let filterGeneration = 0;
  let filterTimer = null;
  let selectionChangeTimer = null;
  let searchConversations = null;

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
    return content.selectionRange();
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
      "annotation-search": "笔记搜索失败",
      "annotation-copy": "复制失败，请使用系统复制命令",
    }[code];
  }

  function sync() {
    const all = store.active();
    const query = controls.filterQuery.value.trim().toLocaleLowerCase();
    const section = controls.filterSection.value;
    const active = all.filter((item) => {
      const itemSection = locator.inspect(item.sourceAnchor.canonicalLocator).start.section;
      if (section && itemSection !== section) return false;
      if (!query) return true;
      if (searchConversations) return searchConversations.has(item.conversationId);
      return `${item.sourceAnchor.selectedText}\n${item.note}`.toLocaleLowerCase().includes(query);
    });
    controls.list.replaceChildren(
      ...active.map((item) => {
        const row = document.createElement("article");
        const button = document.createElement("button");
        const kind = document.createElement("span");
        const quote = document.createElement("span");
        const actions = document.createElement("span");
        const edit = document.createElement("button");
        const removeButton = document.createElement("button");
        row.className = "annotation-item";
        button.type = "button";
        button.className = "annotation-item-main";
        button.dataset.annotationId = item.id;
        button.dataset.annotationAction = onOpenConversation ? "conversation" : "go";
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
        actions.className = "annotation-item-actions";
        edit.type = "button";
        edit.className = "annotation-item-action annotation-item-edit";
        edit.dataset.annotationId = item.id;
        edit.dataset.annotationAction = "edit";
        edit.textContent = "编辑";
        edit.setAttribute("aria-label", item.note ? "编辑笔记" : "为标注添加笔记");
        removeButton.type = "button";
        removeButton.className = "annotation-item-action annotation-item-delete";
        removeButton.dataset.annotationId = item.id;
        removeButton.dataset.annotationAction = "delete";
        removeButton.textContent = "删除";
        removeButton.setAttribute("aria-label", "删除标注");
        actions.append(edit, removeButton);
        row.append(button, actions);
        return row;
      }),
    );
    const code = errorCode();
    controls.status.dataset.error = String(Boolean(code));
    controls.status.textContent =
      statusText(code) ||
      (query || section
        ? `${active.length} 条符合条件，共 ${all.length} 条`
        : `${active.length} 条笔记与标注`);
  }

  async function applyFilters() {
    const generation = ++filterGeneration;
    const query = controls.filterQuery.value.trim();
    const section = controls.filterSection.value || null;
    searchConversations = null;
    sync();
    if (!query || typeof store.search !== "function") return;
    try {
      const hits = await store.search(session.describe().contentVersion, query, section);
      if (generation !== filterGeneration) return;
      searchConversations = new Set(hits.map((hit) => hit.conversationId));
      if (lastError === "annotation-search") lastError = null;
    } catch {
      if (generation !== filterGeneration) return;
      lastError = "annotation-search";
      searchConversations = new Set();
    }
    sync();
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

  async function resolve(item) {
    const candidate = reanchor(item);
    if (!candidate || candidate === false || !candidate.sourceAnchor) {
      return candidate && candidate.range;
    }
    const replaced = await store.reanchor(item.id, candidate.sourceAnchor);
    if (!replaced.ok) lastError = replaced.error;
    return replaced.ok ? candidate.range : "write";
  }

  async function redraw() {
    dismissSelection();
    if (!globalThis.CSS?.highlights || typeof globalThis.Highlight !== "function") {
      renderedRanges = new Map();
      lastError = "annotation-overlay";
      sync();
      return false;
    }
    const ranges = [];
    const nextRenderedRanges = new Map();
    reanchorFailures = 0;
    for (const item of store.active()) {
      const range = await resolve(item);
      if (range === false) reanchorFailures += 1;
      else if (range !== "write" && range) {
        ranges.push(range);
        nextRenderedRanges.set(item.id, range);
      }
    }
    CSS.highlights.delete(HIGHLIGHT_NAME);
    if (ranges.length) CSS.highlights.set(HIGHLIGHT_NAME, new Highlight(...ranges));
    renderedRanges = nextRenderedRanges;
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
    const result = await store.add(sourceAnchor, note, range);
    if (!result.ok) return fail(result.error);
    lastError = null;
    pendingSelection = null;
    content.book.getRootNode().getSelection?.().removeAllRanges();
    await redraw();
    if (note && result.conversationId) onOpenConversation?.(result.conversationId, result.id);
    return result;
  }

  async function addSelection(note) {
    const range = currentSelection() || pendingSelection;
    return range ? addRange(range, note) : fail("annotation-selection");
  }

  async function updateNote(id, note) {
    if (String(note).trim().length > ANNOTATION_MAX_NOTE_LENGTH) return fail("annotation-note");
    const result = await store.updateNote(id, note);
    if (!result.ok) return fail(result.error);
    lastError = null;
    sync();
    return result;
  }

  async function updateRange(id, range) {
    let sourceAnchor;
    try {
      sourceAnchor = await sourceAnchorForRange(range);
    } catch {
      return fail("annotation-selection");
    }
    const result = await store.replaceAnchor(id, sourceAnchor, range);
    if (!result.ok) return fail(result.error);
    lastError = null;
    content.book.getRootNode().getSelection?.().removeAllRanges();
    await redraw();
    return result;
  }

  async function remove(id) {
    const result = await store.remove(id);
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
      if (!(await navigation.goTo(item.sourceAnchor.canonicalLocator))) {
        return fail("annotation-anchor");
      }
    }
    const resolved = await resolve(store.item(id));
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

  function cancelPendingSelectionChange() {
    clearTimeout(selectionChangeTimer);
    selectionChangeTimer = null;
  }

  function dismissSelection() {
    cancelPendingSelectionChange();
    hideSelectionActions();
    pendingSelection = null;
    selectedAnnotationId = null;
    rangeEditingId = null;
  }

  function finishSelection() {
    dismissSelection();
    editingNoteId = null;
    content.book.getRootNode().getSelection?.().removeAllRanges();
  }

  function bind() {
    const description = session.describe();
    controls.filterSection.append(
      ...description.sections.map((section, index) => {
        const option = document.createElement("option");
        option.value = section.id;
        const labels = description.toc
          .filter((item) => item.href.split("#", 1)[0] === section.href)
          .map((item) => item.label);
        option.textContent =
          labels.length > 1 ? `${labels[0]} – ${labels.at(-1)}` : labels[0] || `第 ${index + 1} 节`;
        return option;
      }),
    );
    const scheduleFilters = () => {
      clearTimeout(filterTimer);
      filterTimer = setTimeout(() => applyFilters(), 150);
    };
    controls.filterQuery.addEventListener("input", scheduleFilters);
    controls.filterSection.addEventListener("change", () => applyFilters());
    const selectedItem = () => selectedAnnotationId && store.item(selectedAnnotationId);
    const matchingAnnotation = (range) => {
      const start = offsetForBoundary(range.startContainer, range.startOffset);
      const end = offsetForBoundary(range.endContainer, range.endOffset);
      if (start === null || end === null) return null;
      return (
        [...renderedRanges.entries()]
          .map(([id, candidate]) => ({
            item: store.item(id),
            start: offsetForBoundary(candidate.startContainer, candidate.startOffset),
            end: offsetForBoundary(candidate.endContainer, candidate.endOffset),
          }))
          .filter(({ item, start: candidateStart, end: candidateEnd }) =>
            Boolean(item && candidateStart === start && candidateEnd === end),
          )
          .sort((left, right) => right.item.updatedAt - left.item.updatedAt)[0]?.item || null
      );
    };
    const setSelectionMode = (item) => {
      controls.highlight.hidden = Boolean(item);
      controls.update.hidden = !item;
      controls.delete.hidden = !item;
      const updateLabel = rangeEditingId === item?.id ? "保存" : "重选";
      const noteLabel = item?.note ? "编辑笔记" : "笔记";
      controls.update.querySelector("span").textContent = updateLabel;
      controls.update.setAttribute("aria-label", updateLabel);
      controls.update.title = updateLabel;
      controls.note.querySelector("span").textContent = noteLabel;
      controls.note.setAttribute("aria-label", noteLabel);
      controls.note.title = noteLabel;
      controls.selectionActions.setAttribute(
        "aria-label",
        item ? "已有标注操作" : "选中文字操作",
      );
    };
    const positionSelectionActions = (range) => {
      const rect = range.getBoundingClientRect();
      if (!rect.width && !rect.height) return false;
      controls.selectionActions.hidden = false;
      const toolbar = controls.selectionActions.getBoundingClientRect();
      const maxLeft = Math.max(8, innerWidth - toolbar.width - 8);
      const maxTop = Math.max(8, innerHeight - toolbar.height - 8);
      const above = rect.top - toolbar.height - 8;
      controls.selectionActions.style.left = `${Math.min(maxLeft, Math.max(8, rect.left + rect.width / 2 - toolbar.width / 2))}px`;
      controls.selectionActions.style.top = `${Math.min(maxTop, Math.max(8, above >= 8 ? above : rect.bottom + 8))}px`;
      return true;
    };
    const showSelectionActions = () => {
      const range = currentSelection();
      const text = range?.toString() || "";
      if (!range || !text.trim() || text.length > ANNOTATION_MAX_SELECTED_LENGTH) {
        dismissSelection();
        return;
      }
      const item = selectedItem() || matchingAnnotation(range);
      selectedAnnotationId = item?.id || null;
      setSelectionMode(item);
      if (!positionSelectionActions(range)) {
        dismissSelection();
        return;
      }
      pendingSelection = range;
      controls.selectionStatus.textContent = "";
    };
    const captureSelection = () => {
      cancelPendingSelectionChange();
      requestAnimationFrame(showSelectionActions);
    };
    const selectAnnotationAtPoint = (event) => {
      if (event.button !== 0 || !event.isPrimary) return false;
      // ponytail: O(n) only on pointer-up; add a spatial index if the 1000-item cap becomes measurable.
      const item = [...renderedRanges.entries()]
        .filter(([, range]) =>
          [...range.getClientRects()].some(
            (rect) =>
              event.clientX >= rect.left &&
              event.clientX <= rect.right &&
              event.clientY >= rect.top &&
              event.clientY <= rect.bottom,
          ),
        )
        .map(([id]) => store.item(id))
        .filter(Boolean)
        .sort((left, right) => right.updatedAt - left.updatedAt)[0];
      if (!item) return false;
      const range = renderedRanges.get(item.id);
      const selection = content.book.getRootNode().getSelection?.();
      if (!range || !selection) return false;
      selection.removeAllRanges();
      selection.addRange(range.cloneRange());
      selectedAnnotationId = item.id;
      pendingSelection = range.cloneRange();
      requestAnimationFrame(showSelectionActions);
      return true;
    };
    const openNoteDialog = (id = null) => {
      const item = id ? store.item(id) : null;
      if (id && !item) return fail("annotation-missing");
      editingNoteId = item?.id || null;
      controls.noteHeading.textContent = item ? (item.note ? "编辑笔记" : "添加笔记") : "添加笔记";
      controls.noteInput.value = item?.note || "";
      controls.noteInput.setCustomValidity("");
      controls.noteDialog.showModal();
      requestAnimationFrame(() => controls.noteInput.focus());
      return Object.freeze({ ok: true });
    };
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
      cancelPendingSelectionChange();
      hideSelectionActions();
      pendingSelection = null;
      if (!rangeEditingId) selectedAnnotationId = null;
    });
    content.book.addEventListener("pointerup", (event) => {
      if (currentSelection()) {
        captureSelection();
        return;
      }
      if (selectAnnotationAtPoint(event)) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    });
    content.book.addEventListener("keyup", captureSelection);
    document.addEventListener("selectionchange", () => {
      cancelPendingSelectionChange();
      selectionChangeTimer = setTimeout(() => {
        selectionChangeTimer = null;
        requestAnimationFrame(() => {
          if (!controls.noteDialog.open && currentSelection()) {
            showSelectionActions();
            return;
          }
          if (
            !controls.noteDialog.open &&
            !controls.selectionActions.contains(document.activeElement) &&
            !currentSelection()
          ) {
            hideSelectionActions();
            pendingSelection = null;
            if (!rangeEditingId) selectedAnnotationId = null;
          }
        });
      }, SELECTION_CHANGE_SETTLE_MS);
    });
    controls.selectionActions.addEventListener("pointerdown", (event) => event.preventDefault());
    controls.lookup?.addEventListener("click", () => {
      if (!dispatchDictionaryLookup(pendingSelection)) return fail("annotation-selection");
      finishSelection();
    });
    controls.copy.addEventListener("click", copySelection);
    controls.highlight.addEventListener("click", async () => {
      if ((await addSelection("")).ok) hideSelectionActions();
    });
    controls.update.addEventListener("click", async () => {
      const item = selectedItem();
      if (!item) return fail("annotation-missing");
      if (rangeEditingId !== item.id) {
        rangeEditingId = item.id;
        pendingSelection = null;
        controls.selectionStatus.textContent = "请重新选择标注文字";
        hideSelectionActions();
        content.book.getRootNode().getSelection?.().removeAllRanges();
        return;
      }
      if (!pendingSelection) return fail("annotation-selection");
      await updateRange(item.id, pendingSelection);
    });
    controls.note.addEventListener("click", async () => {
      const item = selectedItem();
      if (!item && !pendingSelection) return fail("annotation-selection");
      if (onOpenConversation) {
        const result = item
          ? { ok: true, id: item.id, conversationId: item.conversationId }
          : await addSelection("");
        if (result?.ok) {
          finishSelection();
          await onOpenConversation(result.conversationId, result.id, true);
        }
        return;
      }
      hideSelectionActions();
      openNoteDialog(item?.id || null);
    });
    controls.delete.addEventListener("click", async () => {
      const item = selectedItem();
      if (!item) return fail("annotation-missing");
      await remove(item.id);
      finishSelection();
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
      const result = editingNoteId
        ? await updateNote(editingNoteId, note)
        : pendingSelection && (await addRange(pendingSelection, note));
      if (result?.ok) controls.noteDialog.close();
    });
    controls.cancelNote.addEventListener("click", () => controls.noteDialog.close());
    controls.noteDialog.addEventListener("close", finishSelection);
    controls.list.addEventListener("click", async (event) => {
      const button = event.target.closest("button[data-annotation-id]");
      if (!button) return;
      if (button.dataset.annotationAction === "edit") {
        const item = store.item(button.dataset.annotationId);
        if (item && onOpenConversation) {
          await onOpenConversation(item.conversationId, item.id, true);
        } else {
          openNoteDialog(button.dataset.annotationId);
        }
      } else if (button.dataset.annotationAction === "delete") {
        await remove(button.dataset.annotationId);
      } else if (button.dataset.annotationAction === "conversation") {
        const item = store.item(button.dataset.annotationId);
        if (item) {
          await onOpenConversation(item.conversationId, item.id);
        }
      } else {
        const result = await go(button.dataset.annotationId);
        if (result.ok) onNavigate?.();
      }
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
      testStore.replaceAnchor(created.id, sourceAnchor).ok &&
        testStore.item(created.id).note === "更新笔记" &&
        testStore.item(created.id).sourceAnchor.selectedText === sourceAnchor.selectedText,
      "sample-boundary",
    );
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
    selectedAnnotationId = "stale-selection";
    rangeEditingId = null;
    content.book.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    assert(selectedAnnotationId === null, "sample-boundary");
    const selection = content.book.getRootNode().getSelection?.();
    const verifySelectionInput = async (event, target = content.book, settle = false) => {
      selection.removeAllRanges();
      selection.addRange(firstTextRange());
      target.dispatchEvent(event);
      if (settle) {
        await new Promise((resolve) => setTimeout(resolve, SELECTION_CHANGE_SETTLE_MS + 20));
      }
      await new Promise(requestAnimationFrame);
      assert(
        !controls.selectionActions.hidden &&
          [controls.copy, controls.note].every(
            (control) => !control.disabled && control.tabIndex >= 0,
          ) &&
          ((!controls.highlight.hidden && controls.update.hidden && controls.delete.hidden) ||
            (controls.highlight.hidden && !controls.update.hidden && !controls.delete.hidden)),
        "sample-boundary",
      );
      dismissSelection();
    };
    assert(selection, "sample-boundary");
    await verifySelectionInput(
      new PointerEvent("pointerup", { bubbles: true, pointerType: "touch" }),
    );
    await verifySelectionInput(new KeyboardEvent("keyup", { bubbles: true, key: "ArrowRight" }));
    await verifySelectionInput(new Event("selectionchange"), document, true);
    selection.removeAllRanges();
    selection.addRange(firstTextRange());
    document.dispatchEvent(new Event("selectionchange"));
    dismissSelection();
    await new Promise((resolve) => setTimeout(resolve, SELECTION_CHANGE_SETTLE_MS + 20));
    await new Promise(requestAnimationFrame);
    assert(controls.selectionActions.hidden, "sample-boundary");
    selection.removeAllRanges();
    content.book.dispatchEvent(new KeyboardEvent("keyup", { bubbles: true, key: "ArrowRight" }));
    await new Promise(requestAnimationFrame);
    assert(controls.selectionActions.hidden, "sample-boundary");
    return Object.freeze({
      sourceAnchor: true,
      noteUpdated: true,
      rangeUpdated: true,
      reanchored: true,
      ambiguousRejected: true,
      missingRejected: true,
      missingSectionRejected: true,
      corruptHashRejected: true,
      writeFailureRejected: true,
      softDeleted: true,
      freshSelectionClearsAnnotation: true,
      touchSelectionActions: true,
      keyboardSelectionActions: true,
      selectionChangeActions: true,
      selectionChangeDismissed: true,
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
