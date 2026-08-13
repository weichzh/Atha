<script lang="ts">
  import { Maximize2, Minus, Plus, X } from "@lucide/svelte";

  const MIN_SCALE = 0.5;
  const MAX_SCALE = 4;
  const SCALE_STEP = 0.25;

  let viewport: HTMLDivElement;
  let scale = $state(1);

  function setScale(next: number) {
    scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
  }

  function resetScale() {
    scale = 1;
    if (viewport) {
      viewport.scrollLeft = 0;
      viewport.scrollTop = 0;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key === "=" || event.key === "+") setScale(scale + SCALE_STEP);
    else if (event.key === "-") setScale(scale - SCALE_STEP);
    else if (event.key === "0") resetScale();
    else return;
    event.preventDefault();
  }
</script>

<dialog
  id="content-dialog"
  aria-labelledby="content-dialog-title"
  style={`--content-viewer-scale: ${scale}`}
  onclose={resetScale}
  onkeydown={handleKeydown}
>
  <header class="content-dialog-heading">
    <h2 id="content-dialog-title">内容预览</h2>
    <form method="dialog">
      <button
        id="close-content-dialog"
        class="content-dialog-icon-button"
        value="close"
        aria-label="关闭"
        title="关闭"
      ><X aria-hidden="true" /></button>
    </form>
  </header>

  <output class="content-dialog-zoom-label" aria-live="polite">{Math.round(scale * 100)}%</output>
  <div class="content-dialog-zoom-controls" role="toolbar" aria-label="缩放">
    <button
      class="content-dialog-icon-button"
      type="button"
      aria-label="放大"
      title="放大"
      disabled={scale >= MAX_SCALE}
      onclick={() => setScale(scale + SCALE_STEP)}
    ><Plus aria-hidden="true" /></button>
    <button
      class="content-dialog-icon-button"
      type="button"
      aria-label="缩小"
      title="缩小"
      disabled={scale <= MIN_SCALE}
      onclick={() => setScale(scale - SCALE_STEP)}
    ><Minus aria-hidden="true" /></button>
    <button
      class="content-dialog-icon-button"
      type="button"
      aria-label="恢复原始大小"
      title="恢复原始大小"
      onclick={resetScale}
    ><Maximize2 aria-hidden="true" /></button>
  </div>

  <div bind:this={viewport} id="content-dialog-viewport">
    <p id="content-dialog-body"></p>
    <img id="content-dialog-image" hidden alt="" />
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div id="content-dialog-table" role="region" tabindex="0" aria-label="表格预览" hidden></div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <pre id="content-dialog-code" role="region" tabindex="0" aria-label="代码预览" hidden></pre>
  </div>
</dialog>
