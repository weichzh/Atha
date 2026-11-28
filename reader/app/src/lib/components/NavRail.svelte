<script lang="ts">
  import { Library, Settings, Sparkles } from "@lucide/svelte";
  import brandIcon from "../../../src-tauri/icons/app-icon.svg?no-inline";
  import type { AppRoute } from "../types.ts";

  interface Props {
    activeTab: Exclude<AppRoute, "reader">;
    onSelect: (tab: Exclude<AppRoute, "reader">) => void;
    bookCount?: number;
  }

  let { activeTab, onSelect, bookCount = 0 }: Props = $props();

  const items: { id: Exclude<AppRoute, "reader">; label: string; icon: typeof Library }[] = [
    { id: "library", label: "书架", icon: Library },
    { id: "memory", label: "阅读记忆", icon: Sparkles },
    { id: "settings", label: "设置", icon: Settings },
  ];
</script>

<nav class="nav-shell" aria-label="主导航">
  <div class="nav-brand">
    <img class="brand-seal" src={brandIcon} width="34" height="34" alt="" />
    <div class="brand-text">
      <span class="brand-name">Atha</span>
      <span class="brand-sub tabular">{bookCount} 册藏书</span>
    </div>
  </div>

  <div class="nav-items" role="tablist" aria-label="视图切换">
    {#each items as item (item.id)}
      {@const Icon = item.icon}
      {@const active = activeTab === item.id}
      <button
        type="button"
        role="tab"
        aria-selected={active}
        class="nav-item"
        class:active
        onclick={() => onSelect(item.id)}
      >
        {#if active}
          <span class="active-indicator" aria-hidden="true"></span>
        {/if}
        <span class="nav-icon">
          <Icon size={18} strokeWidth={active ? 2.1 : 1.75} />
        </span>
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </div>
</nav>

<style>
  .nav-shell {
    position: fixed;
    left: calc(20px + var(--safe-area-left));
    top: calc(24px + var(--safe-area-top));
    bottom: calc(24px + var(--safe-area-bottom));
    width: 196px;
    z-index: 30;
    display: flex;
    flex-direction: column;
    padding: 20px 14px;
    border-radius: var(--radius-xl);
    background: var(--bg-glass);
    backdrop-filter: var(--shell-nav-backdrop, blur(24px) saturate(160%));
    -webkit-backdrop-filter: var(--shell-nav-backdrop, blur(24px) saturate(160%));
    box-shadow: var(--shell-nav-shadow, var(--shadow-island));
  }

  .nav-brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 4px 10px 24px;
  }

  .brand-seal {
    width: 34px;
    height: 34px;
    display: block;
    flex: none;
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand-name {
    font-family: var(--font-serif);
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.2;
    color: var(--ink-primary);
  }

  .brand-sub {
    font-size: 11.5px;
    color: var(--ink-muted);
    margin-top: 2px;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    min-height: 44px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    color: var(--ink-secondary);
    font-size: 14px;
    font-weight: 500;
    transition: color 180ms var(--ease-smooth), background-color 180ms var(--ease-smooth);
    text-align: left;
  }

  .nav-item:hover:not(.active) {
    background: var(--bg-well);
    color: var(--ink-primary);
  }

  .nav-item.active {
    color: var(--ink-primary);
    font-weight: 600;
  }

  .active-indicator {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-md);
    background: var(--bg-pill-active);
    box-shadow: var(--shadow-xs);
    z-index: 0;
  }

  .nav-icon,
  .nav-label {
    position: relative;
    z-index: 1;
    display: inline-flex;
    align-items: center;
  }

  .nav-item.active .nav-icon {
    color: var(--accent);
  }

  @media (max-width: 899px) {
    .nav-shell {
      left: 50%;
      top: auto;
      bottom: max(14px, var(--safe-area-bottom));
      margin-left: calc((var(--safe-area-left) - var(--safe-area-right)) / 2);
      translate: -50% 0;
      width: min(calc(100vw - 28px - var(--safe-area-left) - var(--safe-area-right)), 348px);
      height: auto;
      flex-direction: row;
      padding: 6px;
      border-radius: var(--radius-pill);
      background: var(--bg-glass-strong);
      backdrop-filter: var(--shell-nav-backdrop, blur(28px) saturate(180%));
      -webkit-backdrop-filter: var(--shell-nav-backdrop, blur(28px) saturate(180%));
    }

    .nav-brand {
      display: none;
    }

    .nav-items {
      flex-direction: row;
      width: 100%;
      gap: 4px;
    }

    .nav-item {
      flex: 1;
      justify-content: center;
      gap: 6px;
      padding: 0 10px;
      min-height: 44px;
      border-radius: var(--radius-pill);
      font-size: 13px;
    }

    .active-indicator {
      border-radius: var(--radius-pill);
    }
  }

  @media (forced-colors: active) {
    .nav-item.active {
      color: Highlight;
    }

    .active-indicator {
      border: 1px solid Highlight;
    }
  }
</style>
