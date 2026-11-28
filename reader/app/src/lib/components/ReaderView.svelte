<script lang="ts">
  import {
    ArrowLeft,
    Bookmark,
    ChevronLeft,
    ChevronRight,
    ListTree,
    MessageSquare,
    Search,
    Trash2,
    Type,
    X,
  } from "@lucide/svelte";
  import { Dialog, Slider } from "bits-ui";
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { fade, fly, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { DictionaryFontScale, DictionaryLookup } from "../../dictionary.ts";
  import type { LibraryBook, ReadingMemoryTarget } from "../../library.ts";
  import {
    getDictionaryPreferences,
    listOfflineDictionaries,
    lookupOfflineWord,
  } from "../data/library-adapter.ts";
  import { registerBackHandler, type BackSource } from "../data/back-stack.ts";
  import type { OpeningPaperMeasure } from "../data/open-transition.ts";
  import { resolveEffectiveTheme } from "../engine/preferences.ts";
  import { setNativeSelectionSuppressed, syncSystemBarsMode } from "../data/system-bars.ts";
  import { ReaderEngine } from "../engine/reader-engine.svelte.ts";
  import type { LightboxPayload } from "../types.ts";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import FootnotePopover from "./FootnotePopover.svelte";
  import DictionaryCard from "./DictionaryCard.svelte";
  import NotesWorkspace from "./NotesWorkspace.svelte";
  import ReaderDirectoryToc from "./ReaderDirectoryToc.svelte";
  import ReaderSettingsDialog from "./ReaderSettingsDialog.svelte";
  import SelectionPill from "./SelectionPill.svelte";
  import StructuredLightbox from "./StructuredLightbox.svelte";

  interface Props {
    book: LibraryBook;
    manifestPath: string;
    initialJump?: ReadingMemoryTarget | null;
    onBackToShelf: () => void;
    entryBlocked: boolean;
    openingTheme: "light" | "paper" | "dark";
    onScreenState: (state: "ready" | "error", measure?: OpeningPaperMeasure) => void;
  }

  let { book, manifestPath, initialJump = null, onBackToShelf, entryBlocked, openingTheme, onScreenState }: Props = $props();

  let frameEl = $state<HTMLDivElement | null>(null);
  let readerEl = $state<HTMLDivElement | null>(null);
  let pageEl = $state<HTMLDivElement | null>(null);
  let hostEl = $state<HTMLDivElement | null>(null);
  let bridgeEl = $state<HTMLDivElement | null>(null);

  let viewportWidth = $state(typeof window !== "undefined" ? window.innerWidth : 1440);
  let viewportHeight = $state(typeof window !== "undefined" ? window.innerHeight : 900);

  let chromeOpen = $state(false);
  let lastOverlayClosedAt = 0;
  let settingsOpen = $state(false);
  let directoryOpen = $state(false);
  let directoryTab = $state<"toc" | "bookmarks" | "search">("toc");
  let bookSearchInput = $state("");

  // Notes Workspace C1 state
  let notesMode = $state<"closed" | "list" | "thread">("closed");
  let selectedRootId = $state<string | null>(null);
  let focusThread = $state(false);
  let pendingDraft = $state<Awaited<ReturnType<ReaderEngine["captureSelectionDraft"]>>>(null);

  // Scrubber live preview
  let scrubbingValue = $state<number | null>(null);

  // Dictionary Card
  let dictOpen = $state(false);
  let dictQuery = $state("");
  let dictLookup = $state<DictionaryLookup | null>(null);
  let dictLoading = $state(false);
  let dictScale = $state<DictionaryFontScale>(getDictionaryPreferences().fontScale);

  // Lightbox
  let lightboxPayload = $state<LightboxPayload | null>(null);
  let footnoteDialog = $state<FootnotePopover>();

  // Highlight delete confirm
  let pendingDeleteRootId = $state<string | null>(null);

  const engine = new ReaderEngine({
    contentVersion: untrack(() => book.id),
    bookTitle: untrack(() => book.title),
    callbacks: {
      onToggleChrome: () => {
        chromeOpen = !chromeOpen;
      },
      onOpenNoteThread: (rootId: string) => {
        selectedRootId = rootId;
        notesMode = "thread";
      },
      onOpenLightbox: (target: HTMLElement) => {
        const tag = target.localName.toLowerCase();
        if (tag === "table") {
          lightboxPayload = { kind: "table", title: "表格预览", html: target.outerHTML };
        } else if (tag === "pre") {
          lightboxPayload = { kind: "code", title: "代码块预览", html: target.outerHTML };
        } else if (tag === "svg") {
          lightboxPayload = {
            kind: "svg",
            title: target.getAttribute("aria-label") || "矢量插图",
            html: target.outerHTML,
          };
        } else if (tag === "img") {
          const safeSrc = target.getAttribute("src") || "";
          const alt = (target.getAttribute("alt") || "插图").replace(/[<>&"]/g, "");
          lightboxPayload = {
            kind: "image",
            title: alt,
            html: `<img src="${safeSrc.replace(/"/g, "&quot;")}" alt="${alt}" />`,
          };
        }
      },
    },
  });

  const systemTheme = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  let systemDark = $state(systemTheme?.matches ?? false);
  const resolvedTheme = $derived(!engine.firstScreenReady
    ? openingTheme
    : engine.preferences.effective.theme === "system"
      ? systemDark ? "dark" : "light"
      : resolveEffectiveTheme(engine.preferences.effective.theme));
  let readerShell: HTMLElement;

  $effect(() => {
    const bodyReady = !entryBlocked && engine.firstScreenReady && !engine.error &&
      !settingsOpen && !directoryOpen && !dictOpen && !engine.activeFootnote && !lightboxPayload &&
      !pendingDeleteRootId;
    syncSystemBarsMode(bodyReady && !chromeOpen && notesMode === "closed", resolvedTheme === "dark");
    // A stored highlight is an Atha affordance, not an active WebView text selection.
    setNativeSelectionSuppressed(
      bodyReady && !hideReaderA && !pendingDraft && Boolean(engine.selection && !engine.selection.existingRootId),
    );
  });

  const isCompact = $derived(viewportWidth < 800 || viewportHeight > viewportWidth);
  const isMediumLandscape = $derived(!isCompact && viewportWidth < 1440);
  const isWideLandscape = $derived(!isCompact && viewportWidth >= 1440);

  const hideReaderA = $derived(
    (isMediumLandscape && notesMode === "thread") ||
      (isWideLandscape && notesMode === "thread" && focusThread),
  );

  const displayPercent = $derived(scrubbingValue ?? engine.percent ?? 0);

  $effect(() => {
    const screen = engine.error ? "error" : engine.firstScreenReady ? "ready" : null;
    if (screen) untrack(() => onScreenState(screen, screen === "ready" ? measureOpeningPaper() : undefined));
  });

  function measureOpeningPaper(): OpeningPaperMeasure | undefined {
    if (!entryBlocked || !frameEl || !readerEl || !pageEl) return;
    const frame = frameEl.getBoundingClientRect();
    const page = pageEl.getBoundingClientRect();
    const deviceScale = readerEl.getBoundingClientRect().width / readerEl.offsetWidth;
    const padding = getComputedStyle(readerEl);
    // The paper is the reading column and its own margins, not the whole desktop window.
    const left = Math.max(frame.left, page.left - parseFloat(padding.paddingLeft) * deviceScale);
    const right = Math.min(frame.right, page.right + parseFloat(padding.paddingRight) * deviceScale);
    if (right > left) return { left, width: right - left };
  }

  $effect(() => {
    if (entryBlocked || (!engine.firstScreenReady && !engine.error)) return;
    // Don't steal focus if the reader already moved into an interactive control.
    if (document.activeElement === document.body) readerShell?.focus({ preventScroll: true });
  });

  // Re-measure A whenever C1 panes alter A's width
  let prevWorkspaceSig = "";
  $effect(() => {
    const sig = `${notesMode}:${focusThread}:${hideReaderA}:${viewportWidth}:${viewportHeight}`;
    if (!engine.ready || sig === prevWorkspaceSig) return;
    prevWorkspaceSig = sig;
    if (!hideReaderA) {
      void tick().then(() => {
        requestAnimationFrame(() => {
          void engine.resizeLayout();
        });
      });
    }
  });

  onMount(() => {
    const unregisterBack = registerBackHandler(handleBackAction);
    const onSystemThemeChange = (event: MediaQueryListEvent) => { systemDark = event.matches; };
    systemTheme?.addEventListener("change", onSystemThemeChange);
    const onResize = () => {
      viewportWidth = window.innerWidth;
      viewportHeight = window.innerHeight;
    };
    const onPageHide = () => {
      engine.flushOnExit();
    };
    window.__athaFlushOnExit = () => engine.flushOnExit();
    window.addEventListener("resize", onResize);
    const onOutsideBody = (event: Event) => {
      if (!hostEl || !event.composedPath().includes(hostEl)) setNativeSelectionSuppressed(false);
    };
    // An input or another surface may start its native menu before selectionchange arrives.
    if (window.AthaSystemBars) {
      window.addEventListener("pointerdown", onOutsideBody, true);
      window.addEventListener("focusin", onOutsideBody, true);
    }
    window.addEventListener("pagehide", onPageHide);

    void (async () => {
      if (frameEl && readerEl && pageEl && hostEl && bridgeEl) {
        await engine.mount({
          frame: frameEl,
          reader: readerEl,
          page: pageEl,
          host: hostEl,
          bridgeRoot: bridgeEl,
          manifestPath,
          initialJump,
        });

        if (initialJump && engine.initialJumpLocated) {
          selectedRootId = initialJump.rootMessageId;
          notesMode = "thread";
        }
      }
    })();

    return () => {
      unregisterBack();
      systemTheme?.removeEventListener("change", onSystemThemeChange);
      if (window.__athaFlushOnExit) {
        delete window.__athaFlushOnExit;
      }
      window.removeEventListener("resize", onResize);
      window.removeEventListener("pointerdown", onOutsideBody, true);
      window.removeEventListener("focusin", onOutsideBody, true);
      setNativeSelectionSuppressed(false);
      window.removeEventListener("pagehide", onPageHide);
    };
  });

  onDestroy(() => {
    engine.destroy();
  });

  function formatShortDuration(ms: number): string {
    if (ms <= 0) return "0分";
    const mins = Math.max(1, Math.round(ms / 60000));
    if (mins < 60) return `${mins}分`;
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m ? `${h}时${m}分` : `${h}时`;
  }

  async function handleLookupSelection() {
    const sel = engine.selection;
    if (!sel) return;
    const text = sel.text.slice(0, 64);
    dictQuery = text;
    dictOpen = true;
    dictLoading = true;
    try {
      const dicts = await listOfflineDictionaries();
      const prefs = getDictionaryPreferences();
      const dictId = prefs.dictionaryId || dicts[0]?.id || "";
      dictLookup = await lookupOfflineWord(dictId, text);
    } finally {
      dictLoading = false;
    }
  }

  async function handleHighlightOnly() {
    await engine.createHighlightFromSelection("");
  }

  async function handleDiscussSelection() {
    const sel = engine.selection;
    if (!sel) return;
    if (sel.existingRootId) {
      pendingDraft = null;
      selectedRootId = sel.existingRootId;
      notesMode = "thread";
      engine.selection = null;
      return;
    }
    const draft = await engine.captureSelectionDraft();
    if (!draft) {
      engine.showToast("未能保留原文选区，请重试");
      return;
    }
    if (engine.selection !== sel) return;
    pendingDraft = draft;
    engine.clearSelection();
    selectedRootId = null;
    notesMode = "thread";
  }

  async function handleCopySelection() {
    const sel = engine.selection;
    if (!sel) return;
    try {
      await navigator.clipboard?.writeText(sel.text);
    } catch {
      // Clipboard may be unavailable in restricted contexts
    }
    engine.clearSelection();
  }

  function openNotesList() {
    chromeOpen = false;
    if (notesMode === "closed") {
      notesMode = "list";
    } else {
      notesMode = "closed";
      focusThread = false;
      pendingDraft = null;
    }
  }

  function openDirectory(tab: "toc" | "bookmarks" | "search") {
    directoryTab = tab;
    directoryOpen = true;
    chromeOpen = false;
  }

  function handleBackAction(source: BackSource): boolean {
    if (entryBlocked) return false;
    if (engine.activeFootnote && footnoteDialog) {
      // Escape uses the native dialog cancellation; the Android bridge calls close directly.
      if (source === "escape") return false;
      footnoteDialog.close();
      return true;
    }
    if (engine.footnotePending) {
      engine.dismissFootnote();
      return true;
    }
    if (Date.now() - lastOverlayClosedAt < 120) return true;
    if (lightboxPayload) {
      lightboxPayload = null;
    } else if (pendingDeleteRootId) {
      pendingDeleteRootId = null;
    } else if (settingsOpen) {
      settingsOpen = false;
      lastOverlayClosedAt = Date.now();
    } else if (directoryOpen) {
      directoryOpen = false;
      lastOverlayClosedAt = Date.now();
    } else if (dictOpen) {
      dictOpen = false;
    } else if (engine.selection) {
      engine.clearSelection();
    } else if (notesMode !== "closed") {
      if (notesMode === "thread" && isCompact) {
        notesMode = "list";
        pendingDraft = null;
      } else {
        notesMode = "closed";
        focusThread = false;
        pendingDraft = null;
      }
    } else if (chromeOpen) {
      chromeOpen = false;
    } else if (source === "back" || (!engine.firstScreenReady && !engine.error)) {
      onBackToShelf();
    } else {
      return false;
    }
    return true;
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (entryBlocked || !engine.firstScreenReady || engine.activeFootnote || event.defaultPrevented) return;
    const target = event.target as Element | null;
    const isEditing = Boolean(
      target?.closest("input, textarea, select, [contenteditable], .ProseMirror, .cm-editor"),
    );

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      openDirectory("search");
      return;
    }

    if (!isEditing && !event.ctrlKey && !event.metaKey && !event.altKey) {
      if (event.key.toLowerCase() === "h") {
        event.preventDefault();
        chromeOpen = !chromeOpen;
      } else if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        openNotesList();
      }
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<main
  class="reader-shell"
  bind:this={readerShell}
  tabindex="-1"
  aria-label="分页阅读器"
  aria-busy={!engine.firstScreenReady && !engine.error ? "true" : "false"}
>
<div
  class="reader-root"
  data-reader-theme={resolvedTheme}
  data-app-theme={resolvedTheme === "dark" ? "dark" : "light"}
  data-opening-theme={resolvedTheme}
  data-screen-ready={engine.firstScreenReady}
>
  <div class="reader-workspace-layout">
    <!-- Region A: Reading Surface -->
    <section class="region-a" class:hidden-a={hideReaderA} aria-label="正文阅读区">
      <div class="reader-frame" bind:this={frameEl}>
        <div class="reader" bind:this={readerEl}>
          <div class="page" bind:this={pageEl}>
            <div class="reader-content-host" bind:this={hostEl}></div>
          </div>
        </div>
      </div>

      <div class="kernel-bridge" bind:this={bridgeEl} hidden aria-hidden="true"></div>

      {#if engine.error}
        <div class="error-overlay" role="alert">
          <div class="error-card glass-popover">
            <h2>无法渲染本书章节</h2>
            <p>错误代码：{engine.error}</p>
            <button id="error-back" type="button" class="accent-btn" onclick={onBackToShelf}>返回书架</button>
          </div>
        </div>
      {/if}

      {#if engine.statusToast && !settingsOpen && !directoryOpen && !lightboxPayload}
        <div
          class="reader-toast"
          class:above-chrome={chromeOpen}
          role="status"
          transition:fade={{ duration: 150 }}
        >
          {engine.statusToast}
        </div>
      {/if}

      <!-- Top & Bottom Floating Frosted Glass Islands -->
      <div
        class="reader-controls"
        style={chromeOpen ? "visibility: visible; opacity: 1;" : "visibility: hidden; opacity: 0; pointer-events: none;"}
      >
      {#if chromeOpen}
        <header
          class="top-island glass-island"
          transition:fly={{ y: -18, duration: 220, easing: cubicOut }}
        >
          <div class="island-left">
            <button
              id="reader-back"
              type="button"
              class="island-btn back-btn"
              aria-label="返回书架"
              onclick={onBackToShelf}
            >
              <ArrowLeft size={17} />
              <span class="btn-text">书架</span>
            </button>
          </div>

          <div class="island-title">
            <span class="island-chapter">{engine.chapterLabel}</span>
            <span class="island-book">{engine.bookTitle || book.title}</span>
          </div>

          <div class="island-right">
            <button
              type="button"
              class="island-icon-btn"
              class:active-tint={engine.isBookmarked}
              aria-label={engine.isBookmarked ? "取消当前页书签" : "添加当前页书签"}
              aria-pressed={engine.isBookmarked}
              onclick={() => void engine.toggleBookmark()}
            >
              <Bookmark size={17} fill={engine.isBookmarked ? "currentColor" : "none"} />
            </button>
            <button
              type="button"
              class="island-icon-btn"
              aria-label="目录与全书搜索"
              onclick={() => openDirectory("toc")}
            >
              <ListTree size={17} />
            </button>
            <button
              type="button"
              class="island-icon-btn notes-btn"
              class:active-tint={notesMode !== "closed"}
              aria-label="打开笔记与对话"
              onclick={openNotesList}
            >
              <MessageSquare size={17} />
              {#if engine.roots.length > 0}
                <span class="notes-badge tabular">{engine.roots.length}</span>
              {/if}
            </button>
            <button
              type="button"
              class="island-icon-btn"
              aria-label="排版与主题设置"
              onclick={() => {
                settingsOpen = true;
              }}
            >
              <Type size={17} />
            </button>
          </div>
        </header>

        <!-- Bottom Floating Frosted Glass Island -->
        <footer
          class="bottom-island glass-island"
          transition:fly={{ y: 20, duration: 220, easing: cubicOut }}
        >
          <div class="scrub-row">
            <button
              type="button"
              class="island-icon-btn"
              aria-label="上一页"
              disabled={!engine.canPrev}
              onclick={() => void engine.previousPage()}
            >
              <ChevronLeft size={18} />
            </button>

            <div class="scrubber-wrap">
              {#if scrubbingValue !== null}
                <div class="scrub-callout glass-popover tabular">
                  {engine.chapterLabel} · {scrubbingValue}%
                </div>
              {/if}
              <Slider.Root
                type="single"
                min={0}
                max={100}
                step={1}
                value={displayPercent}
                onValueChange={(v) => (scrubbingValue = v)}
                onValueCommit={(v) => {
                  scrubbingValue = null;
                  void engine.goToProgressPercent(v);
                }}
                class="atha-slider"
                aria-label="全书阅读进度"
              >
                <span class="atha-slider-track">
                  <Slider.Range class="atha-slider-range" />
                </span>
                <Slider.Thumb index={0} class="atha-slider-thumb" />
              </Slider.Root>
            </div>

            <button
              type="button"
              class="island-icon-btn"
              aria-label="下一页"
              disabled={!engine.canNext}
              onclick={() => void engine.nextPage()}
            >
              <ChevronRight size={18} />
            </button>
          </div>

          <div class="stats-strip tabular">
            <span>{engine.progressPosition}</span>
            <span class="dot-sep" aria-hidden="true">·</span>
            <span>约 {engine.percent ?? 0}%</span>
            <span class="dot-sep" aria-hidden="true">·</span>
            <span>今日 {formatShortDuration(engine.stats.todayMs)}</span>
            <span class="dot-sep hide-xs" aria-hidden="true">·</span>
            <span class="hide-xs">本书 {formatShortDuration(engine.stats.bookMs)}</span>
            <span class="dot-sep hide-xs" aria-hidden="true">·</span>
            <span class="hide-xs">连续 {engine.stats.streakDays} 天</span>
          </div>
        </footer>
      {/if}
      </div>
    </section>

    <!-- Regions B & C: C1 Conversation Workspace -->
    <NotesWorkspace
      {engine}
      mode={notesMode}
      {selectedRootId}
      {pendingDraft}
      {viewportWidth}
      {viewportHeight}
      {focusThread}
      onSelectRoot={(id) => {
        pendingDraft = null;
        selectedRootId = id;
        notesMode = "thread";
      }}
      onBackToList={() => {
        pendingDraft = null;
        focusThread = false;
        notesMode = "list";
      }}
      onClose={() => {
        pendingDraft = null;
        focusThread = false;
        notesMode = "closed";
      }}
      onToggleFocusThread={() => {
        focusThread = !focusThread;
      }}
      onCancelDraft={() => {
        pendingDraft = null;
        engine.clearSelection();
        if (engine.roots.length > 0 && !selectedRootId) {
          selectedRootId = engine.roots[0].id;
        } else if (engine.roots.length === 0) {
          notesMode = "list";
        }
      }}
    />
  </div>

  <!-- Floating Selection Capsule -->
  {#if engine.selection && !pendingDraft}
    <SelectionPill
      selection={engine.selection}
      onHighlight={() => void handleHighlightOnly()}
      onDiscuss={() => void handleDiscussSelection()}
      onLookup={() => void handleLookupSelection()}
      onCopy={() => void handleCopySelection()}
      onReselect={engine.selection.existingRootId
        ? () => {
            engine.clearSelection();
            engine.refreshMessages();
          }
        : undefined}
      onDelete={engine.selection.existingRootId
        ? () => {
            pendingDeleteRootId = engine.selection?.existingRootId ?? null;
          }
        : undefined}
    />
  {/if}

  <!-- Offline Dictionary Card -->
  {#if dictOpen}
    <DictionaryCard
      query={dictQuery}
      lookup={dictLookup}
      loading={dictLoading}
      fontScale={dictScale}
      onScaleChange={(s) => (dictScale = s)}
      onClose={() => (dictOpen = false)}
    />
  {/if}

  {#if engine.activeFootnote}
    <FootnotePopover
      bind:this={footnoteDialog}
      footnote={engine.activeFootnote}
      onClose={() => engine.dismissFootnote()}
      onJump={() => void engine.jumpToActiveFootnote()}
    />
  {/if}

  <!-- Lightbox for SVG/Tables/Code/Images -->
  <StructuredLightbox payload={lightboxPayload} onClose={() => (lightboxPayload = null)} />

  <!-- C2 Reader Settings Dialog -->
  <ReaderSettingsDialog
    bind:open={settingsOpen}
    onClosed={() => (lastOverlayClosedAt = Date.now())}
    {engine}
    bookTitle={book.title}
  />

  <!-- Directory / Bookmarks / Search Dialog -->
  <Dialog.Root bind:open={directoryOpen}>
    <Dialog.Portal>
      <Dialog.Overlay forceMount>
        {#snippet child({ props, open: isOpen })}
          {#if isOpen}
            <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 150 }}></div>
          {/if}
        {/snippet}
      </Dialog.Overlay>
      <Dialog.Content forceMount>
        {#snippet child({ props, open: isOpen })}
          {#if isOpen}
            <div
              {...props}
              class="atha-dialog-content wide directory-dialog"
              aria-label="目录、书签与全书搜索"
              transition:scale={{ start: 0.96, duration: 190, easing: cubicOut }}
              onpointerdown={(e) => e.stopPropagation()}
            >
              <div class="dir-head">
                <div class="dir-tabs" role="tablist">
                  <button
                    type="button"
                    role="tab"
                    aria-selected={directoryTab === "toc"}
                    class="dir-tab"
                    class:active={directoryTab === "toc"}
                    onclick={() => (directoryTab = "toc")}
                  >
                    目录
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={directoryTab === "bookmarks"}
                    class="dir-tab"
                    class:active={directoryTab === "bookmarks"}
                    onclick={() => (directoryTab = "bookmarks")}
                  >
                    书签 ({engine.bookmarks.length})
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={directoryTab === "search"}
                    class="dir-tab"
                    class:active={directoryTab === "search"}
                    onclick={() => (directoryTab = "search")}
                  >
                    全书搜索
                  </button>
                </div>
                <button
                  type="button"
                  class="icon-btn"
                  aria-label="关闭目录"
                  onclick={() => (directoryOpen = false)}
                >
                  <X size={18} />
                </button>
              </div>

              {#if directoryTab === "toc"}
                {#if engine.toc.length === 0}
                  <div class="dir-empty">本书未提供章节目录。</div>
                {:else}
                  <ReaderDirectoryToc
                    entries={engine.toc}
                    activeIndex={engine.tocIndex}
                    onSelect={(index) => {
                      void engine.goToToc(index);
                      directoryOpen = false;
                    }}
                  />
                {/if}
              {:else if directoryTab === "bookmarks"}
                <div class="dir-list">
                  <div class="bm-toolbar">
                    <button
                      type="button"
                      class="pill-btn"
                      onclick={() => void engine.toggleBookmark()}
                    >
                      <Bookmark size={14} />
                      <span>{engine.isBookmarked ? "取消当前页书签" : "添加当前页书签"}</span>
                    </button>
                  </div>
                  {#if engine.bookmarks.length === 0}
                    <div class="dir-empty">尚未添加任何书签。</div>
                  {:else}
                    {#each engine.bookmarks as bm (bm.id)}
                      <div class="bm-row">
                        <button
                          type="button"
                          class="bm-jump"
                          onclick={() => {
                            void engine.jumpToBookmark(bm.id);
                            directoryOpen = false;
                          }}
                        >
                          <Bookmark size={14} />
                          <span>{bm.label}</span>
                        </button>
                        <button
                          type="button"
                          class="icon-btn"
                          aria-label="移除书签"
                          onclick={() => void engine.removeBookmarkById(bm.id)}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    {/each}
                  {/if}
                </div>
              {:else}
                <div class="search-pane">
                  <form
                    class="search-form-row"
                    onsubmit={(e) => {
                      e.preventDefault();
                      void engine.runBookSearch(bookSearchInput);
                    }}
                  >
                    <div class="well-input dir-search-input">
                      <Search size={15} />
                      <input
                        type="search"
                        aria-label="搜索本书正文"
                        placeholder="输入关键词回车检索全书…"
                        bind:value={bookSearchInput}
                      />
                    </div>
                    <button type="submit" class="accent-btn">搜索</button>
                  </form>

                  <div class="dir-list">
                    {#if engine.searchStatus === "searching"}
                      <div class="dir-empty">正在检索全书章节…</div>
                    {:else if engine.searchStatus === "error"}
                      <div class="dir-empty">{engine.searchError ?? "搜索失败"}</div>
                    {:else if engine.searchStatus === "complete" && engine.searchResults.length === 0}
                      <div class="dir-empty">未找到匹配的正文内容。</div>
                    {:else}
                      {#each engine.searchResults as hit (hit.id)}
                        <button
                          type="button"
                          class="search-hit-row"
                          onclick={() => {
                            void engine.jumpToSearchHit(hit);
                            directoryOpen = false;
                          }}
                        >
                          <div class="hit-chapter">{hit.sectionLabel}</div>
                          <div class="hit-excerpt">
                            {hit.excerptSegments.before}<mark>{hit.excerptSegments.match}</mark>{hit.excerptSegments.after}
                          </div>
                        </button>
                      {/each}
                    {/if}
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        {/snippet}
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  <ConfirmDialog
    open={Boolean(pendingDeleteRootId)}
    onCancel={() => (pendingDeleteRootId = null)}
    title="删除这段原文标注与对话？"
    description="将隐藏该段高亮及关联对话消息（历史快照记录仍以墓碑形式安全保留）。"
    confirmLabel="确认删除"
    destructive
    onConfirm={async () => {
      if (pendingDeleteRootId) {
        await engine.deleteRootNote(pendingDeleteRootId);
        pendingDeleteRootId = null;
      }
    }}
  />
</div>
</main>

<style>
  .reader-shell {
    width: 100%;
    height: 100vh;
    height: 100dvh;
    overflow: hidden;
  }

  .reader-controls {
    position: absolute;
    inset: 0;
    z-index: 35;
    pointer-events: none;
  }

  .reader-controls :global(.top-island),
  .reader-controls :global(.bottom-island) {
    pointer-events: auto;
  }

  .reader-root {
    width: 100%;
    height: 100vh;
    height: 100dvh;
    overflow: hidden;
    background: var(--opening-canvas);
    color: var(--ink-primary);
  }

  .reader-root[data-reader-theme="dark"] {
    --bg-canvas: #141413;
    --bg-surface: #1d1c19;
    --bg-elevated: #25231f;
    --bg-well: rgba(255, 252, 246, 0.07);
    --bg-well-hover: rgba(255, 252, 246, 0.11);
    --bg-well-active: rgba(255, 252, 246, 0.16);
    --bg-pill-active: #2e2b27;
    --bg-glass: rgba(28, 27, 24, 0.84);
    --bg-glass-strong: rgba(31, 29, 26, 0.94);
    --bg-scrim: rgba(0, 0, 0, 0.54);
    --ink-primary: #ece8df;
    --ink-secondary: #b5aea2;
    --ink-muted: #9e978b;
    --accent: #d9775f;
    --accent-hover: #e28872;
    --accent-ink: #171412;
    --accent-soft: rgba(217, 119, 95, 0.18);
    --accent-soft-hover: rgba(217, 119, 95, 0.26);
    --quote-tint: #23201b;
    --shadow-island: 0 8px 24px rgba(0, 0, 0, 0.36), inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    --shadow-popover: 0 12px 36px rgba(0, 0, 0, 0.48), inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  }

  .reader-workspace-layout {
    display: flex;
    width: 100%;
    height: 100%;
    overflow: hidden;
    padding: var(--safe-area-top) var(--safe-area-right) var(--safe-area-bottom) var(--safe-area-left);
  }

  .region-a {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
  }

  .region-a.hidden-a {
    display: none;
  }

  .error-overlay {
    position: absolute;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: var(--bg-scrim);
  }

  .error-card {
    padding: 26px;
    border-radius: var(--radius-xl);
    max-width: 380px;
    text-align: center;
  }

  .reader-toast {
    position: absolute;
    bottom: 24px;
    left: 50%;
    translate: -50% 0;
    z-index: 55;
    padding: 8px 18px;
    border-radius: var(--radius-pill);
    background: var(--bg-selection-pill);
    color: var(--ink-selection-pill);
    box-shadow: var(--shadow-island);
    font-size: 12.5px;
    font-weight: 500;
    pointer-events: none;
  }

  .reader-toast.above-chrome {
    bottom: 118px;
  }

  /* Floating Top & Bottom Islands */
  .top-island {
    position: absolute;
    top: 16px;
    left: 20px;
    right: 20px;
    z-index: 35;
    margin: 0 auto;
    max-width: 960px;
    min-height: 54px;
    padding: 6px 10px 6px 8px;
    border-radius: var(--radius-pill);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .island-left,
  .island-right {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }

  .island-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 14px;
    border-radius: var(--radius-pill);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink-primary);
    transition: background-color 160ms var(--ease-smooth);
  }

  .island-btn:hover {
    background: var(--bg-well);
  }

  .island-icon-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 40px;
    min-height: 40px;
    border-radius: var(--radius-pill);
    color: var(--ink-secondary);
    transition: background-color 160ms var(--ease-smooth), color 160ms var(--ease-smooth);
  }

  .island-icon-btn:hover:not(:disabled) {
    background: var(--bg-well);
    color: var(--ink-primary);
  }

  .island-icon-btn.active-tint {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .notes-badge {
    position: absolute;
    top: 4px;
    right: 3px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 10px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .island-title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  .island-chapter {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .island-book {
    font-size: 11px;
    color: var(--ink-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .bottom-island {
    position: absolute;
    bottom: 18px;
    left: 20px;
    right: 20px;
    z-index: 35;
    margin: 0 auto;
    max-width: 720px;
    padding: 10px 16px 11px;
    border-radius: var(--radius-xl);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .scrub-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .scrubber-wrap {
    position: relative;
    flex: 1;
  }

  .scrub-callout {
    position: absolute;
    bottom: calc(100% + 12px);
    left: 50%;
    translate: -50% 0;
    padding: 6px 14px;
    border-radius: var(--radius-pill);
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
  }

  .stats-strip {
    display: flex;
    justify-content: center;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 11.5px;
    color: var(--ink-muted);
  }

  .dot-sep {
    opacity: 0.45;
  }

  /* Directory Dialog */
  .directory-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
  }

  .dir-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
  }

  .dir-tabs {
    display: inline-flex;
    padding: 4px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    gap: 4px;
  }

  .dir-tab {
    height: 34px;
    padding: 0 14px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-secondary);
  }

  .dir-tab.active {
    background: var(--bg-pill-active);
    color: var(--ink-primary);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .dir-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 52vh;
    overflow-y: auto;
  }

  .bm-toolbar {
    margin-bottom: 10px;
  }

  .bm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: var(--radius-md);
  }

  .bm-row:hover {
    background: var(--bg-well);
  }

  .bm-jump {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    text-align: left;
    font-size: 14px;
  }

  .search-form-row {
    display: flex;
    gap: 10px;
    margin-bottom: 14px;
  }

  .dir-search-input {
    flex: 1;
    height: 40px;
    padding: 0 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dir-search-input input {
    width: 100%;
    font-size: 14px;
  }

  .search-hit-row {
    width: 100%;
    text-align: left;
    padding: 12px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    margin-bottom: 6px;
  }

  .search-hit-row:hover {
    background: var(--bg-well-hover);
  }

  .hit-chapter {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--accent);
    margin-bottom: 4px;
  }

  .hit-excerpt {
    font-size: 13.5px;
    line-height: 1.55;
  }

  .hit-excerpt :global(mark) {
    background: var(--accent-soft);
    color: var(--accent);
    border-radius: 4px;
    padding: 0 2px;
  }

  .dir-empty {
    padding: 36px 16px;
    text-align: center;
    color: var(--ink-muted);
    font-size: 13.5px;
  }

  @media (max-width: 640px) {
    .top-island {
      left: 10px;
      right: 10px;
      top: 10px;
    }

    .bottom-island {
      left: 10px;
      right: 10px;
      bottom: 12px;
    }

    .btn-text,
    .hide-xs {
      display: none;
    }

    .island-btn {
      padding: 0 10px;
      min-width: 44px;
      min-height: 44px;
      justify-content: center;
    }

    .island-icon-btn {
      min-width: 44px;
      min-height: 44px;
    }
  }
</style>
