import assert from "node:assert/strict";
import test from "node:test";
import { createBookSelection } from "./content.mjs";

class FakeNode {
  constructor(name, root, length = 0) {
    this.name = name;
    this.root = root;
    this.length = length;
  }
  getRootNode() { return this.root; }
}

class FakeRange {
  constructor(book, start, startOffset, end = start, endOffset = startOffset) {
    this.book = book;
    this.startContainer = start;
    this.startOffset = startOffset;
    this.endContainer = end;
    this.endOffset = endOffset;
  }
  get collapsed() {
    return this.startContainer === this.endContainer && this.startOffset === this.endOffset;
  }
  get commonAncestorContainer() {
    return this.startContainer === this.endContainer ? this.startContainer : this.book;
  }
  cloneRange() {
    return new FakeRange(this.book, this.startContainer, this.startOffset, this.endContainer, this.endOffset);
  }
  setStart(node, offset) {
    if (!Number.isInteger(offset) || offset < 0 || offset > node.length) throw new RangeError("offset");
    this.startContainer = node;
    this.startOffset = offset;
    if (this.endContainer === node && offset > this.endOffset) {
      this.endContainer = node;
      this.endOffset = offset;
    }
  }
  setEnd(node, offset) {
    if (!Number.isInteger(offset) || offset < 0 || offset > node.length) throw new RangeError("offset");
    this.endContainer = node;
    this.endOffset = offset;
    if (this.startContainer === node && offset < this.startOffset) {
      this.startContainer = node;
      this.startOffset = offset;
    }
  }
}

class FakeSelection {
  ranges = [];
  composedRanges = null;
  composedOptions = null;
  get rangeCount() { return this.ranges.length; }
  getRangeAt(index) {
    if (!this.ranges[index]) throw new RangeError("range index");
    return this.ranges[index];
  }
  removeAllRanges() { this.ranges = []; }
  addRange(range) { this.ranges.push(range); }
  getComposedRanges(options) {
    this.composedOptions = options;
    return this.composedRanges;
  }
}

function fixture() {
  const root = { getSelection: () => shadowSelection };
  const book = new FakeNode("book", root);
  const text = new FakeNode("text", root, 12);
  const secondText = new FakeNode("second-text", root, 8);
  const outsideRoot = {};
  const outside = new FakeNode("outside", outsideRoot, 10);
  const contains = new Set([book, text, secondText]);
  book.contains = (node) => contains.has(node);
  const shadowSelection = new FakeSelection();
  const windowSelection = new FakeSelection();
  const document = {
    defaultView: { getSelection: () => windowSelection },
    createRange: () => new FakeRange(book, text, 0),
  };
  book.ownerDocument = document;
  return {
    root, book, text, secondText, outside, shadowSelection, windowSelection,
    selection: createBookSelection(book, root),
    range: (start = text, startOffset = 1, end = text, endOffset = 5) =>
      new FakeRange(book, start, startOffset, end, endOffset),
  };
}

test("range reads a cloned direct shadow selection and rejects collapsed or multiple ranges", () => {
  const f = fixture();
  const original = f.range();
  f.shadowSelection.addRange(original);
  f.shadowSelection.isCollapsed = true;

  const selected = f.selection.range();
  assert.notEqual(selected, original);
  assert.deepEqual(
    [selected.startContainer, selected.startOffset, selected.endContainer, selected.endOffset],
    [f.text, 1, f.text, 5],
  );

  f.shadowSelection.ranges = [f.range(f.text, 3, f.text, 3)];
  assert.equal(f.selection.range(), null);
  f.shadowSelection.ranges = [f.range(), f.range(f.text, 6, f.text, 8)];
  assert.equal(f.selection.range(), null);
});

test("clear removes a valid book selection and a collapsed book caret only", () => {
  const f = fixture();
  f.shadowSelection.addRange(f.range());
  assert.equal(f.selection.clear(), true);
  assert.equal(f.shadowSelection.rangeCount, 0);

  f.shadowSelection.addRange(f.range(f.text, 4, f.text, 4));
  assert.equal(f.selection.range(), null);
  assert.equal(f.selection.clear(), true);
  assert.equal(f.shadowSelection.rangeCount, 0);

  f.shadowSelection.addRange(f.range(f.outside, 1, f.outside, 4));
  assert.equal(f.selection.clear(), false);
  assert.equal(f.shadowSelection.rangeCount, 1, "external selection must remain untouched");

  f.shadowSelection.ranges = [f.range(f.text, 2, f.text, 2), f.range(f.text, 5, f.text, 5)];
  assert.equal(f.selection.clear(), false);
  assert.equal(f.shadowSelection.rangeCount, 2, "ambiguous multi-range ownership must remain untouched");

  f.shadowSelection.ranges = [f.range(f.outside, 4, f.outside, 4)];
  assert.equal(f.selection.clear(), false, "a collapsed caret outside the book is not owned");
  assert.equal(f.shadowSelection.rangeCount, 1);

  f.shadowSelection.ranges = [];
  f.windowSelection.addRange(f.range(f.outside, 1, f.outside, 4));
  assert.equal(f.selection.clear(), false, "an external Window selection must remain untouched");
  assert.equal(f.windowSelection.rangeCount, 1);
  f.windowSelection.ranges = [f.range(), f.range(f.text, 6, f.text, 8)];
  assert.equal(f.selection.clear(), false, "multiple Window ranges have ambiguous ownership");
  assert.equal(f.windowSelection.rangeCount, 2);
});

test("set accepts a non-collapsed book range and rejects collapsed or outside ranges", () => {
  const f = fixture();
  const input = f.range(f.text, 2, f.secondText, 3);
  assert.equal(f.selection.set(input), true);
  assert.equal(f.shadowSelection.rangeCount, 1);
  assert.notEqual(f.shadowSelection.getRangeAt(0), input);
  assert.deepEqual(
    [f.selection.range().startOffset, f.selection.range().endContainer, f.selection.range().endOffset],
    [2, f.secondText, 3],
  );

  assert.equal(f.selection.set(f.range(f.text, 2, f.text, 2)), false);
  assert.equal(f.selection.set(f.range(f.outside, 1, f.outside, 4)), false);
  assert.equal(f.shadowSelection.rangeCount, 1, "rejected set must preserve the current selection");
});

test("set does not clear an existing external window selection", () => {
  const f = fixture();
  f.windowSelection.addRange(f.range(f.outside, 2, f.outside, 6));

  assert.equal(f.selection.set(f.range()), false);
  assert.equal(f.windowSelection.rangeCount, 1);
  assert.equal(f.windowSelection.getRangeAt(0).startContainer, f.outside);
});

test("set updates an existing valid Window selection when the shadow selection is empty", () => {
  const f = fixture();
  f.windowSelection.addRange(f.range(f.text, 1, f.text, 3));
  const replacement = f.range(f.text, 6, f.text, 9);

  assert.equal(f.selection.set(replacement), true);
  assert.equal(f.shadowSelection.rangeCount, 0, "empty shadow selection must not mask the active Window selection");
  assert.deepEqual(
    [f.windowSelection.rangeCount, f.selection.range().startOffset, f.selection.range().endOffset],
    [1, 6, 9],
  );
});

test("empty shadow selection recovers one valid composed window range with the root", () => {
  const f = fixture();
  f.windowSelection.addRange(f.range(f.outside, 1, f.outside, 2));
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 2, endContainer: f.text, endOffset: 7 }];

  const selected = f.selection.range();
  assert.deepEqual([selected.startContainer, selected.startOffset, selected.endOffset], [f.text, 2, 7]);
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });
});

test("nonempty invalid shadow selection blocks window fallback", () => {
  for (const withWindowFallback of [false, true]) {
    const f = fixture();
    const invalidShadowRange = f.range(f.outside, 1, f.outside, 4);
    f.shadowSelection.addRange(invalidShadowRange);
    if (withWindowFallback) f.windowSelection.addRange(f.range(f.outside, 2, f.outside, 3));
    f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 2, endContainer: f.text, endOffset: 7 }];

    assert.equal(f.selection.range(), null);
    assert.equal(f.selection.clear(), false);
    assert.equal(f.selection.set(f.range(f.text, 3, f.text, 8)), false);
    assert.equal(f.shadowSelection.rangeCount, 1);
    assert.equal(f.shadowSelection.getRangeAt(0), invalidShadowRange);
    assert.deepEqual(
      [invalidShadowRange.startContainer, invalidShadowRange.startOffset, invalidShadowRange.endContainer, invalidShadowRange.endOffset],
      [f.outside, 1, f.outside, 4],
    );
    assert.equal(f.windowSelection.rangeCount, withWindowFallback ? 1 : 0);
    assert.equal(f.windowSelection.composedOptions, null, "invalid shadow range must not consult fallback");
  }
});

test("composed recovery rejects multiple, cross-root, out-of-bounds, and relocated ranges", () => {
  const f = fixture();
  const retargetWindowSelection = () => {
    f.windowSelection.ranges = [f.range(f.outside, 1, f.outside, 2)];
    f.windowSelection.composedOptions = null;
  };

  retargetWindowSelection();
  f.windowSelection.composedRanges = [
    { startContainer: f.text, startOffset: 1, endContainer: f.text, endOffset: 4 },
    { startContainer: f.text, startOffset: 5, endContainer: f.text, endOffset: 8 },
  ];
  assert.equal(f.selection.range(), null);
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });

  retargetWindowSelection();
  f.secondText.root = f.outside.getRootNode();
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 1, endContainer: f.secondText, endOffset: 4 }];
  assert.equal(f.selection.range(), null);
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] }, "a contained endpoint from another root must be rejected");
  f.secondText.root = f.root;

  retargetWindowSelection();
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 1, endContainer: f.text, endOffset: 99 }];
  assert.equal(f.selection.range(), null, "native Range must reject an offset past the node length");
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });

  retargetWindowSelection();
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 8, endContainer: f.text, endOffset: 3 }];
  assert.equal(f.selection.range(), null, "setEnd relocation must fail endpoint identity validation");
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });
});

test("composed recovery rejects a collapsed range", () => {
  const f = fixture();
  f.windowSelection.addRange(f.range(f.outside, 1, f.outside, 2));
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 4, endContainer: f.text, endOffset: 4 }];
  assert.equal(f.selection.range(), null);
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });
});

test("selection getter and composed-range exceptions fail closed", () => {
  const getterFailure = fixture();
  getterFailure.root.getSelection = () => { throw new Error("closed root"); };
  getterFailure.shadowSelection.addRange(getterFailure.range());
  assert.equal(getterFailure.selection.range(), null);
  assert.equal(getterFailure.selection.clear(), false);
  assert.equal(getterFailure.shadowSelection.rangeCount, 1);

  const composedFailure = fixture();
  composedFailure.windowSelection.addRange(composedFailure.range(composedFailure.outside, 1, composedFailure.outside, 2));
  composedFailure.windowSelection.getComposedRanges = () => { throw new Error("unsupported composed ranges"); };
  assert.equal(composedFailure.selection.range(), null);
});

test("missing shadow getter recovers and clears the composed Window selection", () => {
  const f = fixture();
  delete f.root.getSelection;
  f.windowSelection.addRange(f.range(f.outside, 1, f.outside, 2));
  f.windowSelection.composedRanges = [{ startContainer: f.text, startOffset: 3, endContainer: f.text, endOffset: 8 }];

  const selected = f.selection.range();
  assert.deepEqual([selected.startContainer, selected.startOffset, selected.endOffset], [f.text, 3, 8]);
  assert.deepEqual(f.windowSelection.composedOptions, { shadowRoots: [f.root] });
  assert.equal(f.selection.clear(), true);
  assert.equal(f.windowSelection.rangeCount, 0);
});
