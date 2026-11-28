<script lang="ts">
  import { autoUpdate, computePosition, flip, offset, shift } from "@floating-ui/dom";
  import { BookOpen, Copy, Highlighter, MessageSquarePlus, RefreshCw, Trash2 } from "@lucide/svelte";
  import { prefersReducedMotion as reducedMotion } from "svelte/motion";
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { ActiveSelectionState } from "../types.ts";

  interface Props {
    selection: ActiveSelectionState;
    onHighlight: () => void;
    onDiscuss: () => void;
    onLookup: () => void;
    onCopy: () => void;
    onReselect?: () => void;
    onDelete?: () => void;
  }

  let {
    selection,
    onHighlight,
    onDiscuss,
    onLookup,
    onCopy,
    onReselect,
    onDelete,
  }: Props = $props();

  let pillEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const rect = selection.rect;
    const element = pillEl;
    if (!element) return;
    const virtualEl = { getBoundingClientRect: () => rect };
    let active = true;
    const update = () => {
      const style = getComputedStyle(document.documentElement);
      const inset = (edge: string) => Math.max(14, parseFloat(style.getPropertyValue(`--safe-area-${edge}`)) || 0);
      const padding = { top: inset("top"), right: inset("right"), bottom: inset("bottom"), left: inset("left") };
      void computePosition(virtualEl, element, {
        placement: "top",
        strategy: "fixed",
        middleware: [offset(12), flip({ padding }), shift({ padding, crossAxis: true })],
      }).then(({ x, y }) => {
        if (!active) return;
        element.style.left = `${x}px`;
        element.style.top = `${y}px`;
      });
    };
    const stop = autoUpdate(virtualEl, element, update);
    window.addEventListener("atha-safe-area-change", update);
    return () => {
      active = false;
      stop();
      window.removeEventListener("atha-safe-area-change", update);
    };
  });
</script>

<div
  bind:this={pillEl}
  class="selection-capsule"
  role="toolbar"
  tabindex="-1"
  aria-label="选区操作"
  transition:scale={{ start: 0.94, duration: reducedMotion.current ? 0 : 150, easing: cubicOut }}
>
  {#if selection.existingRootId}
    <button type="button" class="cap-btn" onclick={onDiscuss}>
      <MessageSquarePlus size={15} />
      <span>对话</span>
    </button>
    {#if onReselect}
      <button type="button" class="cap-btn" onclick={onReselect}>
        <RefreshCw size={15} />
        <span>更新</span>
      </button>
    {/if}
    <button type="button" class="cap-btn" onclick={onLookup}>
      <BookOpen size={15} />
      <span>查词</span>
    </button>
    {#if onDelete}
      <button type="button" class="cap-btn danger" onclick={onDelete}>
        <Trash2 size={15} />
        <span>删除</span>
      </button>
    {/if}
  {:else}
    <button type="button" class="cap-btn" onclick={onHighlight}>
      <Highlighter size={15} />
      <span>标注</span>
    </button>
    <button type="button" class="cap-btn primary" onclick={onDiscuss}>
      <MessageSquarePlus size={15} />
      <span>对话</span>
    </button>
    <button type="button" class="cap-btn" onclick={onLookup}>
      <BookOpen size={15} />
      <span>查词</span>
    </button>
    <button type="button" class="cap-btn" onclick={onCopy}
      onpointerdown={(event) => { if (event.pointerType === "mouse") event.preventDefault(); }}>
      <Copy size={15} />
      <span>复制</span>
    </button>
  {/if}
</div>

<style>
  .selection-capsule {
    position: fixed;
    z-index: 65;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 5px 6px;
    border-radius: var(--radius-pill);
    background: var(--bg-selection-pill);
    color: var(--ink-selection-pill);
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    box-shadow: var(--shadow-popover);
    max-width: calc(100vw - max(14px, var(--safe-area-left)) - max(14px, var(--safe-area-right)));
    overflow-x: auto;
  }

  .cap-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 12px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 500;
    color: inherit;
    white-space: nowrap;
    transition: background-color 140ms var(--ease-smooth);
  }

  .cap-btn:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .cap-btn.primary {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .cap-btn.primary:hover {
    background: var(--accent-hover);
  }

  .cap-btn.danger {
    color: #f48c84;
  }

  @media (pointer: coarse) {
    .cap-btn {
      min-height: 44px;
      padding-inline: 8px;
    }
  }
</style>
