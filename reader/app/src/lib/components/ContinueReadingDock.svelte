<script lang="ts">
  import { BookOpen, ChevronUp, Minimize2, X } from "@lucide/svelte";
  import { tick } from "svelte";
  import type { ShelfEntry } from "../../shelf-browse.ts";
  import BookCover from "./BookCover.svelte";

  interface Props {
    entries: ShelfEntry[];
    collapsed: boolean;
    compact: boolean;
    open: boolean;
    scrollX: number;
    coverRevision: Record<string, number>;
    onCollapse: (collapsed: boolean) => void;
    onRead: (entry: ShelfEntry, trigger: HTMLButtonElement) => void;
  }

  let { entries, collapsed, compact, open = $bindable(), scrollX = $bindable(), coverRevision, onCollapse, onRead }: Props = $props();
  let drawer = $state<HTMLDialogElement>();
  let strip = $state<HTMLElement>();
  let historyTrigger = $state<HTMLButtonElement>();
  const latest = $derived(entries[0]);
  const iconOnly = $derived(collapsed || compact);

  $effect(() => {
    if (!drawer) return;
    if (open && !drawer.open) {
      drawer.showModal();
      if (strip) strip.scrollLeft = scrollX;
    }
    if (!open && drawer.open) drawer.close();
  });

  async function setCollapsed(value: boolean) {
    open = false;
    onCollapse(value);
    await tick();
    historyTrigger?.focus({ preventScroll: true });
  }
</script>

{#if latest}
  <aside class="resume-dock" class:icon-only={iconOnly} aria-label="继续阅读">
    {#if iconOnly}
      <button
        class="icon-btn dock-launch"
        type="button"
        bind:this={historyTrigger}
        data-shelf-recent-return
        aria-label="展开最近在读"
        aria-haspopup="dialog"
        onclick={() => (open = true)}
      >
        <BookOpen size={20} />
      </button>
    {:else}
      <div class="resume-bar glass-island">
        <button
          class="resume-book"
          type="button"
          data-shelf-area="recent"
          data-book-id={latest.book.id}
          title={`继续阅读《${latest.book.title}》`}
          onclick={(event) => onRead(latest, event.currentTarget)}
        >
          <div class="mini-cover">
            <BookCover book={latest.book} index={0} compact coverRevision={coverRevision[latest.book.id] ?? 0} />
          </div>
          <div class="resume-caption">
            <span class="resume-label">继续阅读{latest.fact?.percent != null ? ` · ${latest.fact.percent}%` : ""}</span>
            <span class="resume-title">{latest.book.title}</span>
          </div>
        </button>
        <button
          class="icon-btn"
          type="button"
          bind:this={historyTrigger}
          data-shelf-recent-return
          aria-label="展开最近在读"
          aria-haspopup="dialog"
          onclick={() => (open = true)}
        ><ChevronUp size={17} /></button>
        <button class="icon-btn" type="button" aria-label="收起继续阅读" onclick={() => void setCollapsed(true)}>
          <Minimize2 size={16} />
        </button>
      </div>
    {/if}
  </aside>

  <dialog
    class="recent-drawer"
    aria-labelledby="recent-drawer-title"
    bind:this={drawer}
    onclose={() => { if (!drawer?.open) open = false; }}
    oncancel={() => (open = false)}
  >
    <div class="drawer-head">
      <h2 id="recent-drawer-title">最近在读</h2>
      <div class="drawer-controls">
        {#if !compact}
          <button type="button" class="pill-btn" onclick={() => void setCollapsed(!collapsed)}>
            {collapsed ? "恢复阅读条" : "收起阅读条"}
          </button>
        {/if}
        <button type="button" class="icon-btn" aria-label="关闭最近在读" onclick={() => (open = false)}><X size={18} /></button>
      </div>
    </div>
    <div class="recent-strip" bind:this={strip} onscroll={(event) => (scrollX = event.currentTarget.scrollLeft)}>
      {#each entries as entry, index (entry.book.id)}
        <button
          type="button"
          class="recent-book"
          data-shelf-area="recent"
          data-book-id={entry.book.id}
          title={entry.book.title}
          onclick={(event) => {
            onRead(entry, event.currentTarget);
            open = false;
          }}
        >
          <BookCover book={entry.book} {index} percent={entry.fact?.percent ?? null} coverRevision={coverRevision[entry.book.id] ?? 0} />
          <span class="recent-title">{entry.book.title}</span>
        </button>
      {/each}
    </div>
  </dialog>
{/if}

<style>
  .resume-dock {
    position: fixed;
    z-index: 38;
    right: calc(24px + var(--safe-area-right));
    bottom: calc(24px + var(--safe-area-bottom));
  }

  .resume-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    width: min(356px, calc(100vw - 32px));
    padding: 8px;
    border-radius: var(--radius-lg);
    background: var(--bg-glass-strong);
  }

  .resume-book {
    flex: 1;
    min-width: 0;
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 4px;
    border-radius: var(--radius-sm);
    text-align: left;
  }

  .resume-book:hover { background: var(--bg-well); }
  .mini-cover { flex: none; width: 32px; }
  .resume-caption { display: grid; min-width: 0; gap: 2px; }
  .resume-label { color: var(--ink-secondary); font-size: 12px; }
  .resume-title { font-size: 14px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .icon-btn { width: 44px; height: 44px; }
  .dock-launch { background: var(--bg-surface); box-shadow: var(--shadow-popover); color: var(--accent); }

  .recent-drawer {
    position: fixed;
    inset: auto 0 max(16px, var(--safe-area-bottom));
    width: min(640px, calc(100% - 24px));
    margin: 0 auto;
    padding: 16px;
    border: 0;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    color: var(--ink-primary);
    box-shadow: var(--shadow-popover);
  }

  .recent-drawer::backdrop { background: var(--bg-scrim); }
  .drawer-head { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding-bottom: 12px; }
  .drawer-head h2 { font-size: 18px; margin: 0; }
  .drawer-controls { display: flex; align-items: center; gap: 4px; }
  .drawer-controls .pill-btn { min-height: 44px; }

  .recent-strip { display: flex; gap: 14px; overflow-x: auto; padding: 4px; }
  .recent-book { width: 106px; flex: none; display: grid; gap: 7px; align-content: start; text-align: left; }
  .recent-title {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    font-size: 13px;
    line-height: 1.4;
    font-weight: 600;
  }

  @media (max-width: 899px) {
    .resume-dock {
      right: calc(16px + var(--safe-area-right));
      bottom: calc(76px + var(--safe-area-bottom));
    }
    .resume-bar {
      width: min(356px, calc(100vw - 32px - var(--safe-area-left) - var(--safe-area-right)));
    }
    .recent-drawer {
      inset: auto 0 0;
      width: 100%;
      max-width: none;
      padding: 16px calc(16px + var(--safe-area-right)) max(16px, var(--safe-area-bottom)) calc(16px + var(--safe-area-left));
      border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    }
  }
</style>
