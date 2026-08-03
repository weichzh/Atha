<script lang="ts">
  import readerStyleHref from "../../atha-reader.css?url";

  import ContentDialog from "./components/ContentDialog.svelte";
  import ReaderCanvas from "./components/ReaderCanvas.svelte";
  import ReaderChrome from "./components/ReaderChrome.svelte";
</script>

<svelte:head>
  <link id="reader-style-source" rel="stylesheet" href={readerStyleHref} />
</svelte:head>

<main class="reader-shell" aria-label="分页阅读器">
  <ReaderCanvas />
  <p id="error" class="error" role="alert" hidden>无法安全打开这份内容。</p>
  <ReaderChrome />
  <ContentDialog />
</main>
