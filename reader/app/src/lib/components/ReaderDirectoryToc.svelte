<script lang="ts">
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { onMount, untrack } from "svelte";
  import { buildTocTree, initialTocExpansion, type TocNode } from "../data/toc-tree.ts";
  import type { TocEntry } from "../types.ts";

  let { entries, activeIndex, onSelect }: {
    entries: readonly TocEntry[];
    activeIndex: number;
    onSelect: (index: number) => void;
  } = $props();

  const tree = $derived(buildTocTree(entries));
  let expanded = $state.raw(untrack(() => initialTocExpansion(tree, activeIndex)));
  let list: HTMLElement;
  const id = $props.id();

  function toggle(index: number) {
    const next = new Set(expanded);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    expanded = next;
  }

  onMount(() => {
    const active = list.querySelector('[aria-current="location"]');
    if (!active) return;
    const viewport = list.getBoundingClientRect();
    const row = active.getBoundingClientRect();
    if (row.top < viewport.top) list.scrollTop += row.top - viewport.top;
    else if (row.bottom > viewport.bottom) list.scrollTop += row.bottom - viewport.bottom;
  });
</script>

{#snippet branch(nodes: readonly TocNode[], depth: number)}
  <ul class="toc-list">
    {#each nodes as node (node.index)}
      <li>
        <div class="toc-row" style:--toc-depth={depth}>
          <button
            type="button"
            class="toc-jump"
            aria-current={node.index === activeIndex ? "location" : undefined}
            onclick={() => onSelect(node.index)}
          >{node.label}</button>
          {#if node.children.length}
            <button
              type="button"
              class="toc-toggle"
              aria-expanded={expanded.has(node.index)}
              aria-controls={`${id}-${node.index}`}
              aria-label={`${expanded.has(node.index) ? "折叠" : "展开"}：${node.label}`}
              onclick={() => toggle(node.index)}
            >
              {#if expanded.has(node.index)}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
            </button>
          {/if}
        </div>
        {#if node.children.length}
          <div id={`${id}-${node.index}`} hidden={!expanded.has(node.index)}>
            {#if expanded.has(node.index)}
              {@render branch(node.children, depth + 1)}
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div class="toc-viewport" bind:this={list}>
  {@render branch(tree, 0)}
</div>

<style>
  .toc-viewport {
    max-height: 52vh;
    overflow-y: auto;
  }

  .toc-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .toc-row {
    display: flex;
    gap: 4px;
    padding-inline-start: calc(var(--toc-depth) * 12px);
    margin-block-end: 4px;
  }

  .toc-jump, .toc-toggle {
    min-height: 44px;
    border-radius: var(--radius-md);
  }

  .toc-jump {
    flex: 1;
    min-width: 0;
    padding: 11px 14px;
    text-align: start;
    overflow-wrap: anywhere;
    font-size: 14px;
    color: var(--ink-primary);
  }

  .toc-toggle {
    width: 44px;
    flex: none;
    display: grid;
    place-items: center;
    color: var(--ink-secondary);
  }

  .toc-jump:hover, .toc-toggle:hover {
    background: var(--bg-well);
  }

  .toc-jump:focus-visible, .toc-toggle:focus-visible {
    outline-offset: -2px;
  }

  .toc-jump[aria-current] {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
</style>
