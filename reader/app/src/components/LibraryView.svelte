<script lang="ts">
  import {
    Archive,
    ArchiveRestore,
    BookOpen,
    Circle,
    CircleCheck,
    Ellipsis,
    Plus,
    Search,
    Trash2,
  } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    backupMessages,
    coverUrl,
    filterLibraryBooks,
    groupLibraryBooksByProgress,
    importBooks,
    importFailureMessage,
    libraryAvailable,
    listBooks,
    openBook,
    openFailureMessage,
    readStartedBookIds,
    removeBooksSerially,
    restoreMessages,
    type LibraryBook,
    type LibraryViewMode,
  } from "../library";

  const views: { value: LibraryViewMode; label: string }[] = [
    { value: "default", label: "默认" },
    { value: "progress", label: "进度" },
    { value: "title", label: "书名" },
    { value: "author", label: "作者" },
  ];

  let books: LibraryBook[] = [];
  let loading = true;
  let busy = false;
  let status = "";
  let workLabel = "";
  let query = "";
  let view: LibraryViewMode = "default";
  let selecting = false;
  let selectedIds = new Set<string>();
  let failedCovers = new Set<string>();
  let startedBookIds: Set<string> | null = new Set();
  let managementMenu: HTMLDetailsElement | undefined;
  let visibleBooks: LibraryBook[] = [];
  let progressGroups = { reading: [] as LibraryBook[], unread: [] as LibraryBook[] };
  let allVisibleSelected = false;

  $: visibleBooks = filterLibraryBooks(books, query, view);
  $: progressGroups = startedBookIds
    ? groupLibraryBooksByProgress(visibleBooks, startedBookIds)
    : { reading: [], unread: [] };
  $: allVisibleSelected =
    visibleBooks.length > 0 && visibleBooks.every((book) => selectedIds.has(book.id));

  onMount(async () => {
    const systemBars = Reflect.get(globalThis, "AthaSystemBars");
    systemBars?.setReadingMode?.(false, true);
    const syncSafeAreaInsets = () => {
      try {
        const insets = JSON.parse(systemBars?.getSafeAreaInsets?.() ?? "null");
        for (const edge of ["top", "right", "bottom", "left"] as const) {
          if (Number.isFinite(insets?.[edge])) {
            document.documentElement.style.setProperty(
              `--safe-area-${edge}`,
              `${insets[edge] / devicePixelRatio}px`,
            );
          }
        }
      } catch {
        // The native bridge is optional on desktop.
      }
    };
    syncSafeAreaInsets();
    globalThis.addEventListener("atha-safe-area-change", syncSafeAreaInsets);
    try {
      books = await listBooks();
      refreshProgress();
    } catch {
      status = "无法读取本地书架。";
    } finally {
      loading = false;
    }
  });

  function refreshProgress() {
    startedBookIds = readStartedBookIds(books);
    if (startedBookIds === null && view === "progress") view = "default";
  }

  function closeManagementMenu() {
    managementMenu?.removeAttribute("open");
  }

  async function chooseBooks() {
    if (busy) return;
    if (!libraryAvailable) {
      status = "请在 Atha 应用中选择 EPUB、CBZ、FB2、FBZ、Kindle、Markdown 或 TXT。";
      return;
    }
    busy = true;
    status = "";
    workLabel = "正在加入书架…";
    try {
      const report = await importBooks();
      if (!report) {
        status = "";
        return;
      }
      books = report.books;
      refreshProgress();
      if (report.failures.length === 0) {
        status = "已加入书架。";
      } else {
        status = report.failures
          .map((failure) => `${failure.name}：${importFailureMessage(failure.code)}`)
          .join("；");
      }
    } catch {
      status = "无法导入所选书籍。";
    } finally {
      workLabel = "";
      busy = false;
    }
  }

  async function backup() {
    if (busy) return;
    closeManagementMenu();
    if (!libraryAvailable) {
      status = "请在 Atha 桌面应用中备份消息。";
      return;
    }
    busy = true;
    status = "正在创建消息备份…";
    try {
      status = (await backupMessages()) ? "消息备份已保存。" : "已取消消息备份。";
    } catch {
      status = "无法创建消息备份。";
    } finally {
      busy = false;
    }
  }

  async function restore() {
    if (busy) return;
    closeManagementMenu();
    if (!libraryAvailable) {
      status = "请在 Atha 桌面应用中恢复消息。";
      return;
    }
    if (!confirm("恢复会替换全部标注、笔记和对话。请先关闭其他 Atha 窗口。继续？")) {
      status = "已取消消息恢复。";
      return;
    }
    busy = true;
    status = "正在验证并恢复消息备份…";
    try {
      status = (await restoreMessages()) ? "消息已从备份恢复。" : "已取消消息恢复。";
    } catch {
      status = "无法恢复：备份无效、数据库正被占用，或文件不可读取。";
    } finally {
      busy = false;
    }
  }

  async function read(book: LibraryBook) {
    if (busy) return;
    busy = true;
    status = "";
    workLabel = book.prepared ? "" : "首次打开，正在准备书籍…";
    try {
      await openBook(book.id);
    } catch (error) {
      status = `无法打开《${book.title}》：${openFailureMessage(error)}。`;
      workLabel = "";
      busy = false;
    }
  }

  function startSelection() {
    selecting = true;
    selectedIds = new Set();
  }

  function cancelSelection() {
    selecting = false;
    selectedIds = new Set();
  }

  function setQuery(next: string) {
    query = next;
    if (selecting) selectedIds = new Set();
  }

  function setView(next: LibraryViewMode) {
    view = next;
    if (selecting) selectedIds = new Set();
  }

  function toggleSelection(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  function toggleVisibleSelection() {
    if (visibleBooks.length === 0) return;
    const next = new Set(selectedIds);
    for (const book of visibleBooks) {
      if (allVisibleSelected) next.delete(book.id);
      else next.add(book.id);
    }
    selectedIds = next;
  }

  async function removeSelected() {
    const ids = books.filter((book) => selectedIds.has(book.id)).map((book) => book.id);
    if (busy || ids.length === 0) return;
    if (!confirm(`从书架移出已选的 ${ids.length} 本书？阅读进度和导入内容会保留。`)) return;

    busy = true;
    status = "正在移出书架…";
    try {
      const result = await removeBooksSerially(ids);
      if (result.books) books = result.books;
      selectedIds = new Set(result.remainingIds);
      refreshProgress();

      if (result.remainingIds.length === 0) {
        selecting = false;
        status = `已从书架移出 ${result.removedIds.length} 本书。`;
      } else if (result.removedIds.length === 0) {
        status = `无法移出已选的 ${result.remainingIds.length} 本书。`;
      } else {
        status = `已移出 ${result.removedIds.length} 本，另 ${result.remainingIds.length} 本未能移出。`;
      }
    } finally {
      busy = false;
    }
  }

  function markCoverFailed(id: string) {
    failedCovers = new Set(failedCovers).add(id);
  }
</script>

{#snippet bookCard(book: LibraryBook)}
  <article class="library-book">
    <button
      class="library-book-open"
      type="button"
      aria-label={selecting
        ? `${selectedIds.has(book.id) ? "取消选择" : "选择"}《${book.title}》`
        : undefined}
      aria-pressed={selecting ? selectedIds.has(book.id) : undefined}
      onclick={() => (selecting ? toggleSelection(book.id) : read(book))}
      disabled={busy}
    >
      <span class="library-cover">
        {#if book.hasCover && !failedCovers.has(book.id)}
          <img
            src={coverUrl(book.id)}
            alt="书籍封面"
            draggable="false"
            loading="lazy"
            decoding="async"
            onerror={() => markCoverFailed(book.id)}
          />
        {:else}
          <BookOpen aria-hidden="true" />
        {/if}
        {#if selecting}
          <span class:selected={selectedIds.has(book.id)} class="library-book-selection" aria-hidden="true">
            {#if selectedIds.has(book.id)}
              <CircleCheck />
            {:else}
              <Circle />
            {/if}
          </span>
        {/if}
      </span>
      <span class="library-book-title">{book.title}</span>
      <span class="library-book-author">{book.authors.join(" / ") || "未知作者"}</span>
    </button>
  </article>
{/snippet}

<main
  class:library-selecting={selecting}
  class="library-shell"
  aria-label="Atha 书架"
  aria-busy={loading || busy}
>
  <header class="library-header">
    <div class="library-search-bar">
      <label class="library-search-field">
        <Search aria-hidden="true" />
        <span class="visually-hidden">搜索书名或作者</span>
        <input
          value={query}
          type="search"
          placeholder="搜索书名或作者"
          disabled={loading}
          oninput={(event) => setQuery(event.currentTarget.value)}
        />
      </label>
      <details class="library-management" bind:this={managementMenu}>
        <summary aria-label="管理书架">
          <Ellipsis aria-hidden="true" />
          <span>管理</span>
        </summary>
        <div class="library-management-menu" aria-label="书架管理">
          <button type="button" onclick={backup} disabled={busy}>
            <Archive aria-hidden="true" />
            <span>备份消息</span>
          </button>
          <button type="button" onclick={restore} disabled={busy}>
            <ArchiveRestore aria-hidden="true" />
            <span>恢复消息</span>
          </button>
        </div>
      </details>
    </div>

    {#if selecting}
      <div class="library-selection-header">
        <button type="button" onclick={toggleVisibleSelection} disabled={busy || visibleBooks.length === 0}>
          {allVisibleSelected ? "取消全选" : "全选"}
        </button>
        <div>
          <strong>选择书籍</strong>
          <span>已选择 {selectedIds.size} 本</span>
        </div>
        <button type="button" onclick={cancelSelection} disabled={busy}>取消</button>
      </div>
    {:else}
      <div class="library-titlebar">
        <h1>书架</h1>
        <div class="library-primary-actions">
          <button type="button" onclick={chooseBooks} disabled={busy}>
            <Plus aria-hidden="true" />
            <span>导入</span>
          </button>
          <button type="button" onclick={startSelection} disabled={busy || books.length === 0}>
            <CircleCheck aria-hidden="true" />
            <span>选择</span>
          </button>
        </div>
      </div>
    {/if}

    <nav class="library-views" aria-label="书架视图">
      {#each views as item}
        <button
          type="button"
          class:active={view === item.value}
          aria-current={view === item.value ? "page" : undefined}
          onclick={() => setView(item.value)}
          disabled={busy || (item.value === "progress" && startedBookIds === null)}
        >
          {item.label}
        </button>
      {/each}
    </nav>
  </header>

  {#if startedBookIds === null}
    <p class="library-progress-notice" role="status">无法读取本机阅读进度，进度视图暂不可用。</p>
  {/if}

  {#if loading}
    <section class="library-empty" aria-label="正在读取书架">
      <div class="library-loading" aria-hidden="true"></div>
      <p>正在读取书架…</p>
    </section>
  {:else if books.length === 0}
    <section class="library-empty">
      <BookOpen aria-hidden="true" />
      <h2>开始你的书架</h2>
      <p>选择 EPUB、CBZ、FB2、FBZ、Kindle、Markdown 或 TXT，导入后即可随时继续阅读。</p>
      <button type="button" onclick={chooseBooks} disabled={busy}>选择书籍</button>
    </section>
  {:else if visibleBooks.length === 0}
    <section class="library-no-results" aria-label="没有匹配的书籍">
      <Search aria-hidden="true" />
      <h2>没有找到书籍</h2>
      <p>试试其他书名或作者。</p>
    </section>
  {:else if view === "progress" && startedBookIds}
    <div class="library-progress-groups">
      <section aria-labelledby="library-reading-heading">
        <header class="library-group-heading">
          <h2 id="library-reading-heading">在读</h2>
          <span>{progressGroups.reading.length} 本</span>
        </header>
        {#if progressGroups.reading.length > 0}
          <div class="library-grid">
            {#each progressGroups.reading as book (book.id)}
              {@render bookCard(book)}
            {/each}
          </div>
        {:else}
          <p class="library-group-empty">暂无在读书籍</p>
        {/if}
      </section>
      <section aria-labelledby="library-unread-heading">
        <header class="library-group-heading">
          <h2 id="library-unread-heading">未开始</h2>
          <span>{progressGroups.unread.length} 本</span>
        </header>
        {#if progressGroups.unread.length > 0}
          <div class="library-grid">
            {#each progressGroups.unread as book (book.id)}
              {@render bookCard(book)}
            {/each}
          </div>
        {:else}
          <p class="library-group-empty">暂无未开始书籍</p>
        {/if}
      </section>
    </div>
  {:else}
    <section class="library-grid" aria-label="书籍">
      {#each visibleBooks as book (book.id)}
        {@render bookCard(book)}
      {/each}
    </section>
  {/if}

  {#if selecting}
    <div class="library-selection-bar" role="toolbar" aria-label="批量操作">
      <button type="button" onclick={removeSelected} disabled={busy || selectedIds.size === 0}>
        <Trash2 aria-hidden="true" />
        <span>移出书架</span>
      </button>
    </div>
  {/if}

  {#if status}
    <p class="library-status" role="status">{status}</p>
  {/if}

  {#if workLabel}
    <div class="library-work-overlay" role="status" aria-live="polite">
      <div class="library-work-progress">
        <p>{workLabel}</p>
        <progress aria-label={workLabel}></progress>
      </div>
    </div>
  {/if}
</main>
