<script lang="ts">
  import { computePosition, flip, offset, shift } from "@floating-ui/dom";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    Check,
    ChevronDown,
    Filter,
    Heart,
    Image,
    LayoutGrid,
    List,
    MoreHorizontal,
    Plus,
    RotateCcw,
    Search,
    Tag,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { Dialog, Popover, Select, Switch } from "bits-ui";
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
    deleteLibraryBookData,
    importLibraryBooks,
    listLibraryBooks,
    removeLibraryBook,
    resetLibraryBookCover,
    setLibraryBookCover,
    updateLibraryBookOrganization,
  } from "../data/library-adapter.ts";
  import BookCover from "./BookCover.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  interface Props {
    active: boolean;
    onOpenReader: (book: LibraryBook, origin: ShelfOrigin, trigger?: HTMLElement) => Promise<void>;
    onBooksUpdated?: (books: LibraryBook[]) => void;
  }

  let { active, onOpenReader, onBooksUpdated }: Props = $props();

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
  let coverRevision = $state<Record<string, number>>({});
  let statusMessage = $state<string | null>(null);

  // Scroll containers for C4 continuity
  let pageEl = $state<HTMLElement | null>(null);
  let recentStripEl = $state<HTMLElement | null>(null);
  let tagsStripEl = $state<HTMLElement | null>(null);

  // Floating Book Menu
  let menuBook = $state<LibraryBook | null>(null);
  let menuEl = $state<HTMLElement | null>(null);
  let menuAnchorEl = $state<HTMLElement | null>(null);

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
    book: LibraryBook | null;
  }>({ open: false, mode: "remove", book: null });

  const readingMap = $derived(readShelfReading(books));
  const entries = $derived(indexShelf(books, readingMap));
  const recentEntries = $derived(recentShelf(entries));
  const quickFacets = $derived(quickShelfFacets(books));
  const filteredEntries = $derived(browseShelf(entries, searchQuery, filters, sort));
  const facetSummary = $derived(shelfFacetOptions(entries, searchQuery, filters));
  const activeFilterCount = $derived(filterCount(filters));
  const currentSortLabel = $derived(
    shelfSorts.find(([value]) => value === sort)?.[1] ?? "书名 A–Z",
  );

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
    pos.recentX = recentStripEl?.scrollLeft ?? 0;
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
      const selector = `.shelf-catalog [data-shelf-open="${saved.position.focus.id}"], [data-shelf-area="${saved.position.focus.area}"][data-book-id="${saved.position.focus.id}"]`;
      const target = document.querySelector<HTMLElement>(selector);
      if (target) {
        if (saved.position.focus.offset > 0) {
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
    if (recentStripEl && saved.position.recentX > 0) {
      recentStripEl.scrollLeft = saved.position.recentX;
    }
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

  async function openBookMenu(book: LibraryBook, anchor: HTMLElement) {
    if (menuBook?.id === book.id) {
      closeBookMenu();
      return;
    }
    menuBook = book;
    menuAnchorEl = anchor;
    await tick();
    if (menuEl && menuAnchorEl) {
      const { x, y } = await computePosition(menuAnchorEl, menuEl, {
        placement: "bottom-end",
        middleware: [offset(8), flip(), shift({ padding: 12 })],
      });
      menuEl.style.left = `${x}px`;
      menuEl.style.top = `${y}px`;
    }
  }

  function closeBookMenu() {
    menuBook = null;
    menuAnchorEl = null;
  }

  async function handleToggleFavorite(book: LibraryBook) {
    closeBookMenu();
    books = await updateLibraryBookOrganization(book.id, { favorite: !book.favorite });
    onBooksUpdated?.(books);
    notify(book.favorite ? "已取消收藏" : "已加入收藏");
  }

  function openEditDialog(book: LibraryBook) {
    closeBookMenu();
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
    closeBookMenu();
    const updated = await setLibraryBookCover(book.id);
    if (updated) {
      books = updated;
      coverRevision = { ...coverRevision, [book.id]: (coverRevision[book.id] ?? 0) + 1 };
      onBooksUpdated?.(books);
      notify("已更换封面");
    }
  }

  async function handleResetCover(book: LibraryBook) {
    closeBookMenu();
    books = await resetLibraryBookCover(book.id);
    coverRevision = { ...coverRevision, [book.id]: (coverRevision[book.id] ?? 0) + 1 };
    onBooksUpdated?.(books);
    notify("已恢复默认封面");
  }

  function requestBookConfirm(book: LibraryBook, mode: "remove" | "delete") {
    closeBookMenu();
    confirmState = { open: true, mode, book };
  }

  async function executeBookConfirm() {
    const target = confirmState.book;
    if (!target) return;
    if (confirmState.mode === "remove") {
      books = await removeLibraryBook(target.id);
      notify(`已将《${target.title}》移出书架`);
    } else {
      books = await deleteLibraryBookData(target.id);
      notify(`已清理《${target.title}》的本地阅读记录与数据`);
    }
    onBooksUpdated?.(books);
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

  function handleWindowPointerDown(event: PointerEvent) {
    if (!menuBook) return;
    const target = event.target as Node | null;
    if (menuEl?.contains(target) || menuAnchorEl?.contains(target)) return;
    closeBookMenu();
  }
</script>

<svelte:window onpointerdown={handleWindowPointerDown} onkeydown={(e) => e.key === "Escape" && closeBookMenu()} />

<main class="library-shell" bind:this={pageEl} aria-label="书架" aria-busy={loading ? "true" : "false"}>
  {#if dragActive}
    <div class="drop-scrim glass-popover" role="status">松开鼠标，立即导入书籍至书架</div>
  {/if}
  <header class="shelf-header">
    <div class="header-titles">
      <h1>书架</h1>
      <p class="subtitle">
        <span class="tabular">{entries.length}</span> 册藏书
        {#if recentEntries.length > 0}
          · <span class="tabular">{recentEntries.length}</span> 本近期在读
        {/if}
      </p>
    </div>

    <div class="find-bar">
      <div class="well-input search-pill">
        <Search size={16} strokeWidth={1.8} class="search-icon" />
        <input
          type="search"
          aria-label="搜索书名、作者、标签或系列"
          placeholder="搜索书名、作者、标签或系列…"
          bind:value={searchQuery}
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
          <Popover.Trigger class="pill-btn {activeFilterCount > 0 ? 'active-accent' : ''}">
            <Filter size={15} />
            <span>筛选</span>
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

        <Select.Root
          type="single"
          value={sort}
          onValueChange={(value) => {
            if (value) sort = value as ShelfSort;
          }}
          items={shelfSorts.map(([value, label]) => ({ value, label }))}
        >
          <Select.Trigger class="pill-btn sort-trigger" aria-label="排序方式">
            <span>{currentSortLabel}</span>
            <ChevronDown size={14} />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content sideOffset={8} align="end" class="glass-popover select-popover">
              <Select.Viewport>
                {#each shelfSorts as [value, label] (value)}
                  <Select.Item {value} {label} class="select-item">
                    {#snippet children({ selected })}
                      <span>{label}</span>
                      {#if selected}
                        <Check size={14} />
                      {/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>

        <div class="layout-segmented" role="group" aria-label="视图布局">
          <button
            type="button"
            class="seg-btn"
            class:active={layout === "grid"}
            aria-label="封面网格"
            aria-pressed={layout === "grid"}
            onclick={() => (layout = "grid")}
          >
            <LayoutGrid size={15} />
          </button>
          <button
            type="button"
            class="seg-btn"
            class:active={layout === "list"}
            aria-label="清单列表"
            aria-pressed={layout === "list"}
            onclick={() => (layout = "list")}
          >
            <List size={15} />
          </button>
        </div>

        <button type="button" class="accent-btn import-btn" onclick={handleImport}>
          <Plus size={16} />
          <span>导入</span>
        </button>
      </div>
    </div>
  </header>

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

  {#if (!loading || books.length > 0) && recentEntries.length > 0 && !searchQuery.trim() && activeFilterCount === 0}
    <section class="shelf-section" aria-label="继续阅读">
      <div class="section-head">
        <h2>继续阅读</h2>
        <span class="section-hint">最近开启的书页</span>
      </div>
      <div class="recent-strip" bind:this={recentStripEl}>
        {#each recentEntries as entry, idx (entry.book.id)}
          <div class="recent-card-wrap">
            <button
              type="button"
              class="recent-card"
              disabled={loading}
              data-shelf-area="recent"
              data-book-id={entry.book.id}
              title={entry.book.title}
              onclick={(e) => handleOpenBook(entry, "recent", e.currentTarget)}
            >
              <div class="recent-cover">
                <BookCover
                  book={entry.book}
                  index={idx}
                  percent={entry.fact?.percent ?? null}
                  coverRevision={coverRevision[entry.book.id] ?? 0}
                />
              </div>
              <div class="recent-meta">
                <div class="recent-title">{entry.book.title}</div>
                <div class="recent-sub tabular">
                  {#if entry.fact?.percent != null}
                    <span>{entry.fact.percent}%</span>
                  {/if}
                  {#if entry.fact?.lastReadDate}
                    <span>{formatDateBadge(entry.fact.lastReadDate)}</span>
                  {/if}
                </div>
              </div>
            </button>
          </div>
        {/each}
      </div>
    </section>
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
      <div class="book-grid shelf-catalog">
        {#each filteredEntries as entry, idx (entry.book.id)}
          <article class="book-card">
            <div class="book-cover-hit">
              <button
                type="button"
                class="book-open-trigger"
                disabled={loading}
                data-shelf-area="catalog"
                data-shelf-open={entry.book.id}
                data-book-id={entry.book.id}
                title={entry.book.title}
                onclick={(e) => handleOpenBook(entry, "catalog", e.currentTarget)}
              >
                <BookCover
                  book={entry.book}
                  index={idx}
                  percent={entry.fact?.percent ?? null}
                  coverRevision={coverRevision[entry.book.id] ?? 0}
                />
              </button>
              <button
                type="button"
                class="book-more-btn"
                aria-label={`更多操作：${entry.book.title}`}
                onclick={(e) => openBookMenu(entry.book, e.currentTarget)}
              >
                <MoreHorizontal size={16} />
              </button>
            </div>
            <div class="book-info">
              <div class="book-title">{entry.book.title}</div>
              <div class="book-author">{entry.author || "佚名"}</div>
            </div>
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
              disabled={loading}
              data-shelf-area="catalog"
              data-shelf-open={entry.book.id}
              data-book-id={entry.book.id}
              title={entry.book.title}
              onclick={(e) => handleOpenBook(entry, "catalog", e.currentTarget)}
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
            </button>
            <button
              type="button"
              class="icon-btn"
              aria-label={`更多操作：${entry.book.title}`}
              onclick={(e) => openBookMenu(entry.book, e.currentTarget)}
            >
              <MoreHorizontal size={16} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</main>

{#if menuBook}
  {@const activeMenuBook = menuBook}
  <div
    bind:this={menuEl}
    class="glass-popover book-context-menu"
    role="menu"
    aria-label="书籍操作菜单"
    transition:scale={{ start: 0.95, duration: 150, easing: cubicOut }}
  >
    <button
      type="button"
      role="menuitem"
      class="menu-item"
      onclick={() => handleToggleFavorite(activeMenuBook)}
    >
      <Heart size={15} fill={activeMenuBook.favorite ? "currentColor" : "none"} />
      <span>{activeMenuBook.favorite ? "取消收藏" : "加入收藏"}</span>
    </button>
    <button
      type="button"
      role="menuitem"
      class="menu-item"
      onclick={() => openEditDialog(activeMenuBook)}
    >
      <Tag size={15} />
      <span>编辑标签与分类</span>
    </button>
    <button
      type="button"
      role="menuitem"
      class="menu-item"
      onclick={() => handleSetCover(activeMenuBook)}
    >
      <Image size={15} />
      <span>更换封面</span>
    </button>
    {#if activeMenuBook.hasCustomCover}
      <button
        type="button"
        role="menuitem"
        class="menu-item"
        onclick={() => handleResetCover(activeMenuBook)}
      >
        <RotateCcw size={15} />
        <span>恢复默认封面</span>
      </button>
    {/if}
    <div class="menu-gap" role="separator"></div>
    <button
      type="button"
      role="menuitem"
      class="menu-item"
      onclick={() => requestBookConfirm(activeMenuBook, "remove")}
    >
      <X size={15} />
      <span>移出书架</span>
    </button>
    <button
      type="button"
      role="menuitem"
      class="menu-item danger"
      onclick={() => requestBookConfirm(activeMenuBook, "delete")}
    >
      <Trash2 size={15} />
      <span>删除本地数据</span>
    </button>
  </div>
{/if}

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
    ? `仅将《${confirmState.book?.title ?? ""}》从书架列表移除，已生成的对话消息与历史快照仍安全保留。`
    : `将清除《${confirmState.book?.title ?? ""}》在本机保存的书籍文件、阅读进度、书签与本地排版覆盖（消息存档保留历史快照）。`}
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
    gap: 22px;
    margin-bottom: 24px;
  }

  .header-titles h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.025em;
    line-height: 1.2;
  }

  .subtitle {
    margin: 6px 0 0;
    font-size: 13.5px;
    color: var(--ink-secondary);
  }

  .find-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }

  .search-pill {
    flex: 1 1 280px;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 16px;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

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

  .layout-segmented {
    display: inline-flex;
    padding: 3px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    gap: 2px;
  }

  .seg-btn {
    width: 34px;
    height: 30px;
    border-radius: var(--radius-pill);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-secondary);
    transition: background-color 180ms var(--ease-smooth), color 180ms var(--ease-smooth);
  }

  .seg-btn.active {
    background: var(--bg-pill-active);
    color: var(--ink-primary);
    box-shadow: var(--shadow-xs);
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

  .recent-strip {
    display: flex;
    gap: 20px;
    overflow-x: auto;
    padding: 6px 4px 18px;
    margin: -6px -4px 0;
    scrollbar-width: none;
  }

  .recent-strip::-webkit-scrollbar {
    display: none;
  }

  .recent-card-wrap {
    flex: none;
    width: 132px;
  }

  .recent-card {
    width: 100%;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .recent-card:hover .recent-cover {
    transform: translateY(-4px);
  }

  .recent-cover {
    width: 100%;
    transition: transform 240ms var(--ease-spring);
  }

  .recent-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .recent-title {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.35;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .recent-sub {
    display: flex;
    gap: 8px;
    font-size: 11.5px;
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

  .book-cover-hit:hover .book-open-trigger {
    transform: translateY(-4px);
  }

  .book-more-btn {
    position: absolute;
    right: 8px;
    bottom: 16px;
    width: 30px;
    height: 30px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(12px);
    color: var(--ink-primary);
    box-shadow: var(--shadow-sm);
    opacity: 0;
    transition: opacity 160ms var(--ease-smooth), transform 160ms var(--ease-spring);
  }

  .book-cover-hit:hover .book-more-btn,
  .book-cover-hit:focus-within .book-more-btn {
    opacity: 1;
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

  .book-context-menu {
    position: fixed;
    z-index: 70;
    min-width: 196px;
    padding: 6px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 12px;
    border-radius: var(--radius-xs);
    font-size: 13.5px;
    color: var(--ink-primary);
    text-align: left;
    transition: background-color 140ms var(--ease-smooth);
  }

  .menu-item:hover {
    background: var(--bg-well);
  }

  .menu-item.danger {
    color: var(--danger);
  }

  .menu-gap {
    height: 6px;
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

  :global(.select-popover) {
    z-index: 65;
    min-width: 170px;
    padding: 6px;
    border-radius: var(--radius-md);
  }

  :global(.select-item) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 9px 12px;
    border-radius: var(--radius-xs);
    font-size: 13.5px;
    cursor: pointer;
    color: var(--ink-primary);
  }

  :global(.select-item[data-highlighted]) {
    background: var(--bg-well);
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

    .book-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 20px 16px;
    }

    .book-more-btn {
      opacity: 1;
    }

    .recent-card-wrap {
      width: 112px;
    }
  }
</style>
