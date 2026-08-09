const WHEEL_THRESHOLD = 60;
const WHEEL_IDLE_MS = 240;
const SWIPE_DISTANCE = 48;
const CLICK_DRIFT = 8;
const AXIS_ADVANTAGE = 1.5;
const CLICK_SUPPRESSION_MS = 500;

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
    const scale = deltaMode === 1 ? 40 : deltaMode === 2 ? pageHeight : 1;
    const dominant = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
    const amount = dominant * scale;
    if (Math.abs(amount) >= WHEEL_THRESHOLD) {
      total = 0;
      flipped = true;
      return Math.sign(amount);
    }
    if (flipped) return 0;
    total += amount;
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

function scrollSwipeDirection(startY, endY, atStart, atEnd) {
  const deltaY = endY - startY;
  if (deltaY > SWIPE_DISTANCE && atStart) return -1;
  if (deltaY < -SWIPE_DISTANCE && atEnd) return 1;
  return 0;
}

export function createInteraction({ reader, content, navigation, pagination, onCenter, assert, fail }) {
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
  let touch = null;
  let suppressedClick = null;
  const insideEvents = new WeakSet();

  function matchingPath(event, selector) {
    return (
      event.composedPath().find((node) => node instanceof Element && node.matches(selector)) ||
      null
    );
  }

  function protectedTarget(event) {
    return event
      .composedPath()
      .some(
        (node) =>
          node instanceof Element &&
          node.matches(
            "a, button, input, select, textarea, label, summary, details, dialog, table, pre, [contenteditable], [role='button']",
          ),
      );
  }

  function hardProtectedTarget(event) {
    return Boolean(
      matchingPath(
        event,
        "a, button, input, select, textarea, label, summary, details, dialog, [contenteditable], [role='button']:not(img)",
      ),
    );
  }

  function hasSelection() {
    return Boolean(content.selectionRange());
  }

  function wheelProtectedTarget(event) {
    const path = event.composedPath();
    const media = path.some((node) => node instanceof Element && node.matches("img"));
    if (!path.includes(content.book) || !media) return protectedTarget(event);
    return path.some(
      (node) =>
        node instanceof Element &&
        node.matches("input, select, textarea, dialog, table, pre, [contenteditable]"),
    );
  }

  function run(direction, kind) {
    counts[kind] += 1;
    Promise.resolve(direction < 0 ? navigation.previous() : navigation.next())
      .catch((error) => {
        if (!document.documentElement.dataset.error) {
          try {
            fail(error instanceof Error ? error.message : "section-load");
          } catch {
            // fail already recorded the terminal reader state.
          }
        }
      })
      .finally(() => pagination.cancelSwipe());
  }

  function scrollMode() {
    return reader.dataset.readingMode === "scroll";
  }

  function canScrollOverflow(overflow, deltaX) {
    return (
      (deltaX < 0 && overflow.start < overflow.maximum - 1) ||
      (deltaX > 0 && overflow.start > 1)
    );
  }

  function applyOverflow(pointerState) {
    pointerState.scrollFrame = 0;
    pointerState.overflow.element.scrollLeft = Math.max(
      0,
      Math.min(
        pointerState.overflow.maximum,
        pointerState.overflow.start - pointerState.deltaX * pointerState.layoutScale,
      ),
    );
  }

  function previewOverflow(pointerState, deltaX) {
    pointerState.deltaX = deltaX;
    if (pointerState.scrollFrame) return;
    pointerState.scrollFrame = requestAnimationFrame(() => applyOverflow(pointerState));
  }

  function finishOverflow(pointerState) {
    if (!pointerState.scrollFrame) return;
    cancelAnimationFrame(pointerState.scrollFrame);
    applyOverflow(pointerState);
  }

  function cancelPointer(pointerState = pointer) {
    pointer = null;
    if (pointerState?.scrollFrame) cancelAnimationFrame(pointerState.scrollFrame);
    pagination.cancelSwipe();
  }

  function suppressCompatibilityClick(event, anywhere = false) {
    suppressedClick = {
      until: performance.now() + CLICK_SUPPRESSION_MS,
      x: event.clientX,
      y: event.clientY,
      anywhere,
    };
  }

  function onKeydown(event) {
    const control = protectedTarget(event);
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || control) {
      if (control) counts.controlProtected += 1;
      return;
    }
    if (scrollMode()) {
      const backward = ["ArrowUp", "PageUp"].includes(event.key);
      const forward = ["ArrowDown", "PageDown", " "].includes(event.key);
      if (backward && reader.scrollTop <= 0) {
        event.preventDefault();
        run(-1, "keyboard");
      } else if (forward && reader.scrollTop + reader.clientHeight >= reader.scrollHeight - 1) {
        event.preventDefault();
        run(1, "keyboard");
      } else if (backward || forward) {
        event.preventDefault();
        const distance = ["ArrowUp", "ArrowDown"].includes(event.key)
          ? 64 * (backward ? -1 : 1)
          : reader.clientHeight * 0.9 * (backward ? -1 : 1);
        reader.scrollTop += distance;
      }
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
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || wheelProtectedTarget(event)) {
      return;
    }
    if (scrollMode()) {
      if (event.deltaY < 0 && reader.scrollTop <= 0) {
        event.preventDefault();
        run(-1, "wheel");
      } else if (event.deltaY > 0 && reader.scrollTop + reader.clientHeight >= reader.scrollHeight - 1) {
        event.preventDefault();
        run(1, "wheel");
      }
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
      cancelPointer();
      suppressCompatibilityClick(event, true);
      counts.multiTouchProtected += 1;
      return;
    }
    cancelPointer();
    suppressedClick = null;
    const control = scrollMode() ? protectedTarget(event) : hardProtectedTarget(event);
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
    if (hasSelection()) {
      counts.selectionProtected += 1;
      return;
    }
    const media = matchingPath(event, "img[role='button']");
    const structured = matchingPath(event, "table, pre");
    const overflowElement = matchingPath(event, ".atha-structured-overflow");
    const rect = reader.getBoundingClientRect();
    const overflow = overflowElement
      ? {
          element: overflowElement,
          start: overflowElement.scrollLeft,
          maximum: Math.max(0, overflowElement.scrollWidth - overflowElement.clientWidth),
        }
      : null;
    pointer = {
      id: event.pointerId,
      type: event.pointerType || "touch",
      x: event.clientX,
      y: event.clientY,
      owner: null,
      media,
      overflow,
      dragEnabled: (event.pointerType || "touch") === "touch" || Boolean(media || structured),
      rect,
      layoutScale: rect.width > 0 ? reader.clientWidth / rect.width : 1,
      deltaX: 0,
      scrollFrame: 0,
    };
  }

  function onPointerMove(event) {
    if (!pointer || pointer.id !== event.pointerId || scrollMode()) return;
    const deltaX = event.clientX - pointer.x;
    const deltaY = event.clientY - pointer.y;
    const horizontal = Math.abs(deltaX);
    const vertical = Math.abs(deltaY);
    if (!pointer.owner && Math.max(horizontal, vertical) > CLICK_DRIFT) {
      if (vertical > horizontal * AXIS_ADVANTAGE) pointer.owner = "vertical";
      else if (horizontal > vertical * AXIS_ADVANTAGE && pointer.dragEnabled) {
        pointer.owner =
          pointer.overflow && canScrollOverflow(pointer.overflow, deltaX) ? "overflow" : "page";
      }
    }
    if (pointer.owner === "overflow") {
      event.preventDefault();
      previewOverflow(pointer, deltaX);
      return;
    }
    if (pointer.owner !== "page") return;
    event.preventDefault();
    pagination.previewSwipe(deltaX);
  }

  function onPointerUp(event) {
    const start = pointer;
    pointer = null;
    if (!start || start.id !== event.pointerId || hardProtectedTarget(event)) {
      cancelPointer(start);
      return;
    }
    if (start.owner === "overflow") finishOverflow(start);
    if (hasSelection()) {
      pagination.cancelSwipe();
      counts.selectionProtected += 1;
      return;
    }
    if (scrollMode()) {
      const deltaY = event.clientY - start.y;
      if (start.type !== "touch" && Math.hypot(event.clientX - start.x, deltaY) <= CLICK_DRIFT) {
        onCenter();
      }
      return;
    }
    if (start.owner === "page") {
      const direction = swipeDirection(start, event);
      suppressCompatibilityClick(event);
      if (direction) {
        run(direction, start.type === "mouse" ? "mouse" : "touch");
        return;
      }
      pagination.cancelSwipe();
      return;
    }
    if (start.owner === "overflow") {
      suppressCompatibilityClick(event);
      return;
    }
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) > CLICK_DRIFT) {
      suppressCompatibilityClick(event);
      return;
    }
    const ratio = (event.clientX - start.rect.left) / start.rect.width;
    const kind = start.type === "mouse" ? "mouse" : "touch";
    if (ratio < 0.35) {
      suppressCompatibilityClick(event);
      run(-1, kind);
    } else if (ratio > 0.65) {
      suppressCompatibilityClick(event);
      run(1, kind);
    } else if (!start.media) onCenter();
  }

  function onCompatibilityEventCapture(event) {
    if (
      !suppressedClick ||
      performance.now() > suppressedClick.until ||
      (!suppressedClick.anywhere &&
        Math.hypot(event.clientX - suppressedClick.x, event.clientY - suppressedClick.y) >
          CLICK_DRIFT * 2)
    ) {
      return;
    }
    event.preventDefault();
    event.stopImmediatePropagation();
  }

  function onTouchStart(event) {
    if (!scrollMode() || event.touches.length !== 1 || protectedTarget(event)) {
      touch = null;
      return;
    }
    const point = event.touches[0];
    touch = {
      id: point.identifier,
      x: point.clientX,
      y: point.clientY,
      selection: hasSelection(),
    };
  }

  function onTouchEnd(event) {
    const start = touch;
    touch = null;
    if (!start || protectedTarget(event) || start.selection || hasSelection()) return;
    const point = [...event.changedTouches].find((item) => item.identifier === start.id);
    if (!point) return;
    const direction = scrollSwipeDirection(
      start.y,
      point.clientY,
      reader.scrollTop <= 0,
      reader.scrollTop + reader.clientHeight >= reader.scrollHeight - 1,
    );
    if (direction) run(direction, "touch");
    else if (Math.hypot(point.clientX - start.x, point.clientY - start.y) <= CLICK_DRIFT) onCenter();
  }

  function bind() {
    const inside = (handler) => (event) => {
      insideEvents.add(event);
      handler(event);
    };
    const outside = (handler) => (event) => {
      if (!insideEvents.has(event)) handler(event);
    };
    const cancelTouch = () => {
      touch = null;
    };
    content.book.addEventListener("keydown", inside(onKeydown));
    document.addEventListener("keydown", outside(onKeydown));
    content.book.addEventListener("wheel", inside(onWheel), { passive: false });
    reader.addEventListener("wheel", outside(onWheel), { passive: false });
    content.book.addEventListener("pointerdown", inside(onPointerDown));
    reader.addEventListener("pointerdown", outside(onPointerDown));
    content.book.addEventListener("click", onCompatibilityEventCapture, true);
    content.book.addEventListener("dblclick", onCompatibilityEventCapture, true);
    content.book.addEventListener("touchstart", inside(onTouchStart), { passive: true });
    reader.addEventListener("touchstart", outside(onTouchStart), { passive: true });
    content.book.addEventListener("touchend", inside(onTouchEnd), { passive: true });
    reader.addEventListener("touchend", outside(onTouchEnd), { passive: true });
    content.book.addEventListener("touchcancel", inside(cancelTouch), { passive: true });
    reader.addEventListener("touchcancel", outside(cancelTouch), { passive: true });
    window.addEventListener("pointermove", onPointerMove, { passive: false });
    content.book.addEventListener("pointerup", inside(onPointerUp));
    window.addEventListener("pointerup", outside(onPointerUp));
    window.addEventListener("pointercancel", () => {
      cancelPointer();
    });
  }

  const testWheel = createWheelDetector();
  assert(
    testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 0, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 10, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 20, pageHeight: 100 }) === 1 &&
      testWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, timeStamp: 30, pageHeight: 100 }) === 0 &&
      testWheel({ deltaX: 0, deltaY: -60, deltaMode: 0, timeStamp: 300, pageHeight: 100 }) === -1,
    "sample-boundary",
  );
  assert(
    scrollSwipeDirection(100, 160, true, false) === -1 &&
      scrollSwipeDirection(160, 100, false, true) === 1 &&
      scrollSwipeDirection(160, 100, false, false) === 0,
    "sample-boundary",
  );
  const discreteWheel = createWheelDetector();
  assert(
    [0, 100, 200, 300].every(
      (timeStamp) =>
        discreteWheel({ deltaX: 0, deltaY: 100, deltaMode: 0, timeStamp, pageHeight: 100 }) === 1,
    ),
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
