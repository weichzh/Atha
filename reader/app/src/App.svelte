<script lang="ts">
  import readerStyleHref from "../../atha-reader.css?url";

  import ContentDialog from "./components/ContentDialog.svelte";
  import LibraryView from "./components/LibraryView.svelte";
  import ReaderCanvas from "./components/ReaderCanvas.svelte";
  import ReaderChrome from "./components/ReaderChrome.svelte";
  import { isReaderRoute } from "./route";

  const readerRoute = isReaderRoute();
</script>

<svelte:head>
  {#if readerRoute}
    <link id="reader-style-source" rel="stylesheet" href={readerStyleHref} />
  {/if}
</svelte:head>

{#if readerRoute}
  <main class="reader-shell" aria-label="分页阅读器">
    <ReaderChrome />
    <ReaderCanvas />
    <p id="error" class="error" role="alert" hidden>无法安全打开这份内容。</p>
    <ContentDialog />
  </main>
{:else}
  <LibraryView />
{/if}
