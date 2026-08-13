<script lang="ts">
  import {
    BookOpen,
    Clock3,
    History,
    Library,
    MessageSquareText,
    Search,
    X,
  } from "@lucide/svelte";

  import { coverUrl, openBook, openReadingMemoryHit, readRecentBooks, type LibraryBook } from "../library";
  import {
    readingMemoryClient,
    type ReadingMemoryHit,
    type SourceCaptureView,
  } from "../messages";

  export let books: LibraryBook[] = [];
  export let disabled = false;

  let query = "";
  let submittedQuery = "";
  let hits: ReadingMemoryHit[] = [];
  let searching = false;
  let searched = false;
  let status = "";
  let activeAction = "";
  let snapshotDialog: HTMLDialogElement | undefined;
  let snapshotContent: HTMLDivElement | undefined;
  let snapshotTitle = "历史引用";
  let snapshotStatus = "";
  let captures: SourceCaptureView[] = [];
  let selectedSourceId = "";
  let recent = readRecentBooks(books);
  let availableIds = new Set(books.map((book) => book.id));

  $: recent = readRecentBooks(books);
  $: availableIds = new Set(books.map((book) => book.id));

  async function search(event: SubmitEvent) {
    event.preventDefault();
    const value = query.trim();
    if (!value || searching || disabled) return;
    searching = true;
    status = "";
    submittedQuery = value;
    try {
      hits = await readingMemoryClient.search(value);
      searched = true;
    } catch {
      hits = [];
      searched = true;
      status = "无法搜索阅读记忆，请稍后重试。";
    } finally {
      searching = false;
    }
  }

  async function continueReading(book: LibraryBook) {
    if (disabled || activeAction) return;
    activeAction = `book:${book.id}`;
    status = "";
    try {
      await openBook(book.id);
    } catch {
      status = "无法打开这本书，请返回书架检查本地文件。";
      activeAction = "";
    }
  }

  async function jumpToHit(hit: ReadingMemoryHit) {
    if (disabled || activeAction || !availableIds.has(hit.editionId)) return;
    activeAction = `jump:${hit.messageId}`;
    status = "";
    try {
      await openReadingMemoryHit(hit);
    } catch {
      status = "无法验证这条引用的当前原书位置，请查看历史引用。";
      activeAction = "";
    }
  }

  async function showSnapshots(hit: ReadingMemoryHit) {
    if (disabled || activeAction) return;
    activeAction = `snapshot:${hit.messageId}`;
    snapshotTitle = hit.title;
    snapshotStatus = "正在读取历史引用…";
    captures = [];
    selectedSourceId = "";
    snapshotContent?.replaceChildren();
    snapshotDialog?.showModal();
    try {
      captures = await readingMemoryClient.sourceCaptures(hit.rootMessageId);
      const current = captures.find((capture) => capture.current) ?? captures.at(-1);
      if (!current) throw new Error("missing-source-capture");
      await selectCapture(current);
    } catch {
      snapshotStatus = "历史引用不可读取。";
    } finally {
      activeAction = "";
    }
  }

  async function selectCapture(capture: SourceCaptureView) {
    selectedSourceId = capture.source.id;
    snapshotStatus = "正在呈现引用…";
    snapshotContent?.replaceChildren();
    try {
      const { renderSnapshotElement } = await import("../../../web/conversations.mjs");
      const rendered = await renderSnapshotElement(capture, readingMemoryClient);
      if (selectedSourceId !== capture.source.id) return;
      snapshotContent?.replaceChildren(rendered);
      snapshotStatus = "";
    } catch {
      if (selectedSourceId === capture.source.id) snapshotStatus = "这份历史引用已损坏。";
    }
  }

  function closeSnapshots() {
    snapshotDialog?.close();
    captures = [];
    selectedSourceId = "";
    snapshotStatus = "";
    snapshotContent?.replaceChildren();
  }

  function formatDuration(milliseconds: number) {
    if (milliseconds < 60_000) return "<1 分钟";
    const minutes = Math.floor(milliseconds / 60_000);
    if (minutes < 60) return `${minutes} 分钟`;
    const hours = Math.floor(minutes / 60);
    const remainder = minutes % 60;
    return remainder ? `${hours} 小时 ${remainder} 分` : `${hours} 小时`;
  }

  function formatDay(value: string) {
    const [year, month, day] = value.split("-").map(Number);
    return new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(
      new Date(year, month - 1, day),
    );
  }

  function formatTime(value: number) {
    return new Intl.DateTimeFormat("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(new Date(value));
  }
</script>

<section class="memory-center" aria-labelledby="memory-center-heading">
  <header class="memory-heading">
    <div>
      <h1 id="memory-center-heading">阅读记忆</h1>
      <p>最近阅读与跨书消息</p>
    </div>
    <MessageSquareText aria-hidden="true" />
  </header>

  <section class="memory-recent" aria-labelledby="memory-recent-heading">
    <header class="memory-section-heading">
      <div>
        <Clock3 aria-hidden="true" />
        <h2 id="memory-recent-heading">最近阅读</h2>
      </div>
      {#if recent}<span>{recent.length} 本</span>{/if}
    </header>
    {#if recent === null}
      <p class="memory-inline-status" role="status">本机阅读统计不可读取，最近阅读暂不可用。</p>
    {:else if recent.length === 0}
      <p class="memory-inline-status">完成一次有效阅读后，书籍会出现在这里。</p>
    {:else}
      <div class="memory-recent-list">
        {#each recent as item (item.book.id)}
          <button
            type="button"
            onclick={() => continueReading(item.book)}
            disabled={disabled || Boolean(activeAction)}
            aria-label={`继续阅读《${item.book.title}》`}
          >
            <span class="memory-recent-cover">
              {#if item.book.prepared && item.book.hasCover}
                <img src={coverUrl(item.book.id)} alt="" loading="lazy" decoding="async" />
              {:else}
                <BookOpen aria-hidden="true" />
              {/if}
            </span>
            <span class="memory-recent-copy">
              <strong>{item.book.title}</strong>
              <span>{item.book.authors.join(" / ") || "未知作者"}</span>
              <small>{formatDay(item.lastReadDate)} · {formatDuration(item.durationMs)}</small>
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </section>

  <section class="memory-search" aria-labelledby="memory-search-heading">
    <header class="memory-section-heading">
      <div>
        <Search aria-hidden="true" />
        <h2 id="memory-search-heading">搜索消息</h2>
      </div>
    </header>
    <form class="memory-search-form" onsubmit={search}>
      <label>
        <span class="visually-hidden">跨书搜索原文、笔记与回复</span>
        <Search aria-hidden="true" />
        <input
          bind:value={query}
          type="search"
          maxlength="256"
          placeholder="搜索原文、笔记与回复"
          disabled={disabled || searching}
        />
      </label>
      <button type="submit" disabled={disabled || searching || !query.trim()}>
        {searching ? "搜索中" : "搜索"}
      </button>
    </form>

    {#if searching}
      <p class="memory-inline-status" role="status">正在搜索本地消息…</p>
    {:else if searched && hits.length === 0 && !status}
      <div class="memory-search-empty">
        <Search aria-hidden="true" />
        <p>没有找到“{submittedQuery}”</p>
      </div>
    {:else if hits.length > 0}
      <p class="memory-result-count" role="status">找到 {hits.length} 条结果</p>
      <div class="memory-results">
        {#each hits as hit (hit.messageId)}
          <article class="memory-result">
            <header>
              <div class="memory-result-book">
                {#if availableIds.has(hit.editionId)}
                  <span class="memory-result-cover">
                    {#if books.find((book) => book.id === hit.editionId)?.hasCover}
                      <img src={coverUrl(hit.editionId)} alt="" loading="lazy" decoding="async" />
                    {:else}
                      <Library aria-hidden="true" />
                    {/if}
                  </span>
                {:else}
                  <span class="memory-result-cover missing"><Library aria-hidden="true" /></span>
                {/if}
                <div>
                  <h3>{hit.title}</h3>
                  <p>{hit.authors.join(" / ") || "未知作者"}</p>
                </div>
              </div>
              <time datetime={new Date(hit.updatedAt).toISOString()}>{formatTime(hit.updatedAt)}</time>
            </header>
            <blockquote>{hit.selectedText}</blockquote>
            {#if hit.text}<p class="memory-result-text">{hit.text}</p>{/if}
            <footer>
              <span>{availableIds.has(hit.editionId) ? "原书可用" : "原书不在资料库"}</span>
              <div>
                <button
                  type="button"
                  onclick={() => showSnapshots(hit)}
                  disabled={disabled || Boolean(activeAction)}
                >
                  <History aria-hidden="true" />
                  <span>历史引用</span>
                </button>
                {#if availableIds.has(hit.editionId)}
                  <button
                    class="memory-jump"
                    type="button"
                    onclick={() => jumpToHit(hit)}
                    disabled={disabled || Boolean(activeAction)}
                  >
                    <BookOpen aria-hidden="true" />
                    <span>跳回原书</span>
                  </button>
                {/if}
              </div>
            </footer>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  {#if status}<p class="memory-status" role="status">{status}</p>{/if}
</section>

<dialog
  class="memory-snapshot-dialog"
  bind:this={snapshotDialog}
  onclose={() => {
    captures = [];
    selectedSourceId = "";
    snapshotStatus = "";
    snapshotContent?.replaceChildren();
  }}
>
  <header>
    <div>
      <History aria-hidden="true" />
      <h2>{snapshotTitle}</h2>
    </div>
    <button type="button" aria-label="关闭历史引用" title="关闭" onclick={closeSnapshots}>
      <X aria-hidden="true" />
    </button>
  </header>
  {#if captures.length > 0}
    <nav aria-label="引用版本">
      {#each captures as capture, index (capture.source.id)}
        <button
          type="button"
          class:active={selectedSourceId === capture.source.id}
          aria-current={selectedSourceId === capture.source.id ? "page" : undefined}
          onclick={() => selectCapture(capture)}
        >
          {capture.current ? "当前引用" : `历史引用 ${index + 1}`}
        </button>
      {/each}
    </nav>
  {/if}
  {#if snapshotStatus}<p class="memory-snapshot-status" role="status">{snapshotStatus}</p>{/if}
  <div class="memory-snapshot-content" bind:this={snapshotContent}></div>
</dialog>
