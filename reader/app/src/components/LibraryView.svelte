<script lang="ts">
  import {
    Archive,
    ArchiveRestore,
    BookMinus,
    BookOpen,
    Circle,
    CircleCheck,
    Ellipsis,
    HardDrive,
    Plus,
    Search,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    backupLocalData,
    abortLocalDataRestore,
    captureLocalDataState,
    commitLocalDataRestore,
    coverUrl,
    deleteBooksSerially,
    filterLibraryBooks,
    finishLocalDataRestore,
    groupLibraryBooksByProgress,
    importBooks,
    importFailureMessage,
    libraryAvailable,
    listBooks,
    openBook,
    openFailureMessage,
    pendingLocalDataRestore,
    prepareLocalDataRestore,
    readStartedBookIds,
    readStorageUsage,
    replaceLocalDataState,
    resumeBookDataDeletions,
    removeBooksSerially,
    rollbackLocalDataRestore,
    validateLocalDataState,
    type LibraryBook,
    type LibraryViewMode,
    type BrowserState,
    type StorageUsage,
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
  let storageDialog: HTMLDialogElement | undefined;
  let storageUsage: StorageUsage | null = null;
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
      if (libraryAvailable) {
        await resumePendingRestore();
        await resumeBookDataDeletions();
      }
      books = await listBooks();
      refreshProgress();
    } catch {
      status = "无法完成本地资料恢复，请重新启动 Atha 后重试。";
      busy = true;
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
      status = "请在 Atha 应用中备份资料库。";
      return;
    }
    busy = true;
    status = "正在创建资料库备份…";
    try {
      const browserState = captureLocalDataState();
      status = (await backupLocalData(browserState)) ? "资料库备份已保存。" : "已取消备份。";
    } catch (error) {
      status = String(error).includes("missing-book-source")
        ? "有书籍缺少原文件，请重新选择原文件加入书架后再备份。"
        : "无法创建资料库备份，请检查本地数据和可用空间。";
    } finally {
      busy = false;
    }
  }

  async function restore() {
    if (busy) return;
    closeManagementMenu();
    if (!libraryAvailable) {
      status = "请在 Atha 应用中恢复资料库。";
      return;
    }
    if (!confirm("恢复会替换当前书架、书籍文件、词典、消息和阅读状态。继续？")) {
      status = "已取消恢复。";
      return;
    }
    busy = true;
    workLabel = "正在验证资料库…";
    let token = "";
    let previous: BrowserState | null = null;
    try {
      previous = captureLocalDataState();
      const prepared = await prepareLocalDataRestore(previous);
      if (!prepared) {
        status = "已取消恢复。";
        return;
      }
      token = prepared.token;
      if (!validateLocalDataState(prepared.browserState)) {
        const invalidToken = token;
        token = "";
        await abortLocalDataRestore(invalidToken);
        throw new Error("invalid-browser-state");
      }
      workLabel = "正在恢复资料库…";
      const committed = await commitLocalDataRestore(token);
      replaceLocalDataState(committed.browserState);
      await finishLocalDataRestore(token);
      token = "";
      books = await listBooks();
      failedCovers = new Set();
      refreshProgress();
      cancelSelection();
      status = "资料库已恢复。";
    } catch {
      if (token) {
        try {
          const rollback = await rollbackLocalDataRestore(token);
          replaceLocalDataState(rollback);
          await finishLocalDataRestore(token);
          token = "";
        } catch {
          // The durable pending restore will be retried on the next launch.
        }
      }
      status = "无法恢复：备份无效、数据正被占用，或本机存储不可写。";
    } finally {
      workLabel = "";
      busy = false;
    }
  }

  async function resumePendingRestore(): Promise<boolean> {
    const pending = await pendingLocalDataRestore();
    if (!pending) return false;
    if (pending.rollback) {
      replaceLocalDataState(pending.browserState);
      await finishLocalDataRestore(pending.token);
      status = "上次恢复未完成，已还原原资料库。";
      return true;
    }
    try {
      if (!validateLocalDataState(pending.browserState)) throw new Error("invalid-browser-state");
      replaceLocalDataState(pending.browserState);
      await finishLocalDataRestore(pending.token);
      status = "资料库恢复已完成。";
    } catch {
      const rollback = await rollbackLocalDataRestore(pending.token);
      replaceLocalDataState(rollback);
      await finishLocalDataRestore(pending.token);
      status = "上次恢复未完成，已还原原资料库。";
    }
    return true;
  }

  async function showStorage() {
    if (busy) return;
    closeManagementMenu();
    busy = true;
    status = "";
    try {
      storageUsage = await readStorageUsage(captureLocalDataState());
      storageDialog?.showModal();
    } catch {
      status = "无法读取本地存储占用。";
    } finally {
      busy = false;
    }
  }

  function formatBytes(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = bytes / 1024;
    let unit = units[0];
    for (const next of units.slice(1)) {
      if (value < 1024) break;
      value /= 1024;
      unit = next;
    }
    return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${unit}`;
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

  async function deleteSelected() {
    const ids = books.filter((book) => selectedIds.has(book.id)).map((book) => book.id);
    if (busy || ids.length === 0) return;
    if (!confirm(`删除已选 ${ids.length} 本书的本地文件和阅读状态？笔记、标注消息和原文快照会保留。`)) {
      return;
    }
    busy = true;
    status = "正在删除本地数据…";
    try {
      const result = await deleteBooksSerially(ids);
      if (result.pendingRecovery) {
        status = "删除尚未确认，Atha 将重新载入并继续恢复。";
        location.reload();
        return;
      }
      if (result.books) books = result.books;
      selectedIds = new Set(result.remainingIds);
      refreshProgress();
      if (result.remainingIds.length === 0) {
        selecting = false;
        status = `已删除 ${result.removedIds.length} 本书的本地数据。`;
      } else if (result.removedIds.length === 0) {
        status = `无法删除已选的 ${result.remainingIds.length} 本书。`;
      } else {
        status = `已删除 ${result.removedIds.length} 本，另 ${result.remainingIds.length} 本未能删除。`;
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
        {#if book.prepared && book.hasCover && !failedCovers.has(book.id)}
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
            <span>备份资料库</span>
          </button>
          <button type="button" onclick={restore} disabled={busy}>
            <ArchiveRestore aria-hidden="true" />
            <span>恢复资料库</span>
          </button>
          <button type="button" onclick={showStorage} disabled={busy}>
            <HardDrive aria-hidden="true" />
            <span>存储占用</span>
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
      <button class="library-remove-action" type="button" onclick={removeSelected} disabled={busy || selectedIds.size === 0}>
        <BookMinus aria-hidden="true" />
        <span>移出书架</span>
      </button>
      <button class="library-delete-action" type="button" onclick={deleteSelected} disabled={busy || selectedIds.size === 0}>
        <Trash2 aria-hidden="true" />
        <span>删除本地数据</span>
      </button>
    </div>
  {/if}

  <dialog class="library-storage-dialog" bind:this={storageDialog} onclose={() => (storageUsage = null)}>
    <header>
      <div>
        <HardDrive aria-hidden="true" />
        <h2>存储占用</h2>
      </div>
      <button type="button" aria-label="关闭存储占用" title="关闭" onclick={() => storageDialog?.close()}>
        <X aria-hidden="true" />
      </button>
    </header>
    {#if storageUsage}
      <dl>
        <div><dt>书籍文件</dt><dd>{formatBytes(storageUsage.booksBytes)}</dd></div>
        <div><dt>阅读缓存</dt><dd>{formatBytes(storageUsage.cacheBytes)}</dd></div>
        <div><dt>消息与快照</dt><dd>{formatBytes(storageUsage.messagesBytes)}</dd></div>
        <div><dt>离线词典</dt><dd>{formatBytes(storageUsage.dictionariesBytes)}</dd></div>
        <div><dt>阅读设置</dt><dd>{formatBytes(storageUsage.preferencesBytes)}</dd></div>
        <div class="library-storage-total"><dt>合计</dt><dd>{formatBytes(storageUsage.totalBytes)}</dd></div>
      </dl>
    {/if}
  </dialog>

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
