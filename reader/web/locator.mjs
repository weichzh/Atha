const MAX_SERIALIZED_LENGTH = 2048;
const MAX_TEXT_OFFSET = 2147483647;

export function createLocator({ assert }) {
  function invalid(code) {
    throw new Error(code);
  }

  function exactKeys(value, keys) {
    const actual = Object.keys(value).sort();
    const expected = [...keys].sort();
    if (
      actual.length !== expected.length ||
      actual.some((key, index) => key !== expected[index])
    ) {
      invalid("locator-format");
    }
  }

  function sectionOrder(book) {
    return new Map(book.sections.map((section, index) => [section.id, index]));
  }

  function validatePoint(value, order) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      invalid("locator-format");
    }
    exactKeys(value, ["section", "offset"]);
    if (!order.has(value.section)) invalid("locator-section");
    if (
      !Number.isInteger(value.offset) ||
      value.offset < 0 ||
      value.offset > MAX_TEXT_OFFSET
    ) {
      invalid("locator-offset");
    }
    return Object.freeze({ section: value.section, offset: value.offset });
  }

  function comparePoints(left, right, order) {
    const sectionDifference = order.get(left.section) - order.get(right.section);
    return sectionDifference || left.offset - right.offset;
  }

  function validate(value, book) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      invalid("locator-format");
    }
    const keys = ["schema", "contentVersion", "start"];
    if (Object.hasOwn(value, "end")) keys.push("end");
    exactKeys(value, keys);
    if (value.schema !== 1) invalid("locator-format");
    if (value.contentVersion !== book.contentVersion) invalid("locator-version");
    const order = sectionOrder(book);
    const start = validatePoint(value.start, order);
    const end = Object.hasOwn(value, "end") ? validatePoint(value.end, order) : null;
    if (end && end.section !== start.section) invalid("locator-range");
    if (end && comparePoints(start, end, order) > 0) invalid("locator-range");
    const locator = { schema: 1, contentVersion: book.contentVersion, start };
    if (end) locator.end = end;
    return Object.freeze(locator);
  }

  function point(book, section, offset) {
    return validate(
      { schema: 1, contentVersion: book.contentVersion, start: { section, offset } },
      book,
    );
  }

  function range(book, start, end) {
    return validate({ schema: 1, contentVersion: book.contentVersion, start, end }, book);
  }

  function parse(book, serialized) {
    if (
      typeof serialized !== "string" ||
      serialized.length === 0 ||
      serialized.length > MAX_SERIALIZED_LENGTH
    ) {
      invalid("locator-format");
    }
    let value;
    try {
      value = JSON.parse(serialized);
    } catch {
      invalid("locator-format");
    }
    return validate(value, book);
  }

  function serialize(book, value) {
    return JSON.stringify(validate(value, book));
  }

  function compare(book, left, right) {
    const order = sectionOrder(book);
    const a = validate(left, book);
    const b = validate(right, book);
    const startDifference = comparePoints(a.start, b.start, order);
    if (startDifference) return Math.sign(startDifference);
    const endDifference = comparePoints(a.end || a.start, b.end || b.start, order);
    return Math.sign(endDifference);
  }

  function rejected(action, code) {
    try {
      action();
      return false;
    } catch (error) {
      return error instanceof Error && error.message === code;
    }
  }

  function selfCheck() {
    const book = {
      contentVersion: "a".repeat(64),
      sections: [{ id: "one" }, { id: "two" }],
    };
    const first = point(book, "one", 10);
    const second = point(book, "two", 0);
    const ranged = range(book, first.start, { section: "one", offset: 11 });
    assert(compare(book, first, second) < 0, "sample-boundary");
    assert(compare(book, parse(book, serialize(book, first)), first) === 0, "sample-boundary");
    assert(compare(book, first, ranged) < 0, "sample-boundary");
    for (const [action, code] of [
      [() => parse(book, "{"), "locator-format"],
      [
        () => validate({ ...first, contentVersion: "b".repeat(64) }, book),
        "locator-version",
      ],
      [() => point(book, "missing", 0), "locator-section"],
      [() => point(book, "one", -1), "locator-offset"],
      [() => range(book, first.start, second.start), "locator-range"],
      [() => range(book, { section: "one", offset: 11 }, first.start), "locator-range"],
      [() => validate({ ...first, extra: true }, book), "locator-format"],
    ]) {
      assert(rejected(action, code), "sample-boundary");
    }
  }

  selfCheck();
  return Object.freeze({ compare, parse, point, range, serialize });
}
