const WHEEL_THRESHOLD = 60;
const WHEEL_IDLE_MS = 240;
const SWIPE_DISTANCE = 48;
const CLICK_DRIFT = 8;

function createWheelDetector() {
  let total = 0;
  let last = -Infinity;
  let flipped = false;

  return ({ deltaX, deltaY, deltaMode, timeStamp, pageHeight }) => {
    if (timeStamp - last > WHEEL_IDLE_MS) {
      total = 0;
      flipped = false;
    }
    last = timeStamp;
    if (flipped) return 0;
    const scale = deltaMode === 1 ? 40 : deltaMode === 2 ? pageHeight : 1;
    const dominant = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
    total += dominant * scale;
    if (Math.abs(total) < WHEEL_THRESHOLD) return 0;
    flipped = true;
    return Math.sign(total);
  };
}

function swipeDirection(start, event) {
  const deltaX = event.clientX - start.x;
  const deltaY = event.clientY - start.y;
  if (Math.abs(deltaX) < SWIPE_DISTANCE || Math.abs(deltaX) <= Math.abs(deltaY)) return 0;
  return deltaX < 0 ? 1 : -1;
}

export function createInteraction({ reader, content, navigation, assert, fail }) {
  const wheel = createWheelDetector();
  const counts = {
    keyboard: 0,
    wheel: 0,
    mouse: 0,
    touch: 0,
    selectionProtected: 0,
    controlProtected: 0,
    contentProtected: 0,
    multiTouchProtected: 0,
  };
  let pointer = null;
  const insideEvents = new WeakSet();

  function protectedTarget(event) {
    return event
      .composedPath()
      .some(
        (node) =>
          node instanceof Element &&
          node.matches(
            "a, button, input, select, textarea, label, summary, details, [contenteditable], [role='button']",
          ),
      );
  }

  function hasSelection() {
    const root = content.book.getRootNode();
    const selections = [root.getSelection?.(), document.getSelection()];
    return selections.some((selection) => selection && !selection.isCollapsed);
  }

  function run(direction, kind) {
    counts[kind] += 1;
    Promise.resolve(direction < 0 ? navigation.previous() : navigation.next()).catch((error) => {
      if (!document.documentElement.dataset.error) {
        try {
          fail(error instanceof Error ? error.message : "section-load");
        } catch {
          // fail already recorded the terminal reader state.
        }
      }
    });
  }

  function onKeydown(event) {
    const control = protectedTarget(event);
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || control) {
      if (control) counts.controlProtected += 1;
      return;
    }
    const direction =
      event.key === "ArrowLeft" || event.key === "ArrowUp" || event.key === "PageUp"
        ? -1
        : event.key === "ArrowRight" ||
            event.key === "ArrowDown" ||
            event.key === "PageDown" ||
            event.key === " "
          ? 1
          : 0;
    if (!direction) return;
    event.preventDefault();
    run(direction, "keyboard");
  }

  function onWheel(event) {
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || protectedTarget(event)) {
      return;
    }
    event.preventDefault();
    const direction = wheel({
      deltaX: event.deltaX,
      deltaY: event.deltaY,
      deltaMode: event.deltaMode,
      timeStamp: event.timeStamp,
      pageHeight: reader.clientHeight,
    });
    if (direction) run(direction, "wheel");
  }

  function onPointerDown(event) {
    if (!event.isPrimary) {
      pointer = null;
      counts.multiTouchProtected += 1;
      return;
    }
    const control = protectedTarget(event);
    if (control) counts.contentProtected += 1;
    if (
      event.button !== 0 ||
      event.ctrlKey ||
      event.altKey ||
      event.metaKey ||
      event.shiftKey ||
      control
    ) {
      return;
    }
    pointer = {
      id: event.pointerId,
      type: event.pointerType,
      x: event.clientX,
      y: event.clientY,
      selection: hasSelection(),
    };
  }

  function onPointerUp(event) {
    const start = pointer;
    pointer = null;
    if (!start || start.id !== event.pointerId || protectedTarget(event)) return;
    if (start.selection || hasSelection()) {
      counts.selectionProtected += 1;
      return;
    }
    if (start.type === "touch") {
      const direction = swipeDirection(start, event);
      if (direction) run(direction, "touch");
      return;
    }
    if (start.type !== "mouse" || Math.hypot(event.clientX - start.x, event.clientY - start.y) > CLICK_DRIFT) {
      return;
    }
    const rect = reader.getBoundingClientRect();
    const ratio = (event.clientX - rect.left) / rect.width;
    if (ratio < 0.35) run(-1, "mouse");
    else if (ratio > 0.65) run(1, "mouse");
  }

  function bind() {
    const inside = (handler) => (event) => {
      insideEvents.add(event);
      handler(event);
    };
    const outside = (handler) => (event) => {
      if (!insideEvents.has(event)) handler(event);
    };
    content.book.addEventListener("keydown", inside(onKeydown));
    document.addEventListener("keydown", outside(onKeydown));
    content.book.addEventListener("wheel", inside(onWheel), { passive: false });
    reader.addEventListener("wheel", outside(onWheel), { passive: false });
    content.book.addEventListener("pointerdown", inside(onPointerDown));
    reader.addEventListener("pointerdown", outside(onPointerDown));
    content.book.addEventListener("pointerup", inside(onPointerUp));
    window.addEventListener("pointerup", outside(onPointerUp));
    window.addEventListener("pointercancel", () => {
      pointer = null;
    });
  }

  const testWheel = createWheelDetector();
  assert(
    testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 0, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 10, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 20, pageHeight: 100 }) === 1 &&
      testWheel({ deltaX: 0, deltaY: 100, deltaMode: 0, timeStamp: 30, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: -60, deltaMode: 0, timeStamp: 300, pageHeight: 100 }) === -1,
    "sample-boundary",
  );
  assert(
    swipeDirection({ x: 100, y: 100 }, { clientX: 40, clientY: 105 }) === 1 &&
      swipeDirection({ x: 100, y: 100 }, { clientX: 160, clientY: 95 }) === -1 &&
      swipeDirection({ x: 100, y: 100 }, { clientX: 90, clientY: 40 }) === 0,
    "sample-boundary",
  );

  return Object.freeze({ bind, snapshot: () => Object.freeze({ ...counts }) });
}
