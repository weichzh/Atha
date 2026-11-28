<script lang="ts">
  import { onMount, tick } from "svelte";
  import { ArrowLeft } from "@lucide/svelte";
  import { openBook, type LibraryBook, type ReadingMemoryJump, type ReadingMemoryTarget } from "./library.ts";
  import type { ShelfOrigin } from "./shelf-session.ts";
  import AppSettingsView from "./lib/components/AppSettingsView.svelte";
  import BookOpeningCover from "./lib/components/BookOpeningCover.svelte";
  import LibraryView from "./lib/components/LibraryView.svelte";
  import MemoryView from "./lib/components/MemoryView.svelte";
  import NavRail from "./lib/components/NavRail.svelte";
  import { registerBackHandler, installGlobalBackDispatcher } from "./lib/data/back-stack.ts";
  import { DEMO_BOOKS, DEMO_PRIMARY_BOOK_ID } from "./lib/data/demo-seed.ts";
  import {
    activateShelfReader,
    applyAppThemeToDocument,
    getAppTheme,
    launchReadingMemoryHit,
    listLibraryBooks,
    opensShelfReaderInPlace,
    prepareShelfReaderHref,
  } from "./lib/data/library-adapter.ts";
  import {
    captureOpeningCover, consumeReaderLaunchLabel, openingCoverFrame, openingPaperApproach,
    readOpeningTheme, saveReaderLaunchLabel, type OpeningCover, type OpeningPaperMeasure,
  } from "./lib/data/open-transition.ts";
  import { hasReaderLaunchParams, installDemoFetchBridge, isTauriRuntime } from "./lib/data/runtime.ts";
  import type { AppRoute } from "./lib/types.ts";

  interface Props {
    hasLibraryReturn?: boolean;
  }

  let { hasLibraryReturn = false }: Props = $props();

  const initialReaderLaunch = hasReaderLaunchParams();
  const initialParams = initialReaderLaunch ? new URLSearchParams(window.location.search) : null;
  const tauriOnReader = isTauriRuntime() && initialReaderLaunch;

  function parseInitialJump(params: URLSearchParams | null, editionId: string): ReadingMemoryTarget | null {
    if (!params) return null;
    const memConv = params.get("memory-conversation");
    const memMsg = params.get("memory-message");
    const memRoot = params.get("memory-root");
    if (!memConv || !memMsg || !memRoot) return null;
    return {
      conversationId: memConv,
      messageId: memMsg,
      rootMessageId: memRoot,
      editionId,
    };
  }

  let activeRoute = $state<AppRoute>(initialReaderLaunch ? "reader" : "library");
  let activeTab = $state<Exclude<AppRoute, "reader">>("library");
  let bookCount = $state(isTauriRuntime() ? 0 : DEMO_BOOKS.length);
  let shelfSelecting = $state(false);
  let libraryViewRef = $state<Pick<LibraryView, "rememberOpenOrigin" | "restoreOpenFocus" | "refreshSilent"> | null>(null);
  let savedScrollY = 0;
  let shelfReturn: { origin: ShelfOrigin; trigger?: HTMLElement } | null = null;
  let readerLaunchInFlight = $state(false);
  let tabRevision = 0;
  let memoryOpeningFocus: HTMLElement | null = null;
  let shelfOpening = $state.raw<{ origin: ShelfOrigin; trigger?: HTMLElement } | null>(null);
  let entryActive = $state(false);
  let coverPoised = $state(false);
  let readerCover = $state.raw<OpeningCover | null>(null);
  let paperMeasure = $state.raw<(OpeningPaperMeasure & { viewportWidth: number; viewportHeight: number }) | null>(null);
  let readerScreen = $state<"waiting" | "ready" | "error">("waiting");
  let viewportWidth = $state(window.innerWidth);
  let viewportHeight = $state(window.innerHeight);
  let openingTheme = $state(readOpeningTheme());
  let openingBack = $state<HTMLButtonElement>();
  const coverFrame = $derived(readerCover && openingCoverFrame(readerCover.rect, viewportWidth, viewportHeight));
  const paperApproach = $derived(coverFrame && openingPaperApproach(
    coverFrame,
    paperMeasure ?? { left: 0, width: viewportWidth },
  ));
  const coverTurning = $derived(Boolean(coverFrame && coverPoised && readerScreen === "ready"));
  const openingInset = $derived(coverFrame
    ? `${coverFrame.top}px ${viewportWidth - coverFrame.left - coverFrame.width}px ${viewportHeight - coverFrame.top - coverFrame.height}px ${coverFrame.left}px`
    : "0px");
  let ReaderView = $state<typeof import("./lib/components/ReaderView.svelte").default | null>(null);
  let readerBootError = $state(false);
  let readerLaunchReady = $state(!initialReaderLaunch || tauriOnReader);

  const initialVersion =
    initialParams?.get("version") ||
    initialParams?.get("state") ||
    (isTauriRuntime() ? "" : DEMO_PRIMARY_BOOK_ID);
  const launchLabel = initialReaderLaunch ? consumeReaderLaunchLabel(initialVersion) : null;

  let readerBook = $state<LibraryBook>({
    ...DEMO_BOOKS[0],
    id: initialVersion,
    title: launchLabel?.title || initialParams?.get("title") || (isTauriRuntime() ? "" : DEMO_BOOKS[0].title),
    authors: launchLabel?.author ? [launchLabel.author] : isTauriRuntime() ? [] : DEMO_BOOKS[0].authors,
  });
  let readerManifestPath = $state(
    initialParams?.get("manifest") || (isTauriRuntime() ? "" : "demo/manifest.json"),
  );
  let readerJump = $state<ReadingMemoryTarget | null>(
    parseInitialJump(initialParams, initialVersion),
  );

  async function loadReaderView() {
    if (ReaderView) return;
    const module = await import("./lib/components/ReaderView.svelte");
    ReaderView = module.default;
  }

  $effect(() => {
    if (activeRoute !== "reader" || !readerLaunchReady || ReaderView) return;
    void loadReaderView().catch(() => {
      if (activeRoute === "reader") {
        readerBootError = true;
        handleReaderScreen("error");
      }
    });
  });

  $effect(() => {
    if (entryActive) openingBack?.focus({ preventScroll: true });
  });

  $effect(() => {
    if (paperMeasure && (viewportWidth !== paperMeasure.viewportWidth || viewportHeight !== paperMeasure.viewportHeight)) {
      // A real viewport resize invalidates the staged paper geometry; finish at 1:1.
      finishEntry();
    }
  });

  onMount(() => {
    if (!isTauriRuntime()) {
      installDemoFetchBridge();
    }
    if (!initialReaderLaunch) {
      applyAppThemeToDocument(getAppTheme());
    }

    const uninstallBack = installGlobalBackDispatcher();
    const unregisterRootBack = registerBackHandler((source) => {
      if (entryActive || (activeRoute === "reader" && !ReaderView)) {
        backFromOpening();
        return true;
      }
      if (source === "back" && activeRoute !== "reader" && activeTab !== "library") {
        switchTab("library");
        return true;
      }
      return false;
    });

    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onMotionChange = () => {
      if (!motionQuery.matches || !readerCover) return;
      discardCover();
      if (readerScreen === "ready") finishEntry();
    };
    motionQuery.addEventListener("change", onMotionChange);
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const onSystemChange = () => {
      if (activeRoute !== "reader" && getAppTheme() === "system") {
        applyAppThemeToDocument("system");
      }
    };
    mq?.addEventListener("change", onSystemChange);

    if (initialReaderLaunch && !isTauriRuntime()) {
      const manifest = initialParams?.get("manifest") || "demo/manifest.json";
      const version = initialParams?.get("version") || DEMO_PRIMARY_BOOK_ID;
      void listLibraryBooks().then((list) => {
        bookCount = list.length;
        if (activeRoute !== "reader" || readerLaunchReady) return;
        readerBook = list.find((b) => b.id === version) ?? {
          ...DEMO_BOOKS[0],
          id: version,
        };
        readerManifestPath = manifest;
        readerJump = parseInitialJump(initialParams, version);
        readerLaunchReady = true;
      });
    } else if (!initialReaderLaunch && !isTauriRuntime()) {
      void listLibraryBooks().then((list) => {
        bookCount = list.length;
      });
    }

    return () => {
      motionQuery.removeEventListener("change", onMotionChange);
      mq?.removeEventListener("change", onSystemChange);
      unregisterRootBack();
      uninstallBack();
    };
  });

  function cameFromAppLibraryRoot(): boolean {
    if (history.length <= 1) return false;
    if (!document.referrer) return true;
    try {
      const ref = new URL(document.referrer);
      return (
        ref.protocol === location.protocol &&
        ref.host === location.host &&
        (ref.pathname === "/" || ref.pathname === "") &&
        !ref.search &&
        !ref.hash
      );
    } catch {
      return false;
    }
  }

  function switchTab(tab: Exclude<AppRoute, "reader">) {
    tabRevision += 1;
    cancelShelfOpen(false);
    if (activeRoute === "memory" && entryActive) finishMemoryOpening();
    activeTab = tab;
    activeRoute = tab;
  }

  function openReader(book: LibraryBook, manifestPath: string, jump: ReadingMemoryTarget | null = null) {
    if (typeof window !== "undefined") {
      savedScrollY = window.scrollY;
    }
    openingTheme = readOpeningTheme();
    readerBootError = false;
    readerLaunchReady = true;
    readerScreen = "waiting";
    entryActive = true;
    readerBook = book;
    readerManifestPath = manifestPath;
    readerJump = jump;
    activeRoute = "reader";
  }

  function cancelShelfOpen(restoreFocus = true) {
    const request = shelfOpening;
    if (!request) return;
    finishEntry();
    shelfOpening = null;
    if (restoreFocus) void tick().then(() => {
      if (!entryActive && activeRoute === "library") libraryViewRef?.restoreOpenFocus(request.origin, request.trigger);
    });
  }

  function discardCover() {
    readerCover = null;
    paperMeasure = null;
    coverPoised = false;
    (shelfOpening?.trigger ?? shelfReturn?.trigger)?.removeAttribute("data-cover-lifted");
  }

  function finishEntry() {
    discardCover();
    entryActive = false;
  }

  function handleReaderScreen(screen: "ready" | "error", measure?: OpeningPaperMeasure) {
    if (screen === "ready" && readerCover) {
      const width = window.innerWidth;
      paperMeasure = { ...(measure ?? { left: 0, width }), viewportWidth: width, viewportHeight: window.innerHeight };
    }
    readerScreen = screen;
    if (screen === "error" || !readerCover) finishEntry();
  }

  function backFromOpening() {
    if (activeRoute === "reader") void handleBackToShelf();
    else if (activeRoute === "memory") {
      tabRevision += 1;
      finishMemoryOpening();
    } else cancelShelfOpen();
  }

  function finishMemoryOpening() {
    const focus = memoryOpeningFocus;
    memoryOpeningFocus = null;
    finishEntry();
    void tick().then(() => {
      if (!entryActive && activeRoute === "memory") focus?.focus({ preventScroll: true });
    });
  }

  async function openMemoryBook(book: LibraryBook, jump: ReadingMemoryJump) {
    if (readerLaunchInFlight || activeRoute !== "memory") return;
    readerLaunchInFlight = true;
    const revision = tabRevision;
    const inPlace = opensShelfReaderInPlace();
    if (inPlace) {
      memoryOpeningFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      entryActive = true;
    }
    try {
      await launchReadingMemoryHit(
        jump,
        (_id, manifest) => {
          memoryOpeningFocus = null;
          openReader(book, manifest, jump);
        },
        () => activeRoute === "memory" && tabRevision === revision,
      );
    } catch (error) {
      if (tabRevision === revision) throw error;
    } finally {
      if (inPlace && activeRoute === "memory" && tabRevision === revision) finishMemoryOpening();
      readerLaunchInFlight = false;
    }
  }

  async function openShelfBook(book: LibraryBook, origin: ShelfOrigin, trigger?: HTMLElement) {
    if (readerLaunchInFlight || shelfOpening || activeRoute !== "library") return;
    libraryViewRef?.rememberOpenOrigin(origin);
    if (!opensShelfReaderInPlace()) {
      readerLaunchInFlight = true;
      try {
        saveReaderLaunchLabel({ bookId: book.id, title: book.title, author: book.authors.join(" · ") });
        await openBook(book.id);
      } finally {
        readerLaunchInFlight = false;
      }
      return;
    }
    const request = { origin, trigger };
    readerCover = captureOpeningCover(trigger);
    if (readerCover) trigger?.setAttribute("data-cover-lifted", "");
    readerScreen = "waiting";
    openingTheme = readOpeningTheme();
    entryActive = true;
    readerLaunchInFlight = true;
    shelfOpening = request;
    try {
      // Start both loads on the click. Settle native activation before unlocking another open.
      const view = loadReaderView().then(() => true, () => false);
      const href = await prepareShelfReaderHref(book.id).finally(() => { readerLaunchInFlight = false; });
      const loaded = await view;
      if (shelfOpening !== request) return;
      if (!loaded) throw new Error("reader-view-load");
      const manifest = activateShelfReader(href);
      shelfReturn = { origin, trigger };
      shelfOpening = null;
      openReader(book, manifest);
    } catch (error) {
      if (shelfOpening === request) throw error;
    } finally {
      if (shelfOpening === request) cancelShelfOpen();
    }
  }

  async function handleBackToShelf() {
    try {
      window.__athaFlushOnExit?.();
    } catch {
      // A storage flush error must not block exiting the reader view.
    }
    applyAppThemeToDocument(getAppTheme());
    if (tauriOnReader) {
      if (hasLibraryReturn && cameFromAppLibraryRoot()) {
        history.back();
        return;
      }
      location.replace(new URL("/", location.href).href);
      return;
    }
    if (hasReaderLaunchParams()) {
      history.replaceState(null, "", "/");
    }
    finishEntry();
    readerJump = null;
    activeRoute = activeTab;
    await tick();
    if (activeTab === "library") {
      window.scrollTo({ top: savedScrollY, behavior: "instant" as ScrollBehavior });
      const returned = shelfReturn;
      shelfReturn = null;
      if (returned) libraryViewRef?.restoreOpenFocus(returned.origin, returned.trigger);
      void libraryViewRef?.refreshSilent(returned?.origin);
    }
  }
</script>

<svelte:window bind:innerWidth={viewportWidth} bind:innerHeight={viewportHeight} />

{#if !tauriOnReader}
  <div class="app-frame" class:shelf-selecting={activeRoute === "library" && shelfSelecting} class:behind-opening={entryActive} hidden={activeRoute === "reader" && !entryActive} inert={entryActive || activeRoute === "reader"}>
    <NavRail {activeTab} {bookCount} onSelect={switchTab} />

    {#if activeTab === "library"}
      <LibraryView
        bind:this={libraryViewRef}
        active={!entryActive && activeRoute === "library"}
        onOpenReader={openShelfBook}
        onBooksUpdated={(list) => (bookCount = list.length)}
        onSelectingChange={(selecting) => (shelfSelecting = selecting)}
      />
    {:else if activeTab === "memory"}
      <MemoryView
        active={activeRoute === "memory"}
        launchBlocked={readerLaunchInFlight}
        onOpenReaderJump={openMemoryBook}
      />
    {:else}
      <AppSettingsView />
    {/if}
  </div>
{/if}

{#if readerCover && coverFrame && paperApproach}
  <div class="opening-veil" data-opening-theme={openingTheme} aria-hidden="true"></div>
  <div class="opening-art" data-opening-theme={openingTheme}>
    <BookOpeningCover
      cover={readerCover}
      frame={coverFrame}
      approach={paperApproach}
      turning={coverTurning}
      onPoised={() => { coverPoised = true; }}
      onFinish={finishEntry}
    />
  </div>
{/if}

{#if entryActive || (activeRoute === "reader" && readerScreen === "waiting")}
  <button
    bind:this={openingBack}
    type="button"
    class="icon-btn opening-back"
    aria-label={activeTab === "memory" ? "返回阅读记忆" : "返回书架"}
    onclick={backFromOpening}
  >
    <ArrowLeft size={19} strokeWidth={1.8} />
  </button>
{/if}

{#if activeRoute === "reader"}
  <div
    class="reader-route"
    class:entry-hidden={entryActive && !coverTurning}
    class:entry-turning={coverTurning}
    inert={entryActive}
    data-opening-theme={openingTheme}
    style:--cover-inset={openingInset}
    style:--paper-from-x={paperApproach ? `${paperApproach.fromX}px` : undefined}
    style:--paper-from-y={coverFrame ? `${coverFrame.top}px` : undefined}
    style:--paper-from-scale={paperApproach?.scale}
  >
    {#if ReaderView}
      <div class="reader-camera" data-opening-camera={coverTurning ? "" : undefined}>
        <ReaderView
          book={readerBook}
          manifestPath={readerManifestPath}
          initialJump={readerJump}
          {openingTheme}
          entryBlocked={entryActive}
          onScreenState={handleReaderScreen}
          onBackToShelf={() => void handleBackToShelf()}
        />
      </div>
    {:else if readerBootError}
      <div class="reader-boot-error" role="alert">
        <p>未能加载阅读器。</p>
        <button type="button" class="pill-btn" onclick={() => void handleBackToShelf()}>返回书架</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .app-frame {
    min-height: 100vh;
    padding: var(--safe-area-top) var(--safe-area-right) var(--safe-area-bottom) var(--safe-area-left);
    background: var(--bg-canvas);
    color: var(--ink-primary);
  }

  .app-frame :global(.library-shell),
  .app-frame :global(.memory-shell),
  .app-frame :global(.settings-shell) {
    min-height: calc(100vh - var(--safe-area-top) - var(--safe-area-bottom));
  }

  .app-frame.behind-opening {
    isolation: isolate;
  }

  @media (max-width: 899px) {
    .app-frame.shelf-selecting :global(.nav-shell) {
      display: none;
    }
  }

  .reader-route, .opening-veil, .opening-art {
    position: fixed;
    inset: 0;
  }

  .reader-route {
    z-index: 1;
    background: var(--opening-canvas);
  }

  .reader-route.entry-hidden {
    opacity: 0;
  }

  .reader-camera {
    height: 100%;
    transform-origin: top left;
  }

  .opening-veil {
    z-index: 96;
    pointer-events: none;
    background: var(--opening-canvas);
    animation: shade-shelf 420ms var(--ease-smooth) both;
  }

  .opening-art {
    z-index: 98;
    pointer-events: none;
  }

  .opening-back {
    position: fixed;
    top: calc(var(--safe-area-top) + 12px);
    left: calc(var(--safe-area-left) + 12px);
    z-index: 99;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-pill);
    background: var(--bg-canvas);
    color: var(--ink-primary);
  }

  .reader-boot-error {
    display: grid;
    justify-items: center;
    align-content: center;
    height: 100%;
  }

  @media (prefers-reduced-motion: no-preference) {
    .app-frame :global([data-cover-lifted] .cover-art) {
      visibility: hidden;
    }
    .reader-route.entry-turning {
      z-index: 97;
      animation: reveal-paper var(--book-approach);
    }
    .reader-camera[data-opening-camera] {
      animation: approach-paper var(--book-approach);
    }
  }

  @keyframes shade-shelf {
    from { opacity: 0; }
    to { opacity: 0.84; }
  }

  @keyframes reveal-paper {
    from { clip-path: inset(var(--cover-inset) round 7px); }
    100% { clip-path: inset(0 round 0); }
  }

  @keyframes approach-paper {
    from { transform: translate(var(--paper-from-x), var(--paper-from-y)) scale(var(--paper-from-scale)); }
    to { transform: translate(0, 0) scale(1); }
  }
</style>
