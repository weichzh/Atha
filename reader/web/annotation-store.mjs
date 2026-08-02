const SCHEMA = 1;
const MAX_RECORD_LENGTH = 2 * 1024 * 1024;
const MAX_ANNOTATIONS = 1000;
const MAX_SELECTED_LENGTH = 4096;
const MAX_NOTE_LENGTH = 2000;
const CONTEXT_LENGTH = 32;

export async function annotationTextHash(text) {
  const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(bytes)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}

export function createAnnotationStore({ storage, requireDurable, keyPrefix, bookKey, locator }) {
  if (!["atha.reader", "atha.reader.probe"].includes(keyPrefix)) {
    throw new Error("invalid-state-key");
  }
  if (!/^[a-z0-9-]{1,64}$/.test(bookKey || "")) throw new Error("invalid-state-key");
  const key = `${keyPrefix}.annotations.${bookKey}.v1`;
  const volatile = new Map();
  const target =
    storage ||
    Object.freeze({
      getItem: (name) => volatile.get(name) ?? null,
      removeItem: (name) => volatile.delete(name),
      setItem: (name, value) => volatile.set(name, value),
    });
  let items = [];
  let available = Boolean(storage) || !requireDurable;
  let restored = false;
  let lastError = available ? null : "annotation-storage";
  let writes = 0;

  function exact(value, expected) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const keys = Object.keys(value).sort();
    return keys.length === expected.length && keys.every((name, index) => name === expected[index]);
  }

  function validSourceAnchor(value) {
    if (
      !exact(value, [
        "canonicalLocator",
        "contentHash",
        "prefixText",
        "schema",
        "selectedText",
        "suffixText",
      ]) ||
      value.schema !== SCHEMA ||
      typeof value.canonicalLocator !== "string" ||
      typeof value.selectedText !== "string" ||
      !value.selectedText.trim() ||
      value.selectedText.length > MAX_SELECTED_LENGTH ||
      typeof value.prefixText !== "string" ||
      value.prefixText.length > CONTEXT_LENGTH ||
      typeof value.suffixText !== "string" ||
      value.suffixText.length > CONTEXT_LENGTH ||
      typeof value.contentHash !== "string" ||
      !/^[a-f0-9]{64}$/.test(value.contentHash)
    ) {
      return false;
    }
    try {
      const valueLocator = locator.inspect(value.canonicalLocator);
      return Boolean(
        valueLocator.end &&
          valueLocator.contentVersion !== null &&
          JSON.stringify(valueLocator) === value.canonicalLocator &&
          valueLocator.end.offset - valueLocator.start.offset === value.selectedText.length,
      );
    } catch {
      return false;
    }
  }

  function validItem(value) {
    return (
      exact(value, [
        "createdAt",
        "deletedAt",
        "id",
        "note",
        "sourceAnchor",
        "type",
        "updatedAt",
      ]) &&
      typeof value.id === "string" &&
      /^[a-z0-9-]{1,64}$/.test(value.id) &&
      ["highlight", "note"].includes(value.type) &&
      typeof value.note === "string" &&
      value.note.length <= MAX_NOTE_LENGTH &&
      (value.type === "note" ? Boolean(value.note.trim()) : !value.note) &&
      Number.isInteger(value.createdAt) &&
      value.createdAt >= 0 &&
      Number.isInteger(value.updatedAt) &&
      value.updatedAt >= value.createdAt &&
      (value.deletedAt === null ||
        (Number.isInteger(value.deletedAt) && value.deletedAt >= value.createdAt)) &&
      validSourceAnchor(value.sourceAnchor)
    );
  }

  function validRecord(value) {
    return (
      exact(value, ["items", "schema"]) &&
      value.schema === SCHEMA &&
      Array.isArray(value.items) &&
      value.items.length <= MAX_ANNOTATIONS &&
      value.items.every(validItem) &&
      new Set(value.items.map((item) => item.id)).size === value.items.length
    );
  }

  function write(next) {
    if (!available) return false;
    try {
      const serialized = JSON.stringify({ schema: SCHEMA, items: next });
      if (serialized.length > MAX_RECORD_LENGTH) throw new Error("annotation-write");
      target.setItem(key, serialized);
      writes += 1;
      return true;
    } catch {
      available = false;
      lastError = "annotation-write";
      return false;
    }
  }

  function freezeAnchor(value) {
    return Object.freeze({
      schema: value.schema,
      canonicalLocator: value.canonicalLocator,
      selectedText: value.selectedText,
      prefixText: value.prefixText,
      suffixText: value.suffixText,
      contentHash: value.contentHash,
    });
  }

  function freezeItem(value) {
    return Object.freeze({ ...value, sourceAnchor: freezeAnchor(value.sourceAnchor) });
  }

  function active() {
    return items.filter((item) => item.deletedAt === null);
  }

  function item(id) {
    return active().find((value) => value.id === id) || null;
  }

  async function add(sourceAnchor, noteValue) {
    if (!available) return Object.freeze({ ok: false, error: lastError });
    if (!validSourceAnchor(sourceAnchor)) {
      return Object.freeze({ ok: false, error: "annotation-anchor" });
    }
    if ((await annotationTextHash(sourceAnchor.selectedText)) !== sourceAnchor.contentHash) {
      return Object.freeze({ ok: false, error: "annotation-anchor" });
    }
    if (items.length >= MAX_ANNOTATIONS) {
      return Object.freeze({ ok: false, error: "annotation-limit" });
    }
    const note = String(noteValue).trim();
    if (note.length > MAX_NOTE_LENGTH) {
      return Object.freeze({ ok: false, error: "annotation-note" });
    }
    const createdAt = Date.now();
    const value = freezeItem({
      id: crypto.randomUUID(),
      type: note ? "note" : "highlight",
      sourceAnchor,
      note,
      createdAt,
      updatedAt: createdAt,
      deletedAt: null,
    });
    const next = [...items, value];
    if (!write(next)) return Object.freeze({ ok: false, error: "annotation-write" });
    items = next;
    lastError = null;
    return Object.freeze({ ok: true, id: value.id, sourceAnchor: value.sourceAnchor });
  }

  function updateNote(id, noteValue) {
    const index = items.findIndex((value) => value.id === id && value.deletedAt === null);
    if (index < 0) return Object.freeze({ ok: false, error: "annotation-missing" });
    const note = String(noteValue).trim();
    if (note.length > MAX_NOTE_LENGTH) {
      return Object.freeze({ ok: false, error: "annotation-note" });
    }
    const next = [...items];
    next[index] = Object.freeze({
      ...next[index],
      type: note ? "note" : "highlight",
      note,
      updatedAt: Date.now(),
    });
    if (!write(next)) return Object.freeze({ ok: false, error: "annotation-write" });
    items = next;
    lastError = null;
    return Object.freeze({ ok: true });
  }

  function replaceAnchor(id, sourceAnchor) {
    const index = items.findIndex((value) => value.id === id && value.deletedAt === null);
    if (index < 0 || !validSourceAnchor(sourceAnchor)) {
      return Object.freeze({ ok: false, error: "annotation-anchor" });
    }
    const next = [...items];
    next[index] = freezeItem({ ...next[index], sourceAnchor, updatedAt: Date.now() });
    if (!write(next)) return Object.freeze({ ok: false, error: "annotation-write" });
    items = next;
    lastError = null;
    return Object.freeze({ ok: true });
  }

  function remove(id) {
    const index = items.findIndex((value) => value.id === id && value.deletedAt === null);
    if (index < 0) return Object.freeze({ ok: false, error: "annotation-missing" });
    const deletedAt = Date.now();
    const next = [...items];
    next[index] = Object.freeze({ ...next[index], updatedAt: deletedAt, deletedAt });
    if (!write(next)) return Object.freeze({ ok: false, error: "annotation-write" });
    items = next;
    lastError = null;
    return Object.freeze({ ok: true });
  }

  async function restore() {
    if (!available) return snapshot();
    try {
      const raw = target.getItem(key);
      if (raw !== null) {
        if (raw.length > MAX_RECORD_LENGTH) throw new Error("annotation-corrupt");
        const value = JSON.parse(raw);
        if (!validRecord(value)) throw new Error("annotation-corrupt");
        const hashes = await Promise.all(
          value.items.map((item) => annotationTextHash(item.sourceAnchor.selectedText)),
        );
        if (value.items.some((item, index) => item.sourceAnchor.contentHash !== hashes[index])) {
          throw new Error("annotation-corrupt");
        }
        items = value.items.map(freezeItem);
      }
      restored = true;
    } catch {
      available = false;
      lastError = "annotation-corrupt";
    }
    return snapshot();
  }

  function clear() {
    try {
      target.removeItem(key);
      items = [];
      return true;
    } catch {
      available = false;
      lastError = "annotation-write";
      return false;
    }
  }

  function sourceAnchor(id) {
    return items.find((value) => value.id === id)?.sourceAnchor || null;
  }

  function snapshot() {
    return Object.freeze({
      available,
      durable: Boolean(storage),
      restored,
      lastError,
      writes,
      active: Object.freeze(active().map((value) => Object.freeze({ ...value }))),
      tombstones: items.filter((value) => value.deletedAt !== null).length,
    });
  }

  return Object.freeze({
    active,
    add,
    clear,
    item,
    remove,
    replaceAnchor,
    restore,
    snapshot,
    sourceAnchor,
    updateNote,
    validSourceAnchor,
  });
}
