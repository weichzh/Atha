<script lang="ts">
  import { ArrowUpRight, X } from "@lucide/svelte";
  import { onMount } from "svelte";
  import type { FootnotePreview } from "../types.ts";

  let { footnote, onClose, onJump }: {
    footnote: FootnotePreview;
    onClose: () => void;
    onJump: () => void;
  } = $props();
  const titleId = $props.id();
  let dialog: HTMLDialogElement;
  let closeButton: HTMLButtonElement;
  let width = $state(window.innerWidth);
  let height = $state(window.innerHeight);
  let pressedOutside = false;

  const position = $derived.by(() => {
    const rect = footnote.anchorRect;
    const cardWidth = Math.min(360, width - 32);
    const left = Math.max(16, Math.min(width - cardWidth - 16, (rect.left + rect.right - cardWidth) / 2));
    const below = height - rect.bottom - 28;
    const above = rect.top - 28;
    const placeBelow = below >= 260 || below >= above;
    if (width < 640 || Math.max(above, below) < 150) return null;
    return {
      left,
      top: placeBelow ? rect.bottom + 12 : undefined,
      bottom: placeBelow ? undefined : height - rect.top + 12,
      maxHeight: Math.min(320, placeBelow ? below : above),
    };
  });

  onMount(() => {
    dialog.showModal();
    closeButton.focus({ preventScroll: true });
  });

  export function close() {
    dialog?.close();
  }

  function outside(event: PointerEvent): boolean {
    const { left, right, top, bottom } = dialog.getBoundingClientRect();
    return event.clientX < left || event.clientX > right || event.clientY < top || event.clientY > bottom;
  }

  function handleClose() {
    const jump = dialog.returnValue === "jump";
    dialog.returnValue = "";
    if (jump) onJump();
    else onClose();
  }
</script>

<svelte:window bind:innerWidth={width} bind:innerHeight={height} />

<dialog
  bind:this={dialog}
  class="footnote-popover"
  class:anchored={position !== null}
  aria-labelledby={titleId}
  {...{ closedby: "any" }}
  style:--footnote-left={position ? `${position.left}px` : undefined}
  style:--footnote-top={position?.top === undefined ? undefined : `${position.top}px`}
  style:--footnote-bottom={position?.bottom === undefined ? undefined : `${position.bottom}px`}
  style:--footnote-height={position ? `${position.maxHeight}px` : undefined}
  onclose={handleClose}
  onpointerdown={(event) => { pressedOutside = !("closedBy" in HTMLDialogElement.prototype) && outside(event); }}
  onpointerup={(event) => {
    if (pressedOutside && outside(event)) dialog.close();
    pressedOutside = false;
  }}
  onpointercancel={() => { pressedOutside = false; }}
>
  <header class="footnote-head">
    <h1 id={titleId}>脚注 {footnote.label}</h1>
    <form method="dialog" class="footnote-actions">
      <button type="submit" value="jump" class="pill-btn">
        <ArrowUpRight size={14} />
        跳转原文
      </button>
      <button bind:this={closeButton} type="submit" value="close" class="icon-btn" aria-label="关闭注释">
        <X size={17} />
      </button>
    </form>
  </header>
  <p class="footnote-text">{footnote.text || "这条脚注没有可预览的文字。可跳转到原文查看。"}</p>
</dialog>

<style>
  .footnote-popover {
    position: fixed;
    inset: auto calc(12px + var(--safe-area-right)) calc(12px + var(--safe-area-bottom))
      calc(12px + var(--safe-area-left));
    width: auto;
    margin: 0;
    max-width: none;
    max-height: min(52dvh, 360px);
    overflow: auto;
    overscroll-behavior: contain;
    padding: 14px 16px 16px;
    border: 0;
    border-radius: var(--radius-lg);
    background: var(--bg-elevated);
    color: var(--ink-primary);
    box-shadow: var(--shadow-popover);
  }

  .footnote-popover.anchored {
    inset: var(--footnote-top, auto) auto var(--footnote-bottom, auto) var(--footnote-left);
    width: 360px;
    max-height: var(--footnote-height);
  }

  .footnote-popover::backdrop {
    overflow: hidden;
    overscroll-behavior: contain;
    background: var(--bg-scrim);
  }

  .footnote-head, .footnote-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .footnote-head {
    justify-content: space-between;
    margin-bottom: 5px;
  }

  h1 {
    font-size: 15px;
    margin: 0;
    overflow-wrap: anywhere;
  }

  .icon-btn, .pill-btn {
    min-height: 44px;
  }

  .icon-btn {
    width: 44px;
  }

  .footnote-text {
    margin: 0;
    font-size: 15px;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
</style>
