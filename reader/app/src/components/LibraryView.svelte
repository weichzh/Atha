<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    Archive,
    ArchiveRestore,
    BookMinus,
    BookOpen,
    Circle,
    CircleCheck,
    Download,
    Grid2X2,
    HardDrive,
    ImageOff,
    ImagePlus,
    Library,
    List as ListIcon,
    MessageSquareText,
    Plus,
    Search,
    Settings,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { onMount, tick } from "svelte";

  import MemoryCenter from "./MemoryCenter.svelte";
  import DictionarySettings from "./panels/DictionarySettings.svelte";

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
    importBookPaths,
    importBooks,
    importFailureMessage,
    libraryAvailable,
    listBooks,
    openBook,
    openFailureMessage,
    pendingLocalDataRestore,
    prepareLocalDataRestore,
    readAppTheme,
    readStartedBookIds,
    readStorageUsage,
    resetReaderApplicationPreferences,
    replaceLocalDataState,
    resumeBookDataDeletions,
    resetBookCover,
    removeBooksSerially,
    setBookCover,
    rollbackLocalDataRestore,
    takeStartupImport,
    validateLocalDataState,
    writeAppTheme,
    type AppTheme,
    type ImportReport,
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
  let storageDialog: HTMLDialogElement | undefined;
  let storageUsage: StorageUsage | null = null;
  let section: "library" | "memory" | "settings" = "library";
  let layout: "grid" | "list" = "grid";
  let dragActive = false;
  let visibleBooks: LibraryBook[] = [];
  let progressGroups = { reading: [] as LibraryBook[], unread: [] as LibraryBook[] };
  let allVisibleSelected = false;
  let appTheme = readAppTheme();
  let selectedBook: LibraryBook | null = null;
  let bookMenu: { book: LibraryBook; x: number; y: number } | null = null;
  let bookMenuElement: HTMLDivElement | undefined;
  let pressedBookId = "";
  let suppressedClickId = "";
  let activePress: {
    id: string;
    pointerId: number;
    x: number;
    y: number;
    timer: ReturnType<typeof globalThis.setTimeout>;
  } | null = null;
  let coverRevisions = new Map<string, number>();

  $: visibleBooks = filterLibraryBooks(books, query, view);
  $: progressGroups = startedBookIds
    ? groupLibraryBooksByProgress(visibleBooks, startedBookIds)
    : { reading: [], unread: [] };
  $: allVisibleSelected =
    visibleBooks.length > 0 && visibleBooks.every((book) => selectedIds.has(book.id));
  $: selectedBook = selectedIds.size === 1
    ? books.find((book) => selectedIds.has(book.id)) ?? null
    : null;

  function syncAppTheme() {
    document.documentElement.dataset.appTheme = appTheme;
    Reflect.get(globalThis, "AthaSystemBars")?.setReadingMode?.(
      false,
      appTheme === "dark" ||
        (appTheme === "system" && globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches),
    );
  }

  function chooseAppTheme(theme: AppTheme) {
    if (theme === appTheme) return;
    try {
      writeAppTheme(theme);
      appTheme = theme;
      syncAppTheme();
      status = "";
    } catch {
      status = "无法保存应用主题。";
    }
  }

  function resetReadingDefaults() {
    if (!confirm("恢复后，新打开的书会使用默认阅读主题、字号、字体和行距。本书设置不变。继续？")) {
      return;
    }
    status = resetReaderApplicationPreferences()
      ? "阅读默认已恢复。"
      : "无法恢复阅读默认，请先在阅读界面修复损坏的设置。";
  }

  onMount(() => {
    const darkScheme = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
    let mounted = true;
    let stopDragDrop: (() => void) | undefined;
    syncAppTheme();
    const syncSystemTheme = () => {
      if (appTheme === "system") syncAppTheme();
    };
    darkScheme?.addEventListener("change", syncSystemTheme);
    const syncSafeAreaInsets = () => {
      try {
        const systemBars = Reflect.get(globalThis, "AthaSystemBars");
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
    const closeMenuFromPointer = (event: PointerEvent) => {
      if (bookMenu && !bookMenuElement?.contains(event.target as Node)) bookMenu = null;
    };
    const closeMenuFromKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") bookMenu = null;
    };
    const closeMenu = () => (bookMenu = null);
    globalThis.addEventListener("pointerdown", closeMenuFromPointer);
    globalThis.addEventListener("keydown", closeMenuFromKey);
    globalThis.addEventListener("resize", closeMenu);
    globalThis.addEventListener("scroll", closeMenu, true);
    void initializeLibrary();
    if (libraryAvailable) {
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          if (!mounted) return;
          const payload = event.payload;
          if (payload.type === "enter") {
            dragActive = !loading && !busy && payload.paths.length > 0;
          } else if (payload.type === "over") {
            dragActive = !loading && !busy;
          } else if (payload.type === "drop") {
            dragActive = false;
            void addDroppedBooks(payload.paths);
          } else {
            dragActive = false;
          }
        })
        .then((unlisten) => {
          if (mounted) stopDragDrop = unlisten;
          else unlisten();
        })
        .catch(() => {
          if (mounted) status = "无法启用拖放导入，可继续使用导入按钮。";
        });
    }
    return () => {
      mounted = false;
      stopDragDrop?.();
      darkScheme?.removeEventListener("change", syncSystemTheme);
      globalThis.removeEventListener("atha-safe-area-change", syncSafeAreaInsets);
      globalThis.removeEventListener("pointerdown", closeMenuFromPointer);
      globalThis.removeEventListener("keydown", closeMenuFromKey);
      globalThis.removeEventListener("resize", closeMenu);
      globalThis.removeEventListener("scroll", closeMenu, true);
      cancelBookPress();
    };
  });

  async function initializeLibrary() {
    try {
      if (libraryAvailable) {
        await resumePendingRestore();
        await resumeBookDataDeletions();
      }
      books = await listBooks();
      refreshProgress();
      const startup = await takeStartupImport();
      if (startup?.bookId) {
        const book = books.find((item) => item.id === startup.bookId);
        if (book) {
          await read(book);
          return;
        }
      }
      if (startup?.failed) status = "无法打开关联的书籍，请改用导入按钮重试。";
    } catch {
      status = "无法完成本地资料恢复，请重新启动 Atha 后重试。";
      busy = true;
    } finally {
      loading = false;
    }
  }

  function refreshProgress() {
    startedBookIds = readStartedBookIds(books);
    if (startedBookIds === null && view === "progress") view = "default";
  }

  function setSection(next: "library" | "memory" | "settings") {
    if (loading || busy || next === section) return;
    if (selecting) cancelSelection();
    status = "";
    bookMenu = null;
    section = next;
  }

  async function chooseBooks() {
    if (busy) return;
    if (!libraryAvailable) {
      status = "请在 Atha 应用中选择 EPUB、CBZ、FB2、FBZ、Kindle、Markdown 或 TXT。";
      return;
    }
    await addBooks(importBooks);
  }

  async function addDroppedBooks(paths: string[]) {
    if (loading || busy || paths.length === 0) return;
    if (selecting) cancelSelection();
    section = "library";
    await addBooks(() => importBookPaths(paths));
  }

  async function addBooks(importer: () => Promise<ImportReport | null>) {
    busy = true;
    status = "";
    workLabel = "正在加入书架…";
    try {
      const report = await importer();
      if (!report) {
        status = "";
        return;
      }
      books = report.books;
      refreshProgress();
      if (report.failures.length === 0) {
        status = "已加入书架。";
      } else {
        status = [...new Set(report.failures.map((failure) => importFailureMessage(failure.code)))]
          .join("；");
      }
    } catch {
      status = "无法加入这些书籍。";
    } finally {
      workLabel = "";
      busy = false;
    }
  }

  async function backup() {
    if (busy) return;
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

  function cancelBookPress() {
    if (activePress) globalThis.clearTimeout(activePress.timer);
    activePress = null;
    pressedBookId = "";
  }

  function beginBookPress(event: PointerEvent, book: LibraryBook) {
    if (busy || selecting || event.pointerType === "mouse" || event.button !== 0) return;
    cancelBookPress();
    const timer = globalThis.setTimeout(() => {
      if (!activePress || activePress.id !== book.id) return;
      suppressedClickId = book.id;
      selecting = true;
      selectedIds = new Set([book.id]);
      pressedBookId = "";
    }, 500);
    activePress = {
      id: book.id,
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      timer,
    };
    pressedBookId = book.id;
  }

  function moveBookPress(event: PointerEvent) {
    if (
      activePress?.pointerId === event.pointerId &&
      Math.hypot(event.clientX - activePress.x, event.clientY - activePress.y) > 10
    ) cancelBookPress();
  }

  function activateBook(book: LibraryBook) {
    cancelBookPress();
    if (suppressedClickId === book.id) {
      suppressedClickId = "";
      return;
    }
    if (selecting) toggleSelection(book.id);
    else void read(book);
  }

  async function showBookMenu(book: LibraryBook, x: number, y: number) {
    cancelBookPress();
    bookMenu = {
      book,
      x: Math.max(8, Math.min(x, innerWidth - 228)),
      y: Math.max(8, Math.min(y, innerHeight - 356)),
    };
    await tick();
    bookMenuElement?.querySelector<HTMLButtonElement>("button")?.focus();
  }

  function handleBookContextMenu(event: MouseEvent, book: LibraryBook) {
    event.preventDefault();
    if (busy || event.button !== 2) return;
    void showBookMenu(book, event.clientX, event.clientY);
  }

  function handleBookKeydown(event: KeyboardEvent, book: LibraryBook) {
    if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return;
    event.preventDefault();
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    void showBookMenu(book, bounds.left + 12, bounds.top + 40);
  }

  function selectFromMenu(book: LibraryBook) {
    bookMenu = null;
    if (!selecting) {
      selecting = true;
      selectedIds = new Set([book.id]);
    } else {
      toggleSelection(book.id);
    }
  }

  function bumpCover(id: string) {
    const next = new Map(coverRevisions);
    next.set(id, (next.get(id) ?? 0) + 1);
    coverRevisions = next;
    const failed = new Set(failedCovers);
    failed.delete(id);
    failedCovers = failed;
  }

  async function changeCover(book: LibraryBook) {
    if (busy) return;
    bookMenu = null;
    busy = true;
    workLabel = "正在更新封面…";
    try {
      const updated = await setBookCover(book.id);
      if (!updated) return;
      books = updated;
      bumpCover(book.id);
      status = "封面已更新。";
    } catch (error) {
      status = String(error).includes("invalid-library-cover")
        ? "请选择有效的 JPEG、PNG 或 WebP 图片。"
        : "无法更新封面。";
    } finally {
      workLabel = "";
      busy = false;
    }
  }

  async function restoreCover(book: LibraryBook) {
    if (busy || !book.hasCustomCover) return;
    bookMenu = null;
    busy = true;
    try {
      books = await resetBookCover(book.id);
      bumpCover(book.id);
      status = "已恢复书籍内置封面。";
    } catch {
      status = "无法恢复书籍内置封面。";
    } finally {
      busy = false;
    }
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

  function removeSelected() {
    return removeBooks(books.filter((book) => selectedIds.has(book.id)).map((book) => book.id));
  }

  async function removeBooks(ids: string[]) {
    if (busy || ids.length === 0) return;
    if (!confirm(`从书架移出已选的 ${ids.length} 本书？阅读进度和导入内容会保留。`)) return;

    bookMenu = null;
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

  function deleteSelected() {
    return deleteBooks(books.filter((book) => selectedIds.has(book.id)).map((book) => book.id));
  }

  async function deleteBooks(ids: string[]) {
    if (busy || ids.length === 0) return;
    if (!confirm(`删除已选 ${ids.length} 本书的本地文件和阅读状态？笔记、标注消息和原文快照会保留。`)) {
      return;
    }
    bookMenu = null;
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
      class:pressed={pressedBookId === book.id}
      type="button"
      aria-label={selecting
        ? `${selectedIds.has(book.id) ? "取消选择" : "选择"}《${book.title}》`
        : undefined}
      aria-pressed={selecting ? selectedIds.has(book.id) : undefined}
      aria-haspopup="menu"
      onclick={() => activateBook(book)}
      onpointerdown={(event) => beginBookPress(event, book)}
      onpointermove={moveBookPress}
      onpointerup={cancelBookPress}
      onpointercancel={cancelBookPress}
      oncontextmenu={(event) => handleBookContextMenu(event, book)}
      onkeydown={(event) => handleBookKeydown(event, book)}
      disabled={busy}
    >
      <span class="library-cover">
        {#if book.hasCover && !failedCovers.has(book.id)}
          <img
            src={coverUrl(book.id, coverRevisions.get(book.id) ?? 0)}
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
  class:library-selecting={selecting && section === "library"}
  class:library-list-view={layout === "list" && section === "library"}
  class="library-shell"
  data-app-theme={appTheme}
  aria-label="Atha 资料库"
  aria-busy={loading || busy}
>
  <header class="library-header">
    <nav class="library-sections" aria-label="资料库主导航">
      <button
        type="button"
        class:active={section === "library"}
        aria-current={section === "library" ? "page" : undefined}
        onclick={() => setSection("library")}
        disabled={loading || busy}
      >
        <Library aria-hidden="true" />
        <span>书架</span>
      </button>
      <button
        type="button"
        class:active={section === "memory"}
        aria-current={section === "memory" ? "page" : undefined}
        onclick={() => setSection("memory")}
        disabled={loading || busy}
      >
        <MessageSquareText aria-hidden="true" />
        <span>阅读记忆</span>
      </button>
      <button
        type="button"
        class:active={section === "settings"}
        aria-current={section === "settings" ? "page" : undefined}
        onclick={() => setSection("settings")}
        disabled={loading || busy}
      >
        <Settings aria-hidden="true" />
        <span>设置</span>
      </button>
    </nav>

    {#if section === "library"}
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
          <div class="library-layout-switch" role="group" aria-label="书架布局">
            <button
              type="button"
              aria-label="网格视图"
              title="网格视图"
              aria-pressed={layout === "grid"}
              onclick={() => (layout = "grid")}
              disabled={loading || busy}
            >
              <Grid2X2 aria-hidden="true" />
            </button>
            <button
              type="button"
              aria-label="列表视图"
              title="列表视图"
              aria-pressed={layout === "list"}
              onclick={() => (layout = "list")}
              disabled={loading || busy}
            >
              <ListIcon aria-hidden="true" />
            </button>
          </div>
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
    {/if}
  </header>

  {#if section === "memory"}
    <MemoryCenter {books} disabled={loading || busy} />
  {:else if section === "settings"}
    <section class="library-settings" aria-labelledby="library-settings-heading">
      <header class="library-settings-heading">
        <h1 id="library-settings-heading">设置</h1>
      </header>

      <section class="library-settings-group" aria-labelledby="appearance-settings-heading">
        <h2 id="appearance-settings-heading">应用外观</h2>
        <div class="library-setting-row">
          <span>界面主题</span>
          <div class="library-theme-control" role="radiogroup" aria-label="应用界面主题">
            {#each [
              ["system", "跟随系统"],
              ["light", "浅色"],
              ["dark", "深色"],
            ] as [theme, label]}
              <button
                type="button"
                role="radio"
                aria-checked={appTheme === theme}
                onclick={() => chooseAppTheme(theme as AppTheme)}
                disabled={busy}
              >{label}</button>
            {/each}
          </div>
        </div>
      </section>

      <section class="library-settings-group" aria-labelledby="reading-defaults-settings-heading">
        <h2 id="reading-defaults-settings-heading">阅读默认</h2>
        <p class="library-settings-description">
          新打开的书沿用阅读界面保存的书页主题、字号、字体和行距；当前书的布局与样式仍在阅读时调整。
        </p>
        <div class="library-data-actions">
          <button type="button" onclick={resetReadingDefaults} disabled={busy}>
            <BookOpen aria-hidden="true" />
            <span>恢复阅读默认</span>
          </button>
        </div>
      </section>

      <DictionarySettings disabled={busy} />

      <section class="library-settings-group" aria-labelledby="local-data-settings-heading">
        <h2 id="local-data-settings-heading">本地资料</h2>
        <div class="library-data-actions">
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
      </section>
    </section>
  {:else}
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
      <button type="button" onclick={() => selectedBook && changeCover(selectedBook)} disabled={busy || !selectedBook}>
        <ImagePlus aria-hidden="true" />
        <span>封面</span>
      </button>
      {#if selectedBook?.hasCustomCover}
        <button type="button" onclick={() => selectedBook && restoreCover(selectedBook)} disabled={busy}>
          <ImageOff aria-hidden="true" />
          <span>恢复</span>
        </button>
      {/if}
      <button class="library-remove-action" type="button" onclick={removeSelected} disabled={busy || selectedIds.size === 0}>
        <BookMinus aria-hidden="true" />
        <span>移出</span>
      </button>
      <button class="library-delete-action" type="button" onclick={deleteSelected} disabled={busy || selectedIds.size === 0}>
        <Trash2 aria-hidden="true" />
        <span>删除</span>
      </button>
    </div>
  {/if}
  {/if}

  {#if bookMenu}
    <div
      bind:this={bookMenuElement}
      class="library-book-menu"
      role="menu"
      aria-label={`《${bookMenu.book.title}》操作`}
      style={`left: ${bookMenu.x}px; top: ${bookMenu.y}px`}
    >
      <button type="button" role="menuitem" onclick={() => { const book = bookMenu?.book; bookMenu = null; if (book) void read(book); }}>
        <BookOpen aria-hidden="true" /><span>打开</span>
      </button>
      <button type="button" role="menuitem" onclick={() => bookMenu && selectFromMenu(bookMenu.book)}>
        <CircleCheck aria-hidden="true" /><span>{selecting && selectedIds.has(bookMenu.book.id) ? "取消选择" : "选择"}</span>
      </button>
      <button type="button" role="menuitem" onclick={() => bookMenu && changeCover(bookMenu.book)}>
        <ImagePlus aria-hidden="true" /><span>更换封面</span>
      </button>
      {#if bookMenu.book.hasCustomCover}
        <button type="button" role="menuitem" onclick={() => bookMenu && restoreCover(bookMenu.book)}>
          <ImageOff aria-hidden="true" /><span>恢复内置封面</span>
        </button>
      {/if}
      <button type="button" role="menuitem" onclick={() => bookMenu && removeBooks([bookMenu.book.id])}>
        <BookMinus aria-hidden="true" /><span>移出书架</span>
      </button>
      <button class="danger" type="button" role="menuitem" onclick={() => bookMenu && deleteBooks([bookMenu.book.id])}>
        <Trash2 aria-hidden="true" /><span>删除本地数据</span>
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

  {#if dragActive}
    <div class="library-drop-overlay" role="status" aria-live="polite">
      <Download aria-hidden="true" />
      <strong>松开以加入书架</strong>
      <span>EPUB、CBZ、FB2、FBZ、Kindle、Markdown 或 TXT</span>
    </div>
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
