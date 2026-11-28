import { invoke } from "@tauri-apps/api/core";
import { createBookmarks } from "../../../../web/bookmarks.mjs";
import { createContent } from "../../../../web/content.mjs";
import { createInteraction } from "../../../../web/interaction.mjs";
import { createLocator } from "../../../../web/locator.mjs";
import { createMessageStore } from "../../../../web/message-store.mjs";
import { createNavigation } from "../../../../web/navigation.mjs";
import { createPagination } from "../../../../web/pagination.mjs";
import { createReaderState, createReadingStatistics } from "../../../../web/reader-state.mjs";
import { createReadingSession } from "../../../../web/session.mjs";
import { createSearch } from "../../../../web/search.mjs";
import { annotationTextHash, createAnnotationStore } from "../../../../web/annotation-store.mjs";
import formulaPlaceholderUrl from "../../assets/formula-placeholder.svg?no-inline";
import type { ReadingMemoryTarget } from "../../library.ts";
import type { ConversationView, RichTextInput, SourceAnchorInput, SourceSnapshotInput } from "../../messages.ts";
import bookReaderCssUrl from "../../styles/book-reader.css?url";
import bookThemeCssUrl from "../../styles/book-theme.css?url";
import type PageTurnStage from "../components/PageTurnStage.svelte";
import { uiPrefs } from "../data/ui-prefs.svelte.ts";
import { docHasRichFormatting } from "../data/message-doc.ts";
import { getMessageClient } from "../data/message-adapter.ts";
import { installDemoFetchBridge, isTauriRuntime, isWebKitGtkRuntime } from "../data/runtime.ts";
import type {
  ActiveSelectionState,
  FootnotePreview,
  PreferencesSnapshot,
  ReaderBookmarkItem,
  RootNoteItem,
  SearchHit,
  TocEntry,
} from "../types.ts";
import { ensureKernelGlobals } from "./kernel-globals.ts";
import { createReaderPreferences, DEFAULT_APPLICATION_PREFS, DEFAULT_BOOK_PREFS } from "./preferences.ts";

export interface ReaderEngineCallbacks {
  getPageTurnStage?: () => Pick<PageTurnStage, "preview" | "commit" | "cancel" | "stop"> | undefined;
  wantsFacingPages?: () => boolean;
  beginVerticalSwipe?: () => ((phase: "move" | "end" | "cancel", deltaY: number) => void) | undefined;
  onToggleChrome: () => void;
  onOpenNoteThread: (rootId: string) => void;
  onOpenLightbox: (target: HTMLElement) => void;
}

interface TextSegment {
  node: Text;
  start: number;
  end: number;
}

function collectTextSegments(book: HTMLElement): { segments: TextSegment[]; fullText: string } {
  const segments: TextSegment[] = [];
  let fullText = "";
  const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const node = walker.currentNode as Text;
    const text = node.textContent || "";
    const start = fullText.length;
    fullText += text;
    segments.push({ node, start, end: fullText.length });
  }
  return { segments, fullText };
}

function domOffsetForNode(
  segments: TextSegment[],
  book: HTMLElement,
  container: Node,
  nodeOffset: number,
): number {
  if (container.nodeType === Node.TEXT_NODE) {
    const seg = segments.find((s) => s.node === container);
    if (seg) return Math.min(seg.end, seg.start + Math.max(0, nodeOffset));
  }
  const probe = document.createRange();
  probe.setStart(book, 0);
  try {
    probe.setEnd(container, nodeOffset);
  } catch {
    return 0;
  }
  let total = 0;
  for (const seg of segments) {
    if (probe.intersectsNode(seg.node)) {
      const text = seg.node.textContent || "";
      if (container === seg.node) {
        total += Math.min(text.length, nodeOffset);
        break;
      }
      const pos = container.compareDocumentPosition(seg.node);
      if (pos & Node.DOCUMENT_POSITION_PRECEDING || container.contains(seg.node)) {
        // Compute exact overlap
        const nodeRange = document.createRange();
        nodeRange.selectNodeContents(seg.node);
        if (probe.compareBoundaryPoints(Range.END_TO_END, nodeRange) >= 0) {
          total += text.length;
        }
      }
    }
  }
  return total;
}

function offsetRangeToDomRange(
  segments: TextSegment[],
  startOffset: number,
  endOffset: number,
): Range | null {
  if (segments.length === 0 || endOffset <= startOffset) return null;
  let startNode: Text | null = null;
  let startLocal = 0;
  let endNode: Text | null = null;
  let endLocal = 0;
  for (const seg of segments) {
    if (!startNode && startOffset >= seg.start && startOffset <= seg.end) {
      if (startOffset < seg.end || seg === segments.at(-1)) {
        startNode = seg.node;
        startLocal = startOffset - seg.start;
      }
    }
    if (endOffset >= seg.start && endOffset <= seg.end) {
      endNode = seg.node;
      endLocal = endOffset - seg.start;
      if (endLocal > 0 || startNode) break;
    }
  }
  if (!startNode || !endNode) return null;
  try {
    const range = document.createRange();
    range.setStart(startNode, startLocal);
    range.setEnd(endNode, endLocal);
    return range;
  } catch {
    return null;
  }
}

function resolveAnchorRange(
  book: HTMLElement,
  anchor: { canonicalLocator: string; selectedText: string; prefixText: string; suffixText: string },
  locatorApi: ReturnType<typeof createLocator>,
): Range | null {
  const { segments, fullText } = collectTextSegments(book);
  if (!fullText) return null;
  try {
    const parsed = locatorApi.inspect(anchor.canonicalLocator);
    const start = parsed.start.offset;
    const end = parsed.end?.offset ?? start + anchor.selectedText.length;
    if (start >= 0 && end <= fullText.length && fullText.slice(start, end) === anchor.selectedText) {
      return offsetRangeToDomRange(segments, start, end);
    }
  } catch {
    // Fallback to text search inside current section
  }
  const needle = anchor.selectedText;
  if (!needle) return null;
  let idx = fullText.indexOf(needle);
  while (idx >= 0) {
    const before = fullText.slice(Math.max(0, idx - 32), idx);
    const after = fullText.slice(idx + needle.length, idx + needle.length + 32);
    const prefixOk = !anchor.prefixText || before.endsWith(anchor.prefixText.slice(-12));
    const suffixOk = !anchor.suffixText || after.startsWith(anchor.suffixText.slice(0, 12));
    if (prefixOk || suffixOk) {
      return offsetRangeToDomRange(segments, idx, idx + needle.length);
    }
    idx = fullText.indexOf(needle, idx + 1);
  }
  const first = fullText.indexOf(needle);
  return first >= 0 ? offsetRangeToDomRange(segments, first, first + needle.length) : null;
}

function hasActiveModalOverlay(event?: Event): boolean {
  if (typeof document === "undefined") return false;
  if (
    document.querySelector(
      'dialog[open], [role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], .atha-lightbox',
    )
  ) {
    return true;
  }
  const target = event?.target;
  if (
    target instanceof Element &&
    target.closest("input, textarea, select, [contenteditable], .cm-editor, .ProseMirror")
  ) {
    return true;
  }
  return false;
}

const ALLOWED_TELEMETRY_ERRORS = new Set([
  "active-content",
  "active-link",
  "active-style",
  "book-load",
  "css-subresource",
  "event-handler",
  "external-resource",
  "formula-selectors",
  "image-load",
  "invalid-formula-size",
  "invalid-manifest",
  "invalid-svg",
  "invalid-xhtml",
  "layout-cut",
  "missing-book-url",
  "missing-stylesheet",
  "manifest-load",
  "network-block",
  "reader-style-load",
  "sample-boundary",
  "section-index",
  "section-load",
  "state-persistence",
  "stylesheet-load",
  "svg-event-handler",
  "svg-external-resource",
  "svg-external-style",
  "svg-load",
  "unstable-layout",
  "unsupported-resource-attribute",
  "undeclared-resource",
]);

export class ReaderEngine {
  ready = $state(false);
  firstScreenReady = $state(false);
  initialJumpLocated = $state(false);
  loading = $state(true);
  error = $state<string | null>(null);
  sessionState = $state<string>("closed");
  bookTitle = $state("");
  page = $state(0);
  pages = $state(1);
  sectionIndex = $state(0);
  sectionCount = $state(1);
  currentSectionId = $state<string>("");
  percent = $state<number | null>(0);
  chapterLabel = $state("加载中…");
  progressPosition = $state("1 / 1");
  canPrev = $state(false);
  canNext = $state(false);
  isBookmarked = $state(false);
  toc = $state<TocEntry[]>([]);
  tocIndex = $state(0);
  bookmarks = $state<ReaderBookmarkItem[]>([]);
  roots = $state<RootNoteItem[]>([]);
  conversations = $state<ConversationView[]>([]);
  searchStatus = $state<"idle" | "searching" | "canceled" | "complete" | "error">("idle");
  searchQuery = $state("");
  searchResults = $state<SearchHit[]>([]);
  searchError = $state<string | null>(null);
  stats = $state({
    todayMs: 0,
    weekMs: 0,
    bookMs: 0,
    streakDays: 0,
  });
  preferences = $state<PreferencesSnapshot>({
    application: { ...DEFAULT_APPLICATION_PREFS },
    book: { ...DEFAULT_BOOK_PREFS, styleModules: [] },
    effective: { ...DEFAULT_APPLICATION_PREFS, ...DEFAULT_BOOK_PREFS, styleModules: [] },
  });
  selection = $state<ActiveSelectionState | null>(null);
  statusToast = $state<string | null>(null);
  activeFootnote = $state.raw<FootnotePreview | null>(null);
  footnotePending = $state(false);

  private footnoteRevision = 0;

  private disposed = false;
  private cleanups: (() => void)[] = [];
  private activeSectionRanges: { rootId: string; range: Range }[] = [];
  private searchHighlightRange: Range | null = null;
  private activeRootHighlightId: string | null = null;
  private lastPointerDownPoint: { x: number; y: number } | null = null;
  private lastPointerDownInteractive = false;
  private toastTimer: ReturnType<typeof setTimeout> | null = null;
  private telemetryChain = Promise.resolve();
  private telemetryStage: "initialization" | "opening" | "content-loaded" | "layout-stable" | "layout" =
    "initialization";

  private content!: ReturnType<typeof createContent>;
  private locator!: ReturnType<typeof createLocator>;
  private session!: ReturnType<typeof createReadingSession>;
  private pagination!: ReturnType<typeof createPagination>;
  private navigation!: ReturnType<typeof createNavigation>;
  private interaction!: ReturnType<typeof createInteraction>;
  private searchEngine!: ReturnType<typeof createSearch>;
  private readerState!: ReturnType<typeof createReaderState>;
  private statistics?: ReturnType<typeof createReadingStatistics>;
  private bookmarksEngine!: ReturnType<typeof createBookmarks>;
  private messageStore!: ReturnType<typeof createMessageStore>;
  private prefController!: ReturnType<typeof createReaderPreferences>;
  private bridgeRefs!: {
    progressRange: HTMLInputElement;
    fontSizeControl: HTMLInputElement;
    previous: HTMLButtonElement;
    next: HTMLButtonElement;
    bookmarkAdd: HTMLButtonElement;
    toc: HTMLSelectElement;
    position: HTMLElement;
    chapterLabel: HTMLElement;
    progressChapter: HTMLElement;
    progressBook: HTMLElement;
    progressPosition: HTMLElement;
    bookmarkStatus: HTMLElement;
    searchForm: HTMLFormElement;
    searchQuery: HTMLInputElement;
    searchCancel: HTMLButtonElement;
    searchGo: HTMLButtonElement;
    searchResults: HTMLSelectElement;
    searchStatus: HTMLElement;
  };
  private callbacks: ReaderEngineCallbacks;
  private contentVersion: string;

  constructor(options: {
    contentVersion: string;
    bookTitle: string;
    callbacks: ReaderEngineCallbacks;
  }) {
    this.contentVersion = options.contentVersion;
    this.bookTitle = options.bookTitle;
    this.callbacks = options.callbacks;
  }

  private emitTelemetry(message: string) {
    if (!isTauriRuntime()) return;
    this.telemetryChain = this.telemetryChain
      .then(() => invoke<void>("reader_event", { message }))
      .catch(() => {
        // Non-fatal in standalone/preview or if window navigated mid-flight
      });
  }

  showToast(msg: string) {
    this.statusToast = msg;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => {
      this.statusToast = null;
    }, 2600);
  }

  private captureDomBindings(fn: () => void, guardInteraction = false) {
    const origDocAdd = document.addEventListener.bind(document);
    const origWinAdd = window.addEventListener.bind(window);
    const origSetInterval = window.setInterval.bind(window);
    const records: { target: EventTarget; type: string; listener: EventListenerOrEventListenerObject; opts?: boolean | AddEventListenerOptions }[] = [];
    const intervals: number[] = [];

    const wrapTarget = (target: EventTarget, origAdd: typeof document.addEventListener) => {
      return ((type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions) => {
        let wrapped = listener;
        if (guardInteraction && type === "keydown" && typeof listener === "function") {
          const inner = listener;
          wrapped = (event: Event) => {
            if (this.disposed || hasActiveModalOverlay(event)) return;
            inner.call(target, event);
          };
        }
        records.push({ target, type, listener: wrapped, opts: options });
        return origAdd(type, wrapped, options);
      }) as typeof document.addEventListener;
    };

    document.addEventListener = wrapTarget(document, origDocAdd);
    window.addEventListener = wrapTarget(window, origWinAdd);
    window.setInterval = ((handler: TimerHandler, timeout?: number, ...args: unknown[]) => {
      const id = origSetInterval(handler, timeout, ...args);
      intervals.push(id);
      return id;
    }) as typeof window.setInterval;

    try {
      fn();
    } finally {
      document.addEventListener = origDocAdd;
      window.addEventListener = origWinAdd;
      window.setInterval = origSetInterval;
    }

    this.cleanups.push(() => {
      for (const rec of records) {
        rec.target.removeEventListener(rec.type, rec.listener, rec.opts);
      }
      for (const id of intervals) {
        clearInterval(id);
      }
    });
  }

  private syncSnapshots() {
    if (this.disposed) return;
    const sess = this.session.snapshot();
    const pag = this.pagination.snapshot();
    const nav = this.navigation.snapshot();
    this.sessionState = sess.state;
    this.page = pag.page;
    this.pages = Math.max(1, pag.pages);
    this.sectionIndex = Math.max(0, sess.currentIndex);
    this.sectionCount = Math.max(1, sess.sections);
    this.currentSectionId = sess.currentSection ?? "";
    this.percent = this.navigation.currentPercent();
    this.chapterLabel = this.bridgeRefs.chapterLabel.textContent || this.bookTitle;
    this.progressPosition = this.bridgeRefs.progressPosition.textContent || `${this.page + 1} / ${this.pages}`;
    this.canPrev = !this.bridgeRefs.previous.disabled;
    this.canNext = !this.bridgeRefs.next.disabled;
    this.tocIndex = Number.isFinite(nav.tocIndex) && nav.tocIndex >= 0 ? nav.tocIndex : 0;
    this.bookmarksEngine?.syncCurrent();
    this.isBookmarked = this.bridgeRefs.bookmarkAdd.getAttribute("aria-pressed") === "true";
    this.bookmarks = [...(this.readerState?.snapshot().bookmarks ?? [])];
    this.stats = {
      todayMs: this.statistics?.snapshot().todayMs ?? 0,
      weekMs: this.statistics?.snapshot().weekMs ?? 0,
      bookMs: this.statistics?.snapshot().bookMs ?? 0,
      streakDays: this.statistics?.snapshot().streakDays ?? 0,
    };
  }

  private mountReaderEl: HTMLElement | null = null;

  async mount(elements: {
    frame: HTMLElement;
    reader: HTMLElement;
    page: HTMLElement;
    host: HTMLElement;
    bridgeRoot: HTMLElement;
    isBodyVisible: () => boolean;
    manifestPath: string;
    initialJump?: ReadingMemoryTarget | null;
  }) {
    ensureKernelGlobals();
    installDemoFetchBridge();

    const fail = (code: string) => {
      this.error = code;
      throw new Error(code);
    };
    const assert = (cond: unknown, code: string) => {
      if (!cond) fail(code);
    };

    const { frame, reader, page, host, bridgeRoot, isBodyVisible, manifestPath, initialJump } = elements;
    this.mountReaderEl = reader;

    // Build hidden bridge elements required by pure kernel modules
    const makeEl = <K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Record<string, string> = {}) => {
      const el = document.createElement(tag);
      for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
      return el;
    };
    const tocLabel = makeEl("label");
    const toc = makeEl("select");
    tocLabel.append(toc);
    const details = makeEl("details");
    const previous = makeEl("button", { type: "button" });
    const next = makeEl("button", { type: "button" });
    details.append(previous, next);
    const progressRange = makeEl("input", { type: "range", min: "0", max: "1000", value: "0" });
    const fontSizeControl = makeEl("input", { type: "range", min: "16", max: "40", value: "19" });
    const bookmarkAdd = makeEl("button", { type: "button", "aria-pressed": "false" });
    const position = makeEl("span");
    const chapterLabel = makeEl("span");
    const progressChapter = makeEl("span");
    const progressBook = makeEl("span");
    const progressPosition = makeEl("span");
    const bookmarkStatus = makeEl("span");
    const searchForm = makeEl("form");
    const searchQuery = makeEl("input", { type: "search", class: "reader-search-field" });
    const searchSubmit = makeEl("button", { type: "submit" });
    const searchCancel = makeEl("button", { type: "button" });
    const searchGo = makeEl("button", { type: "button" });
    const searchResults = makeEl("select");
    const searchStatus = makeEl("span");
    searchForm.append(searchQuery, searchSubmit, searchCancel, searchGo, searchResults, searchStatus);
    bridgeRoot.replaceChildren(
      tocLabel,
      details,
      progressRange,
      fontSizeControl,
      bookmarkAdd,
      position,
      chapterLabel,
      progressChapter,
      progressBook,
      progressPosition,
      bookmarkStatus,
      searchForm,
    );

    this.bridgeRefs = {
      progressRange,
      fontSizeControl,
      previous,
      next,
      bookmarkAdd,
      toc,
      position,
      chapterLabel,
      progressChapter,
      progressBook,
      progressPosition,
      bookmarkStatus,
      searchForm,
      searchQuery,
      searchCancel,
      searchGo,
      searchResults,
      searchStatus,
    };

    const params = new URLSearchParams(typeof location !== "undefined" ? location.search : "");
    if (!params.has("manifest") && !params.has("book") && !params.has("entry")) {
      params.set("manifest", manifestPath);
      params.set("version", this.contentVersion);
    }
    const bookKey = params.get("state") || this.contentVersion.slice(0, 16);

    this.locator = createLocator({ assert });
    const webKitGtk = isWebKitGtkRuntime();
    this.content = createContent({
      host,
      reader,
      // WebKitGTK ignores the img's color scheme for an embedded SVG.
      // Retire this override when the native Linux mismatch case passes.
      projectSvgColorScheme: webKitGtk,
      // WebKitGTK does not resolve @counter-style defined in the reading shadow.
      projectShadowCounters: webKitGtk,
      themeStyleSource: { href: new URL(bookThemeCssUrl, location.href).href },
      readerStyleSource: { href: new URL(bookReaderCssUrl, location.href).href },
      formulaPlaceholderUrl: new URL(formulaPlaceholderUrl, location.href).href,
      onLateLayout: (anchor: { offset: number; pageIndex: number | null; scrollTop: number | null }) => {
        if (this.disposed) return;
        void this.navigation.resize(anchor.offset, this.session.snapshot().currentIndex, anchor.pageIndex, anchor.scrollTop);
      },
    });

    this.prefController = createReaderPreferences({
      content: this.content,
      reader,
      host,
      frame,
      onSync: (snap) => {
        this.preferences = snap;
      },
    });

    const stage = () => this.callbacks.getPageTurnStage?.();
    const reducePageMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    this.pagination = createPagination({
      pageTurn: {
        effect: () => {
          const style = uiPrefs.state.pageTurnStyle;
          return stage() && (style === "cover" || style === "curl") &&
              !reducePageMotion.matches
            ? style : null;
        },
        preview: (pageStep: number, from: number, pages: number, dx: number, style: "cover" | "curl", y?: number) =>
          stage()?.preview({ frame, reader, page, book: this.content.book, pageStep }, from, pages, dx, style, y),
        commit: (pageStep: number, from: number, dir: 1 | -1, style: "cover" | "curl") =>
          stage()?.commit({ frame, reader, page, book: this.content.book, pageStep }, from, dir, style),
        cancel: () => stage()?.cancel(),
        stop: () => stage()?.stop(),
      },
      book: this.content.book,
      measureViewportHeight: () => frame.clientHeight,
      wantsFacingPages: this.callbacks.wantsFacingPages,
      reader,
      page,
      position,
      progressRange,
      previous,
      next,
      fontSizeControl,
      onPageShown: async (includeNext: boolean, beforeLayoutChange: () => unknown) => {
        return this.content.loadVisible(includeNext, beforeLayoutChange);
      },
      assert,
      fail,
    });

    this.session = createReadingSession({
      params,
      content: this.content,
      render: async () => {
        await this.content.renderCached();
        await this.pagination.renderFromStart();
      },
      onState: (nextState: string) => {
        this.sessionState = nextState;
        if (nextState === "opening" && this.footnotePending) this.dismissFootnote();
        if (nextState === "content-loaded") {
          this.telemetryStage = "content-loaded";
        } else if (nextState === "layout-stable") {
          this.telemetryStage = "layout-stable";
          this.content.warmRemaining();
          this.statistics?.setStable(true);
          this.syncHighlights();
        } else {
          this.statistics?.setStable(false);
        }
      },
      assert,
      fail,
    });

    this.navigation = createNavigation({
      session: this.session,
      pagination: this.pagination,
      locator: this.locator,
      preferences: this.prefController,
      canResizeViewport: () => !this.disposed && isBodyVisible(),
      toc,
      chapterLabel,
      progressChapter,
      progressBook,
      progressPosition,
      toolbarChapter: null,
      toolbarProgress: null,
      progressRange,
      previous,
      next,
      fontSizeControl,
      onFallback: (reason: string) => {
        if (reason) this.showToast("已定位至最接近的阅读位置");
      },
      onPreferences: (scope: "application" | "book") => {
        return this.readerState.savePreferences(scope);
      },
      onStable: (active: boolean, reachedEnd: boolean, movedAt: number | null) => {
        if (this.firstScreenReady) {
          (this.readerState?.scheduleProgress as
            | ((a: boolean, b: boolean, c: number | null) => void)
            | undefined)?.(active, reachedEnd, movedAt);
        }
        this.syncSnapshots();
      },
      assert,
      fail,
    });

    const handleCenterTap = () => {
      if (this.lastPointerDownInteractive) return;
      const hitRootId = this.findHighlightAtPoint(this.lastPointerDownPoint);
      if (hitRootId) {
        const root = this.roots.find((r) => r.id === hitRootId);
        const match = this.activeSectionRanges.find((item) => item.rootId === hitRootId);
        if (root && match) {
          this.selection = {
            text: root.sourceAnchor.selectedText,
            rect: match.range.getBoundingClientRect(),
            range: match.range,
            existingRootId: root.id,
          };
          return;
        }
      }
      this.callbacks.onToggleChrome();
    };

    this.interaction = createInteraction({
      reader,
      content: this.content,
      navigation: this.navigation,
      pagination: this.pagination,
      onCenter: handleCenterTap,
      beginVerticalSwipe: this.callbacks.beginVerticalSwipe,
      assert,
      fail,
    });

    this.searchEngine = createSearch({
      session: this.session,
      navigation: this.navigation,
      pagination: this.pagination,
      locator: this.locator,
      controls: {
        form: searchForm,
        query: searchQuery,
        cancel: searchCancel,
        go: searchGo,
        results: searchResults,
        resultList: null,
        status: searchStatus,
      },
      assert,
      emit: () => {},
      onNavigate: () => {
        this.syncSnapshots();
      },
    });

    this.readerState = createReaderState({
      storage: typeof window !== "undefined" ? window.localStorage : null,
      keyPrefix: "atha.reader",
      bookKey,
      session: this.session,
      navigation: this.navigation,
      pagination: this.pagination,
      preferences: this.prefController,
      locator: this.locator,
      assert,
      recordShelfReading: true,
    });

    this.bookmarksEngine = createBookmarks({
      state: this.readerState,
      navigation: this.navigation,
      pagination: this.pagination,
      session: this.session,
      locator: this.locator,
      controls: {
        add: bookmarkAdd,
        list: toc,
        status: bookmarkStatus,
      },
      assert,
    });

    const legacyAnnotations = createAnnotationStore({
      storage: typeof window !== "undefined" ? window.localStorage : null,
      requireDurable: false,
      keyPrefix: "atha.reader",
      bookKey,
      locator: this.locator,
    });

    this.messageStore = createMessageStore({
      client: getMessageClient(),
      legacy: legacyAnnotations,
      keyPrefix: "atha.reader",
      bookKey,
      session: this.session,
      content: this.content,
      preferences: this.prefController,
      locator: this.locator,
    });

    // Pointer + Selection + Lightbox tracking inside book content
    const onBookPointerDown = (event: PointerEvent) => {
      this.lastPointerDownPoint = { x: event.clientX, y: event.clientY };
      const target = event.target;
      this.lastPointerDownInteractive =
        target instanceof Element &&
        Boolean(target.closest("img[role='button'], table, pre, svg"));
    };
    const onBookPointerUp = () => {
      requestAnimationFrame(() => this.refreshSelectionState());
    };
    const onBookClick = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const anchor = target.closest("a[href]") as HTMLAnchorElement | null;
      if (anchor) {
        event.preventDefault();
        const info = this.content.describeLink(anchor.getAttribute("href") || "");
        if (info.kind === "internal" && this.isFootnoteReference(anchor, new URL(info.href).hash)) {
          void this.previewFootnote(anchor, info.href, info.sameSection);
          return;
        }
        if (this.footnotePending) this.dismissFootnote();
        if (info.kind === "internal") {
          void this.navigation.goToHref(info.href).then(() => this.syncSnapshots());
        } else {
          this.showToast(`外部链接已安全拦截：${info.href}`);
        }
        return;
      }
      const interactive = target.closest("img[role='button'], table, pre, svg");
      if (interactive instanceof HTMLElement || interactive instanceof SVGElement) {
        if (!this.content.selectionRange()) {
          this.callbacks.onOpenLightbox(interactive as unknown as HTMLElement);
        }
      }
    };
    const onSelectionChange = () => {
      if (this.disposed) return;
      requestAnimationFrame(() => this.refreshSelectionState());
    };

    this.content.book.addEventListener("pointerdown", onBookPointerDown);
    this.content.book.addEventListener("pointerup", onBookPointerUp);
    this.content.book.addEventListener("click", onBookClick);
    document.addEventListener("selectionchange", onSelectionChange);
    this.cleanups.push(() => {
      this.content.book.removeEventListener("pointerdown", onBookPointerDown);
      this.content.book.removeEventListener("pointerup", onBookPointerUp);
      this.content.book.removeEventListener("click", onBookClick);
      document.removeEventListener("selectionchange", onSelectionChange);
    });

    try {
      await this.content.initialize();
      if (this.disposed) return;
      this.pagination.initialize();
      this.prefController.reset("application");
      this.prefController.reset("book");
      this.telemetryStage = "opening";
      const opened = await this.session.open(0);
      if (!opened || this.disposed) return;

      const description = this.session.describe();
      this.contentVersion = description.contentVersion;
      this.bookTitle = description.title || this.bookTitle;
      this.toc = [...description.toc];

      this.statistics = createReadingStatistics({
        storage: typeof window !== "undefined" ? window.localStorage : null,
        keyPrefix: "atha.reader",
        contentVersion: this.contentVersion,
      });

      this.captureDomBindings(() => this.navigation.bindControls());
      this.cleanups.push(this.pagination.bindResize(() => this.navigation.resize()));
      this.captureDomBindings(() => this.interaction.bind(), true);
      this.captureDomBindings(() => this.readerState.bind());
      this.captureDomBindings(() => this.statistics!.bind());
      this.searchEngine.bind();

      await this.readerState.restore(() => this.disposed);
      if (this.disposed) return;
      if (!initialJump) {
        this.readerState.activateShelfReading();
        this.statistics.setStable(true);
        this.syncSnapshots();
        this.firstScreenReady = true;
      }
      await this.bookmarksEngine.bind();
      if (this.disposed) return;
      await this.messageStore.restore();
      if (this.disposed) return;
      await this.refreshMessages();
      if (this.disposed) return;

      if (initialJump) {
        const root = initialJump.editionId === this.contentVersion
          ? this.roots.find((candidate) =>
              candidate.id === initialJump.rootMessageId &&
              candidate.conversationId === initialJump.conversationId)
          : null;
        this.initialJumpLocated = Boolean(root && await this.navigation.goTo(root.sourceAnchor.canonicalLocator));
        if (this.disposed) return;
        this.readerState.activateShelfReading();
        this.statistics.setStable(true);
        if (!this.initialJumpLocated) this.showToast("未找到原文位置，可返回阅读记忆查看历史快照");
      }

      this.readerState.scheduleProgress(false, false, null);
      this.readerState.flushProgress();

      this.syncHighlights();
      this.syncSnapshots();
      this.firstScreenReady = true;
      this.ready = true;
      this.loading = false;
      document.documentElement.dataset.status = "pass";
      delete document.documentElement.dataset.error;
      document.documentElement.setAttribute("data-reader-ready", "");
      this.emitTelemetry(`ready|${Math.min(10_000, Math.max(1, this.pages))}|0|0|0`);
    } catch (err) {
      if (this.disposed) return;
      const rawCode = err instanceof Error ? err.message : "book-load";
      const safeCode = ALLOWED_TELEMETRY_ERRORS.has(rawCode) ? rawCode : "book-load";
      this.error = rawCode;
      this.loading = false;
      document.documentElement.dataset.status = "fail";
      document.documentElement.dataset.error = rawCode;
      document.documentElement.setAttribute("data-reader-ready", "");
      this.emitTelemetry(`error|${safeCode}|${this.telemetryStage}`);
    }
  }

  private findHighlightAtPoint(point: { x: number; y: number } | null): string | null {
    if (!point) return null;
    for (const item of this.activeSectionRanges) {
      for (const rect of item.range.getClientRects()) {
        if (
          point.x >= rect.left - 4 &&
          point.x <= rect.right + 4 &&
          point.y >= rect.top - 4 &&
          point.y <= rect.bottom + 4
        ) {
          return item.rootId;
        }
      }
    }
    return null;
  }

  refreshSelectionState() {
    if (this.disposed || !this.content) return;
    const range = this.content.selectionRange();
    if (!range) {
      if (this.selection && !this.selection.existingRootId) {
        this.selection = null;
      }
      return;
    }
    const text = range.toString().trim();
    if (!text) {
      this.selection = null;
      return;
    }
    const rect = range.getBoundingClientRect();
    const fallbackRect = range.getClientRects()[0] ?? rect;
    const finalRect = rect.width > 0 && rect.height > 0 ? rect : fallbackRect;
    this.selection = {
      text,
      rect: finalRect,
      range,
      existingRootId: null,
    };
  }

  clearSelection() {
    this.selection = null;
    this.content?.clearSelection();
  }

  async refreshMessages() {
    if (this.disposed) return;
    await this.messageStore.reload();
    this.roots = [...this.messageStore.active()];
    const client = getMessageClient();
    const convs = await client.conversations(this.session.describe().contentVersion, null);
    this.conversations = convs;
    this.syncHighlights();
  }

  syncHighlights() {
    if (typeof CSS === "undefined" || !("highlights" in CSS) || !this.content || !this.session) return;
    let currentSection = "";
    try {
      currentSection = this.session.snapshot().currentSection ?? "";
    } catch {
      return;
    }
    if (!currentSection) return;

    const noteRanges: Range[] = [];
    const highlightRanges: Range[] = [];
    const activeRanges: Range[] = [];
    const nextSectionRanges: { rootId: string; range: Range }[] = [];

    for (const root of this.roots) {
      try {
        const parsed = this.locator.inspect(root.sourceAnchor.canonicalLocator);
        if (parsed.start.section !== currentSection) continue;
        const range = resolveAnchorRange(this.content.book, root.sourceAnchor, this.locator);
        if (!range) continue;
        nextSectionRanges.push({ rootId: root.id, range });
        if (root.id === this.activeRootHighlightId) {
          activeRanges.push(range);
        } else if (root.type === "note") {
          noteRanges.push(range);
        } else {
          highlightRanges.push(range);
        }
      } catch {
        // Skip invalid locators safely
      }
    }

    this.activeSectionRanges = nextSectionRanges;
    CSS.highlights.set("atha-annotations", new Highlight(...noteRanges));
    CSS.highlights.set("atha-highlights", new Highlight(...highlightRanges));
    CSS.highlights.set("atha-active-annotation", new Highlight(...activeRanges));
    if (this.searchHighlightRange) {
      CSS.highlights.set("atha-search", new Highlight(this.searchHighlightRange));
    } else {
      CSS.highlights.delete("atha-search");
    }
  }

  setActiveRootHighlight(rootId: string | null) {
    this.activeRootHighlightId = rootId;
    this.syncHighlights();
  }

  private extractPlainTextFromMarkup(fragmentHtml: string): string {
    if (!fragmentHtml) return "";
    const tpl = document.createElement("template");
    tpl.innerHTML = fragmentHtml;
    return tpl.content.textContent ?? "";
  }


  private async buildAnchorFromRange(range: Range, fragmentHtml?: string): Promise<SourceAnchorInput> {
    const description = this.session.describe();
    const sectionId = this.session.snapshot().currentSection || description.sections[0].id;
    const { segments, fullText } = collectTextSegments(this.content.book);
    let startOffset = domOffsetForNode(segments, this.content.book, range.startContainer, range.startOffset);
    let endOffset = domOffsetForNode(segments, this.content.book, range.endContainer, range.endOffset);
    if (endOffset <= startOffset) {
      const rawText = range.toString().trim();
      const idx = fullText.indexOf(rawText);
      if (idx >= 0) {
        startOffset = idx;
        endOffset = idx + rawText.length;
      }
    }
    const domSelectedText = fullText.slice(startOffset, endOffset);
    const markupText = fragmentHtml ? this.extractPlainTextFromMarkup(fragmentHtml) : "";
    const selectedText = markupText.length > 0 ? markupText : domSelectedText;
    const prefixText = fullText.slice(Math.max(0, startOffset - 32), startOffset);
    const suffixText = fullText.slice(endOffset, Math.min(fullText.length, endOffset + 32));
    const canonicalLocator = this.locator.serialize(
      description,
      this.locator.range(
        description,
        { section: sectionId, offset: startOffset },
        { section: sectionId, offset: endOffset },
      ),
    );
    const contentHash = await annotationTextHash(selectedText);
    return {
      canonicalLocator,
      section: sectionId,
      selectedText,
      prefixText,
      suffixText,
      contentHash,
    };
  }

  async captureSelectionDraft(): Promise<{
    text: string;
    anchor: SourceAnchorInput;
    snapshot: SourceSnapshotInput;
  } | null> {
    const sel = this.selection;
    if (!sel) return null;
    try {
      const snapshot = (await this.content.captureRange(
        sel.range,
        this.prefController.snapshot().effective,
      )) as SourceSnapshotInput;
      const anchor = await this.buildAnchorFromRange(sel.range, snapshot?.fragmentHtml);
      if (!anchor.selectedText.trim()) return null;
      return { text: anchor.selectedText, anchor, snapshot };
    } catch {
      return null;
    }
  }

  async createHighlightFromSelection(
    noteText = "",
    richText: RichTextInput | null = null,
    draft?: { anchor: SourceAnchorInput; snapshot: SourceSnapshotInput } | null,
  ): Promise<string | null> {
    try {
      let anchor: SourceAnchorInput;
      let snapshot: SourceSnapshotInput;
      let range: Range | null = null;
      if (draft) {
        anchor = draft.anchor;
        snapshot = draft.snapshot;
      } else {
        const sel = this.selection;
        if (!sel) return null;
        range = sel.range;
        snapshot = (await this.content.captureRange(
          sel.range,
          this.prefController.snapshot().effective,
        )) as SourceSnapshotInput;
        anchor = await this.buildAnchorFromRange(sel.range, snapshot?.fragmentHtml);
      }
      if (!anchor.selectedText.trim()) {
        this.showToast("请先选择有效正文文字");
        return null;
      }
      const normalizedNote = noteText.trim();
      const result = await this.messageStore.add(
        anchor, normalizedNote, range, snapshot,
        richText && docHasRichFormatting(richText.document) ? richText : null,
      );
      if (result.ok && "id" in result && result.id) {
        try {
          await this.refreshMessages();
        } catch {
          this.clearSelection();
          this.showToast("已保存，重新打开笔记可查看");
          return result.id;
        }
        this.clearSelection();
        this.showToast(normalizedNote || richText ? "已创建对话标注" : "已高亮原文");
        return result.id;
      }
    } catch {
      // Fall through to error toast
    }
    this.showToast("标注保存失败");
    return null;
  }

  dismissFootnote() {
    this.footnoteRevision += 1;
    this.footnotePending = false;
    this.activeFootnote = null;
  }

  async jumpToActiveFootnote() {
    const current = this.activeFootnote;
    this.dismissFootnote();
    if (this.disposed || !current) return;
    await this.navigation.goToHref(current.href);
    this.syncSnapshots();
  }

  private isFootnoteReference(anchor: HTMLAnchorElement, hash: string): boolean {
    const semanticType = anchor.getAttributeNS("http://www.idpf.org/2007/ops", "type")
      ?? anchor.getAttribute("epub:type") ?? anchor.getAttribute("type") ?? "";
    if (semanticType.split(/\s+/).includes("noteref") ||
      (anchor.getAttribute("role") ?? "").split(/\s+/).includes("doc-noteref") ||
      anchor.matches(".noteref, .footnote-ref, .fnref")) return true;

    // Recognize conventional note fragments without treating ordinary numeric links as notes.
    return /^#(?:fn|footnote|endnote|rearnote)(?:[-_.]|\d|$)/i.test(hash)
      && /^(?:\[\d+\]|\(\d+\)|注\s*\d+|[*※]+|\d{1,3})$/.test(anchor.textContent.trim());
  }

  private async previewFootnote(anchor: HTMLAnchorElement, href: string, sameSection: boolean) {
    const ticket = ++this.footnoteRevision;
    this.footnotePending = true;
    const sourcePage = this.pagination.snapshot().page;
    let body: ParentNode | null = null;
    let fragment = "";
    try {
      const target = new URL(href);
      fragment = decodeURIComponent(target.hash.slice(1));
      target.hash = "";
      if (fragment) {
        body = sameSection ? this.content.book : await this.session.previewSectionBody(target.href);
      }
    } catch {
      // Unavailable/invalid note targets never turn a page or activate a section.
    }
    if (this.disposed || ticket !== this.footnoteRevision) return;
    this.footnotePending = false;
    if (!anchor.isConnected || this.pagination.snapshot().page !== sourcePage || hasActiveModalOverlay()) return;
    const target = fragment ? body?.querySelector(`#${CSS.escape(fragment)}`) : null;
    if (!target) {
      this.showToast("未找到可预览的脚注");
      return;
    }
    this.activeFootnote = {
      label: anchor.textContent.trim(),
      text: this.footnoteText(target),
      href,
      anchorRect: anchor.getBoundingClientRect(),
    };
  }

  private footnoteText(target: Element): string {
    const content = target.cloneNode(true) as Element;
    for (const node of content.querySelectorAll("script, style, svg, img, [role~='doc-backlink']")) node.remove();
    for (const link of content.querySelectorAll("a")) {
      if (/^[↩↑^↖回返\s]*$/u.test(link.textContent) ||
        (link.getAttribute("epub:type") ?? "").split(/\s+/).includes("backlink")) link.remove();
    }
    for (const block of content.querySelectorAll("p, li, br")) block.after("\n");
    return content.textContent.trim();
  }

  async reselectHighlight(rootId: string): Promise<boolean> {
    const sel = this.selection;
    if (!sel) return false;
    const anchor = await this.buildAnchorFromRange(sel.range);
    const result = await this.messageStore.replaceAnchor(rootId, anchor, sel.range);
    if (result.ok) {
      await this.refreshMessages();
      this.clearSelection();
      this.showToast("已更新原文选区与快照");
      return true;
    }
    return false;
  }

  async deleteRootNote(rootId: string): Promise<boolean> {
    const result = await this.messageStore.remove(rootId);
    if (result.ok) {
      await this.refreshMessages();
      if (this.selection?.existingRootId === rootId) this.selection = null;
      this.showToast("已删除标注与对话");
      return true;
    }
    return false;
  }

  async jumpToLocator(canonicalLocator: string): Promise<boolean> {
    const ok = await this.navigation.goTo(canonicalLocator);
    this.syncSnapshots();
    if (ok) {
      try {
        const parsed = this.locator.inspect(canonicalLocator);
        const { segments } = collectTextSegments(this.content.book);
        const endOffset = parsed.end?.offset ?? parsed.start.offset + 1;
        this.searchHighlightRange = offsetRangeToDomRange(segments, parsed.start.offset, endOffset);
        this.syncHighlights();
      } catch {
        this.searchHighlightRange = null;
      }
    }
    return ok;
  }

  async previousPage() {
    await this.navigation.previous();
    this.syncSnapshots();
  }

  async nextPage() {
    await this.navigation.next();
    this.syncSnapshots();
  }

  async goToToc(index: number) {
    await this.navigation.goToToc(index);
    this.syncSnapshots();
  }

  async goToAdjacentSection(direction: -1 | 1) {
    const index = this.session.snapshot().currentIndex + direction;
    const target = this.session.describe().sections[index];
    if (!target) return;
    await this.navigation.goToHref(target.url);
    this.syncSnapshots();
  }

  async goToProgressPercent(pct: number) {
    const val = Math.max(0, Math.min(1000, Math.round((pct / 100) * 1000)));
    this.bridgeRefs.progressRange.value = String(val);
    this.bridgeRefs.progressRange.dispatchEvent(new Event("change", { bubbles: true }));
    await this.navigation.idle();
    this.syncSnapshots();
  }

  async toggleBookmark() {
    this.bridgeRefs.bookmarkAdd.click();
    await this.bookmarksEngine.idle();
    this.syncSnapshots();
    const status = this.bridgeRefs.bookmarkStatus.textContent;
    if (status) this.showToast(status);
  }

  async jumpToBookmark(id: string) {
    await this.bookmarksEngine.go(id);
    this.syncSnapshots();
  }

  async removeBookmarkById(id: string) {
    this.readerState.removeBookmark(id);
    this.bookmarksEngine.syncCurrent();
    this.syncSnapshots();
  }

  async runBookSearch(query: string) {
    this.searchQuery = query;
    const snap = await this.searchEngine.search(query);
    this.searchStatus = snap.status as typeof this.searchStatus;
    if ("results" in snap) {
      this.searchResults = [...snap.results];
      this.searchError = snap.lastError ?? null;
    } else {
      this.searchResults = [];
      this.searchError = null;
    }
  }

  async jumpToSearchHit(hit: SearchHit) {
    await this.jumpToLocator(hit.locator);
  }

  clearSearch() {
    this.searchEngine.cancel();
    this.searchStatus = "idle";
    this.searchQuery = "";
    this.searchResults = [];
    this.searchError = null;
    this.searchHighlightRange = null;
    this.syncHighlights();
  }

  async updatePreferences(scope: "application" | "book", patch: Record<string, unknown>) {
    await this.navigation.setPreferences(scope, patch);
    this.syncSnapshots();
  }

  async resetPreferences(scope: "application" | "book") {
    await this.navigation.resetPreferences(scope);
    this.syncSnapshots();
  }

  async resizeLayout() {
    if (this.disposed || !this.ready) return;
    await this.navigation.resize();
  }

  validateCss(css: string): { valid: boolean; error: string | null } {
    try {
      this.content.validateStylesheet(css);
      return { valid: true, error: null };
    } catch (err) {
      const code = err instanceof Error ? err.message : "invalid-user-style";
      const map: Record<string, string> = {
        "css-subresource": "禁止在用户样式中使用 @import、url() 或反斜杠转义。",
        "active-style": "禁止使用 :host、::part 或 ::slotted 等穿透选择器。",
        "invalid-user-style": "CSS 语法有误或超出 64KB 大小限制。",
      };
      return { valid: false, error: map[code] ?? "样式校验失败" };
    }
  }

  getEditionId(): string {
    return this.contentVersion;
  }

  flushOnExit() {
    if (this.disposed) return;
    try {
      if (this.firstScreenReady && this.mountReaderEl?.isConnected) this.readerState?.flushOnExit();
      else this.readerState?.flushProgress();
      this.statistics?.flush();
    } catch {
      // Storage failure must not block exiting
    }
  }

  destroy() {
    if (this.disposed) return;
    this.disposed = true;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    try {
      // The leaving route already sampled its live viewport.
      this.readerState?.flushProgress();
      this.statistics?.flush();
      this.session?.close();
    } catch {
      // Ignore cleanup errors
    }
    if (typeof CSS !== "undefined" && "highlights" in CSS) {
      CSS.highlights.delete("atha-annotations");
      CSS.highlights.delete("atha-highlights");
      CSS.highlights.delete("atha-active-annotation");
      CSS.highlights.delete("atha-search");
    }
    for (const cleanup of this.cleanups) cleanup();
    this.cleanups = [];
  }
}
