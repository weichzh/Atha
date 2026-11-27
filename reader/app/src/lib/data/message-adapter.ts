import {
  messageClient as tauriMessageClient,
  readingMemoryClient as tauriReadingMemoryClient,
  type ConversationView,
  type EditionInput,
  type LegacyImport,
  type MessageClient,
  type MessageReferencePreview,
  type MessageSearchHit,
  type MessageView,
  type ReadingMemoryClient,
  type ReadingMemoryHit,
  type ReplyDraft,
  type ReselectDraft,
  type RevisionView,
  type RichTextInput,
  type RootMessageDraft,
  type RootMessageView,
  type SourceCaptureView,
} from "../../messages.ts";
import { buildDemoMessageSeed, type DemoMessageStoreSeed } from "./demo-seed.ts";
import { isTauriRuntime } from "./runtime.ts";

const DEMO_MSG_KEY = "atha.demo.messages.v1";

function randomHex32(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function loadStore(): DemoMessageStoreSeed {
  try {
    const raw = window.localStorage.getItem(DEMO_MSG_KEY);
    if (!raw) {
      const seed = buildDemoMessageSeed();
      window.localStorage.setItem(DEMO_MSG_KEY, JSON.stringify(seed));
      return seed;
    }
    return JSON.parse(raw) as DemoMessageStoreSeed;
  } catch {
    const seed = buildDemoMessageSeed();
    window.localStorage.setItem(DEMO_MSG_KEY, JSON.stringify(seed));
    return seed;
  }
}

function saveStore(store: DemoMessageStoreSeed): void {
  window.localStorage.setItem(DEMO_MSG_KEY, JSON.stringify(store));
}

function findMessage(store: DemoMessageStoreSeed, messageId: string): { conv: ConversationView; msg: MessageView } | null {
  for (const conv of store.conversations) {
    const msg = conv.messages.find((m) => m.id === messageId);
    if (msg) return { conv, msg };
  }
  return null;
}

function buildReferencePreviews(store: DemoMessageStoreSeed, ids: string[]): MessageReferencePreview[] {
  return ids.map((id) => {
    const found = findMessage(store, id);
    if (!found) return { id, text: "", deleted: true };
    const text = found.msg.deleted
      ? ""
      : found.msg.text || found.msg.source?.selectedText || "";
    return { id, text, deleted: found.msg.deleted };
  });
}

function toRootView(store: DemoMessageStoreSeed, conv: ConversationView): RootMessageView | null {
  const root = conv.messages[0];
  if (!root || root.deleted || !root.source) return null;
  const activeMessages = conv.messages.filter((m) => !m.deleted);
  const latest = [...activeMessages].reverse().find((m) => m.kind === "text");
  return {
    conversationId: conv.id,
    messageId: root.id,
    revisionId: root.revisionId,
    kind: root.kind === "text" ? "text" : "source-only",
    text: root.text,
    latestText: latest?.text ?? "",
    source: root.source,
    updatedAt: activeMessages.at(-1)?.updatedAt ?? root.updatedAt,
  };
}

const demoMessageClient: MessageClient = {
  async edition(contentVersion: string): Promise<EditionInput> {
    const store = loadStore();
    return (
      store.editions[contentVersion] ?? {
        contentVersion,
        title: "潮汐与纸页：数字阅读中的对话形态",
        authors: ["林澄", "Arthur Vance"],
      }
    );
  },

  async roots(editionId: string, section: string | null = null): Promise<RootMessageView[]> {
    const store = loadStore();
    return store.conversations
      .filter((conv) => conv.editionId === editionId)
      .map((conv) => toRootView(store, conv))
      .filter((root): root is RootMessageView => Boolean(root))
      .filter((root) => !section || root.source.section === section);
  },

  async conversation(conversationId: string): Promise<ConversationView> {
    const store = loadStore();
    const conv = store.conversations.find((c) => c.id === conversationId);
    if (!conv) throw new Error("conversation-missing");
    return {
      ...conv,
      messages: conv.messages.map((m) => ({
        ...m,
        referencePreviews: buildReferencePreviews(store, m.referenceIds),
      })),
    };
  },

  async conversations(editionId: string, section: string | null = null): Promise<ConversationView[]> {
    const store = loadStore();
    return store.conversations
      .filter((conv) => {
        if (conv.editionId !== editionId) return false;
        const root = conv.messages[0];
        if (!root || root.deleted) return false;
        if (section && root.source?.section !== section) return false;
        return true;
      })
      .map((conv) => ({
        ...conv,
        messages: conv.messages.map((m) => ({
          ...m,
          referencePreviews: buildReferencePreviews(store, m.referenceIds),
        })),
      }));
  },

  async createRoot(draft: RootMessageDraft) {
    const store = loadStore();
    store.editions[draft.edition.contentVersion] = draft.edition;
    const now = Date.now();
    const conversationId = randomHex32();
    const messageId = randomHex32();
    const revisionId = randomHex32();
    const sourceId = randomHex32();
    const text = (draft.text ?? "").trim();
    const kind = text ? "text" : "source-only";
    const source = {
      id: sourceId,
      originalLocator: draft.anchor.canonicalLocator,
      canonicalLocator: draft.anchor.canonicalLocator,
      section: draft.anchor.section,
      selectedText: draft.anchor.selectedText,
      prefixText: draft.anchor.prefixText,
      suffixText: draft.anchor.suffixText,
      contentHash: draft.anchor.contentHash,
    };
    const rootMsg: MessageView = {
      id: messageId,
      revisionId,
      kind,
      text,
      contentJson: text
        ? JSON.stringify({
            type: "doc",
            content: [{ type: "paragraph", content: [{ type: "text", text }] }],
          })
        : "",
      replyToMessageId: null,
      referenceIds: [],
      referencePreviews: [],
      source,
      deleted: false,
      createdAt: now,
      updatedAt: now,
    };
    store.conversations.push({
      id: conversationId,
      editionId: draft.edition.contentVersion,
      messages: [rootMsg],
    });
    store.revisions[messageId] = [
      { id: revisionId, kind, text, contentJson: rootMsg.contentJson, createdAt: now },
    ];
    store.captures[messageId] = [
      {
        source,
        snapshot: {
          ...draft.snapshot,
          resources: draft.snapshot.resources.map((r) => ({
            path: r.path,
            mediaType: r.mediaType,
            bytes: r.bytes,
            contentHash: draft.anchor.contentHash,
            byteLength: r.bytes.length,
          })),
        },
        current: true,
        createdAt: now,
      },
    ];
    saveStore(store);
    return { conversationId, messageId, revisionId };
  },

  async revise(
    messageId: string,
    expectedRevisionId: string,
    text: string | null,
    richText: RichTextInput | null = null,
  ) {
    const store = loadStore();
    const found = findMessage(store, messageId);
    if (!found || found.msg.revisionId !== expectedRevisionId) {
      throw new Error("revision-conflict");
    }
    const clean = (text ?? "").trim();
    const kind = clean ? "text" : "source-only";
    const revisionId = randomHex32();
    const now = Date.now();
    const contentJson = richText ? JSON.stringify(richText.document) : found.msg.contentJson;
    found.msg.revisionId = revisionId;
    found.msg.kind = kind;
    found.msg.text = clean;
    found.msg.contentJson = contentJson;
    found.msg.updatedAt = now;
    store.revisions[messageId] = [
      ...(store.revisions[messageId] ?? []),
      { id: revisionId, kind, text: clean, contentJson, createdAt: now },
    ];
    saveStore(store);
    return { messageId, revisionId };
  },

  async reply(draft: ReplyDraft) {
    const store = loadStore();
    const conv = store.conversations.find((c) => c.id === draft.conversationId);
    if (!conv) throw new Error("conversation-missing");
    const clean = draft.text.trim();
    if (!clean) throw new Error("empty-reply");
    const messageId = randomHex32();
    const revisionId = randomHex32();
    const now = Date.now();
    const contentJson = draft.richText
      ? JSON.stringify(draft.richText.document)
      : JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph", content: [{ type: "text", text: clean }] }],
        });
    const msg: MessageView = {
      id: messageId,
      revisionId,
      kind: "text",
      text: clean,
      contentJson,
      replyToMessageId: draft.replyToMessageId,
      referenceIds: draft.referenceIds,
      referencePreviews: buildReferencePreviews(store, draft.referenceIds),
      source: null,
      deleted: false,
      createdAt: now,
      updatedAt: now,
    };
    conv.messages.push(msg);
    store.revisions[messageId] = [
      { id: revisionId, kind: "text", text: clean, contentJson, createdAt: now },
    ];
    saveStore(store);
    return { messageId, revisionId };
  },

  async remove(messageId: string, expectedRevisionId: string): Promise<void> {
    const store = loadStore();
    const found = findMessage(store, messageId);
    if (!found || found.msg.revisionId !== expectedRevisionId) {
      throw new Error("revision-conflict");
    }
    found.msg.deleted = true;
    found.msg.kind = "deleted";
    found.msg.updatedAt = Date.now();
    saveStore(store);
  },

  async search(editionId: string, text: string, section: string | null = null): Promise<MessageSearchHit[]> {
    const store = loadStore();
    const q = text.trim().toLocaleLowerCase();
    if (!q) return [];
    const hits: MessageSearchHit[] = [];
    for (const conv of store.conversations) {
      if (conv.editionId !== editionId) continue;
      const root = conv.messages[0];
      if (!root || root.deleted || !root.source) continue;
      if (section && root.source.section !== section) continue;
      for (const msg of conv.messages) {
        if (msg.deleted) continue;
        const haystack = `${root.source.selectedText}\n${msg.text}`.toLocaleLowerCase();
        if (haystack.includes(q)) {
          hits.push({
            messageId: msg.id,
            conversationId: conv.id,
            section: root.source.section,
            selectedText: root.source.selectedText,
            text: msg.text,
          });
        }
      }
    }
    return hits;
  },

  async relationships(messageId: string) {
    const store = loadStore();
    const found = findMessage(store, messageId);
    const references = found?.msg.referenceIds ?? [];
    const referencedBy: string[] = [];
    for (const conv of store.conversations) {
      for (const msg of conv.messages) {
        if (!msg.deleted && msg.referenceIds.includes(messageId)) {
          referencedBy.push(msg.id);
        }
      }
    }
    return { references, referencedBy };
  },

  async revisions(messageId: string): Promise<RevisionView[]> {
    const store = loadStore();
    const existing = store.revisions[messageId];
    if (existing?.length) return existing;
    const found = findMessage(store, messageId);
    if (!found || found.msg.kind === "deleted") return [];
    return [
      {
        id: found.msg.revisionId,
        kind: found.msg.kind,
        text: found.msg.text,
        contentJson: found.msg.contentJson,
        createdAt: found.msg.updatedAt,
      },
    ];
  },

  async sourceCaptures(messageId: string): Promise<SourceCaptureView[]> {
    const store = loadStore();
    return store.captures[messageId] ?? [];
  },

  async snapshotResource() {
    throw new Error("resource-missing");
  },

  async reselect(draft: ReselectDraft) {
    const store = loadStore();
    const found = findMessage(store, draft.messageId);
    if (!found || !found.msg.source) throw new Error("source-missing");
    const sourceId = randomHex32();
    const newSource = {
      id: sourceId,
      originalLocator: draft.anchor.canonicalLocator,
      canonicalLocator: draft.anchor.canonicalLocator,
      section: draft.anchor.section,
      selectedText: draft.anchor.selectedText,
      prefixText: draft.anchor.prefixText,
      suffixText: draft.anchor.suffixText,
      contentHash: draft.anchor.contentHash,
    };
    found.msg.source = newSource;
    const prev = store.captures[draft.messageId] ?? [];
    store.captures[draft.messageId] = [
      ...prev.map((c) => ({ ...c, current: false })),
      {
        source: newSource,
        snapshot: {
          ...draft.snapshot,
          resources: draft.snapshot.resources.map((r) => ({
            path: r.path,
            mediaType: r.mediaType,
            bytes: r.bytes,
            contentHash: draft.anchor.contentHash,
            byteLength: r.bytes.length,
          })),
        },
        current: true,
        createdAt: Date.now(),
      },
    ];
    saveStore(store);
    return { sourceId };
  },

  async reanchor(sourceId: string, _expectedLocator: string, currentLocator: string): Promise<void> {
    const store = loadStore();
    for (const conv of store.conversations) {
      for (const msg of conv.messages) {
        if (msg.source?.id === sourceId) {
          msg.source.canonicalLocator = currentLocator;
        }
      }
    }
    saveStore(store);
  },

  async importLegacy(input: LegacyImport) {
    return { imported: 0, alreadyComplete: true, recordHash: input.recordHash };
  },

  async export(editionId: string, conversationId: string | null = null): Promise<boolean> {
    const store = loadStore();
    const convs = store.conversations.filter(
      (c) => c.editionId === editionId && (!conversationId || c.id === conversationId),
    );
    const edition = store.editions[editionId];
    const lines: string[] = [`# ${edition?.title ?? "Atha 笔记导出"}`, ""];
    for (const conv of convs) {
      const root = conv.messages[0];
      if (!root || root.deleted || !root.source) continue;
      lines.push(`> ${root.source.selectedText}`, "");
      for (const msg of conv.messages) {
        if (!msg.deleted && msg.text) lines.push(`- ${msg.text}`);
      }
      lines.push("");
    }
    const blob = new Blob([lines.join("\n")], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `atha-notes-${conversationId ? conversationId.slice(0, 8) : editionId.slice(0, 8)}.md`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return true;
  },
};

const demoReadingMemoryClient: ReadingMemoryClient = {
  async search(query: string): Promise<ReadingMemoryHit[]> {
    const store = loadStore();
    const q = query.trim().toLocaleLowerCase();
    const hits: ReadingMemoryHit[] = [];
    for (const conv of store.conversations) {
      const root = conv.messages[0];
      if (!root || root.deleted || !root.source) continue;
      const edition = store.editions[conv.editionId] ?? {
        contentVersion: conv.editionId,
        title: "未知书籍",
        authors: [],
      };
      for (const msg of conv.messages) {
        if (msg.deleted) continue;
        const haystack = `${edition.title}\n${edition.authors.join(" ")}\n${root.source.selectedText}\n${msg.text}`.toLocaleLowerCase();
        if (!q || haystack.includes(q)) {
          hits.push({
            messageId: msg.id,
            rootMessageId: root.id,
            conversationId: conv.id,
            editionId: conv.editionId,
            title: edition.title,
            authors: edition.authors,
            section: root.source.section,
            selectedText: root.source.selectedText,
            text: msg.text || "（仅高亮标注）",
            canonicalLocator: root.source.canonicalLocator,
            updatedAt: msg.updatedAt,
          });
        }
      }
    }
    return hits.sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 200);
  },

  async sourceCaptures(rootMessageId: string): Promise<SourceCaptureView[]> {
    return demoMessageClient.sourceCaptures(rootMessageId);
  },

  async snapshotResource(sourceId: string, sourcePath: string) {
    return demoMessageClient.snapshotResource(sourceId, sourcePath);
  },
};

export function getMessageClient(): MessageClient {
  return isTauriRuntime() ? tauriMessageClient : demoMessageClient;
}

export function getReadingMemoryClient(): ReadingMemoryClient {
  return isTauriRuntime() ? tauriReadingMemoryClient : demoReadingMemoryClient;
}
