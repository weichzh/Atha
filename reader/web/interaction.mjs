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

export function createInteraction({ reader, content, navigation, pagination, preferences, onCenter, assert, fail }) {
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
  const insideEvents = new WeakSet();

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
    return preferences.snapshot().book.readingMode === "scroll";
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
      type: event.pointerType || "touch",
      x: event.clientX,
      y: event.clientY,
      axis: null,
      selection: hasSelection(),
    };
  }

  function onPointerMove(event) {
    if (!pointer || pointer.id !== event.pointerId || pointer.type !== "touch" || scrollMode()) return;
    const deltaX = event.clientX - pointer.x;
    const deltaY = event.clientY - pointer.y;
    if (!pointer.axis && Math.max(Math.abs(deltaX), Math.abs(deltaY)) > CLICK_DRIFT) {
      pointer.axis = Math.abs(deltaX) > Math.abs(deltaY) ? "horizontal" : "vertical";
    }
    if (pointer.axis !== "horizontal") return;
    event.preventDefault();
    pagination.previewSwipe(deltaX);
  }

  function onPointerUp(event) {
    const start = pointer;
    pointer = null;
    if (!start || start.id !== event.pointerId || protectedTarget(event)) {
      pagination.cancelSwipe();
      return;
    }
    if (start.selection || hasSelection()) {
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
    if (start.type === "touch") {
      const direction = swipeDirection(start, event);
      if (direction) {
        run(direction, "touch");
        return;
      }
      pagination.cancelSwipe();
      if (Math.hypot(event.clientX - start.x, event.clientY - start.y) > CLICK_DRIFT) return;
      const rect = reader.getBoundingClientRect();
      const ratio = (event.clientX - rect.left) / rect.width;
      if (ratio < 0.35) {
        run(-1, "touch");
      } else if (ratio > 0.65) {
        run(1, "touch");
      } else onCenter();
      return;
    }
    if (start.type !== "mouse" || Math.hypot(event.clientX - start.x, event.clientY - start.y) > CLICK_DRIFT) {
      return;
    }
    const rect = reader.getBoundingClientRect();
    const ratio = (event.clientX - rect.left) / rect.width;
    if (ratio < 0.35) {
      run(-1, "mouse");
    } else if (ratio > 0.65) {
      run(1, "mouse");
    } else onCenter();
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
      pointer = null;
      pagination.cancelSwipe();
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
