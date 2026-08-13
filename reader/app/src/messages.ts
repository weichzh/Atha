import { invoke } from "@tauri-apps/api/core";

export interface EditionInput {
  contentVersion: string;
  title: string;
  authors: string[];
}

export interface SourceAnchorInput {
  canonicalLocator: string;
  section: string;
  selectedText: string;
  prefixText: string;
  suffixText: string;
  contentHash: string;
}

export interface SnapshotResourceInput {
  path: string;
  mediaType: string;
  bytes: number[];
}

export interface SourceSnapshotInput {
  fragmentHtml: string;
  readerCss: string;
  bookCss: string;
  userCss: string;
  presentationJson: string;
  resources: SnapshotResourceInput[];
}

export interface RootMessageDraft {
  edition: EditionInput;
  anchor: SourceAnchorInput;
  snapshot: SourceSnapshotInput;
  text: string | null;
}

export interface ReplyDraft {
  conversationId: string;
  replyToMessageId: string;
  text: string;
  richText: RichTextInput | null;
  referenceIds: string[];
}

export interface RichTextInput {
  schema: 1;
  document: Record<string, unknown>;
}

export interface ReselectDraft {
  messageId: string;
  expectedSourceId: string;
  anchor: SourceAnchorInput;
  snapshot: SourceSnapshotInput;
}

export interface MessageSource {
  id: string;
  originalLocator: string;
  canonicalLocator: string;
  section: string;
  selectedText: string;
  prefixText: string;
  suffixText: string;
  contentHash: string;
}

export interface MessageView {
  id: string;
  revisionId: string;
  kind: "source-only" | "text" | "deleted";
  text: string;
  contentJson: string;
  replyToMessageId: string | null;
  referenceIds: string[];
  referencePreviews: MessageReferencePreview[];
  source: MessageSource | null;
  deleted: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface MessageReferencePreview {
  id: string;
  text: string;
  deleted: boolean;
}

export interface ConversationView {
  id: string;
  editionId: string;
  messages: MessageView[];
}

export interface RootMessageView {
  conversationId: string;
  messageId: string;
  revisionId: string;
  kind: "source-only" | "text";
  text: string;
  source: MessageSource;
  updatedAt: number;
}

export interface MessageSearchHit {
  messageId: string;
  conversationId: string;
  section: string;
  selectedText: string;
  text: string;
}

export interface ReadingMemoryHit {
  messageId: string;
  rootMessageId: string;
  conversationId: string;
  editionId: string;
  title: string;
  authors: string[];
  section: string;
  selectedText: string;
  text: string;
  canonicalLocator: string;
  updatedAt: number;
}

export interface RevisionView {
  id: string;
  kind: "source-only" | "text";
  text: string;
  contentJson: string;
  createdAt: number;
}

export interface SnapshotResourceView {
  path: string;
  mediaType: string;
  contentHash: string;
  byteLength: number;
}

export interface SourceCaptureView {
  source: MessageSource;
  snapshot: SourceSnapshotInput & { resources: SnapshotResourceView[] };
  current: boolean;
  createdAt: number;
}

export interface LegacyAnnotationInput {
  id: string;
  anchor: SourceAnchorInput;
  note: string | null;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
}

export interface LegacyImport {
  edition: EditionInput;
  sourceKey: string;
  recordHash: string;
  items: LegacyAnnotationInput[];
}

export const messageClient = Object.freeze({
  edition: (contentVersion: string) =>
    invoke<EditionInput>("message_edition_context", { contentVersion }),
  roots: (editionId: string, section: string | null = null) =>
    invoke<RootMessageView[]>("message_roots", { editionId, section }),
  conversation: (conversationId: string) =>
    invoke<ConversationView>("message_conversation", { conversationId }),
  conversations: (editionId: string, section: string | null = null) =>
    invoke<ConversationView[]>("message_conversations", { editionId, section }),
  createRoot: (draft: RootMessageDraft) =>
    invoke<{ conversationId: string; messageId: string; revisionId: string }>(
      "message_create_root",
      { draft },
    ),
  revise: (
    messageId: string,
    expectedRevisionId: string,
    text: string | null,
    richText: RichTextInput | null = null,
  ) =>
    invoke<{ messageId: string; revisionId: string }>("message_revise", {
      messageId,
      expectedRevisionId,
      text,
      richText,
    }),
  reply: (draft: ReplyDraft) =>
    invoke<{ messageId: string; revisionId: string }>("message_reply", { draft }),
  remove: (messageId: string, expectedRevisionId: string) =>
    invoke<void>("message_delete", { messageId, expectedRevisionId }),
  search: (editionId: string, text: string, section: string | null = null) =>
    invoke<MessageSearchHit[]>("message_search", { search: { editionId, text, section } }),
  relationships: (messageId: string) =>
    invoke<{ references: string[]; referencedBy: string[] }>("message_relationships", {
      messageId,
    }),
  revisions: (messageId: string) =>
    invoke<RevisionView[]>("message_revisions", { messageId }),
  sourceCaptures: (messageId: string) =>
    invoke<SourceCaptureView[]>("message_source_captures", { messageId }),
  snapshotResource: (sourceId: string, sourcePath: string) =>
    invoke<{ mediaType: string; contentHash: string; bytes: number[] }>(
      "message_snapshot_resource",
      { sourceId, sourcePath },
    ),
  reselect: (draft: ReselectDraft) =>
    invoke<{ sourceId: string }>("message_reselect", { draft }),
  reanchor: (sourceId: string, expectedLocator: string, currentLocator: string) =>
    invoke<void>("message_reanchor", { sourceId, expectedLocator, currentLocator }),
  importLegacy: (input: LegacyImport) =>
    invoke<{ imported: number; alreadyComplete: boolean; recordHash: string }>(
      "message_import_legacy",
      { input },
    ),
  export: (editionId: string, conversationId: string | null = null) =>
    invoke<boolean>("message_export", { editionId, conversationId }),
});

export const readingMemoryClient = Object.freeze({
  search: (query: string) =>
    invoke<ReadingMemoryHit[]>("reading_memory_search", { query }),
  sourceCaptures: (rootMessageId: string) =>
    invoke<SourceCaptureView[]>("reading_memory_source_captures", { rootMessageId }),
  snapshotResource: (sourceId: string, sourcePath: string) =>
    invoke<{ mediaType: string; contentHash: string; bytes: number[] }>(
      "reading_memory_snapshot_resource",
      { sourceId, sourcePath },
    ),
});

export type MessageClient = typeof messageClient;
export type ReadingMemoryClient = typeof readingMemoryClient;
