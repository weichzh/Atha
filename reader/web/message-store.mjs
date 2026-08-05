async function messageRecordHash(value) {
  const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(bytes)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export function createMessageStore({
  client,
  legacy,
  keyPrefix,
  bookKey,
  session,
  content,
  preferences,
  locator,
}) {
  let items = [];
  let lastError = null;
  let writes = 0;
  let edition;

  const currentEdition = async () => {
    edition ||= await client.edition(session.describe().contentVersion);
    return edition;
  };

  function freezeItem(root) {
    const sourceAnchor = Object.freeze({
      schema: 1,
      canonicalLocator: root.source.canonicalLocator,
      selectedText: root.source.selectedText,
      prefixText: root.source.prefixText,
      suffixText: root.source.suffixText,
      contentHash: root.source.contentHash,
    });
    return Object.freeze({
      id: root.messageId,
      type: root.kind === "text" ? "note" : "highlight",
      note: root.text,
      createdAt: root.updatedAt,
      updatedAt: root.updatedAt,
      deletedAt: null,
      sourceAnchor,
      conversationId: root.conversationId,
      revisionId: root.revisionId,
      sourceId: root.source.id,
    });
  }

  async function reload() {
    items = (await client.roots(session.describe().contentVersion)).map(freezeItem);
    lastError = null;
    return items;
  }

  async function migrateLegacy() {
    await legacy.restore();
    const legacyItems = legacy.all();
    const serialized = JSON.stringify({ schema: 1, items: legacyItems });
    await client.importLegacy({
      edition: await currentEdition(),
      sourceKey: `${keyPrefix}.annotations.${bookKey}.v1`,
      recordHash: await messageRecordHash(serialized),
      items: legacyItems.map((item) => ({
        id: item.id,
        anchor: {
          ...item.sourceAnchor,
          section: locator.inspect(item.sourceAnchor.canonicalLocator).start.section,
        },
        note: item.note || null,
        createdAt: item.createdAt,
        updatedAt: item.updatedAt,
        deletedAt: item.deletedAt,
      })),
    });
  }

  async function restore() {
    try {
      await migrateLegacy();
      await reload();
    } catch {
      lastError = "annotation-write";
    }
    return snapshot();
  }

  function active() {
    return [...items];
  }

  function item(id) {
    return items.find((value) => value.id === id) || null;
  }

  async function capture(range) {
    return content.captureRange(range, preferences.snapshot().effective);
  }

  async function add(sourceAnchor, noteValue, range) {
    try {
      const created = await client.createRoot({
        edition: await currentEdition(),
        anchor: {
          ...sourceAnchor,
          section: locator.inspect(sourceAnchor.canonicalLocator).start.section,
        },
        snapshot: await capture(range),
        text: String(noteValue).trim() || null,
      });
      writes += 1;
      await reload();
      return Object.freeze({
        ok: true,
        id: created.messageId,
        conversationId: created.conversationId,
        sourceAnchor,
      });
    } catch {
      lastError = "annotation-write";
      return Object.freeze({ ok: false, error: lastError });
    }
  }

  async function updateNote(id, noteValue) {
    const value = item(id);
    if (!value) return Object.freeze({ ok: false, error: "annotation-missing" });
    try {
      await client.revise(id, value.revisionId, String(noteValue).trim() || null);
      writes += 1;
      await reload();
      return Object.freeze({ ok: true });
    } catch {
      lastError = "annotation-write";
      return Object.freeze({ ok: false, error: lastError });
    }
  }

  async function reanchor(id, sourceAnchor) {
    const value = item(id);
    if (!value) return Object.freeze({ ok: false, error: "annotation-missing" });
    try {
      await client.reanchor(
        value.sourceId,
        value.sourceAnchor.canonicalLocator,
        sourceAnchor.canonicalLocator,
      );
      items = items.map((candidate) =>
        candidate.id === id
          ? Object.freeze({ ...candidate, sourceAnchor: Object.freeze({ ...sourceAnchor }) })
          : candidate,
      );
      writes += 1;
      return Object.freeze({ ok: true });
    } catch {
      lastError = "annotation-write";
      return Object.freeze({ ok: false, error: lastError });
    }
  }

  async function replaceAnchor(id, sourceAnchor, range) {
    const value = item(id);
    if (!value) return Object.freeze({ ok: false, error: "annotation-missing" });
    try {
      await client.reselect({
        messageId: id,
        expectedSourceId: value.sourceId,
        anchor: {
          ...sourceAnchor,
          section: locator.inspect(sourceAnchor.canonicalLocator).start.section,
        },
        snapshot: await capture(range),
      });
      writes += 1;
      await reload();
      return Object.freeze({ ok: true });
    } catch {
      lastError = "annotation-write";
      return Object.freeze({ ok: false, error: lastError });
    }
  }

  async function remove(id) {
    const value = item(id);
    if (!value) return Object.freeze({ ok: false, error: "annotation-missing" });
    try {
      await client.remove(id, value.revisionId);
      writes += 1;
      await reload();
      return Object.freeze({ ok: true });
    } catch {
      lastError = "annotation-write";
      return Object.freeze({ ok: false, error: lastError });
    }
  }

  function snapshot() {
    return Object.freeze({
      available: true,
      durable: true,
      restored: !lastError,
      lastError,
      writes,
      active: Object.freeze(active()),
      tombstones: 0,
    });
  }

  function compareSources(left, right) {
    const description = session.describe();
    return locator.compare(
      description,
      locator.inspect(left.canonicalLocator),
      locator.inspect(right.canonicalLocator),
    );
  }

  return Object.freeze({
    active,
    add,
    all: active,
    clear: () => false,
    item,
    reanchor,
    reload,
    remove,
    replaceAnchor,
    restore,
    snapshot,
    sourceAnchor: (id) => item(id)?.sourceAnchor || null,
    updateNote,
    validSourceAnchor: legacy.validSourceAnchor,
    conversation: client.conversation,
    conversations: client.conversations,
    compareSources,
    export: client.export,
    relationships: client.relationships,
    reply: client.reply,
    revise: async (...arguments_) => {
      const result = await client.revise(...arguments_);
      await reload();
      return result;
    },
    deleteMessage: async (...arguments_) => {
      const result = await client.remove(...arguments_);
      await reload();
      return result;
    },
    revisions: client.revisions,
    search: client.search,
    sourceCaptures: client.sourceCaptures,
    snapshotResource: client.snapshotResource,
  });
}
