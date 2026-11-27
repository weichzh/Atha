<script lang="ts">
  import { BookOpen, X } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { DictionaryFontScale, DictionaryLookup } from "../../dictionary.ts";
  import { dictionaryFontScales, saveDictionaryFontScale } from "../data/library-adapter.ts";

  interface Props {
    query: string;
    lookup: DictionaryLookup | null;
    loading: boolean;
    fontScale: DictionaryFontScale;
    onScaleChange: (scale: DictionaryFontScale) => void;
    onClose: () => void;
  }

  let { query, lookup, loading, fontScale, onScaleChange, onClose }: Props = $props();

  function sanitizeDefHtml(raw: string): string {
    const parser = new DOMParser();
    const doc = parser.parseFromString(`<body>${raw}</body>`, "text/html");
    doc.querySelectorAll("script, iframe, object, embed, style, link").forEach((el) => el.remove());
    for (const el of doc.body.querySelectorAll("*")) {
      for (const attr of [...el.attributes]) {
        if (attr.name.toLowerCase().startsWith("on")) el.removeAttribute(attr.name);
      }
    }
    return doc.body.innerHTML;
  }

  function stepScale(delta: number) {
    const idx = dictionaryFontScales.indexOf(fontScale);
    const nextIdx = Math.max(0, Math.min(dictionaryFontScales.length - 1, idx + delta));
    const next = dictionaryFontScales[nextIdx];
    saveDictionaryFontScale(next);
    onScaleChange(next);
  }
</script>

<aside
  class="dict-card glass-popover"
  aria-label="离线查词结果"
  transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
  onpointerdown={(e) => e.stopPropagation()}
>
  <div class="dict-head">
    <div class="dict-badge">
      <BookOpen size={14} />
      <span>离线词典</span>
    </div>
    <div class="dict-controls">
      <button
        type="button"
        class="scale-step"
        aria-label="缩小词典字号"
        disabled={fontScale === dictionaryFontScales[0]}
        onclick={() => stepScale(-1)}
      >
        A-
      </button>
      <span class="scale-val tabular">{Math.round(fontScale * 100)}%</span>
      <button
        type="button"
        class="scale-step"
        aria-label="放大词典字号"
        disabled={fontScale === dictionaryFontScales.at(-1)}
        onclick={() => stepScale(1)}
      >
        A+
      </button>
      <button type="button" class="icon-btn" aria-label="关闭词典" onclick={onClose}>
        <X size={16} />
      </button>
    </div>
  </div>

  <div class="dict-body" style:font-size={`${Math.round(14.5 * fontScale)}px`}>
    {#if loading}
      <p class="dict-loading">正在查询 “{query}”…</p>
    {:else if lookup}
      <div class="dict-html">{@html sanitizeDefHtml(lookup.definitionHtml || lookup.definition)}</div>
    {:else}
      <p class="dict-loading">未找到 “{query}” 的词条。</p>
    {/if}
  </div>
</aside>

<style>
  .dict-card {
    position: fixed;
    right: 24px;
    bottom: 84px;
    z-index: 66;
    width: min(calc(100vw - 32px), 380px);
    max-height: 54vh;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-xl);
    padding: 16px 18px 18px;
  }

  .dict-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 10px;
  }

  .dict-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--accent);
  }

  .dict-controls {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .scale-step {
    min-width: 30px;
    height: 28px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-secondary);
  }

  .scale-val {
    font-size: 11.5px;
    color: var(--ink-muted);
    min-width: 38px;
    text-align: center;
  }

  .dict-body {
    overflow-y: auto;
    line-height: 1.65;
    color: var(--ink-primary);
  }

  .dict-loading {
    margin: 12px 0;
    color: var(--ink-muted);
  }

  .dict-html :global(p) {
    margin: 0 0 0.65em;
  }

  .dict-html :global(p:last-child) {
    margin-bottom: 0;
  }

  @media (max-width: 640px) {
    .dict-card {
      left: 16px;
      right: 16px;
      width: auto;
      bottom: 86px;
    }
  }
</style>
