const SOURCE_FONT_SIZE = 16;
const DISPLAY_FORMULA_MULTIPLIER = 1.5;

export function createPagination({
  book,
  reader,
  page,
  position,
  previous,
  next,
  fontSizeControl,
  assert,
  fail,
}) {
  const state = { page: 0, pages: 0, fontSize: 32 };

  function syncPageDeviceScale() {
    const scale = 1 / devicePixelRatio;
    document.documentElement.style.setProperty("--page-scale", String(scale));
    document.documentElement.style.setProperty("--reader-display-width", `${1264 * scale}px`);
    document.documentElement.style.setProperty("--reader-display-height", `${1680 * scale}px`);
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
    position.textContent = `${state.page + 1} / ${state.pages}`;
    previous.disabled = state.page === 0;
    next.disabled = state.page + 1 === state.pages;
    document.title = `Atha Reader — ${state.page + 1} / ${state.pages}`;
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

  function countCutRects() {
    const savedPage = state.page;
    book.style.transform = "none";
    const pageRect = page.getBoundingClientRect();
    const tolerance = 0.75;
    let cuts = 0;
    const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (!walker.currentNode.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(walker.currentNode);
      for (const rect of range.getClientRects()) {
        if (
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

  async function waitForStableLayout() {
    let previousSignature = "";
    let equalFrames = 0;
    for (let frame = 0; frame < 20; frame += 1) {
      await nextFrame();
      const signature = layoutSignature();
      equalFrames = signature === previousSignature ? equalFrames + 1 : 0;
      if (equalFrames >= 2) return;
      previousSignature = signature;
    }
    fail("unstable-layout");
  }

  async function renderFromStart() {
    state.page = 0;
    layout();
    await waitForStableLayout();
    assert(countCutRects() === 0, "layout-cut");
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
        const expectedScale = Math.min(
          (state.fontSize / SOURCE_FONT_SIZE) *
            (isDisplay ? DISPLAY_FORMULA_MULTIPLIER : 1),
          columnWidth / sourceWidth,
        );
        assert(Math.abs(width / sourceWidth - expectedScale) <= 0.02, "formula-selectors");
        assert(
          Math.abs(width / height - sourceWidth / sourceHeight) <= 0.02,
          "formula-selectors",
        );
        assert(width <= columnWidth + 0.75, "layout-cut");
        if (isDisplay) {
          const logicalLeft = (rect.left - bookRect.left) / fitScale;
          const column = Math.round(logicalLeft / columnStep);
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
    assert(Math.abs(readerRect.width * devicePixelRatio - 1264) <= 1, "layout-cut");
    assert(Math.abs(readerRect.height * devicePixelRatio - 1680) <= 1, "layout-cut");
    assert(previous.getBoundingClientRect().height >= 44, "layout-cut");
    assert(next.getBoundingClientRect().height >= 44, "layout-cut");
  }

  async function setFontSize(value) {
    state.fontSize = Number(value);
    layout();
    await waitForStableLayout();
    assert(countCutRects() === 0, "layout-cut");
  }

  async function verifySizes() {
    for (const size of [24, 32, 40]) {
      await setFontSize(size);
      verifyFormulaLayout();
    }
    await setFontSize(32);
    fontSizeControl.value = "32";
  }

  async function show(index) {
    state.page = index;
    showPage();
    await nextFrame();
  }

  function snapshot() {
    return { ...state };
  }

  function bindControls() {
    syncPageDeviceScale();
    window.addEventListener("resize", syncPageDeviceScale);
    previous.addEventListener("click", async () => {
      if (state.page > 0) state.page -= 1;
      showPage();
      await nextFrame();
    });
    next.addEventListener("click", async () => {
      if (state.page + 1 < state.pages) state.page += 1;
      showPage();
      await nextFrame();
    });
    fontSizeControl.addEventListener("change", () => {
      setFontSize(fontSizeControl.value).catch((error) => {
        if (!document.documentElement.dataset.error) {
          fail(error instanceof Error ? error.message : "layout-cut");
        }
      });
    });
  }

  return Object.freeze({
    bindControls,
    countCutRects,
    nextFrame,
    renderFromStart,
    setFontSize,
    show,
    snapshot,
    verifyDisplayGeometry,
    verifyFormulaLayout,
    verifySizes,
  });
}
