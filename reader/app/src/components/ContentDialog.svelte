<script lang="ts">
  import { Maximize2, Minus, Plus, X } from "@lucide/svelte";
  import { onMount } from "svelte";

  const MIN_SCALE = 0.5;
  const MAX_SCALE = 4;
  const SCALE_STEP = 0.25;

  let viewport: HTMLDivElement;
  let scale = $state(1);
  let pinchDistance = 0;
  let pinchScale = 1;

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

  function isScalablePreview() {
    return viewport?.closest("dialog")?.matches(".media-preview, .table-preview") ?? false;
  }

  function zoomAt(next: number, clientX: number, clientY: number) {
    const previous = scale;
    const clamped = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    if (clamped === previous) return;
    const bounds = viewport.getBoundingClientRect();
    const x = clientX - bounds.left;
    const y = clientY - bounds.top;
    scale = clamped;
    requestAnimationFrame(() => {
      const ratio = clamped / previous;
      viewport.scrollLeft = (viewport.scrollLeft + x) * ratio - x;
      viewport.scrollTop = (viewport.scrollTop + y) * ratio - y;
    });
  }

  function touchDistance(touches: TouchList) {
    return Math.hypot(
      touches[1].clientX - touches[0].clientX,
      touches[1].clientY - touches[0].clientY,
    );
  }

  onMount(() => {
    const handleWheel = (event: WheelEvent) => {
      if (!isScalablePreview()) return;
      event.preventDefault();
      const unit = event.deltaMode === WheelEvent.DOM_DELTA_LINE
        ? 16
        : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
          ? viewport.clientHeight
          : 1;
      const delta = Math.max(-240, Math.min(240, event.deltaY * unit));
      zoomAt(scale * Math.exp(-delta * 0.002), event.clientX, event.clientY);
    };
    const handleTouchStart = (event: TouchEvent) => {
      if (!isScalablePreview() || event.touches.length !== 2) return;
      event.preventDefault();
      pinchDistance = touchDistance(event.touches);
      pinchScale = scale;
    };
    const handleTouchMove = (event: TouchEvent) => {
      if (!isScalablePreview() || event.touches.length !== 2 || pinchDistance <= 0) return;
      event.preventDefault();
      const x = (event.touches[0].clientX + event.touches[1].clientX) / 2;
      const y = (event.touches[0].clientY + event.touches[1].clientY) / 2;
      zoomAt(pinchScale * touchDistance(event.touches) / pinchDistance, x, y);
    };
    const finishPinch = () => {
      pinchDistance = 0;
    };

    viewport.addEventListener("wheel", handleWheel, { passive: false });
    viewport.addEventListener("touchstart", handleTouchStart, { passive: false });
    viewport.addEventListener("touchmove", handleTouchMove, { passive: false });
    viewport.addEventListener("touchend", finishPinch);
    viewport.addEventListener("touchcancel", finishPinch);
    return () => {
      viewport.removeEventListener("wheel", handleWheel);
      viewport.removeEventListener("touchstart", handleTouchStart);
      viewport.removeEventListener("touchmove", handleTouchMove);
      viewport.removeEventListener("touchend", finishPinch);
      viewport.removeEventListener("touchcancel", finishPinch);
    };
  });

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
