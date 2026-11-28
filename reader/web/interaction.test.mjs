import assert from "node:assert/strict";
import test from "node:test";
import { createInteraction } from "./interaction.mjs";

// Explicit event order and selection-state inputs; real WebView DOM coverage
// is exercised separately by the reader Linux diagnostics.
class Surface {
  listeners = new Map();
  dataset = { readingMode: 'paged' };
  clientWidth = 360;
  clientHeight = 760;
  children = [];
  matches(selector) {
    return (this.overflow && selector === '.atha-structured-overflow') ||
      (this.role === 'dialog' && /\[role=(?:"dialog"|'dialog'|dialog)\]/.test(selector));
  }
  contains(node) { return this === node || this.children.includes(node); }
  getBoundingClientRect() { return { left: 0, top: 0, width: 360, height: 760 }; }
  addEventListener(type, fn) {
    const listeners = this.listeners.get(type) || [];
    listeners.push(fn);
    this.listeners.set(type, listeners);
  }
  emit(type, event) { for (const fn of this.listeners.get(type) || []) fn(event); }
}
async function run(steps, verticalSwipe = false) {
  let now = 0, selected = false, timerId = 0;
  const timers = new Map(), actions = [];
  const book = new Surface(), reader = new Surface();
  const overflow = new Surface();
  const appConversation = Object.assign(new Surface(), { id: 'message-conversation', role: 'dialog' });
  const bookDialog = Object.assign(new Surface(), { id: 'message-conversation', role: 'dialog' });
  book.children.push(bookDialog);
  Object.assign(overflow, { overflow: true, scrollLeft: 0, scrollWidth: 1000 });
  globalThis.Element = Surface;
  globalThis.window = new Surface();
  globalThis.document = new Surface();
  document.documentElement = { dataset: {} };
  globalThis.performance = { now: () => now };
  globalThis.setTimeout = (fn, delay) => { const id = ++timerId; timers.set(id, { at: now + delay, fn }); return id; };
  globalThis.clearTimeout = (id) => timers.delete(id);
  globalThis.requestAnimationFrame = (fn) => setTimeout(fn, 16);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  const interaction = createInteraction({
    reader, content: { book, selectionRange: () => selected ? {} : null },
    navigation: { previous: () => actions.push('previous'), next: () => actions.push('next') },
    pagination: { cancelSwipe() {}, previewSwipe: () => actions.push('preview'), finishSwipe() {} },
    onCenter: () => actions.push('center'),
    beginVerticalSwipe: verticalSwipe ? () => (phase, distance) => actions.push(`${phase}:${distance}`) : undefined,
    assert: (v) => assert.ok(v), fail: (e) => { throw Error(e); },
  });
  interaction.bind();
  for (const step of steps) {
    for (; !step.skipTimers;) {
      const due = [...timers].filter(([, t]) => t.at <= step.at).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      const [id, timer] = due;
      now = timer.at;
      timers.delete(id);
      timer.fn();
    }
    now = step.at;
    if ('scrollLeft' in step) assert.equal(overflow.scrollLeft, step.scrollLeft, 'internal overflow scroll');
    if (step.mode) reader.dataset.readingMode = step.mode;
    if ('selected' in step) {
      selected = step.selected;
      document.emit('selectionchange', {});
    }
    if (!step.type) continue;
    const event = {
      pointerId: step.id ?? 1, pointerType: step.pointerType ?? 'touch', button: 0,
      isPrimary: step.primary ?? true, clientX: step.x ?? 300, clientY: step.y ?? 200,
      key: step.key, deltaX: 0, deltaY: step.deltaY, deltaMode: 0, timeStamp: now,
      composedPath: () => step.conversation ? [appConversation, document] :
        [step.bookDialog ? bookDialog : step.overflow ? overflow : null, book, reader].filter(Boolean),
      preventDefault() { this.defaultPrevented = true; },
      stopImmediatePropagation() { this.stopped = true; },
    };
    if (step.type.startsWith('touch')) {
      const point = { identifier: step.id ?? 1, clientX: event.clientX, clientY: event.clientY };
      event.touches = ['touchstart', 'touchmove'].includes(step.type) ? [point] : [];
      event.changedTouches = [point];
    }
    const surface = step.conversation ? document :
      ['pointermove', 'pointercancel'].includes(step.type) ? window : book;
    surface.emit(step.type, event);
    if ('prevented' in step) {
      assert.equal(!!event.defaultPrevented, step.prevented, 'compatibility click default');
      assert.equal(!!event.stopped, step.prevented, 'compatibility click propagation');
    }
    await Promise.resolve();
  }
  return { actions, counts: interaction.snapshot() };
}
const down = { at: 0, type: 'pointerdown' };
const up = (at, x = 300, extra = {}) => ({ at, type: 'pointerup', x, ...extra });
const move = (at, x, extra = {}) => ({ at, type: 'pointermove', x, ...extra });
const cases = [
  { name: 'short right tap', steps: [down, up(80)], want: ['next'] },
  { name: 'short left tap', steps: [{ ...down, x: 40 }, up(80, 40)], want: ['previous'] },
  { name: 'short center tap', steps: [{ ...down, x: 180 }, up(80, 180)], want: ['center'] },
  { name: '5px short tap drift', steps: [down, up(80, 305)], want: ['next'] },
  { name: 'long stationary hold', steps: [down, up(1500)], want: [], noPreview: true },
  { name: 'long hold with 5px drift', steps: [down, move(300, 305), up(1500, 305)], want: [], noPreview: true },
  { name: 'long hold then late selection', steps: [down, up(1500), { at: 1501, selected: true }], want: [], noPreview: true },
  { name: 'hold first then drift 15px', steps: [down, move(900, 285), up(1000, 285)], want: [], noPreview: true, assumption: 'a stationary 500ms hold reserves the gesture' },
  { name: 'quick horizontal swipe', steps: [down, move(30, 280), up(100, 220)], want: ['next'] },
  { name: 'slow continuous swipe', steps: [down, move(30, 280), move(600, 240), up(1500, 220)], want: ['next'] },
  { name: 'reverse horizontal swipe', steps: [{ ...down, x: 40 }, move(30, 60), up(100, 120)], want: ['previous'] },
  { name: 'vertical movement', steps: [down, move(30, 301, { y: 240 }), up(100, 302, { y: 280 })], want: [], noPreview: true },
  { name: 'vertical chrome drag follows final displacement on release', verticalSwipe: true,
    steps: [down, move(30, 300, { y: 340 }), move(80, 300, { y: 210 }), up(100)],
    want: ['move:140', 'move:10', 'end:0'], noPreview: true },
  { name: 'cancelled vertical chrome drag never commits after touchend', verticalSwipe: true,
    steps: [down, move(30, 300, { y: 340 }), { at: 40, type: 'pointercancel' }, { at: 60, type: 'touchend', y: 340 }, { at: 61 }],
    want: ['move:140', 'cancel:0'], noPreview: true },
  { name: 'selection takes ownership from a vertical drag', verticalSwipe: true,
    steps: [down, move(30, 300, { y: 340 }), { at: 40, selected: true }, { at: 50, selected: false }, up(60, 300, { y: 340 })],
    want: ['move:140', 'cancel:0'], noPreview: true },
  { name: 'second finger cancels an armed vertical drag', verticalSwipe: true,
    steps: [down, move(30, 300, { y: 340 }), { ...down, at: 50, id: 2, primary: false }, up(80, 300, { y: 340 })],
    want: ['move:140', 'cancel:0'], noPreview: true },
  { name: 'structured content does not start a vertical chrome drag', verticalSwipe: true,
    steps: [{ ...down, overflow: true }, move(30, 300, { y: 340 }), up(50, 300, { y: 340 })],
    want: [], noPreview: true },
  { name: 'a horizontal swipe stays with the page-turn renderer', verticalSwipe: true,
    steps: [down, move(30, 240), move(50, 240, { y: 320 }), up(60, 210)],
    want: ['next'] },
  { name: 'selected during movement', steps: [down, { at: 300, selected: true }, move(350, 285), up(400, 285)], want: [], noPreview: true },
  { name: 'selection transient before movement', steps: [down, { at: 200, selected: true }, { at: 250, selected: false }, move(300, 280), up(400, 220)], want: [], noPreview: true },
  { name: 'selection transient after preview', steps: [down, move(30, 280), { at: 50, selected: true }, { at: 70, selected: false }, up(100, 220)], want: [] },
  { name: 'existing selection then deselect', steps: [{ at: 0, selected: true }, { ...down, at: 1 }, { at: 50, selected: false }, move(100, 250), up(150, 200)], want: [], noPreview: true },
  { name: 'selection latch resets next gesture', steps: [down, { at: 50, selected: true }, up(100), { at: 150, selected: false }, { ...down, at: 200, id: 2 }, up(280, 300, { id: 2 })], want: ['next'], noPreview: true },
  { name: 'cancelled stationary press', steps: [down, { at: 100, type: 'pointercancel' }, { at: 110, type: 'touchend' }, { at: 111 }], want: [], noPreview: true },
  { name: 'existing recognized swipe fallback', steps: [down, move(30, 270), { at: 40, type: 'pointercancel' }, { at: 50, type: 'touchmove', x: 220 }, { at: 60, type: 'touchend', x: 220 }, { at: 61 }], want: ['next'] },
  { name: 'cancel before movement then touch swipe', steps: [down, { at: 20, type: 'pointercancel' }, { at: 30, type: 'touchmove', x: 220 }, { at: 40, type: 'touchend', x: 220 }, { at: 41 }], want: ['next'], assumption: 'retain the current orphan-touch fallback pending device evidence' },
  { name: 'touchend without pointerup', steps: [down, move(30, 270), { at: 40, type: 'touchend', x: 220 }, { at: 41 }], want: ['next'] },
  { name: 'mouse long press semantics unchanged', steps: [{ ...down, pointerType: 'mouse' }, up(1500, 300, { pointerType: 'mouse' })], want: ['next'] },
  { name: '499ms remains a tap', steps: [down, up(499)], want: ['next'] },
  { name: '500ms reserves a hold', steps: [down, up(500)], want: [] },
  { name: 'delayed hold timer cannot admit a late move', steps: [down, move(600, 240, { skipTimers: true }), up(650, 240)], want: [], noPreview: true },
  { name: 'hold consumes compatibility click before and after release', steps: [down, { at: 600, type: 'click', prevented: true }, up(700), { at: 701, type: 'click', prevented: true }], want: [] },
  { name: 'selection drag consumes compatibility click at another point', steps: [down, { at: 200, selected: true }, up(300, 220), { at: 301, type: 'click', x: 240, prevented: true }], want: [] },
  { name: 'cancelled stationary press consumes compatibility click', steps: [down, { at: 100, type: 'pointercancel' }, { at: 110, type: 'touchend' }, { at: 111, type: 'click', prevented: true }], want: [] },
  { name: 'hold clears for the next touch', steps: [down, up(600), { ...down, at: 650, id: 2 }, up(700, 300, { id: 2 })], want: ['next'] },
  { name: 'replaced pointer cannot inherit the old hold timer', steps: [down, { ...down, at: 400, id: 2 }, up(550, 300, { id: 2 })], want: ['next'] },
  { name: 'late old release cannot end a newer touch', steps: [down, { ...down, at: 400, id: 2 }, up(450), up(550, 300, { id: 2 })], want: ['next'] },
  { name: 'second finger cancels a pending hold', steps: [down, { ...down, at: 100, id: 2, primary: false }, up(600)], want: [], noPreview: true },
  { name: 'vertical drag returning to start is not a tap', steps: [down, move(50, 300, { y: 240 }), up(100)], want: [], noPreview: true },
  { name: 'ambiguous diagonal returning to start is not a tap', steps: [down, move(50, 310, { y: 210 }), up(100)], want: [], noPreview: true },
  { name: 'touchcancel removes the pending hold', steps: [down, { at: 100, type: 'touchcancel' }, { ...down, at: 400, id: 2 }, up(550, 300, { id: 2 })], want: ['next'] },
  { name: 'scroll hold does not open tools on release', steps: [{ ...down, mode: 'scroll' }, { at: 1, type: 'touchstart' }, { at: 1000, type: 'touchend' }, { at: 1001 }], want: [] },
  { name: 'scroll short tap keeps the tool action', steps: [{ ...down, mode: 'scroll' }, { at: 1, type: 'touchstart' }, { at: 80, type: 'touchend' }, { at: 81 }], want: ['center'] },
  { name: 'long hold permits internal code scrolling', steps: [{ ...down, overflow: true }, move(600, 220), up(650, 220, { overflow: true }), { at: 700, scrollLeft: 80 }], want: [], noPreview: true },
  { name: 'held overflow boundary cannot hand off to page turn', steps: [{ ...down, overflow: true }, move(600, 360), up(650, 360, { overflow: true }), { at: 700, scrollLeft: 0 }], want: [], noPreview: true },
  { name: 'selection owns held code scrolling', steps: [{ ...down, overflow: true }, { at: 200, selected: true }, move(600, 220), up(650, 220, { overflow: true }), { at: 700, scrollLeft: 0 }], want: [], noPreview: true },
  { name: 'cancel cleanup is not finger release', steps: [down, { at: 200, type: 'pointercancel' }, { at: 1500, type: 'touchend' }, { at: 1501, type: 'click', prevented: true }], want: [] },
  { name: 'late pointer release refreshes click protection', steps: [down, { at: 200, type: 'pointercancel' }, up(1500), { at: 1501, type: 'click', prevented: true }], want: [] },
  { name: 'cancelled WebView click without touchend is consumed', steps: [down, { at: 200, type: 'pointercancel' }, { at: 1500, type: 'click', prevented: true }], want: [] },
  { name: 'new pointer clears cancelled touch ownership', steps: [down, { at: 200, type: 'pointercancel' }, { ...down, at: 1500, id: 2 }, up(1550, 300, { id: 2 })], want: ['next'] },
  {
    name: 'focused app conversation protects keyboard without turning the page',
    steps: ['PageDown', 'ArrowRight', ' '].map((key, at) => ({ at, type: 'keydown', key, conversation: true })),
    want: [], counts: { controlProtected: 3, keyboard: 0 },
  },
  {
    name: 'book role dialog retains pointer, wheel and keyboard turns even with an app-like ID',
    steps: [
      { ...down, bookDialog: true, pointerType: 'mouse' },
      up(80, 300, { bookDialog: true, pointerType: 'mouse' }),
      { at: 100, type: 'wheel', deltaY: 100, bookDialog: true },
      { at: 120, type: 'keydown', key: 'PageDown', bookDialog: true },
    ],
    want: ['next', 'next', 'next'], counts: { controlProtected: 0, mouse: 1, wheel: 1, keyboard: 1 },
  },
];

for (const scenario of cases) {
  test(scenario.name, async () => {
    const names = ["Element", "window", "document", "performance", "setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"];
    const descriptors = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
    try {
      const { actions, counts } = await run(scenario.steps, scenario.verticalSwipe);
      assert.deepEqual(actions.filter(action => action !== "preview"), scenario.want);
      for (const [name, count] of Object.entries(scenario.counts || {})) assert.equal(counts[name], count, name);
      if (scenario.noPreview) assert.ok(!actions.includes("preview"), "selection/hold must not preview a turn");
    } finally {
      for (const [name, descriptor] of descriptors) {
        if (descriptor) Object.defineProperty(globalThis, name, descriptor);
        else delete globalThis[name];
      }
    }
  });
}
