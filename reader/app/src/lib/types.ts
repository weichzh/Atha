import type { RichTextInput, SourceAnchorInput, SourceSnapshotInput } from "../messages.ts";

export type AppRoute = "library" | "memory" | "settings" | "reader";
export type ReaderTheme = "system" | "light" | "paper" | "dark";
export type ReaderFontFamily = "book" | "serif" | "sans";
export type ReaderDensity = "compact" | "standard" | "comfortable";
export type ReadingMode = "paged" | "scroll";
export type PageMargin = "narrow" | "standard" | "wide";
export type ParagraphIndent = "none" | "two";
export type ParagraphSpacing = "book" | "compact" | "comfortable";

export interface StyleModule {
  id: string;
  name: string;
  group: string;
  enabled: boolean;
  css: string;
}

export interface ReaderApplicationPreferences {
  theme: ReaderTheme;
  brightness: number;
  fontSize: number;
  fontFamily: ReaderFontFamily;
  density: ReaderDensity;
}

export interface ReaderBookPreferences {
  sourceStyles: boolean;
  userStylesEnabled: boolean;
  readingMode: ReadingMode;
  pageMargin: PageMargin;
  paragraphIndent: ParagraphIndent;
  paragraphSpacing: ParagraphSpacing;
  styleModules: StyleModule[];
}

export interface ReaderEffectivePreferences
  extends ReaderApplicationPreferences,
    ReaderBookPreferences {}

export interface PreferencesSnapshot {
  application: ReaderApplicationPreferences;
  book: ReaderBookPreferences;
  effective: ReaderEffectivePreferences;
}

export interface RootNoteItem {
  id: string;
  type: "highlight" | "note";
  note: string;
  latestText: string;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  sourceAnchor: {
    schema: 1;
    canonicalLocator: string;
    selectedText: string;
    prefixText: string;
    suffixText: string;
    contentHash: string;
  };
  conversationId: string;
  revisionId: string;
  sourceId: string;
}

export interface ReaderBookmarkItem {
  id: string;
  label: string;
  locator: string;
  currentVersion: boolean;
}

export interface SearchHit {
  id: string;
  section: string;
  sectionLabel: string;
  excerpt: string;
  excerptSegments: {
    before: string;
    match: string;
    after: string;
    text: string;
  };
  locator: string;
}

export interface TocEntry {
  label: string;
  href: string;
}

export interface NotesScopeOption {
  value: "current" | "chapter" | "book";
  label: string;
}

export interface LightboxPayload {
  kind: "image" | "svg" | "table" | "code";
  title: string;
  html: string;
}

export interface ComposerSubmitPayload {
  text: string;
  richText: RichTextInput | null;
}

export interface ActiveSelectionState {
  text: string;
  rect: DOMRect;
  range: Range;
  existingRootId: string | null;
  anchorDraft?: SourceAnchorInput;
  snapshotDraft?: SourceSnapshotInput;
}
