<script lang="ts">
  import { onMount } from "svelte";
  import type { LibraryBook, ReadingMemoryJump } from "./library.ts";
  import AppSettingsView from "./lib/components/AppSettingsView.svelte";
  import LibraryView from "./lib/components/LibraryView.svelte";
  import MemoryView from "./lib/components/MemoryView.svelte";
  import NavRail from "./lib/components/NavRail.svelte";
  import { DEMO_BOOKS, DEMO_PRIMARY_BOOK_ID } from "./lib/data/demo-seed.ts";
  import {
    applyAppThemeToDocument,
    getAppTheme,
    listLibraryBooks,
  } from "./lib/data/library-adapter.ts";
  import { hasReaderLaunchParams, installDemoFetchBridge, isTauriRuntime } from "./lib/data/runtime.ts";
  import type { AppRoute } from "./lib/types.ts";

  interface Props {
    hasLibraryReturn?: boolean;
  }

  let { hasLibraryReturn = false }: Props = $props();

  const initialReaderLaunch = hasReaderLaunchParams();
  const initialParams = initialReaderLaunch ? new URLSearchParams(window.location.search) : null;
  const tauriOnReader = isTauriRuntime() && initialReaderLaunch;

  function parseInitialJump(params: URLSearchParams | null, editionId: string): ReadingMemoryJump | null {
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
      canonicalLocator: JSON.stringify({
        schema: 1,
        contentVersion: editionId,
        start: { section: "ch1", offset: 0 },
      }),
    };
  }

  let activeRoute = $state<AppRoute>(tauriOnReader ? "reader" : "library");
  let activeTab = $state<Exclude<AppRoute, "reader">>("library");
  let bookCount = $state(isTauriRuntime() ? 0 : DEMO_BOOKS.length);

  const initialVersion =
    initialParams?.get("version") ||
    initialParams?.get("state") ||
    (isTauriRuntime() ? "" : DEMO_PRIMARY_BOOK_ID);

  let readerBook = $state<LibraryBook>({
    ...DEMO_BOOKS[0],
    id: initialVersion,
    title: isTauriRuntime() ? "正在打开…" : DEMO_BOOKS[0].title,
  });
  let readerManifestPath = $state(
    initialParams?.get("manifest") || (isTauriRuntime() ? "" : "demo/manifest.json"),
  );
  let readerJump = $state<ReadingMemoryJump | null>(
    parseInitialJump(initialParams, initialVersion),
  );

  onMount(() => {
    if (!isTauriRuntime()) {
      installDemoFetchBridge();
    }
    if (!initialReaderLaunch) {
      applyAppThemeToDocument(getAppTheme());
    }

    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const onSystemChange = () => {
      if (!hasReaderLaunchParams() && getAppTheme() === "system") {
        applyAppThemeToDocument("system");
      }
    };
    mq?.addEventListener("change", onSystemChange);

    if (initialReaderLaunch && !isTauriRuntime()) {
      const manifest = initialParams?.get("manifest") || "demo/manifest.json";
      const version = initialParams?.get("version") || DEMO_PRIMARY_BOOK_ID;
      void listLibraryBooks().then((list) => {
        bookCount = list.length;
        readerBook = list.find((b) => b.id === version) ?? {
          ...DEMO_BOOKS[0],
          id: version,
        };
        readerManifestPath = manifest;
        readerJump = parseInitialJump(initialParams, version);
        activeRoute = "reader";
      });
    } else if (!initialReaderLaunch && !isTauriRuntime()) {
      void listLibraryBooks().then((list) => {
        bookCount = list.length;
      });
    }

    return () => {
      mq?.removeEventListener("change", onSystemChange);
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
    activeTab = tab;
    activeRoute = tab;
  }

  function openReader(book: LibraryBook, manifestPath: string, jump: ReadingMemoryJump | null = null) {
    readerBook = book;
    readerManifestPath = manifestPath;
    readerJump = jump;
    activeRoute = "reader";
  }

  function handleBackToShelf() {
    try {
      window.__athaFlushOnExit?.();
    } catch {
      // A storage flush error must not block exiting the reader view.
    }
    applyAppThemeToDocument(getAppTheme());
    if (hasReaderLaunchParams()) {
      if (hasLibraryReturn && cameFromAppLibraryRoot()) {
        history.back();
        return;
      }
      location.replace(new URL("/", location.href).href);
      return;
    }
    readerJump = null;
    activeTab = "library";
    activeRoute = "library";
  }
</script>

{#if activeRoute === "reader"}
  {#await import("./lib/components/ReaderView.svelte")}
    <div class="reader-boot">正在开启书页…</div>
  {:then { default: ReaderView }}
    <ReaderView
      book={readerBook}
      manifestPath={readerManifestPath}
      initialJump={readerJump}
      onBackToShelf={handleBackToShelf}
    />
  {/await}
{:else}
  <div class="app-frame">
    <NavRail {activeTab} {bookCount} onSelect={switchTab} />

    {#if activeTab === "library"}
      <LibraryView
        onOpenReader={(book, manifest) => openReader(book, manifest, null)}
        onBooksUpdated={(list) => (bookCount = list.length)}
      />
    {:else if activeTab === "memory"}
      <MemoryView
        onOpenReaderJump={(book, manifest, jump) => openReader(book, manifest, jump)}
      />
    {:else}
      <AppSettingsView />
    {/if}
  </div>
{/if}

<style>
  .app-frame {
    min-height: 100vh;
    background: var(--bg-canvas);
    color: var(--ink-primary);
  }

  .reader-boot {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-canvas);
    color: var(--ink-muted);
    font-size: 14px;
  }
</style>
