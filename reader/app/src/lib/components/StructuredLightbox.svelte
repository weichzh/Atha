<script lang="ts">
  import { X } from "@lucide/svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { LightboxPayload } from "../types.ts";

  interface Props {
    payload: LightboxPayload | null;
    colorScheme: "light" | "dark";
    onClose: () => void;
  }

  let { payload, colorScheme, onClose }: Props = $props();
</script>

<svelte:window onkeydown={(e) => payload && e.key === "Escape" && onClose()} />

{#if payload}
  <div
    class="atha-lightbox"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={payload.title}
    transition:fade={{ duration: 180 }}
    onclick={(e) => e.target === e.currentTarget && onClose()}
    onkeydown={(e) => e.key === "Escape" && onClose()}
  >
    <div
      class="lightbox-surface"
      transition:scale={{ start: 0.96, duration: 200, easing: cubicOut }}
    >
      <div class="lightbox-head">
        <span class="lightbox-title">{payload.title}</span>
        <button type="button" class="lightbox-close" aria-label="关闭大图" onclick={onClose}>
          <X size={18} />
        </button>
      </div>
      <div class="lightbox-stage" style:color-scheme={colorScheme}>
        {@html payload.html}
      </div>
    </div>
  </div>
{/if}

<style>
  .atha-lightbox {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 28px;
    background: rgba(16, 15, 13, 0.78);
    backdrop-filter: blur(22px);
    -webkit-backdrop-filter: blur(22px);
  }

  .lightbox-surface {
    width: min(92vw, 880px);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-xl);
    background: var(--bg-surface);
    color: var(--ink-primary);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  .lightbox-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 22px;
  }

  .lightbox-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--ink-secondary);
  }

  .lightbox-close {
    width: 34px;
    height: 34px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-well);
    color: var(--ink-primary);
  }

  .lightbox-stage {
    padding: 8px 26px 28px;
    overflow: auto;
    display: flex;
    justify-content: center;
  }

  .lightbox-stage :global(svg),
  .lightbox-stage :global(img) {
    max-width: 100%;
    height: auto;
  }

  .lightbox-stage :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }

  .lightbox-stage :global(th),
  .lightbox-stage :global(td) {
    padding: 10px 14px;
    text-align: left;
  }

  .lightbox-stage :global(thead th) {
    background: var(--bg-well);
    font-weight: 600;
  }

  .lightbox-stage :global(tbody tr:nth-child(even)) {
    background: var(--bg-well);
  }

  .lightbox-stage :global(pre) {
    width: 100%;
    margin: 0;
    padding: 18px 20px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    font-family: var(--font-mono);
    font-size: 13.5px;
    line-height: 1.65;
    overflow-x: auto;
  }
</style>
