<script lang="ts">
  import { ArrowUpRight, History, Search, Sparkles, X } from "@lucide/svelte";
  import { Dialog } from "bits-ui";
  import { untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { LibraryBook, ReadingMemoryJump } from "../../library.ts";
  import type { ReadingMemoryHit, SourceCaptureView } from "../../messages.ts";
  import { readShelfReading } from "../../shelf-reading.ts";
  import { indexShelf, recentShelf } from "../../shelf-browse.ts";
  import { openFailureMessage } from "../../library.ts";
  import { listLibraryBooks } from "../data/library-adapter.ts";
  import { getReadingMemoryClient } from "../data/message-adapter.ts";
  import BookCover from "./BookCover.svelte";
  import SnapshotPreview from "./SnapshotPreview.svelte";

  interface Props {
    active: boolean;
    launchBlocked: boolean;
    onOpenReaderJump: (book: LibraryBook, jump: ReadingMemoryJump) => Promise<void>;
  }

  let { active, launchBlocked, onOpenReaderJump }: Props = $props();

  let query = $state("");
  let hits = $state<ReadingMemoryHit[]>([]);
  let books = $state<LibraryBook[]>([]);
  let loading = $state(true);
  let resultQuery = $state<string | null>(null);
  let loadError = $state(false);

  let jumpError = $state("");
  let snapshotModalOpen = $state(false);
  let snapshotLoading = $state(false);
  let activeHit = $state<ReadingMemoryHit | null>(null);
  let activeCaptures = $state<SourceCaptureView[]>([]);

  const readingMap = $derived(readShelfReading(books));
  const recentBooks = $derived(recentShelf(indexShelf(books, readingMap)));
  const bookMap = $derived(new Map(books.map((b) => [b.id, b])));

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let requestRevision = 0;
  let shelfRequest: Promise<LibraryBook[]> | null = null;

  async function refresh(term: string, revision: number) {
    const shelf = shelfRequest ??= listLibraryBooks();
    try {
      const [nextBooks, nextHits] = await Promise.all([shelf, getReadingMemoryClient().search(term)]);
      if (revision !== requestRevision) return;
      books = nextBooks;
      hits = nextHits;
      resultQuery = term;
    } catch {
      if (revision !== requestRevision) return;
      shelfRequest = null;
      loadError = true;
    } finally {
      if (revision === requestRevision) loading = false;
    }
  }

  function reload() {
    clearTimeout(searchTimer);
    const revision = ++requestRevision;
    shelfRequest = null;
    loadError = false;
    loading = true;
    void refresh(untrack(() => query), revision);
  }

  function invalidate() {
    clearTimeout(searchTimer);
    requestRevision += 1;
  }

  $effect(() => {
    if (!active) return;
    reload();
    const onPageShow = (event: PageTransitionEvent) => { if (event.persisted) reload(); };
    window.addEventListener("pagehide", invalidate);
    window.addEventListener("pageshow", onPageShow);
    return () => {
      window.removeEventListener("pagehide", invalidate);
      window.removeEventListener("pageshow", onPageShow);
      invalidate();
    };
  });

  function handleSearchInput(value: string) {
    query = value;
    clearTimeout(searchTimer);
    const revision = ++requestRevision;
    loadError = false;
    loading = true;
    searchTimer = setTimeout(() => void refresh(value, revision), 150);
  }

  async function handleJumpToBook(hit: ReadingMemoryHit) {
    const book = bookMap.get(hit.editionId);
    if (!book) return;
    jumpError = "";
    try {
      await onOpenReaderJump(book, {
        messageId: hit.messageId,
        rootMessageId: hit.rootMessageId,
        conversationId: hit.conversationId,
        editionId: hit.editionId,
        canonicalLocator: hit.canonicalLocator,
      });
    } catch (error) {
      jumpError = openFailureMessage(error);
    }
  }

  async function openSnapshotModal(hit: ReadingMemoryHit) {
    activeHit = hit;
    snapshotModalOpen = true;
    snapshotLoading = true;
    try {
      activeCaptures = await getReadingMemoryClient().sourceCaptures(hit.rootMessageId);
    } finally {
      snapshotLoading = false;
    }
  }

  function formatTimestamp(ms: number): string {
    return new Date(ms).toLocaleString("zh-CN", {
      month: "numeric",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<section class="memory-shell" aria-label="阅读记忆">
  <header class="memory-header">
    <h1>阅读记忆</h1>
    <p class="subtitle">
      跨书搜索笔记、摘录与对话回复。每段思考都保存了原文上下文与当时快照。
    </p>

    <div class="well-input search-pill">
      <Search size={16} strokeWidth={1.8} class="search-icon" />
      <input
        type="search"
        aria-label="搜索笔记与回复"
        placeholder="搜索笔记、回复或原文引语…"
        value={query}
        oninput={(e) => handleSearchInput(e.currentTarget.value)}
      />
      {#if query}
        <button
          type="button"
          class="clear-btn"
          aria-label="清空搜索"
          onclick={() => handleSearchInput("")}
        >
          <X size={14} />
        </button>
      {/if}
    </div>
  </header>
  {#if jumpError}<p role="alert">{jumpError}</p>{/if}

  {#if recentBooks.length > 0 && !query.trim()}
    <section class="memory-recent" aria-label="最近阅读">
      <div class="section-head">
        <h2>最近翻阅</h2>
        <span class="section-count tabular">{recentBooks.length} 册</span>
      </div>
      <div class="recent-pills">
        {#each recentBooks.slice(0, 6) as entry, idx (entry.book.id)}
          <div class="recent-chip">
            <div class="chip-cover">
              <BookCover book={entry.book} index={idx} compact />
            </div>
            <div class="chip-info">
              <div class="chip-title">{entry.book.title}</div>
              <div class="chip-meta tabular">
                {entry.fact?.percent != null ? `${entry.fact.percent}%` : "在读"}
              </div>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="memory-hits" aria-label="记忆检索结果" aria-busy={loading}>
    <div class="section-head">
      <h2>{query.trim() ? "搜索笔记与回复" : "全部记忆片段"}</h2>
      {#if !loadError && resultQuery === query}
        <span class="section-count tabular">{hits.length} 条</span>
      {/if}
    </div>

    {#if loadError}
      <div class="empty-memory">
        <p role="status">暂时无法读取阅读记忆。</p>
        <button type="button" class="pill-btn" onclick={reload}>重试</button>
      </div>
    {:else if loading && resultQuery !== query}
      <div class="empty-memory">正在翻检跨书阅读记忆…</div>
    {:else if hits.length === 0}
      <div class="empty-memory">
        <Sparkles size={24} strokeWidth={1.6} />
        <p>没有找到匹配的笔记或摘录。</p>
      </div>
    {:else}
      <div class="hits-stream">
        {#each hits as hit (hit.messageId)}
          {@const bookInShelf = bookMap.has(hit.editionId)}
          <article class="hit-row">
            <div class="hit-meta">
              <span class="hit-book">{hit.title}</span>
              <span class="hit-time tabular">{formatTimestamp(hit.updatedAt)}</span>
            </div>

            <blockquote class="hit-quote">
              <span class="quote-bar" aria-hidden="true"></span>
              <p>{hit.selectedText}</p>
            </blockquote>

            {#if hit.text && hit.text !== "（仅高亮标注）"}
              <p class="hit-note">{hit.text}</p>
            {:else}
              <p class="hit-note muted">纯原文标注，尚无后续回应</p>
            {/if}

            <div class="hit-actions">
              <button type="button" class="pill-btn" onclick={() => openSnapshotModal(hit)}>
                <History size={14} />
                <span>历史快照</span>
              </button>
              {#if bookInShelf}
                <button type="button" class="pill-btn jump-btn" aria-disabled={launchBlocked}
                  onclick={() => { if (!launchBlocked) void handleJumpToBook(hit); }}>
                  <span>跳回原书</span>
                  <ArrowUpRight size={14} />
                </button>
              {:else}
                <span class="missing-edition-note">原书未在本地书架 · 仅快照可读</span>
              {/if}
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</section>

<Dialog.Root bind:open={snapshotModalOpen}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 160 }}></div>
        {/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content wide snapshot-dialog"
            transition:scale={{ start: 0.96, duration: 200, easing: cubicOut }}
          >
            <div class="snapshot-dialog-head">
              <div>
                <Dialog.Title class="dialog-heading">原文历史快照</Dialog.Title>
                <Dialog.Description class="dialog-sub">
                  {activeHit?.title ?? ""} · 隔离 Shadow DOM 只读存档
                </Dialog.Description>
              </div>
              <button
                type="button"
                class="icon-btn"
                aria-label="关闭快照"
                onclick={() => (snapshotModalOpen = false)}
              >
                <X size={18} />
              </button>
            </div>

            {#if snapshotLoading}
              <div class="snapshot-empty">正在读取存档快照…</div>
            {:else if activeCaptures.length === 0}
              <div class="snapshot-empty">
                <p>当前条目尚未生成离线快照，以下展示原始引文：</p>
                <blockquote class="hit-quote">
                  <span class="quote-bar" aria-hidden="true"></span>
                  <p>{activeHit?.selectedText}</p>
                </blockquote>
              </div>
            {:else}
              <div class="captures-list">
                {#each activeCaptures as capture (capture.source.id)}
                  <div class="capture-block">
                    <div class="capture-badge-row tabular">
                      <span class="capture-badge">
                        {capture.current ? "当前版本快照" : "历史捕获版本"}
                      </span>
                      <span class="capture-date">{formatTimestamp(capture.createdAt)}</span>
                    </div>
                    <SnapshotPreview {capture} resourceClient={getReadingMemoryClient()} />
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  .memory-shell {
    padding: 36px 40px 96px 248px;
    max-width: 1080px;
    margin: 0 auto;
    min-height: 100vh;
  }

  .memory-header h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }

  .subtitle {
    margin: 8px 0 22px;
    font-size: 14px;
    color: var(--ink-secondary);
  }

  .search-pill {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 46px;
    padding: 0 18px;
    max-width: 620px;
  }

  .search-pill :global(.search-icon) {
    color: var(--ink-muted);
    flex: none;
  }

  .search-pill input {
    width: 100%;
    font-size: 14.5px;
  }

  .clear-btn {
    width: 24px;
    height: 24px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-muted);
  }

  .memory-recent,
  .memory-hits {
    margin-top: 34px;
  }

  .section-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  .section-head h2 {
    margin: 0;
    font-size: 16.5px;
    font-weight: 600;
  }

  .section-count {
    font-size: 12.5px;
    color: var(--ink-muted);
  }

  .recent-pills {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding-bottom: 6px;
    scrollbar-width: none;
  }

  .recent-pills::-webkit-scrollbar {
    display: none;
  }

  .recent-chip {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs);
    max-width: 240px;
  }

  .chip-cover {
    width: 34px;
    flex: none;
  }

  .chip-info {
    min-width: 0;
  }

  .chip-title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chip-meta {
    font-size: 11.5px;
    color: var(--accent);
    margin-top: 2px;
  }

  .hits-stream {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .hit-row {
    padding: 22px 24px;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs);
    display: flex;
    flex-direction: column;
    gap: 14px;
    transition: box-shadow 200ms var(--ease-smooth);
  }

  .hit-row:hover {
    box-shadow: var(--shadow-sm);
  }

  .hit-meta {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }

  .hit-book {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }

  .hit-time {
    font-size: 12px;
    color: var(--ink-muted);
  }

  .hit-quote {
    position: relative;
    margin: 0;
    padding: 12px 16px 12px 18px;
    border-radius: var(--radius-md);
    background: var(--quote-tint);
    font-family: var(--font-serif);
    font-size: 14.5px;
    line-height: 1.72;
    color: var(--ink-primary);
  }

  .hit-quote p {
    margin: 0;
  }

  .quote-bar {
    position: absolute;
    left: 6px;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 999px;
    background: var(--accent);
    opacity: 0.72;
  }

  .hit-note {
    margin: 0;
    font-size: 14.5px;
    line-height: 1.68;
    color: var(--ink-primary);
  }

  .hit-note.muted {
    color: var(--ink-muted);
    font-size: 13.5px;
  }

  .hit-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  :global(.jump-btn) {
    background: var(--accent-soft);
    color: var(--accent);
  }

  :global(.jump-btn:hover) {
    background: var(--accent-soft-hover);
  }

  .missing-edition-note {
    font-size: 12.5px;
    color: var(--ink-muted);
  }

  .empty-memory {
    padding: 64px 24px;
    text-align: center;
    color: var(--ink-muted);
    background: var(--bg-surface);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-xs);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .snapshot-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
  }

  .snapshot-dialog-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 16px;
  }

  .captures-list {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .capture-badge-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    font-size: 12px;
    color: var(--ink-muted);
  }

  .capture-badge {
    padding: 3px 10px;
    border-radius: var(--radius-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .snapshot-empty {
    padding: 20px 0;
    color: var(--ink-secondary);
    font-size: 14px;
  }

  @media (max-width: 899px) {
    .memory-shell {
      padding: 22px 16px 104px;
    }

    .memory-header h1 {
      font-size: 26px;
    }

    .hit-row {
      padding: 18px;
    }
  }
</style>
