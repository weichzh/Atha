<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    Check,
    CheckSquare,
    Filter,
    Heart,
    Image,
    LayoutGrid,
    List,
    Plus,
    RotateCcw,
    Search,
    Tag,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { Dialog, Popover, Switch } from "bits-ui";
  import { onMount, tick } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import {
    abortLocalDataRestore,
    finishLocalDataRestore,
    importBookPaths,
    importFailureMessage,
    libraryAvailable,
    openBook,
    openFailureMessage,
    pendingLocalDataRestore,
    replaceLocalDataState,
    resumeBookDataDeletions,
    rollbackLocalDataRestore,
    takeStartupImport,
    type LibraryBook,
  } from "../../library.ts";
  import { registerBackHandler } from "../data/back-stack.ts";
  import { isTauriRuntime } from "../data/runtime.ts";
  import {
    browseShelf,
    emptyFilters,
    facetKeys,
    facetLabel,
    filterCount,
    indexShelf,
    parseOrganizationField,
    quickShelfFacets,
    recentShelf,
    shelfFacetOptions,
    shelfFacets,
    shelfSorts,
    toggleFacet,
    type Facet,
    type ShelfEntry,
    type ShelfFilters,
    type ShelfLayout,
    type ShelfSort,
  } from "../../shelf-browse.ts";
  import { readShelfReading } from "../../shelf-reading.ts";
  import {
    emptyShelfPosition,
    readShelfSession,
    writeShelfSession,
    type ShelfOrigin,
  } from "../../shelf-session.ts";
  import {
    applyShelfRemoval,
    importLibraryBooks,
    listLibraryBooks,
    resetLibraryBookCover,
    setLibraryBookCover,
    updateLibraryBookOrganization,
  } from "../data/library-adapter.ts";
  import { uiPrefs } from "../data/ui-prefs.svelte.ts";
  import ContinueReadingDock from "./ContinueReadingDock.svelte";
  import BookCover from "./BookCover.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  interface Props {
    active: boolean;
    onOpenReader: (book: LibraryBook, origin: ShelfOrigin, trigger?: HTMLElement) => Promise<void>;
    onBooksUpdated?: (books: LibraryBook[]) => void;
    onSelectingChange: (selecting: boolean) => void;
  }

  let { active, onOpenReader, onBooksUpdated, onSelectingChange }: Props = $props();

  let books = $state<LibraryBook[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let dragActive = $state(false);
  let sessionReady = $state(false);
  let shelfLoadGeneration = 0;
  let searchQuery = $state("");
  let filters = $state<ShelfFilters>(emptyFilters());
  let sort = $state<ShelfSort>("title");
  let layout = $state<ShelfLayout>("grid");
  let filterPopoverOpen = $state(false);
  let searchInput = $state<HTMLInputElement>();
  let coverRevision = $state<Record<string, number>>({});
  let statusMessage = $state<string | null>(null);

  // Scroll containers for C4 continuity
  let pageEl = $state<HTMLElement | null>(null);
  let recentScroll = $state(0);
  let recentDrawerOpen = $state(false);
  let tagsStripEl = $state<HTMLElement | null>(null);

  let selecting = $state(false);
  let selectedIds = $state<ReadonlySet<string>>(new Set());
  let batchBusy = $state(false);
  let selectionToggle = $state<HTMLButtonElement>();
  let selectionOrigin: HTMLElement | undefined;
  let press: { id: number; x: number; y: number; target: HTMLElement } | undefined;
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let ignoreClick: HTMLElement | undefined;

  // Edit Organization Dialog
  let editBook = $state<LibraryBook | null>(null);
  let editFavorite = $state(false);
  let editTagsText = $state("");
  let editSubjectsText = $state("");
  let editSeriesText = $state("");
  let editDialogOpen = $state(false);

  // Confirm Dialog for remove / delete
  let confirmState = $state<{
    open: boolean;
    mode: "remove" | "delete";
    targets: LibraryBook[];
  }>({ open: false, mode: "remove", targets: [] });
  const confirmTarget = $derived(confirmState.targets.length === 1
    ? `《${confirmState.targets[0].title}》`
    : `选中的 ${confirmState.targets.length} 册书`);

  const readingMap = $derived(readShelfReading(books));
  const entries = $derived(indexShelf(books, readingMap));
  const recentEntries = $derived(recentShelf(entries));
  const quickFacets = $derived(quickShelfFacets(books));
  const filteredEntries = $derived(browseShelf(entries, searchQuery, filters, sort));
  const selectedBooks = $derived(
    filteredEntries.filter(({ book }) => selectedIds.has(book.id)).map(({ book }) => book),
  );
  const singleBook = $derived(selectedBooks.length === 1 ? selectedBooks[0] : undefined);
  const favoriteLabel = $derived(selectedBooks.every((book) => book.favorite) ? "取消收藏" : "收藏");
  const facetSummary = $derived(shelfFacetOptions(entries, searchQuery, filters));
  const activeFilterCount = $derived(filterCount(filters));

  let statusTimer: ReturnType<typeof setTimeout> | null = null;
  function notify(msg: string) {
    statusMessage = msg;
    if (statusTimer) clearTimeout(statusTimer);
    statusTimer = setTimeout(() => {
      statusMessage = null;
    }, 2600);
  }

  function persistSession(origin?: ShelfOrigin) {
    const pos = emptyShelfPosition();
    pos.y = pageEl?.scrollTop ?? window.scrollY ?? 0;
    pos.recentX = recentScroll;
    pos.tagsX = tagsStripEl?.scrollLeft ?? 0;
    if (origin) {
      pos.anchor = origin;
      pos.focus = origin;
    }
    writeShelfSession({
      query: searchQuery,
      filters,
      sort,
      layout,
      position: pos,
    });
  }

  export function rememberOpenOrigin(origin: ShelfOrigin) {
    persistSession(origin);
  }

  async function restoreSessionAndFocus() {
    const saved = readShelfSession();
    searchQuery = saved.query;
    filters = saved.filters;
    sort = saved.sort;
    layout = saved.layout;
    await tick();
    await tick();
    if (saved.position.focus) {
      const target = findOpenTrigger(saved.position.focus);
      if (target) {
        if (saved.position.focus.area === "catalog" && saved.position.focus.offset > 0) {
          window.scrollBy(0, target.getBoundingClientRect().top - saved.position.focus.offset);
          await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));
          const drift = target.getBoundingClientRect().top - saved.position.focus.offset;
          if (Math.abs(drift) > 1) window.scrollBy(0, drift);
        }
        target.focus({ preventScroll: true });
      }
    } else if (saved.position.y > 0) {
      window.scrollTo({ top: saved.position.y, behavior: "instant" as ScrollBehavior });
    }
    recentScroll = saved.position.recentX;
    if (tagsStripEl && saved.position.tagsX > 0) {
      tagsStripEl.scrollLeft = saved.position.tagsX;
    }
    sessionReady = true;
  }

  async function loadShelf(isReturn = false) {
    const generation = ++shelfLoadGeneration;
    loading = true;
    sessionReady = false;
    loadFailed = false;
    try {
      if (!isReturn && isTauriRuntime() && libraryAvailable) {
        const pending = await pendingLocalDataRestore();
        if (generation !== shelfLoadGeneration) return;
        if (pending) {
          if (pending.rollback) {
            replaceLocalDataState(pending.browserState);
            await abortLocalDataRestore(pending.token);
          } else {
            try {
              replaceLocalDataState(pending.browserState);
              await finishLocalDataRestore(pending.token);
            } catch {
              const rolledBack = await rollbackLocalDataRestore(pending.token);
              replaceLocalDataState(rolledBack);
            }
          }
          if (generation !== shelfLoadGeneration) return;
        }
        await resumeBookDataDeletions();
        if (generation !== shelfLoadGeneration) return;
      }

      const loaded = await listLibraryBooks();
      if (generation !== shelfLoadGeneration) return;
      books = loaded;
      onBooksUpdated?.(books);
      loading = false;
      await restoreSessionAndFocus();
      if (generation !== shelfLoadGeneration) return;

      if (!isReturn && isTauriRuntime() && libraryAvailable) {
        const startup = await takeStartupImport();
        if (generation !== shelfLoadGeneration) return;
        if (startup?.bookId) {
          const target = books.find((b) => b.id === startup.bookId);
          if (target) {
            const cardEl = document.querySelector<HTMLElement>(
              `.shelf-catalog [data-shelf-open="${target.id}"]`,
            );
            persistSession({
              id: target.id,
              area: "catalog",
              offset: cardEl ? cardEl.getBoundingClientRect().top : 0,
            });
          }
          await openBook(startup.bookId);
          return;
        }
        if (startup?.failed) {
          notify("未能识别或导入所选文件");
        }
      }
    } catch {
      if (generation !== shelfLoadGeneration) return;
      loadFailed = true;
      loading = false;
      notify("无法加载本地书库");
    }
  }

  onMount(() => {
    let disposed = false;
    let unlistenDrop: (() => void) | null = null;
    const onPageShow = (event: PageTransitionEvent) => {
      if (event.persisted) {
        void loadShelf(true);
      }
    };
    window.addEventListener("pageshow", onPageShow);
    const unregisterBack = registerBackHandler((source) => {
      if (!active) return false;
      if (recentDrawerOpen && source === "back") {
        recentDrawerOpen = false;
        return true;
      }
      if (batchBusy) return true;
      if (confirmState.open || editDialogOpen || filterPopoverOpen) {
        if (source === "escape") return false; // Let Bits UI dismiss its own top layer.
        if (confirmState.open) confirmState.open = false;
        else if (editDialogOpen) editDialogOpen = false;
        else filterPopoverOpen = false;
        return true;
      }
      if (!selecting) {
        if (source === "back" && (searchQuery || document.activeElement === searchInput)) {
          searchQuery = "";
          searchInput?.blur();
          return true;
        }
        return false;
      }
      exitSelection(true);
      return true;
    });

    if (isTauriRuntime() && libraryAvailable) {
      void getCurrentWebview()
        .onDragDropEvent(async (event) => {
          if (disposed) return;
          const payload = event.payload;
          if (payload.type === "enter" || payload.type === "over") {
            dragActive = true;
          } else if (payload.type === "leave") {
            dragActive = false;
          } else if (payload.type === "drop") {
            dragActive = false;
            if (loading || !payload.paths.length) return;
            try {
              const report = await importBookPaths(payload.paths);
              books = report.books;
              onBooksUpdated?.(books);
              if (report.failures.length) {
                notify(importFailureMessage(report.failures[0].code));
              } else {
                notify("已导入拖放的书籍");
              }
            } catch (err) {
              notify(openFailureMessage(err));
            }
          }
        })
        .then((fn) => {
          if (disposed) fn();
          else unlistenDrop = fn;
        })
        .catch(() => {});
    }

    void loadShelf(false);

    return () => {
      disposed = true;
      window.removeEventListener("pageshow", onPageShow);
      unlistenDrop?.();
      unregisterBack();
      cancelHold();
      onSelectingChange(false);
    };
  });

  $effect(() => {
    if (!loading && sessionReady) {
      persistSession();
    }
  });

  function findOpenTrigger(origin: ShelfOrigin, trigger?: HTMLElement): HTMLElement | null {
    if (trigger?.isConnected && trigger.getClientRects().length && !trigger.closest("[inert]")) return trigger;
    const id = CSS.escape(origin.id);
    const find = (selector: string) => [...(pageEl?.querySelectorAll<HTMLElement>(selector) ?? [])]
      .find((el) => el.getClientRects().length && !el.closest("[inert]"));
    return find(`[data-shelf-area="${origin.area}"][data-book-id="${id}"]`)
      ?? (origin.area === "recent" ? find("[data-shelf-recent-return]") : undefined)
      ?? find(`[data-shelf-open="${id}"]`) ?? null;
  }

  export function restoreOpenFocus(origin: ShelfOrigin, trigger?: HTMLElement) {
    findOpenTrigger(origin, trigger)?.focus({ preventScroll: true });
  }

  /** Refresh progress after return, preserving the originating row if the sort reorders it. */
  export async function refreshSilent(origin?: ShelfOrigin) {
    try {
      const loaded = await listLibraryBooks();
      const target = origin && active ? findOpenTrigger(origin) : null;
      const offset = target?.getBoundingClientRect().top;
      const focusWasOnTarget = document.activeElement === target;
      const scrollY = window.scrollY;
      books = loaded;
      onBooksUpdated?.(books);
      if (origin && origin.area === "catalog" && offset !== undefined && focusWasOnTarget) {
        await tick();
        if (!active || window.scrollY !== scrollY) return;
        const next = findOpenTrigger(origin);
        if (next && (document.activeElement === target || document.activeElement === document.body)) {
          window.scrollBy({ top: next.getBoundingClientRect().top - offset, behavior: "instant" as ScrollBehavior });
          next.focus({ preventScroll: true });
        }
      }
    } catch {
      // Keep cached books intact on background refresh failure
    }
  }

  $effect(() => {
    if (!active) {
      recentDrawerOpen = false;
      cancelHold();
      if (selecting) exitSelection();
    }
  });

  $effect(() => {
    if (selectedIds.size === selectedBooks.length) return;
    selectedIds = new Set(selectedBooks.map((book) => book.id));
  });

  function enterSelection(book?: LibraryBook, trigger?: HTMLElement) {
    if (!selecting) {
      selectionOrigin = trigger;
      onSelectingChange(true);
    }
    selecting = true;
    if (book) selectedIds = new Set([...selectedIds, book.id]);
  }

  function exitSelection(restoreFocus = false) {
    selecting = false;
    onSelectingChange(false);
    selectedIds = new Set();
    const origin = selectionOrigin;
    selectionOrigin = undefined;
    if (restoreFocus) {
      void tick().then(() => {
        if (active && !selecting) {
          (origin?.isConnected ? origin : selectionToggle)?.focus({ preventScroll: true });
        }
      });
    }
  }

  function selectBook(id: string) {
    const next = new Set(selectedIds);
    if (!next.delete(id)) next.add(id);
    selectedIds = next;
  }

  function cancelHold() {
    clearTimeout(holdTimer);
    press = undefined;
  }

  function startBookHold(book: LibraryBook, event: PointerEvent) {
    ignoreClick = undefined;
    if (selecting || !event.isPrimary || event.button !== 0 || event.pointerType === "mouse") return;
    cancelHold();
    const target = event.currentTarget as HTMLElement;
    press = { id: event.pointerId, x: event.clientX, y: event.clientY, target };
    holdTimer = setTimeout(() => {
      if (!active || !target.isConnected) return;
      ignoreClick = target;
      enterSelection(book, target);
    }, 420);
  }

  function checkHoldMovement(event: PointerEvent) {
    if (press?.id === event.pointerId && Math.hypot(event.clientX - press.x, event.clientY - press.y) > 10) {
      cancelHold();
    }
  }

  function endBookHold(event: PointerEvent) {
    if (press?.id === event.pointerId) cancelHold();
  }

  function selectFromContext(book: LibraryBook, event: MouseEvent | KeyboardEvent) {
    if (event instanceof KeyboardEvent && event.key !== "ContextMenu" &&
      !(event.shiftKey && event.key === "F10")) return;
    event.preventDefault();
    if (batchBusy) return;
    const target = event.currentTarget as HTMLElement;
    if (press?.target === target) {
      ignoreClick = target;
      cancelHold();
    }
    enterSelection(book, target);
  }

  function handleCatalogClick(entry: ShelfEntry, event: MouseEvent) {
    const target = event.currentTarget as HTMLButtonElement;
    if (event.detail > 0 && ignoreClick === target) {
      ignoreClick = undefined;
      return;
    }
    if (selecting) {
      if (!batchBusy) selectBook(entry.book.id);
    } else {
      void handleOpenBook(entry, "catalog", target);
    }
  }

  async function handleOpenBook(entry: ShelfEntry, area: "catalog" | "recent", triggerEl?: HTMLElement) {
    const origin: ShelfOrigin = {
      id: entry.book.id,
      area,
      offset: triggerEl ? triggerEl.getBoundingClientRect().top : 0,
    };
    try {
      await onOpenReader(entry.book, origin, triggerEl);
    } catch (err) {
      notify(openFailureMessage(err));
    }
  }

  async function handleImport() {
    const report = await importLibraryBooks();
    if (!report) return;
    books = report.books;
    onBooksUpdated?.(books);
    notify(report.failures.length ? `导入完成（${report.failures.length} 本未能识别）` : "已导入新书至书架");
  }

  async function handleBatchFavorite() {
    if (batchBusy || selectedBooks.length === 0) return;
    const targets = [...selectedBooks];
    const favorite = targets.some((book) => !book.favorite);
    let completed = 0;
    batchBusy = true;
    try {
      for (const book of targets) {
        books = await updateLibraryBookOrganization(book.id, { favorite });
        completed++;
      }
      exitSelection(true);
      notify(favorite ? `已收藏 ${completed} 册` : `已取消收藏 ${completed} 册`);
    } catch {
      selectedIds = new Set(targets.slice(completed).map((book) => book.id));
      notify(`已更新 ${completed} 册收藏状态，剩余未处理。请重试。`);
    } finally {
      batchBusy = false;
      onBooksUpdated?.(books);
    }
  }

  function openEditDialog(book: LibraryBook) {
    editBook = book;
    editFavorite = Boolean(book.favorite);
    editTagsText = (book.tags ?? []).join("\n");
    editSubjectsText = (book.subjects ?? []).join("\n");
    editSeriesText = (book.series ?? []).join("\n");
    editDialogOpen = true;
  }

  async function saveEditDialog(event: SubmitEvent) {
    event.preventDefault();
    if (!editBook) return;
    books = await updateLibraryBookOrganization(editBook.id, {
      favorite: editFavorite,
      tags: parseOrganizationField(editTagsText),
      subjects: parseOrganizationField(editSubjectsText),
      series: parseOrganizationField(editSeriesText),
    });
    onBooksUpdated?.(books);
    editDialogOpen = false;
    notify("已保存分类信息");
  }

  async function handleSetCover(book: LibraryBook) {
    const updated = await setLibraryBookCover(book.id);
    if (updated) {
      books = updated;
      coverRevision = { ...coverRevision, [book.id]: (coverRevision[book.id] ?? 0) + 1 };
      onBooksUpdated?.(books);
      notify("已更换封面");
    }
  }

  async function handleResetCover(book: LibraryBook) {
    books = await resetLibraryBookCover(book.id);
    coverRevision = { ...coverRevision, [book.id]: (coverRevision[book.id] ?? 0) + 1 };
    onBooksUpdated?.(books);
    editBook = books.find((item) => item.id === book.id) ?? book;
    notify("已恢复默认封面");
  }

  function requestBookConfirm(mode: "remove" | "delete") {
    if (batchBusy || selectedBooks.length === 0) return;
    confirmState = { open: true, mode, targets: [...selectedBooks] };
  }

  async function executeBookConfirm() {
    if (batchBusy) return;
    const { mode, targets } = confirmState;
    if (targets.length === 0) return;
    batchBusy = true;
    try {
      const result = await applyShelfRemoval(targets.map(({ id }) => id), mode);
      if (result.books) {
        books = result.books;
        onBooksUpdated?.(books);
      }
      const verb = mode === "remove" ? "移出" : "删除";
      if (result.pendingRecovery) {
        exitSelection(true);
        notify(`已${verb} ${result.removedIds.length} 册；删除未完成，请重开应用恢复后再继续。`);
      } else if (result.remainingIds.length > 0) {
        selectedIds = new Set(result.remainingIds);
        notify(`已${verb} ${result.removedIds.length} 册，剩余未处理。请重试。`);
      } else {
        exitSelection(true);
        notify(`已${verb} ${result.removedIds.length} 册`);
      }
    } finally {
      batchBusy = false;
    }
  }

  function toggleFilterFacet(key: Facet, value: string) {
    filters = toggleFacet(filters, key, value);
  }

  function clearAllFilters() {
    searchQuery = "";
    filters = emptyFilters();
  }

  function formatDateBadge(dateStr: string | null | undefined): string {
    if (!dateStr) return "";
    return dateStr.slice(5).replace("-", "/");
  }

</script>

<svelte:window
  onpointermove={checkHoldMovement}
  onpointerup={endBookHold}
  onpointercancel={endBookHold}
  onblur={cancelHold}
/>

<main
  class="library-shell"
  class:selecting
  class:has-resume={!selecting && recentEntries.length > 0}
  bind:this={pageEl}
  aria-label="书架"
  aria-busy={loading || batchBusy}
>
  {#if dragActive}
    <div class="drop-scrim glass-popover" role="status">松开鼠标，立即导入书籍至书架</div>
  {/if}
  <header class="shelf-header">
    <div class="header-titles">
      <h1>书架</h1>
      <button type="button" class="accent-btn import-btn" onclick={handleImport}>
        <Plus size={16} />
        <span>导入</span>
      </button>
    </div>

    <div class="find-bar">
      <div class="well-input search-pill">
        <Search size={16} strokeWidth={1.8} class="search-icon" />
        <input
          type="search"
          aria-label="搜索书名、作者、标签或系列"
          placeholder="搜索藏书…"
          bind:value={searchQuery}
          bind:this={searchInput}
        />
        {#if searchQuery}
          <button
            type="button"
            class="clear-search"
            aria-label="清空搜索"
            onclick={() => (searchQuery = "")}
          >
            <X size={14} />
          </button>
        {/if}
      </div>

      <div class="bar-controls">
        <Popover.Root bind:open={filterPopoverOpen}>
          <Popover.Trigger class="pill-btn filter-trigger {activeFilterCount > 0 ? 'active-accent' : ''}" aria-label="筛选藏书">
            <Filter size={15} />
            <span class="filter-label">筛选</span>
            {#if activeFilterCount > 0}
              <span class="count-badge tabular">{activeFilterCount}</span>
            {/if}
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content sideOffset={10} align="end" class="glass-popover filter-popover">
              <div class="filter-head">
                <span class="filter-title">筛选藏书</span>
                <span class="filter-matches tabular">匹配 {facetSummary.matches} 册</span>
              </div>

              <div class="filter-favorite-row">
                <label class="switch-label" for="shelf-fav-switch">
                  <Heart size={15} />
                  <span>仅看收藏书籍</span>
                </label>
                <Switch.Root
                  id="shelf-fav-switch"
                  checked={filters.favorite}
                  onCheckedChange={(checked) => (filters = { ...filters, favorite: checked })}
                  class="atha-switch"
                >
                  <Switch.Thumb class="atha-switch-thumb" />
                </Switch.Root>
              </div>

              {#each facetSummary.groups as group (group.key)}
                {#if group.options.length > 0}
                  <div class="facet-group">
                    <div class="facet-label">{shelfFacets[group.key]}</div>
                    <div class="facet-pills">
                      {#each group.options as option (option.value)}
                        {@const selected = filters[group.key].includes(option.value)}
                        <button
                          type="button"
                          class="facet-pill"
                          class:selected
                          onclick={() => toggleFilterFacet(group.key, option.value)}
                        >
                          <span>{facetLabel(group.key, option.value)}</span>
                          <span class="facet-count tabular">{option.count}</span>
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              {/each}

              {#if activeFilterCount > 0}
                <div class="filter-footer">
                  <button type="button" class="pill-btn" onclick={clearAllFilters}>
                    清除全部条件
                  </button>
                </div>
              {/if}
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>

        <button
          type="button"
          class="icon-btn shelf-layout-toggle"
          aria-label={layout === "grid" ? "清单列表" : "封面网格"}
          title={layout === "grid" ? "切换清单列表" : "切换封面网格"}
          onclick={() => (layout = layout === "grid" ? "list" : "grid")}
        >
          {#if layout === "grid"}<List size={17} />{:else}<LayoutGrid size={17} />{/if}
        </button>

        <button
          type="button"
          class="pill-btn select-trigger"
          class:active-accent={selecting}
          aria-pressed={selecting}
          disabled={loading || batchBusy}
          bind:this={selectionToggle}
          onclick={() => selecting ? exitSelection() : enterSelection()}
        >
          {#if selecting}<X size={15} />{:else}<CheckSquare size={15} />{/if}
          <span>{selecting ? "取消选择" : "选择"}</span>
        </button>

      </div>
    </div>

    <div class="sort-strip" role="group" aria-label="书架排序">
      {#each shelfSorts as [value, label] (value)}
        <button
          type="button"
          class="sort-chip"
          aria-pressed={sort === value}
          onclick={() => (sort = value)}
        >{label}</button>
      {/each}
      <button
        type="button"
        class="sort-chip favorite-toggle"
        aria-pressed={filters.favorite}
        onclick={() => (filters = { ...filters, favorite: !filters.favorite })}
      ><Heart size={14} /> 收藏</button>
    </div>
  </header>

  {#if selecting}
    <div class="shelf-actions glass-island" role="group" aria-label="选中书籍操作">
      <div class="selection-summary">
        <span class="selection-count tabular" aria-live="polite">已选 {selectedBooks.length} 册</span>
        <button type="button" class="shelf-action selection-close" aria-label="退出选择" disabled={batchBusy} onclick={() => exitSelection(true)}>
          <X size={17} />
        </button>
      </div>
      <div class="selection-options">
        {#if selectedBooks.length === 0}
          <span class="selection-hint">点按书籍，可选多册</span>
        {:else}
          <button type="button" class="shelf-action" disabled={batchBusy} onclick={handleBatchFavorite}>
            <Heart size={17} />
            <span>{favoriteLabel}</span>
          </button>
          {#if singleBook}
            <button type="button" class="shelf-action" disabled={batchBusy} onclick={() => openEditDialog(singleBook!)}>
              <Tag size={17} />
              <span>编辑信息</span>
            </button>
            <button type="button" class="shelf-action" disabled={batchBusy} onclick={() => void handleSetCover(singleBook!)}>
              <Image size={17} />
              <span>换封面</span>
            </button>
          {/if}
          <button type="button" class="shelf-action" disabled={batchBusy} onclick={() => requestBookConfirm("remove")}>
            <X size={17} />
            <span>移出</span>
          </button>
          <button type="button" class="shelf-action danger" disabled={batchBusy} onclick={() => requestBookConfirm("delete")}>
            <Trash2 size={17} />
            <span>删除</span>
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if activeFilterCount > 0 || searchQuery.trim()}
    <div class="active-conditions" aria-label="已选条件">
      {#if searchQuery.trim()}
        <button type="button" class="condition-chip" onclick={() => (searchQuery = "")}>
          <span>搜索：{searchQuery.trim()}</span>
          <X size={13} />
        </button>
      {/if}
      {#if filters.favorite}
        <button
          type="button"
          class="condition-chip"
          onclick={() => (filters = { ...filters, favorite: false })}
        >
          <span>仅看收藏</span>
          <X size={13} />
        </button>
      {/if}
      {#each facetKeys as key (key)}
        {#each filters[key] as val (val)}
          <button
            type="button"
            class="condition-chip"
            onclick={() => toggleFilterFacet(key, val)}
          >
            <span>{shelfFacets[key]}：{facetLabel(key, val)}</span>
            <X size={13} />
          </button>
        {/each}
      {/each}
      <button type="button" class="clear-all-link" onclick={clearAllFilters}>重置全部</button>
    </div>
  {/if}

  {#if statusMessage}
    <div class="toast-banner glass-island" role="status">{statusMessage}</div>
  {/if}

  {#if !selecting && recentEntries.length}
    <ContinueReadingDock
      entries={recentEntries}
      collapsed={uiPrefs.state.shelfDockCollapsed}
      compact={Boolean(searchQuery.trim()) || activeFilterCount > 0}
      bind:open={recentDrawerOpen}
      bind:scrollX={recentScroll}
      {coverRevision}
      onRead={(entry, trigger) => void handleOpenBook(entry, "recent", trigger)}
      onCollapse={(collapsed) => uiPrefs.update({ shelfDockCollapsed: collapsed })}
    />
  {/if}

  {#if quickFacets.length > 0}
    <div class="quick-facets" bind:this={tagsStripEl} aria-label="常用分类">
      {#each quickFacets as item (`${item.key}:${item.value}`)}
        {@const active = filters[item.key].includes(item.value)}
        <button
          type="button"
          class="quick-pill"
          class:active
          onclick={() => toggleFilterFacet(item.key, item.value)}
        >
          {item.value}
        </button>
      {/each}
    </div>
  {/if}

  <section class="shelf-section" aria-label="全部藏书">
    <div class="section-head">
      <h2>{searchQuery.trim() || activeFilterCount > 0 ? "筛选结果" : "全部藏书"}</h2>
      <span class="section-hint tabular">{filteredEntries.length} 册</span>
    </div>

    {#if loading && books.length === 0}
      <div class="empty-shelf">正在整理书房藏书…</div>
    {:else if loadFailed && filteredEntries.length === 0}
      <div class="empty-shelf">
        <p>未能读取本地书架，请检查资料目录状态后重试。</p>
        <button type="button" class="pill-btn" onclick={() => void loadShelf(false)}>重新加载</button>
      </div>
    {:else if filteredEntries.length === 0}
      <div class="empty-shelf">
        <p>没有符合当前条件的书籍。</p>
        <button type="button" class="pill-btn" onclick={clearAllFilters}>显示全部藏书</button>
      </div>
    {:else if layout === "grid"}
      <div
        class="book-grid shelf-catalog"
        data-density={uiPrefs.state.shelfDesktopDensity}
        style:--shelf-mobile-cols={uiPrefs.state.shelfMobileColumns}
      >
        {#each filteredEntries as entry, idx (entry.book.id)}
          <article class="book-card">
            <div class="book-cover-hit">
              <button
                type="button"
                class="book-open-trigger"
                disabled={loading || batchBusy}
                aria-pressed={selecting ? selectedIds.has(entry.book.id) : undefined}
                data-shelf-area="catalog"
                data-shelf-open={entry.book.id}
                data-book-id={entry.book.id}
                title={entry.book.title}
                onclick={(e) => handleCatalogClick(entry, e)}
                onpointerdown={(e) => startBookHold(entry.book, e)}
                oncontextmenu={(e) => selectFromContext(entry.book, e)}
                onkeydown={(e) => selectFromContext(entry.book, e)}
              >
                <BookCover
                  book={entry.book}
                  index={idx}
                  percent={entry.fact?.percent ?? null}
                  coverRevision={coverRevision[entry.book.id] ?? 0}
                />
                {#if selecting}
                  <span class="book-selection-mark" aria-hidden="true">
                    {#if selectedIds.has(entry.book.id)}<Check size={16} strokeWidth={2.5} />{/if}
                  </span>
                {/if}
              </button>
            </div>
            {#if uiPrefs.state.shelfShowMeta}
              <div class="book-info">
                <div class="book-title">{entry.book.title}</div>
                <div class="book-author">{entry.author || "佚名"}</div>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {:else}
      <div class="book-list shelf-catalog">
        {#each filteredEntries as entry, idx (entry.book.id)}
          <div class="book-list-row">
            <button
              type="button"
              class="list-open-trigger"
              disabled={loading || batchBusy}
              aria-pressed={selecting ? selectedIds.has(entry.book.id) : undefined}
              data-shelf-area="catalog"
              data-shelf-open={entry.book.id}
              data-book-id={entry.book.id}
              title={entry.book.title}
              onclick={(e) => handleCatalogClick(entry, e)}
              onpointerdown={(e) => startBookHold(entry.book, e)}
              oncontextmenu={(e) => selectFromContext(entry.book, e)}
              onkeydown={(e) => selectFromContext(entry.book, e)}
            >
              <div class="list-thumb">
                <BookCover
                  book={entry.book}
                  index={idx}
                  compact
                  coverRevision={coverRevision[entry.book.id] ?? 0}
                />
              </div>
              <div class="list-main">
                <div class="list-title">{entry.book.title}</div>
                <div class="list-sub">
                  <span>{entry.author || "佚名"}</span>
                  {#if entry.book.tags?.length}
                    <span class="list-tags">· {entry.book.tags.slice(0, 3).join(" / ")}</span>
                  {/if}
                </div>
              </div>
              <div class="list-stats tabular">
                {#if entry.fact?.percent != null}
                  <span class="list-percent">{entry.fact.percent}%</span>
                {/if}
                {#if entry.fact?.lastReadDate}
                  <span class="list-date">{formatDateBadge(entry.fact.lastReadDate)}</span>
                {/if}
              </div>
              {#if selecting}
                <span class="book-selection-mark" aria-hidden="true">
                  {#if selectedIds.has(entry.book.id)}<Check size={16} strokeWidth={2.5} />{/if}
                </span>
              {/if}
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</main>

<Dialog.Root bind:open={editDialogOpen}>
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
        {#if isOpen && editBook}
          <div
            {...props}
            class="atha-dialog-content edit-dialog"
            transition:scale={{ start: 0.96, duration: 200, easing: cubicOut }}
          >
            <Dialog.Title class="dialog-heading">编辑分类 · {editBook.title}</Dialog.Title>
            <Dialog.Description class="dialog-sub">
              每行填写一个条目，保存后即可在书架筛选条件中使用。
            </Dialog.Description>
            <form class="edit-form" onsubmit={saveEditDialog}>
              <div class="edit-switch-row">
                <label for="edit-fav-switch">收藏书籍</label>
                <Switch.Root id="edit-fav-switch" bind:checked={editFavorite} class="atha-switch">
                  <Switch.Thumb class="atha-switch-thumb" />
                </Switch.Root>
              </div>
              <label class="field-block">
                <span>标签（每行一个）</span>
                <textarea rows="3" bind:value={editTagsText} placeholder="例如：精读"></textarea>
              </label>
              <label class="field-block">
                <span>主题</span>
                <textarea rows="2" bind:value={editSubjectsText} placeholder="例如：数字人文"></textarea>
              </label>
              <label class="field-block">
                <span>系列</span>
                <textarea rows="2" bind:value={editSeriesText} placeholder="例如：现代书房论集"></textarea>
              </label>
              <div class="edit-actions">
                {#if editBook.hasCustomCover}
                  <button type="button" class="pill-btn" onclick={() => void handleResetCover(editBook!)}>
                    <RotateCcw size={14} /> 恢复默认封面
                  </button>
                {/if}
                <button type="button" class="pill-btn" onclick={() => (editDialogOpen = false)}>
                  取消
                </button>
                <button type="submit" class="accent-btn">保存更改</button>
              </div>
            </form>
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<ConfirmDialog
  bind:open={confirmState.open}
  title={confirmState.mode === "remove" ? "将书籍移出书架？" : "彻底删除本地阅读数据？"}
  description={confirmState.mode === "remove"
    ? `从书架移出${confirmTarget}，保留对话消息和历史快照。`
    : `删除${confirmTarget}及本机的阅读进度、书签和排版设置。对话和历史快照仍保留。`}
  confirmLabel={confirmState.mode === "remove" ? "移出书架" : "删除本地数据"}
  destructive={confirmState.mode === "delete"}
  onConfirm={executeBookConfirm}
/>

<style>
  .library-shell {
    padding: 36px 40px 96px 248px;
    max-width: 1480px;
    margin: 0 auto;
    min-height: 100vh;
  }

  .drop-scrim {
    position: fixed;
    inset: 24px;
    z-index: 95;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-xl);
    font-size: 16px;
    font-weight: 600;
    color: var(--ink-primary);
    pointer-events: none;
  }

  .shelf-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 18px;
  }

  .header-titles {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .header-titles h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.025em;
    line-height: 1.2;
  }

  .find-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .search-pill {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 12px;
  }

  .search-pill:focus-within {
    background: var(--bg-well-hover);
    box-shadow: none;
  }

  .search-pill input:focus-visible {
    outline-offset: 2px;
  }

  .search-pill :global(.search-icon) {
    color: var(--ink-muted);
    flex: none;
  }

  .search-pill input {
    width: 100%;
    min-width: 0;
    font-size: 14px;
  }

  .search-pill input::placeholder {
    color: var(--ink-muted);
  }

  .clear-search {
    width: 24px;
    height: 24px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-muted);
  }

  .clear-search:hover {
    background: var(--bg-well-hover);
    color: var(--ink-primary);
  }

  .bar-controls {
    display: flex;
    flex: none;
    align-items: center;
    gap: 6px;
  }

  .bar-controls :global(.filter-trigger),
  .shelf-layout-toggle,
  .select-trigger,
  .import-btn {
    min-width: 44px;
    min-height: 44px;
  }

  .shelf-layout-toggle {
    background: var(--bg-well);
  }

  .sort-strip {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    overflow-x: auto;
    padding: 2px 2px 4px;
    scroll-snap-type: x proximity;
    scrollbar-width: thin;
  }

  .sort-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
    min-height: 44px;
    padding: 0 11px;
    border-radius: var(--radius-pill);
    color: var(--ink-secondary);
    font-size: 14px;
    white-space: nowrap;
    scroll-snap-align: start;
  }

  .sort-chip:hover { background: var(--bg-well); }
  .sort-chip[aria-pressed="true"] { background: var(--accent-soft); color: var(--accent); font-weight: 600; }

  :global(.pill-btn.active-accent) {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .count-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 11px;
    font-weight: 600;
  }


  .active-conditions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 20px;
  }

  .condition-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: var(--radius-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12.5px;
    font-weight: 500;
  }

  .clear-all-link {
    font-size: 12.5px;
    color: var(--ink-secondary);
    padding: 4px 8px;
    border-radius: var(--radius-pill);
  }

  .clear-all-link:hover {
    background: var(--bg-well);
  }

  .shelf-section {
    margin-top: 28px;
  }

  .section-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  .section-head h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .section-hint {
    font-size: 12.5px;
    color: var(--ink-muted);
  }

  .quick-facets {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 4px 0 8px;
    scrollbar-width: none;
  }

  .quick-facets::-webkit-scrollbar {
    display: none;
  }

  .quick-pill {
    flex: none;
    height: 32px;
    padding: 0 14px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    color: var(--ink-secondary);
    font-size: 12.5px;
    font-weight: 500;
    transition: background-color 180ms var(--ease-smooth), color 180ms var(--ease-smooth);
  }

  .quick-pill:hover {
    background: var(--bg-well-hover);
    color: var(--ink-primary);
  }

  .quick-pill.active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .book-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 28px 24px;
  }

  @media (min-width: 900px) {
    .book-grid[data-density="comfortable"] {
      grid-template-columns: repeat(auto-fill, minmax(188px, 1fr));
    }

    .book-grid[data-density="compact"] {
      grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
    }
  }

  .book-card {
    content-visibility: auto;
    contain-intrinsic-size: 280px;
    display: flex;
    flex-direction: column;
    gap: 11px;
  }

  .book-cover-hit {
    position: relative;
  }

  .book-open-trigger {
    display: block;
    width: 100%;
    text-align: left;
    border-radius: 10px;
    transition: transform 240ms var(--ease-spring);
  }

  .book-cover-hit:hover .book-open-trigger:not([aria-pressed]) {
    transform: translateY(-4px);
  }

  .book-open-trigger,
  .list-open-trigger {
    user-select: none;
    -webkit-touch-callout: none;
  }

  .book-open-trigger[aria-pressed="true"] {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .book-selection-mark {
    width: 26px;
    height: 26px;
    display: inline-grid;
    flex: none;
    place-items: center;
    border: 2px solid var(--ink-secondary);
    border-radius: 50%;
    background: var(--bg-surface);
    color: var(--accent);
  }

  .book-open-trigger .book-selection-mark {
    position: absolute;
    top: 8px;
    left: 8px;
  }

  [aria-pressed="true"] > .book-selection-mark {
    border-color: var(--accent);
  }

  .book-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .book-title {
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.38;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .book-author {
    font-size: 12px;
    color: var(--ink-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .book-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .book-list-row {
    content-visibility: auto;
    contain-intrinsic-size: 76px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    transition: background-color 160ms var(--ease-smooth);
  }

  .book-list-row:hover {
    background: var(--bg-well);
  }

  .list-open-trigger {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 16px;
    text-align: left;
  }

  .list-thumb {
    width: 44px;
    flex: none;
  }

  .list-main {
    flex: 1;
    min-width: 0;
  }

  .list-title {
    font-size: 14.5px;
    font-weight: 600;
    color: var(--ink-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list-sub {
    font-size: 12.5px;
    color: var(--ink-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list-tags {
    color: var(--ink-muted);
  }

  .list-stats {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 12.5px;
    color: var(--ink-muted);
  }

  .list-percent {
    color: var(--accent);
    font-weight: 600;
  }

  .empty-shelf {
    padding: 56px 24px;
    text-align: center;
    color: var(--ink-secondary);
    background: var(--bg-surface);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-xs);
  }

  .toast-banner {
    position: fixed;
    top: 20px;
    left: 50%;
    translate: -50% 0;
    z-index: 60;
    padding: 10px 20px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-primary);
  }

  .shelf-actions {
    position: fixed;
    z-index: 42;
    left: max(248px, calc((100vw - 1480px) / 2 + 248px));
    bottom: calc(24px + var(--safe-area-bottom));
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    border-radius: var(--radius-lg);
    background: var(--bg-glass-strong);
  }

  .selection-count,
  .selection-hint {
    padding: 0 8px;
    color: var(--ink-secondary);
    font-size: 13px;
    white-space: nowrap;
  }

  .selection-summary,
  .selection-options {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .shelf-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 44px;
    min-width: 44px;
    padding: 0 9px;
    border-radius: var(--radius-md);
    font-size: 13px;
    white-space: nowrap;
  }

  .shelf-action:hover,
  .shelf-action:focus-visible {
    background: var(--bg-well);
  }

  .shelf-action.danger {
    color: var(--danger);
  }

  /* Filter & Select Popovers (portal-rendered) */
  :global(.filter-popover) {
    z-index: 65;
    width: min(calc(100vw - 32px), 380px);
    max-height: 70vh;
    overflow-y: auto;
    padding: 18px;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .filter-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .filter-title {
    font-size: 15px;
    font-weight: 600;
  }

  .filter-matches {
    font-size: 12px;
    color: var(--ink-muted);
  }

  .filter-favorite-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .switch-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13.5px;
    font-weight: 500;
  }

  :global(.atha-switch) {
    position: relative;
    width: 42px;
    height: 24px;
    border-radius: 999px;
    background: var(--bg-well-active);
    padding: 2px;
    transition: background-color 180ms var(--ease-smooth);
  }

  :global(.atha-switch[data-state="checked"]) {
    background: var(--accent);
  }

  :global(.atha-switch-thumb) {
    display: block;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    background: #ffffff;
    box-shadow: var(--shadow-xs);
    transition: transform 200ms var(--ease-spring);
  }

  :global(.atha-switch[data-state="checked"] .atha-switch-thumb) {
    transform: translateX(18px);
  }

  .facet-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .facet-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-muted);
  }

  .facet-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .facet-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 11px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    color: var(--ink-secondary);
    font-size: 12.5px;
  }

  .facet-pill:hover {
    background: var(--bg-well-hover);
  }

  .facet-pill.selected {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .facet-count {
    font-size: 11px;
    opacity: 0.72;
  }

  .filter-footer {
    display: flex;
    justify-content: flex-end;
  }

  .edit-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
  }

  :global(.dialog-heading) {
    margin: 0 0 6px;
    font-size: 18px;
    font-weight: 600;
  }

  :global(.dialog-sub) {
    margin: 0 0 18px;
    font-size: 13px;
    color: var(--ink-secondary);
  }

  .edit-form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .edit-switch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    font-size: 14px;
    font-weight: 500;
  }

  .field-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-secondary);
  }

  .field-block textarea {
    width: 100%;
    padding: 10px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    color: var(--ink-primary);
    resize: vertical;
  }

  .field-block textarea:focus {
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs), 0 0 0 2px var(--ring);
  }

  .edit-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 6px;
  }

  @media (max-width: 899px) {
    .library-shell {
      padding: 22px 16px 104px;
    }

    .header-titles h1 {
      font-size: 26px;
    }

    .bar-controls :global(.filter-trigger) {
      position: relative;
      padding: 0 8px;
    }

    .filter-label {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }

    .bar-controls .count-badge {
      position: absolute;
      top: 3px;
      right: 3px;
      min-width: 14px;
      height: 14px;
      padding: 0 2px;
      font-size: 9px;
    }

    .select-trigger { padding: 0 10px; }

    .book-grid {
      grid-template-columns: repeat(var(--shelf-mobile-cols), minmax(0, 1fr));
      gap: 18px 12px;
    }

    .library-shell.selecting {
      padding-bottom: 130px;
    }

    .shelf-actions {
      left: 50%;
      translate: -50% 0;
      bottom: max(14px, var(--safe-area-bottom));
      margin-left: calc((var(--safe-area-left) - var(--safe-area-right)) / 2);
      width: min(calc(100% - 32px - var(--safe-area-left) - var(--safe-area-right)), 420px);
      flex-direction: column;
      align-items: stretch;
      gap: 0;
      padding: 5px 8px;
    }

    .selection-summary {
      justify-content: space-between;
    }

    .selection-options {
      justify-content: space-evenly;
      flex-wrap: wrap;
    }

    .shelf-action {
      flex-direction: column;
      gap: 3px;
      padding: 3px 4px;
    }

    .library-shell.has-resume {
      padding-bottom: 164px;
    }
  }
</style>
