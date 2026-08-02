const STRUCTURED_BLOCKS = new Set([
  "address",
  "blockquote",
  "dd",
  "div",
  "dl",
  "dt",
  "figcaption",
  "figure",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "li",
  "ol",
  "p",
  "ul",
]);

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
  let trigger = null;
  const counts = { table: 0, code: 0 };

  function projectedText(source) {
    let text = "";
    const walker = document.createTreeWalker(
      source,
      NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
    );
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (node.nodeType === Node.TEXT_NODE) text += node.data;
      else if (node instanceof HTMLImageElement) text += node.alt.slice(0, 160);
      else if (
        node instanceof HTMLBRElement ||
        (STRUCTURED_BLOCKS.has(node.localName) && text && !text.endsWith("\n"))
      ) {
        text += "\n";
      }
    }
    return text.trim();
  }

  function projectTable(source) {
    const table = document.createElement("table");
    if (source.caption) {
      const caption = document.createElement("caption");
      caption.textContent = projectedText(source.caption);
      table.append(caption);
    }
    for (const sourceRow of source.rows) {
      const row = document.createElement("tr");
      for (const sourceCell of sourceRow.cells) {
        const cell = document.createElement(sourceCell.localName === "th" ? "th" : "td");
        cell.textContent = projectedText(sourceCell);
        if (sourceCell.rowSpan > 1 && sourceCell.rowSpan <= 100) cell.rowSpan = sourceCell.rowSpan;
        if (sourceCell.colSpan > 1 && sourceCell.colSpan <= 100) cell.colSpan = sourceCell.colSpan;
        row.append(cell);
      }
      table.append(row);
    }
    return table;
  }

  function reset() {
    tablePreview.hidden = true;
    tablePreview.replaceChildren();
    codePreview.hidden = true;
    codePreview.textContent = "";
    dialog.classList.remove("structured-preview");
  }

  function show(source) {
    reset();
    body.hidden = true;
    image.hidden = true;
    image.removeAttribute("src");
    image.className = "";
    dialog.classList.remove("media-preview");
    const table = source.localName === "table";
    const preview = table ? tablePreview : codePreview;
    title.textContent = table ? "表格" : "代码";
    if (table) tablePreview.append(projectTable(source));
    else codePreview.textContent = source.textContent;
    preview.hidden = false;
    dialog.classList.add("structured-preview");
    counts[table ? "table" : "code"] += 1;
    trigger = source;
    if (!dialog.open) dialog.showModal();
    preview.focus();
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
      content.book.getRootNode().getSelection?.()?.isCollapsed === false
    ) {
      return;
    }
    const source = event.target.closest("table, pre");
    if (!source || !content.book.contains(source)) return;
    event.preventDefault();
    show(source);
  }

  function bind() {
    content.book.addEventListener("dblclick", onDoubleClick);
    content.book.addEventListener("keydown", onKeydown);
    dialog.addEventListener("close", () => {
      if (trigger?.isConnected) trigger.focus({ preventScroll: true });
      trigger = null;
      reset();
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
      source.dispatchEvent(
        key
          ? new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true })
          : new MouseEvent("dblclick", { bubbles: true, cancelable: true }),
      );
      assert(
        dialog.open &&
          body.hidden &&
          image.hidden &&
          preview.hidden === false &&
          document.activeElement === preview &&
          title.textContent === (table ? "表格" : "代码"),
        "sample-boundary",
      );
      if (table) {
        const projected = tablePreview.querySelector("table");
        const sourceCells = [...source.querySelectorAll("th, td")];
        const projectedCells = projected ? [...projected.querySelectorAll("th, td")] : [];
        assert(
          projected &&
            projected.caption?.textContent ===
              (source.caption ? projectedText(source.caption) : undefined) &&
            projected.querySelectorAll("tr").length === source.rows.length &&
            projectedCells.length === sourceCells.length &&
            projectedCells.every(
              (cell, index) =>
                cell.localName === sourceCells[index].localName &&
                cell.textContent === projectedText(sourceCells[index]) &&
                cell.rowSpan ===
                  (sourceCells[index].rowSpan > 1 && sourceCells[index].rowSpan <= 100
                    ? sourceCells[index].rowSpan
                    : 1) &&
                cell.colSpan ===
                  (sourceCells[index].colSpan > 1 && sourceCells[index].colSpan <= 100
                    ? sourceCells[index].colSpan
                    : 1),
            ) &&
            !projected.querySelector("a, img, style, script, iframe, object, embed, [style]"),
          "sample-boundary",
        );
      } else {
        assert(
          codePreview.textContent === source.textContent && codePreview.childElementCount === 0,
          "sample-boundary",
        );
      }
      assert(
        preview.tabIndex === 0 &&
          preview.getAttribute("aria-label") &&
          getComputedStyle(preview).overflowX === "auto" &&
          getComputedStyle(preview).overflowY === "auto",
        "sample-boundary",
      );
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
    const sourceTable = content.book.querySelector("table");
    const sourceCode = content.book.querySelector("pre");
    if (sourceTable?.querySelector("th, td")) {
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

  return Object.freeze({ bind, snapshot: () => Object.freeze({ ...counts }), verify });
}
