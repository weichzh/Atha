const SOURCE_FONT_SIZE = 16;
const DISPLAY_FORMULA_MULTIPLIER = 1.5;

export function createPagination({
  book,
  reader,
  page,
  position,
  progressRange,
  previous,
  next,
  fontSizeControl,
  onPageShown,
  assert,
  fail,
}) {
  const state = { page: 0, pages: 0, fontSize: 32 };

  function textNodes() {
    const nodes = [];
    const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  function withoutTranslation(action) {
    const transform = book.style.transform;
    book.style.transform = "none";
    try {
      return action();
    } finally {
      book.style.transform = transform;
    }
  }

  function columnForRect(rect, bookLeft, step, scale) {
    return Math.max(0, Math.floor((rect.left - bookLeft) / (step * scale)));
  }

  function captureOffset() {
    return withoutTranslation(() => {
      const style = getComputedStyle(book);
      const step = parseFloat(style.width) + parseFloat(style.columnGap);
      const bookLeft = book.getBoundingClientRect().left;
      const scale = reader.getBoundingClientRect().width / reader.clientWidth;
      let base = 0;
      for (const node of textNodes()) {
        const text = node.textContent || "";
        const whole = document.createRange();
        whole.selectNodeContents(node);
        const reachesPage = [...whole.getClientRects()].some(
          (rect) => rect.width && columnForRect(rect, bookLeft, step, scale) === state.page,
        );
        if (reachesPage) {
          for (let offset = 0; offset < text.length; offset += 1) {
            const character = document.createRange();
            character.setStart(node, offset);
            character.setEnd(node, offset + 1);
            const rect = [...character.getClientRects()].find((item) => item.width);
            if (rect && columnForRect(rect, bookLeft, step, scale) === state.page) {
              return base + offset;
            }
          }
        }
        base += text.length;
      }
      return base;
    });
  }

  function pointForOffset(offset) {
    const nodes = textNodes().filter((node) => (node.textContent || "").length > 0);
    const total = nodes.reduce((sum, node) => sum + node.textContent.length, 0);
    if (!Number.isInteger(offset) || offset < 0 || offset > total) {
      return { exact: false, node: null, offset: 0 };
    }
    let remaining = offset;
    for (const node of nodes) {
      if (remaining < node.textContent.length) {
        return { exact: true, node, offset: remaining };
      }
      remaining -= node.textContent.length;
    }
    const node = nodes.at(-1) || null;
    return { exact: true, node, offset: Math.max(0, (node?.textContent.length || 1) - 1) };
  }

  function pageForOffset(offset) {
    return withoutTranslation(() => {
      const point = pointForOffset(offset);
      if (!point.exact || !point.node) return { exact: point.exact, page: 0 };
      const nodes = textNodes().filter((node) => (node.textContent || "").length > 0);
      const start = nodes.indexOf(point.node);
      const visibleRect = (node, from, to, last = false) => {
        if (from >= to) return null;
        const range = document.createRange();
        range.setStart(node, from);
        range.setEnd(node, to);
        const rects = [...range.getClientRects()].filter((item) => item.width && item.height);
        return last ? rects.at(-1) : rects[0];
      };
      let rect = visibleRect(point.node, point.offset, point.offset + 1);
      for (let index = start; !rect && index < nodes.length; index += 1) {
        const node = nodes[index];
        rect = visibleRect(node, index === start ? point.offset : 0, node.textContent.length);
      }
      for (let index = start; !rect && index >= 0; index -= 1) {
        const node = nodes[index];
        rect = visibleRect(node, 0, index === start ? point.offset : node.textContent.length, true);
      }
      if (!rect) return { exact: true, page: Math.min(state.page, state.pages - 1) };
      const style = getComputedStyle(book);
      const step = parseFloat(style.width) + parseFloat(style.columnGap);
      const scale = reader.getBoundingClientRect().width / reader.clientWidth;
      const pageIndex = columnForRect(rect, book.getBoundingClientRect().left, step, scale);
      return { exact: true, page: Math.min(pageIndex, state.pages - 1) };
    });
  }

  function offsetForFragment(fragment) {
    const target = [...book.querySelectorAll("[id]")].find((element) => element.id === fragment);
    if (!target) return null;
    const nodes = textNodes();
    let offset = 0;
    let start = nodes.length;
    for (let index = 0; index < nodes.length; index += 1) {
      const node = nodes[index];
      if (
        target.contains(node) ||
        target.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING
      ) {
        start = index;
        break;
      }
      offset += (node.textContent || "").length;
    }
    const renderedCharacter = (node, first) => {
      const length = (node.textContent || "").length;
      if (!length) return null;
      const hasRect = (startOffset, endOffset) => {
        const range = document.createRange();
        range.setStart(node, startOffset);
        range.setEnd(node, endOffset);
        return [...range.getClientRects()].some((rect) => rect.width > 0 && rect.height > 0);
      };
      if (!hasRect(0, length)) return null;
      let low = 0;
      let high = length - 1;
      while (low < high) {
        const middle = first ? Math.floor((low + high) / 2) : Math.ceil((low + high) / 2);
        if (first ? hasRect(0, middle + 1) : hasRect(middle, length)) {
          if (first) high = middle;
          else low = middle;
        } else if (first) low = middle + 1;
        else high = middle - 1;
      }
      return low;
    };
    let current = offset;
    for (let nodeIndex = start; nodeIndex < nodes.length; nodeIndex += 1) {
      const node = nodes[nodeIndex];
      const index = renderedCharacter(node, true);
      if (index !== null) return current + index;
      current += (node.textContent || "").length;
    }
    for (let nodeIndex = start - 1; nodeIndex >= 0; nodeIndex -= 1) {
      const node = nodes[nodeIndex];
      offset -= (node.textContent || "").length;
      const index = renderedCharacter(node, false);
      if (index !== null) return offset + index;
    }
    return 0;
  }

  function syncViewportDeviceSize() {
    const scale = 1 / devicePixelRatio;
    const width = Math.max(1, Math.round(innerWidth * devicePixelRatio));
    const height = Math.max(1, Math.round(innerHeight * devicePixelRatio));
    document.documentElement.style.setProperty("--page-scale", String(scale));
    document.documentElement.style.setProperty("--reader-width", `${width}px`);
    document.documentElement.style.setProperty("--reader-height", `${height}px`);
  }

  function applyFormulaScale() {
    const fontScale = state.fontSize / SOURCE_FONT_SIZE;
    const contentWidth = book.clientWidth;
    for (const formula of book.querySelectorAll("img.math-inline, img.math-display")) {
      if (!formula.dataset.sourceWidth) {
        const width = Number(formula.getAttribute("width"));
        const height = Number(formula.getAttribute("height"));
        assert(
          Number.isFinite(width) && width > 0 && Number.isFinite(height) && height > 0,
          "invalid-formula-size",
        );
        formula.dataset.sourceWidth = String(width);
        formula.dataset.sourceHeight = String(height);
        formula.dataset.sourceAlign = String(parseFloat(formula.style.verticalAlign) || 0);
      }
      const sourceWidth = Number(formula.dataset.sourceWidth);
      const sourceHeight = Number(formula.dataset.sourceHeight);
      const isDisplay = formula.classList.contains("math-display");
      const requestedScale = fontScale * (isDisplay ? DISPLAY_FORMULA_MULTIPLIER : 1);
      const scale = Math.min(requestedScale, contentWidth / sourceWidth);
      formula.style.width = `${sourceWidth * scale}px`;
      formula.style.height = `${sourceHeight * scale}px`;
      formula.style.verticalAlign = isDisplay
        ? "0px"
        : `${Number(formula.dataset.sourceAlign) * scale}px`;
    }
  }

  function showPage() {
    const style = getComputedStyle(book);
    const step = parseFloat(style.width) + parseFloat(style.columnGap);
    book.style.transform = `translateX(${-state.page * step}px)`;
    const label = `${state.page + 1} / ${state.pages}`;
    position.textContent = label;
    document.title = `Atha Reader — ${label}`;
  }

  function layout() {
    book.style.fontSize = `${state.fontSize}px`;
    book.style.transform = "none";
    applyFormulaScale();
    const style = getComputedStyle(book);
    const width = parseFloat(style.width);
    const gap = parseFloat(style.columnGap);
    state.pages = Math.max(1, Math.round((book.scrollWidth + gap) / (width + gap)));
    state.page = Math.min(state.page, state.pages - 1);
    showPage();
  }

  function countCutRects(visibleOnly = false) {
    const savedPage = state.page;
    book.style.transform = "none";
    const pageRect = page.getBoundingClientRect();
    const bookRect = book.getBoundingClientRect();
    const style = getComputedStyle(book);
    const step = parseFloat(style.width) + parseFloat(style.columnGap);
    const scale = pageRect.width / page.clientWidth;
    const relevant = (rect) => {
      if (!visibleOnly) return true;
      const column = columnForRect(rect, bookRect.left, step, scale);
      return column >= state.page && column <= state.page + 1;
    };
    const tolerance = 0.75;
    let cuts = 0;
    const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (!walker.currentNode.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(walker.currentNode);
      for (const rect of range.getClientRects()) {
        if (
          relevant(rect) &&
          rect.height &&
          (rect.top < pageRect.top - tolerance || rect.bottom > pageRect.bottom + tolerance)
        ) {
          cuts += 1;
        }
      }
    }
    for (const atomic of book.querySelectorAll("img, table, pre, figure")) {
      const rect = atomic.getBoundingClientRect();
      if (
        relevant(rect) &&
        rect.height &&
        (rect.top < pageRect.top - tolerance || rect.bottom > pageRect.bottom + tolerance)
      ) {
        cuts += 1;
      }
    }
    state.page = savedPage;
    showPage();
    return cuts;
  }

  function layoutSignature() {
    const rect = book.getBoundingClientRect();
    return [
      state.pages,
      book.scrollWidth,
      book.scrollHeight,
      rect.width.toFixed(2),
      rect.height.toFixed(2),
    ].join(":");
  }

  function nextFrame() {
    return new Promise((resolve) => requestAnimationFrame(resolve));
  }

  async function waitForStableLayout(operationStage) {
    let previousSignature = "";
    let equalFrames = 0;
    for (let frame = 0; frame < 20; frame += 1) {
      await nextFrame();
      const signature = layoutSignature();
      equalFrames = signature === previousSignature ? equalFrames + 1 : 0;
      if (equalFrames >= 2) return;
      previousSignature = signature;
    }
    fail("unstable-layout", operationStage);
  }

  async function relayoutAtOffset(anchor, operationStage) {
    await document.fonts.ready;
    layout();
    await waitForStableLayout(operationStage);
    const target = pageForOffset(anchor);
    assert(target.exact, "locator-offset", operationStage);
    state.page = target.page;
    showPage();
    await nextFrame();
  }

  async function renderFromStart() {
    state.page = 0;
    layout();
    await waitForStableLayout("初次分页");
    if ((await onPageShown(true)) > 0) {
      await waitForStableLayout("初次分页");
      await relayoutAtOffset(0, "初次分页");
    }
    assert(countCutRects(true) === 0, "layout-cut", "初次分页");
  }

  function verifyFormulaLayout() {
    const inline = [...book.querySelectorAll("img.math-inline")];
    const display = [...book.querySelectorAll("img.math-display")];
    const formulas = [...inline, ...display];
    if (formulas.length === 0) return;
    const savedTransform = book.style.transform;
    book.style.transform = "none";
    const fitScale = page.getBoundingClientRect().width / page.clientWidth;
    const bookRect = book.getBoundingClientRect();
    const style = getComputedStyle(book);
    const columnWidth = parseFloat(style.width);
    const columnStep = columnWidth + parseFloat(style.columnGap);
    try {
      for (const formula of formulas) {
        const rect = formula.getBoundingClientRect();
        const sourceWidth = Number(formula.dataset.sourceWidth);
        const sourceHeight = Number(formula.dataset.sourceHeight);
        const width = rect.width / fitScale;
        const height = rect.height / fitScale;
        const isDisplay = formula.classList.contains("math-display");
        const maximumScale = Math.min(
          (state.fontSize / SOURCE_FONT_SIZE) *
            (isDisplay ? DISPLAY_FORMULA_MULTIPLIER : 1),
          columnWidth / sourceWidth,
        );
        const actualScale = width / sourceWidth;
        assert(
          Number.isFinite(actualScale) && actualScale > 0 && actualScale <= maximumScale + 0.02,
          "formula-selectors",
        );
        assert(
          Math.abs(width / height - sourceWidth / sourceHeight) <= 0.02,
          "formula-selectors",
        );
        assert(width <= columnWidth + 0.75, "layout-cut");
        if (isDisplay) {
          const logicalLeft = (rect.left - bookRect.left) / fitScale;
          const column = Math.floor(logicalLeft / columnStep);
          const centerOffset =
            logicalLeft + width / 2 - (column * columnStep + columnWidth / 2);
          assert(Math.abs(centerOffset) <= 2, "formula-selectors");
        }
      }
    } finally {
      book.style.transform = savedTransform;
    }
  }

  function verifyDisplayGeometry() {
    const readerRect = reader.getBoundingClientRect();
    const scale = readerRect.width / reader.clientWidth;
    assert(Math.abs(readerRect.width - innerWidth) <= 1, "layout-cut");
    assert(Math.abs(readerRect.height - innerHeight) <= 1, "layout-cut");
    assert(
      Math.abs(reader.clientWidth - Math.round(innerWidth * devicePixelRatio)) <= 1,
      "layout-cut",
    );
    assert(
      Math.abs(reader.clientHeight - Math.round(innerHeight * devicePixelRatio)) <= 1,
      "layout-cut",
    );
    const progress = previous.closest("details");
    const wasOpen = progress.open;
    progress.open = true;
    assert(previous.getBoundingClientRect().height >= 44, "layout-cut");
    assert(next.getBoundingClientRect().height >= 44, "layout-cut");
    progress.open = wasOpen;
    if (state.pages > 1) {
      const style = getComputedStyle(book);
      const step = parseFloat(style.width) + parseFloat(style.columnGap);
      const bookLeft = book.getBoundingClientRect().left;
      const laterRect = textNodes()
        .flatMap((node) => {
          const range = document.createRange();
          range.selectNodeContents(node);
          return [...range.getClientRects()];
        })
        .find((rect) => rect.width && (rect.left - bookLeft) / scale >= step);
      assert(laterRect && columnForRect(laterRect, bookLeft, step, scale) > 0, "sample-boundary");
    }
  }

  async function setFontSize(value, anchor = captureOffset()) {
    state.fontSize = Number(value);
    await document.fonts.ready;
    layout();
    await waitForStableLayout("字号与布局重排");
    assert(
      await showOffset(anchor, "字号与布局重排"),
      "locator-offset",
      "字号与布局重排",
    );
    assert(countCutRects(true) === 0, "layout-cut", "字号与布局重排");
  }

  async function resizeViewport(anchor) {
    syncViewportDeviceSize();
    await relayoutAtOffset(anchor, "窗口尺寸重排");
    if ((await onPageShown(true)) > 0) {
      await waitForStableLayout("窗口尺寸重排");
      await relayoutAtOffset(anchor, "窗口尺寸重排");
    }
    assert(countCutRects(true) === 0, "layout-cut", "窗口尺寸重排");
  }

  async function verifySizes() {
    for (const size of [24, 32, 40]) {
      await setFontSize(size);
      verifyFormulaLayout();
    }
    await setFontSize(32);
    fontSizeControl.value = "32";
  }

  async function show(index, anchor, operationStage = "翻页与公式加载") {
    state.page = Math.max(0, Math.min(index, state.pages - 1));
    showPage();
    await nextFrame();
    const restoreOffset = anchor ?? captureOffset();
    if ((await onPageShown(true)) > 0) {
      await waitForStableLayout(operationStage);
      await relayoutAtOffset(restoreOffset, operationStage);
      assert(countCutRects(true) === 0, "layout-cut", operationStage);
    }
  }

  async function showOffset(offset, operationStage) {
    const target = pageForOffset(offset);
    if (!target.exact) return false;
    await show(target.page, offset, operationStage);
    return true;
  }

  function isOffsetVisible(offset) {
    const target = pageForOffset(offset);
    return target.exact && target.page === state.page;
  }

  function hasOffset(offset) {
    return pointForOffset(offset).exact;
  }

  async function move(delta) {
    const target = state.page + delta;
    if (target < 0 || target >= state.pages) return false;
    await show(target);
    return true;
  }

  function snapshot() {
    return { ...state };
  }

  function bindControls({ onPrevious, onNext, onFontSize, onProgress }) {
    const run = (action) => {
      Promise.resolve(action()).catch((error) => {
        if (!document.documentElement.dataset.error) {
          fail(error instanceof Error ? error.message : "layout-cut");
        }
      });
    };
    previous.addEventListener("click", () => run(onPrevious));
    next.addEventListener("click", () => run(onNext));
    fontSizeControl.addEventListener("change", () => {
      run(() => onFontSize(fontSizeControl.value));
    });
    progressRange.addEventListener("change", () => run(() => onProgress(progressRange.value)));
  }

  function initialize() {
    syncViewportDeviceSize();
  }

  function bindResize(onResize) {
    let timer;
    window.addEventListener("resize", () => {
      clearTimeout(timer);
      delete document.documentElement.dataset.viewportStable;
      timer = setTimeout(() => {
        Promise.resolve()
          .then(onResize)
          .then(() => {
            document.documentElement.dataset.viewportStable = `${reader.clientWidth}x${reader.clientHeight}`;
          })
          .catch((error) => {
            if (!document.documentElement.dataset.error) {
              fail(error instanceof Error ? error.message : "layout-cut");
            }
          });
      }, 120);
    });
  }

  return Object.freeze({
    bindControls,
    bindResize,
    captureOffset,
    countCutRects,
    hasOffset,
    initialize,
    isOffsetVisible,
    move,
    nextFrame,
    offsetForFragment,
    renderFromStart,
    resizeViewport,
    setFontSize,
    show,
    showOffset,
    snapshot,
    verifyDisplayGeometry,
    verifyFormulaLayout,
    verifySizes,
  });
}
