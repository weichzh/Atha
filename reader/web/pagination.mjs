const SOURCE_FONT_SIZE = 16;
const DISPLAY_FORMULA_MULTIPLIER = 1.5;
const MAX_TRANSFORM_WIDTH = 20_000;
const SETTLE_DURATION_MS = 300;

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
  function ensure(condition, code) {
    if (!condition) throw new Error(code);
  }

  const state = { page: 0, pages: 0, fontSize: 19 };
  let fontPreviewAnchor = null;
  let settleTimer = 0;
  let settleFrame = 0;
  let pageStep = 0;
  let visualScale = 1;
  let swipeFrame = 0;
  let swipeDelta = 0;
  let swipeOrigin = 0;
  let offsetCache = null;
  const fragmentOffsetCache = new Map();
  let nativePagedScroll = false;

  function isScrolled() {
    return reader.dataset.readingMode === "scroll";
  }

  function syncScrollPage() {
    state.pages = Math.max(1, Math.ceil(reader.scrollHeight / reader.clientHeight));
    const atEnd = reader.scrollTop + reader.clientHeight >= reader.scrollHeight - 1;
    state.page = atEnd
      ? state.pages - 1
      : Math.max(0, Math.min(state.pages - 1, Math.floor(reader.scrollTop / reader.clientHeight)));
  }

  function updatePosition() {
    const label = `${state.page + 1} / ${state.pages}`;
    position.textContent = label;
    document.title = `Atha Reader — section ${document.documentElement.dataset.sectionPosition} — page ${label}`;
  }

  function textNodes() {
    const nodes = [];
    const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  function columnForRect(rect, bookLeft, step, scale) {
    return Math.max(0, Math.floor((rect.left - bookLeft) / (step * scale)));
  }

  function scanOffset() {
    if (isScrolled()) {
      const readerTop = reader.getBoundingClientRect().top;
      let base = 0;
      for (const node of textNodes()) {
        const text = node.textContent || "";
        const whole = document.createRange();
        whole.selectNodeContents(node);
        if ([...whole.getClientRects()].some((rect) => rect.height && rect.bottom > readerTop + 1)) {
          for (let offset = 0; offset < text.length; offset += 1) {
            const character = document.createRange();
            character.setStart(node, offset);
            character.setEnd(node, offset + 1);
            const rect = [...character.getClientRects()].find((item) => item.height);
            if (rect && rect.bottom > readerTop + 1) return base + offset;
          }
        }
        base += text.length;
      }
      return base;
    }
    const bookLeft = book.getBoundingClientRect().left;
    let base = 0;
    for (const node of textNodes()) {
      const text = node.textContent || "";
      const whole = document.createRange();
      whole.selectNodeContents(node);
      const reachesPage = [...whole.getClientRects()].some(
        (rect) => rect.width && columnForRect(rect, bookLeft, pageStep, visualScale) === state.page,
      );
      if (reachesPage) {
        for (let offset = 0; offset < text.length; offset += 1) {
          const character = document.createRange();
          character.setStart(node, offset);
          character.setEnd(node, offset + 1);
          const rect = [...character.getClientRects()].find((item) => item.width);
          if (rect && columnForRect(rect, bookLeft, pageStep, visualScale) === state.page) {
            return base + offset;
          }
        }
      }
      base += text.length;
    }
    return base;
  }

  function captureOffset() {
    if (fontPreviewAnchor !== null) return fontPreviewAnchor;
    const key = isScrolled() ? `scroll:${reader.scrollTop}` : `page:${state.page}`;
    if (offsetCache?.key === key) return offsetCache.value;
    const value = scanOffset();
    offsetCache = { key, value };
    return value;
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
    const point = pointForOffset(offset);
    if (!point.exact || !point.node) return { exact: point.exact, page: 0 };
    if (isScrolled()) {
      const range = document.createRange();
      range.setStart(point.node, point.offset);
      range.setEnd(point.node, Math.min(point.offset + 1, point.node.textContent.length));
      const rect = [...range.getClientRects()].find((item) => item.height);
      if (!rect) return { exact: true, page: state.page };
      const viewport = reader.getBoundingClientRect();
      const scale = reader.clientHeight / viewport.height;
      const absolute = reader.scrollTop + (rect.top - viewport.top) * scale;
      return {
        exact: true,
        page: Math.max(0, Math.min(state.pages - 1, Math.floor(absolute / reader.clientHeight))),
      };
    }
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
    const pageIndex = columnForRect(
      rect,
      book.getBoundingClientRect().left,
      pageStep,
      visualScale,
    );
    return { exact: true, page: Math.min(pageIndex, state.pages - 1) };
  }

  function offsetForFragment(fragment) {
    if (fragmentOffsetCache.has(fragment)) return fragmentOffsetCache.get(fragment);
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
      if (index !== null) {
        fragmentOffsetCache.set(fragment, current + index);
        return current + index;
      }
      current += (node.textContent || "").length;
    }
    for (let nodeIndex = start - 1; nodeIndex >= 0; nodeIndex -= 1) {
      const node = nodes[nodeIndex];
      offset -= (node.textContent || "").length;
      const index = renderedCharacter(node, false);
      if (index !== null) {
        fragmentOffsetCache.set(fragment, offset + index);
        return offset + index;
      }
    }
    fragmentOffsetCache.set(fragment, 0);
    return 0;
  }

  function syncViewportDeviceSize() {
    visualScale = 1 / devicePixelRatio;
    const width = Math.max(1, Math.round(innerWidth * devicePixelRatio));
    const height = Math.max(1, Math.round(innerHeight * devicePixelRatio));
    document.documentElement.style.setProperty("--page-scale", String(visualScale));
    document.documentElement.style.setProperty("--reader-width", `${width}px`);
    document.documentElement.style.setProperty("--reader-height", `${height}px`);
  }

  function applyFormulaScale() {
    const fontScale = fontSizePixels(state.fontSize) / SOURCE_FONT_SIZE;
    const contentWidth = book.clientWidth;
    for (const formula of book.querySelectorAll("img.math-inline, img.math-display")) {
      if (!formula.dataset.sourceWidth) {
        const width = Number(formula.getAttribute("width"));
        const height = Number(formula.getAttribute("height"));
        ensure(
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

  function stopSettle() {
    if (settleFrame) cancelAnimationFrame(settleFrame);
    settleFrame = 0;
    clearTimeout(settleTimer);
    settleTimer = 0;
    book.removeAttribute("data-swipe-settling");
  }

  function animatePageScroll(target) {
    const start = page.scrollLeft;
    const distance = target - start;
    const started = performance.now();
    const tick = (now) => {
      const progress = Math.min(1, (now - started) / SETTLE_DURATION_MS);
      page.scrollLeft = start + distance * (1 - (1 - progress) ** 2);
      if (progress < 1) settleFrame = requestAnimationFrame(tick);
      else {
        settleFrame = 0;
        page.scrollLeft = target;
        book.removeAttribute("data-swipe-settling");
      }
    };
    settleFrame = requestAnimationFrame(tick);
  }

  function showPage() {
    if (swipeFrame) cancelAnimationFrame(swipeFrame);
    swipeFrame = 0;
    swipeDelta = 0;
    const wasDragging = book.hasAttribute("data-swipe-dragging");
    stopSettle();
    book.removeAttribute("data-swipe-dragging");
    delete reader.dataset.swipeDragging;
    if (wasDragging) {
      book.setAttribute("data-swipe-settling", "");
    }
    if (isScrolled()) {
      book.style.transform = "none";
      page.scrollLeft = 0;
      reader.scrollTop = state.page * reader.clientHeight;
    } else if (nativePagedScroll) {
      book.style.transform = "none";
      const target = state.page * pageStep;
      if (wasDragging) animatePageScroll(target);
      else page.scrollLeft = target;
    } else {
      page.scrollLeft = 0;
      book.style.transform = `translateX(${-state.page * pageStep}px)`;
      if (wasDragging) {
        settleTimer = setTimeout(() => {
          settleTimer = 0;
          book.removeAttribute("data-swipe-settling");
        }, 320);
      }
    }
    updatePosition();
  }

  function layout() {
    offsetCache = null;
    fragmentOffsetCache.clear();
    const pixels = fontSizePixels(state.fontSize);
    book.style.fontSize = `${pixels}px`;
    book.style.setProperty("--atha-display-formula-margin", `${pixels * 0.9}px`);
    reader.dataset.fontPixels = String(pixels);
    stopSettle();
    nativePagedScroll = false;
    reader.removeAttribute("data-native-page-scroll");
    page.scrollLeft = 0;
    book.style.transform = "none";
    applyFormulaScale();
    if (isScrolled()) {
      syncScrollPage();
      reader.dataset.pageColumns = "1";
      reader.dataset.scrollable = String(reader.scrollHeight > reader.clientHeight);
      updatePosition();
      return;
    }
    const style = getComputedStyle(book);
    const width = parseFloat(style.width);
    const gap = parseFloat(style.columnGap);
    pageStep = width + gap;
    state.pages = contentPageCount();
    state.page = Math.min(state.page, state.pages - 1);
    nativePagedScroll = book.scrollWidth * visualScale > MAX_TRANSFORM_WIDTH;
    reader.toggleAttribute("data-native-page-scroll", nativePagedScroll);
    reader.dataset.pageColumns = "paged";
    reader.dataset.scrollable = "false";
    showPage();
  }

  function contentPageCount() {
    if (isScrolled() || !pageStep) return state.pages;
    const savedTransform = book.style.transform;
    book.style.transform = "none";
    const left = book.getBoundingClientRect().left;
    const walker = document.createTreeWalker(
      book,
      NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
    );
    let lastContent = null;
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (
        (node instanceof Text && node.textContent.trim()) ||
        (node instanceof Element &&
          node.matches(
            "img, svg, math, table, figure, hr, .atha-image-error, .atha-cbz-page-error",
          ))
      ) {
        lastContent = node;
      }
    }
    const range = document.createRange();
    range.setStart(book, 0);
    if (lastContent instanceof Text) range.setEnd(lastContent, lastContent.length);
    else if (lastContent) range.setEndAfter(lastContent);
    else range.setEnd(book, 0);
    const right = range.getBoundingClientRect().right;
    book.style.transform = savedTransform;
    return Math.max(1, Math.floor((right - left - 0.5) / (pageStep * visualScale)) + 1);
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
    const isCut = (rect) =>
      rect.top < pageRect.top - tolerance || rect.bottom > pageRect.bottom + tolerance;
    let cuts = 0;
    const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (!walker.currentNode.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(walker.currentNode);
      for (const rect of range.getClientRects()) {
        if (relevant(rect) && rect.height && isCut(rect)) {
          cuts += 1;
        }
      }
    }
    for (const atomic of book.querySelectorAll("img, table, figure")) {
      const rect = atomic.getBoundingClientRect();
      if (relevant(rect) && rect.height && isCut(rect)) {
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
    throw new Error("unstable-layout");
  }

  async function relayoutAtOffset(anchor, operationStage) {
    await document.fonts.ready;
    layout();
    await waitForStableLayout(operationStage);
    if (isScrolled()) {
      ensure(restoreScrollOffset(anchor), "locator-offset");
      await nextFrame();
      syncScrollPage();
      updatePosition();
      return;
    }
    const target = pageForOffset(anchor);
    ensure(target.exact, "locator-offset");
    state.page = target.page;
    showPage();
    await nextFrame();
  }

  async function renderFromStart() {
    if (swipeFrame) cancelAnimationFrame(swipeFrame);
    stopSettle();
    swipeFrame = 0;
    swipeDelta = 0;
    settleTimer = 0;
    book.removeAttribute("data-swipe-dragging");
    book.removeAttribute("data-swipe-settling");
    delete reader.dataset.swipeDragging;
    state.page = 0;
    layout();
    await waitForStableLayout("初次分页");
    const shown = await onPageShown(true, () => ({
      offset: 0,
      pageIndex: state.page,
      scrollTop: reader.scrollTop,
    }));
    if (shown.loaded > 0 || shown.layoutChanged) {
      await waitForStableLayout("初次分页");
      await relayoutAtOffset(0, "初次分页");
    }
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
          (fontSizePixels(state.fontSize) / SOURCE_FONT_SIZE) *
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
    fontPreviewAnchor = null;
    await document.fonts.ready;
    layout();
    await waitForStableLayout("字号与布局重排");
    assert(
      await showOffset(anchor, "字号与布局重排"),
      "locator-offset",
      "字号与布局重排",
    );
  }

  async function resizeViewport(anchor) {
    syncViewportDeviceSize();
    await relayoutAtOffset(anchor, "窗口尺寸重排");
    if (
      (
        await onPageShown(true, () => ({
          offset: anchor,
          pageIndex: state.page,
          scrollTop: reader.scrollTop,
        }))
      ).layoutChanged
    ) {
      await waitForStableLayout("窗口尺寸重排");
      await relayoutAtOffset(anchor, "窗口尺寸重排");
    }
  }

  async function verifySizes() {
    for (const size of [16, 19, 40]) {
      await setFontSize(size);
      verifyFormulaLayout();
    }
    await setFontSize(19);
    fontSizeControl.value = "19";
  }

  async function show(index, anchor, operationStage = "翻页与公式加载") {
    state.page = Math.max(0, Math.min(index, state.pages - 1));
    showPage();
    await nextFrame();
    let restoreOffset = anchor ?? null;
    const shown = await onPageShown(true, () => {
      if (restoreOffset === null) restoreOffset = captureOffset();
      return { offset: restoreOffset, pageIndex: state.page, scrollTop: reader.scrollTop };
    });
    if (shown.layoutChanged) {
      ensure(restoreOffset !== null, "locator-offset");
      await waitForStableLayout(operationStage);
      await relayoutAtOffset(restoreOffset, operationStage);
    }
  }

  function restoreScrollOffset(offset) {
    const point = pointForOffset(offset);
    if (!point.exact || !point.node) return false;
    const range = document.createRange();
    range.setStart(point.node, point.offset);
    range.setEnd(point.node, Math.min(point.offset + 1, point.node.textContent.length));
    const rect = [...range.getClientRects()].find((item) => item.height);
    if (rect) {
      const viewport = reader.getBoundingClientRect();
      reader.scrollTop += (rect.top - viewport.top) * (reader.clientHeight / viewport.height);
    }
    return true;
  }

  async function showOffset(offset, operationStage) {
    if (isScrolled()) {
      if (!restoreScrollOffset(offset)) return false;
      await nextFrame();
      syncScrollPage();
      updatePosition();
      if (
        (
          await onPageShown(true, () => ({
            offset,
            pageIndex: state.page,
            scrollTop: reader.scrollTop,
          }))
        ).layoutChanged
      ) {
        await relayoutAtOffset(offset, operationStage);
      }
      return true;
    }
    const target = pageForOffset(offset);
    if (!target.exact) return false;
    await show(target.page, offset, operationStage);
    return true;
  }

  async function settleScrolledContent() {
    if (!isScrolled()) return;
    const anchor = captureOffset();
    const shown = await onPageShown(true, () => ({
      offset: anchor,
      pageIndex: state.page,
      scrollTop: reader.scrollTop,
    }));
    if (!shown.layoutChanged) return;
    await document.fonts.ready;
    layout();
    await waitForStableLayout("滚动图片加载");
    ensure(restoreScrollOffset(anchor), "locator-offset");
    await nextFrame();
    syncScrollPage();
    updatePosition();
  }

  function isOffsetVisible(offset) {
    if (isScrolled()) {
      const point = pointForOffset(offset);
      if (!point.exact || !point.node) return false;
      const range = document.createRange();
      range.setStart(point.node, point.offset);
      range.setEnd(point.node, Math.min(point.offset + 1, point.node.textContent.length));
      const rect = [...range.getClientRects()].find((item) => item.height);
      const viewport = reader.getBoundingClientRect();
      return Boolean(rect && rect.bottom > viewport.top && rect.top < viewport.bottom);
    }
    const target = pageForOffset(offset);
    return target.exact && target.page === state.page;
  }

  function hasOffset(offset) {
    return pointForOffset(offset).exact;
  }

  async function move(delta) {
    if (
      isScrolled() &&
      ((delta < 0 && reader.scrollTop <= 0) ||
        (delta > 0 && reader.scrollTop + reader.clientHeight >= reader.scrollHeight - 1))
    ) {
      return false;
    }
    const target = state.page + delta;
    if (target < 0 || target >= state.pages) return false;
    await show(target);
    return true;
  }

  function snapshot() {
    return { ...state, nativePagedScroll };
  }

  function previewFontSize(value) {
    if (fontPreviewAnchor === null) fontPreviewAnchor = captureOffset();
    state.fontSize = Number(value);
    layout();
    const target = pageForOffset(fontPreviewAnchor);
    state.page = target.page;
    showPage();
  }

  function applySwipe(deltaX) {
    if (nativePagedScroll) {
      page.scrollLeft = swipeOrigin - deltaX / visualScale;
    } else {
      book.style.transform = `translateX(${-state.page * pageStep + deltaX / visualScale}px)`;
    }
  }

  function previewSwipe(deltaX) {
    if (isScrolled()) return;
    if (!book.hasAttribute("data-swipe-dragging")) {
      stopSettle();
      swipeOrigin = nativePagedScroll ? page.scrollLeft : state.page * pageStep;
      book.setAttribute("data-swipe-dragging", "");
      reader.dataset.swipeDragging = "true";
    }
    swipeDelta = deltaX;
    if (swipeFrame) return;
    swipeFrame = requestAnimationFrame(() => {
      swipeFrame = 0;
      applySwipe(swipeDelta);
    });
  }

  function finishSwipe(deltaX) {
    if (!book.hasAttribute("data-swipe-dragging")) return;
    swipeDelta = deltaX;
    if (swipeFrame) cancelAnimationFrame(swipeFrame);
    swipeFrame = 0;
    applySwipe(swipeDelta);
    if (!nativePagedScroll) void getComputedStyle(book).transform;
  }

  function cancelSwipe() {
    if (book.hasAttribute("data-swipe-dragging")) showPage();
  }

  function bindControls({ onPrevious, onNext, onFontSize, onProgress, onScroll }) {
    const run = (action) => {
      Promise.resolve(action()).catch((error) => {
        if (!document.documentElement.dataset.error) {
          fail(error instanceof Error ? error.message : "layout-cut");
        }
      });
    };
    previous.addEventListener("click", () => run(onPrevious));
    next.addEventListener("click", () => run(onNext));
    let fontFrame = 0;
    fontSizeControl.addEventListener("change", () => {
      if (fontFrame) cancelAnimationFrame(fontFrame);
      fontFrame = 0;
      run(() => onFontSize(fontSizeControl.value));
    });
    fontSizeControl.addEventListener("input", () => {
      if (fontFrame) return;
      fontFrame = requestAnimationFrame(() => {
        fontFrame = 0;
        previewFontSize(fontSizeControl.value);
      });
    });
    progressRange.addEventListener("change", () => run(() => onProgress(progressRange.value)));
    let scrollFrame = 0;
    let scrollIdle = 0;
    let scrollDirty = false;
    const settleScroll = () => {
      clearTimeout(scrollIdle);
      if (!scrollDirty) return;
      scrollDirty = false;
      run(onScroll);
    };
    reader.addEventListener("scroll", () => {
      if (!isScrolled()) return;
      scrollDirty = true;
      if (scrollFrame) return;
      scrollFrame = requestAnimationFrame(() => {
        scrollFrame = 0;
        syncScrollPage();
        updatePosition();
        clearTimeout(scrollIdle);
        scrollIdle = setTimeout(settleScroll, 160);
      });
    });
    window.addEventListener("pagehide", settleScroll);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") settleScroll();
    });
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
    cancelSwipe,
    captureOffset,
    contentPageCount,
    countCutRects,
    finishSwipe,
    hasOffset,
    initialize,
    isOffsetVisible,
    isScrolled,
    move,
    nextFrame,
    offsetForFragment,
    previewSwipe,
    renderFromStart,
    resizeViewport,
    setFontSize,
    show,
    showOffset,
    snapshot,
    scrollPosition: () => reader.scrollTop,
    settleScrolledContent,
    verifyDisplayGeometry,
    verifyFormulaLayout,
    verifySizes,
  });
}
