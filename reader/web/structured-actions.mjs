export function createStructuredActions({
  content,
  navigation,
  dialog,
  title,
  body,
  image,
  tablePreview,
  codePreview,
  contentActions,
  assert,
}) {
  const PREVIEW_IMAGE_CONCURRENCY = 3;
  let trigger = null;
  let previewLoad = null;
  const counts = { table: 0, code: 0 };

  function projectTable(source) {
    const table = source.cloneNode(true);
    const sourceImages = [...source.querySelectorAll("img")];
    const previewImages = [...table.querySelectorAll("img")];
    const sourceByPreview = new Map(
      previewImages.map((preview, index) => [preview, sourceImages[index]]),
    );
    const pending = [];
    for (const element of [table, ...table.querySelectorAll("*")]) {
      for (const attribute of [...element.attributes]) {
        const name = attribute.name.toLowerCase();
        if (
          name === "id" ||
          name === "style" ||
          name === "tabindex" ||
          name === "role" ||
          name === "contenteditable" ||
          name === "autofocus" ||
          name === "accesskey" ||
          name === "draggable" ||
          name === "data-atha-resource" ||
          name === "href" ||
          name.endsWith(":href") ||
          name.startsWith("on")
        ) {
          element.removeAttribute(attribute.name);
        }
      }
      if (element instanceof HTMLImageElement) {
        const sourceImage = sourceByPreview.get(element);
        const formula = [...element.classList].find((name) =>
          ["math-inline", "math-display"].includes(name),
        );
        element.className = formula || "";
        if (sourceImage?.matches(".atha-resource-pending[data-atha-resource]")) {
          element.classList.add("atha-preview-pending");
          element.setAttribute("aria-busy", "true");
          pending.push([sourceImage, element]);
        }
        element.loading = "eager";
        element.decoding = "async";
      } else {
        element.removeAttribute("class");
      }
    }
    return { table, pending };
  }

  function reset() {
    previewLoad?.abort();
    previewLoad = null;
    tablePreview.hidden = true;
    tablePreview.replaceChildren();
    codePreview.hidden = true;
    codePreview.textContent = "";
    dialog.classList.remove("structured-preview", "table-preview", "code-preview");
  }

  function loadPreviewImages(pending, materialize = content.materializePreviewImage) {
    if (pending.length === 0) return Promise.resolve();
    const controller = new AbortController();
    previewLoad = controller;
    const queue = [...pending];
    controller.signal.addEventListener("abort", () => queue.splice(0), { once: true });
    const worker = async () => {
      while (!controller.signal.aborted) {
        const pair = queue.shift();
        if (!pair) return;
        await materialize(...pair, controller.signal);
      }
    };
    return Promise.all(
      Array.from({ length: Math.min(PREVIEW_IMAGE_CONCURRENCY, queue.length) }, worker),
    ).finally(() => {
      if (previewLoad === controller) previewLoad = null;
    });
  }

  function show(source, materialize) {
    reset();
    body.hidden = true;
    image.hidden = true;
    image.removeAttribute("src");
    image.className = "";
    dialog.classList.remove("media-preview");
    const table = source.localName === "table";
    const preview = table ? tablePreview : codePreview;
    let loaded = Promise.resolve();
    title.textContent = table ? "表格" : "代码";
    if (table) {
      const projection = projectTable(source);
      tablePreview.append(projection.table);
      loaded = loadPreviewImages(projection.pending, materialize);
    }
    else codePreview.textContent = source.textContent;
    preview.hidden = false;
    dialog.classList.add("structured-preview", table ? "table-preview" : "code-preview");
    counts[table ? "table" : "code"] += 1;
    trigger = source;
    if (!dialog.open) dialog.showModal();
    preview.focus();
    return loaded;
  }

  function onClick(event) {
    if (
      !(event.target instanceof Element) ||
      event.target.closest("a[href]") ||
      content.selectionRange()
    ) {
      return;
    }
    const source = event.target.closest("table");
    if (!source || !content.book.contains(source)) return;
    event.preventDefault();
    show(source);
  }

  function onKeydown(event) {
    if (!["Enter", " "].includes(event.key) || !(event.target instanceof Element)) return;
    if (event.target.closest("a[href]")) return;
    const source = event.target.closest("table, pre");
    if (!source || event.target !== source || !content.book.contains(source)) return;
    event.preventDefault();
    show(source);
  }

  function onDoubleClick(event) {
    if (
      !(event.target instanceof Element) ||
      event.target.closest("a[href], img[role='button']") ||
      content.selectionRange()
    ) {
      return;
    }
    const source = event.target.closest("table, pre");
    if (!source || !content.book.contains(source)) return;
    event.preventDefault();
    show(source);
  }

  function bind() {
    content.book.addEventListener("click", onClick);
    content.book.addEventListener("dblclick", onDoubleClick);
    content.book.addEventListener("keydown", onKeydown);
    dialog.addEventListener("close", () => {
      if (trigger?.isConnected) trigger.focus({ preventScroll: true });
      trigger = null;
      reset();
    });
  }

  async function verifyPendingFormulaQueue() {
    const formulaCount = 8;
    const waitFor = async (predicate) => {
      for (let turn = 0; turn < 120; turn += 1) {
        if (predicate()) return;
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
      assert(false, "sample-boundary");
    };
    assert(!dialog.open, "sample-boundary");

    const source = document.createElement("table");
    for (let index = 0; index < formulaCount; index += 1) {
      const formula = document.createElement("img");
      formula.className = `${index % 2 ? "math-inline" : "math-display"} atha-resource-pending`;
      formula.dataset.athaResource = `https://atha-book.localhost/formula-${index}.svg`;
      formula.alt = `formula-${index}`;
      formula.width = 8;
      formula.height = 4;
      source.insertRow().insertCell().append(formula);
    }

    let active = 0;
    let started = 0;
    let maxInFlight = 0;
    let abortedInFlight = 0;
    let lateDetachedWrites = 0;
    const releases = [];
    const blockedMaterialize = (_source, preview, signal) =>
      new Promise((resolve) => {
        active += 1;
        started += 1;
        maxInFlight = Math.max(maxInFlight, active);
        let aborted = false;
        const onAbort = () => {
          if (aborted) return;
          aborted = true;
          abortedInFlight += 1;
        };
        signal.addEventListener("abort", onAbort, { once: true });
        releases.push(() => {
          signal.removeEventListener("abort", onAbort);
          active -= 1;
          if (!signal.aborted && preview.isConnected) {
            preview.src = "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'/%3E";
            lateDetachedWrites += 1;
          }
          resolve(false);
        });
      });

    const firstLoad = show(source, blockedMaterialize);
    const firstProjection = [...tablePreview.querySelectorAll("img")];
    assert(
      dialog.open &&
        firstProjection.length === formulaCount &&
        started === PREVIEW_IMAGE_CONCURRENCY &&
        maxInFlight === PREVIEW_IMAGE_CONCURRENCY,
      "sample-boundary",
    );
    dialog.close();
    await waitFor(
      () =>
        !dialog.open &&
        tablePreview.childElementCount === 0 &&
        abortedInFlight === PREVIEW_IMAGE_CONCURRENCY,
    );
    assert(
      firstProjection.every((formula) => !formula.isConnected && !formula.hasAttribute("src")),
      "sample-boundary",
    );

    const loadedFormula =
      "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='8'%20height='4'/%3E";
    const reopenedLoad = show(source, async (_source, preview, signal) => {
      assert(!signal.aborted && preview.isConnected, "sample-boundary");
      preview.src = loadedFormula;
      preview.classList.remove("atha-preview-pending");
      preview.removeAttribute("aria-busy");
      return true;
    });
    await reopenedLoad;
    const reopened = [...tablePreview.querySelectorAll("img")];
    assert(
      dialog.open &&
        reopened.length === formulaCount &&
        reopened.every(
          (formula) =>
            formula instanceof HTMLImageElement &&
            formula.src === loadedFormula &&
            !formula.classList.contains("atha-preview-pending") &&
            !formula.hasAttribute("aria-busy"),
        ),
      "sample-boundary",
    );

    for (const release of releases) release();
    await firstLoad;
    await Promise.resolve();
    assert(
      started === PREVIEW_IMAGE_CONCURRENCY &&
        active === 0 &&
        lateDetachedWrites === 0 &&
        firstProjection.every((formula) => !formula.hasAttribute("src")) &&
        reopened.every((formula) => formula.isConnected && formula.src === loadedFormula),
      "sample-boundary",
    );

    dialog.close();
    await waitFor(() => !dialog.open && tablePreview.childElementCount === 0);
    return Object.freeze({
      formulas: formulaCount,
      maxInFlight,
      startedBeforeClose: started,
      notStartedAfterClose: formulaCount - started,
      abortedInFlight,
      lateDetachedWrites,
      reopenedImages: reopened.length,
      reopenedPending: reopened.filter((formula) =>
        formula.classList.contains("atha-preview-pending"),
      ).length,
    });
  }

  async function verify({ pagination, session }) {
    const waitFor = async (predicate) => {
      for (let frame = 0; frame < 120; frame += 1) {
        if (predicate()) return;
        await new Promise(requestAnimationFrame);
      }
      assert(false, "sample-boundary");
    };
    const probe = async (source, key) => {
      const table = source.localName === "table";
      const preview = table ? tablePreview : codePreview;
      const pageBefore = pagination.snapshot().page;
      const sectionBefore = session.snapshot().currentIndex;
      const locatorBefore = JSON.stringify(navigation.current());
      source.focus({ preventScroll: true });
      if (key) {
        source.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
      } else if (table) source.click();
      else source.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
      assert(
        dialog.open &&
          body.hidden &&
          image.hidden &&
          preview.hidden === false &&
          document.activeElement === preview &&
          title.textContent === (table ? "表格" : "代码") &&
          dialog.classList.contains(table ? "table-preview" : "code-preview"),
        "sample-boundary",
      );
      if (table) {
        const projected = tablePreview.querySelector("table");
        const sourceCells = [...source.querySelectorAll("th, td")];
        const projectedCells = projected ? [...projected.querySelectorAll("th, td")] : [];
        assert(
          projected &&
            projected.caption?.textContent === source.caption?.textContent &&
            projected.querySelectorAll("tr").length === source.rows.length &&
            projectedCells.length === sourceCells.length &&
            projectedCells.every(
              (cell, index) =>
                cell.localName === sourceCells[index].localName &&
                cell.textContent === sourceCells[index].textContent &&
                cell.rowSpan ===
                  (sourceCells[index].rowSpan > 1 && sourceCells[index].rowSpan <= 100
                    ? sourceCells[index].rowSpan
                    : 1) &&
                cell.colSpan ===
                  (sourceCells[index].colSpan > 1 && sourceCells[index].colSpan <= 100
                    ? sourceCells[index].colSpan
                    : 1),
            ) &&
            projected.querySelectorAll("img").length === source.querySelectorAll("img").length &&
            !projected.querySelector(
              "a[href], style, script, iframe, object, embed, [style], [id], [tabindex], [role]",
            ),
          "sample-boundary",
        );
      } else {
        assert(
          codePreview.textContent === source.textContent && codePreview.childElementCount === 0,
          "sample-boundary",
        );
      }
      const previewOverflow = getComputedStyle(preview);
      const viewport = document.querySelector("#content-dialog-viewport");
      assert(
        preview.tabIndex === 0 &&
          preview.getAttribute("aria-label") &&
          (table
            ? viewport &&
              getComputedStyle(viewport).overflowX === "auto" &&
              previewOverflow.overflowX === "visible"
            : previewOverflow.overflowX === "auto" && previewOverflow.overflowY === "auto"),
        "sample-boundary",
      );
      if (table) {
        const zoomIn = dialog.querySelector("button[aria-label='放大']");
        const resetZoom = dialog.querySelector("button[aria-label='恢复原始大小']");
        const zoomLabel = dialog.querySelector(".content-dialog-zoom-label");
        zoomIn?.click();
        await new Promise(requestAnimationFrame);
        assert(
          zoomIn &&
            resetZoom &&
            zoomLabel?.textContent === "125%" &&
            dialog.style.getPropertyValue("--content-viewer-scale").trim() === "1.25",
          "sample-boundary",
        );
        resetZoom.click();
        await new Promise(requestAnimationFrame);
        assert(zoomLabel.textContent === "100%", "sample-boundary");
        preview.focus();
      }
      dialog.close();
      await waitFor(
        () => !dialog.open && content.book.getRootNode().activeElement === source,
      );
      assert(
        content.book.getRootNode().activeElement === source &&
          pagination.snapshot().page === pageBefore &&
          session.snapshot().currentIndex === sectionBefore &&
          JSON.stringify(navigation.current()) === locatorBefore,
        "sample-boundary",
      );
      return true;
    };

    await session.open(0);
    await pagination.show(0);
    const richTable = document.createElement("table");
    const richCell = richTable.insertRow().insertCell();
    const richLink = document.createElement("a");
    const richFormula = document.createElement("img");
    richLink.href = "#formula";
    richFormula.src =
      "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='8'%20height='4'/%3E";
    richFormula.className = "math-display book-class";
    richFormula.alt = "x + y";
    richFormula.width = 8;
    richFormula.height = 4;
    richFormula.setAttribute("contenteditable", "true");
    richFormula.setAttribute("autofocus", "");
    richFormula.setAttribute("accesskey", "x");
    richFormula.setAttribute("draggable", "true");
    richLink.append(richFormula);
    richCell.append(richLink);
    const richProjection = projectTable(richTable).table;
    const projectedFormula = richProjection.querySelector("img.math-display");
    assert(
      projectedFormula?.src === richFormula.src &&
        projectedFormula.alt === "x + y" &&
        projectedFormula.width === 8 &&
        projectedFormula.height === 4 &&
        !richProjection.querySelector(
          "a[href], .book-class, [tabindex], [role], [contenteditable], [autofocus], [accesskey], [draggable]",
        ),
      "sample-boundary",
    );
    const pendingTable = document.createElement("table");
    const pendingFormula = document.createElement("img");
    pendingFormula.className = "math-inline atha-resource-pending";
    pendingFormula.dataset.athaResource = "https://atha-book.localhost/formula.svg";
    pendingTable.insertRow().insertCell().append(pendingFormula);
    const pendingProjection = projectTable(pendingTable);
    assert(
      pendingProjection.pending.length === 1 &&
        pendingProjection.table.querySelector("img.math-inline.atha-preview-pending") &&
        !pendingProjection.table.querySelector("[data-atha-resource]"),
      "sample-boundary",
    );
    const sourceTable = content.book.querySelector("table");
    const sourceCode = content.book.querySelector("pre");
    if (sourceTable?.querySelector("th, td")) {
      const frame = sourceTable.closest(".atha-table-frame");
      assert(
        frame &&
          getComputedStyle(frame).overflowX === "hidden" &&
          getComputedStyle(sourceTable).tableLayout === "fixed" &&
          sourceTable.offsetWidth <= frame.clientWidth + 1,
        "sample-boundary",
      );
      const nested = sourceTable.querySelector("th, td");
      nested.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
      );
      assert(!dialog.open, "sample-boundary");
    }
    const tablePreviewed = sourceTable ? await probe(sourceTable) : null;
    const codePreviewed = sourceCode ? await probe(sourceCode, " ") : null;

    const selectable = sourceCode || sourceTable;
    const text = selectable
      ? document
          .createTreeWalker(selectable, NodeFilter.SHOW_TEXT, {
            acceptNode: (node) =>
              node.data.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP,
          })
          .nextNode()
      : null;
    const selection = content.book.getRootNode().getSelection?.();
    if (selectable && text && selection && text.length > 0) {
      const start = text.data.search(/\S/);
      const range = document.createRange();
      range.setStart(text, start);
      range.setEnd(text, start + 1);
      selection.removeAllRanges();
      selection.addRange(range);
      assert(selection.toString(), "sample-boundary");
      const count = counts.table + counts.code;
      selectable.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
      assert(!dialog.open && counts.table + counts.code === count, "sample-boundary");
      selection.removeAllRanges();
    }

    const structuredLink = content.book.querySelector("table a[href], pre a[href]");
    let structuredLinkProtected = null;
    if (structuredLink) {
      const count = counts.table + counts.code;
      const internal = contentActions.snapshot().internal;
      structuredLink.click();
      await contentActions.idle();
      assert(
        !dialog.open &&
          counts.table + counts.code === count &&
          contentActions.snapshot().internal === internal + 1,
        "sample-boundary",
      );
      structuredLinkProtected = true;
      await session.open(0);
      await pagination.show(0);
    }

    return Object.freeze({
      ...counts,
      tablePreview: tablePreviewed,
      codePreview: codePreviewed,
      structuredLinkProtected,
      structuredPagePreserved: true,
      structuredProjectionSafe: true,
      structuredSelectionProtected: true,
    });
  }

  return Object.freeze({
    bind,
    snapshot: () => Object.freeze({ ...counts }),
    verify,
    verifyPendingFormulaQueue,
  });
}
