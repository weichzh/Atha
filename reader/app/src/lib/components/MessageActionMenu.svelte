<script lang="ts">
  import { computePosition, flip, offset, shift } from "@floating-ui/dom";
  import { Edit3, History, Link2, Reply, Trash2 } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  interface Props {
    anchorEl: HTMLElement;
    canEdit?: boolean;
    onReply: () => void;
    onReference: () => void;
    onEdit: () => void;
    onRevisions: () => void;
    onDelete: () => void;
    onClose: () => void;
  }

  let {
    anchorEl,
    canEdit = true,
    onReply,
    onReference,
    onEdit,
    onRevisions,
    onDelete,
    onClose,
  }: Props = $props();

  let menuEl = $state<HTMLDivElement | null>(null);

  onMount(async () => {
    await tick();
    if (!menuEl || !anchorEl) return;
    const { x, y } = await computePosition(anchorEl, menuEl, {
      placement: "bottom-end",
      middleware: [offset(6), flip(), shift({ padding: 12 })],
    });
    menuEl.style.left = `${x}px`;
    menuEl.style.top = `${y}px`;
    const first = menuEl.querySelector<HTMLButtonElement>('[role="menuitem"]');
    first?.focus();
  });

  function handleKeydown(event: KeyboardEvent) {
    if (!menuEl) return;
    const items = [...menuEl.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    if (!items.length) return;
    const currentIdx = items.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      anchorEl.focus();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      items[(currentIdx + 1) % items.length].focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      items[(currentIdx - 1 + items.length) % items.length].focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      items[0].focus();
    } else if (event.key === "End") {
      event.preventDefault();
      items[items.length - 1].focus();
    }
  }

  function handleWindowPointerDown(event: PointerEvent) {
    const target = event.target as Node | null;
    if (menuEl?.contains(target) || anchorEl.contains(target)) return;
    onClose();
  }
</script>

<svelte:window onpointerdown={handleWindowPointerDown} />

<div
  bind:this={menuEl}
  class="glass-popover msg-menu"
  role="menu"
  aria-label="消息操作菜单"
  tabindex="-1"
  onkeydown={handleKeydown}
  transition:scale={{ start: 0.95, duration: 140, easing: cubicOut }}
>
  <button type="button" role="menuitem" class="msg-menu-item" onclick={() => { onReply(); onClose(); }}>
    <Reply size={14} />
    <span>回复这条消息</span>
  </button>
  <button type="button" role="menuitem" class="msg-menu-item" onclick={() => { onReference(); onClose(); }}>
    <Link2 size={14} />
    <span>引用此消息</span>
  </button>
  {#if canEdit}
    <button type="button" role="menuitem" class="msg-menu-item" onclick={() => { onEdit(); onClose(); }}>
      <Edit3 size={14} />
      <span>编辑正文</span>
    </button>
  {/if}
  <button type="button" role="menuitem" class="msg-menu-item" onclick={() => { onRevisions(); onClose(); }}>
    <History size={14} />
    <span>修订历史</span>
  </button>
  <div class="menu-spacer" role="separator"></div>
  <button type="button" role="menuitem" class="msg-menu-item danger" onclick={() => { onDelete(); onClose(); }}>
    <Trash2 size={14} />
    <span>删除</span>
  </button>
</div>

<style>
  .msg-menu {
    position: fixed;
    z-index: 75;
    min-width: 168px;
    padding: 6px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .msg-menu-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 8px 11px;
    border-radius: var(--radius-xs);
    font-size: 13px;
    color: var(--ink-primary);
    text-align: left;
    transition: background-color 140ms var(--ease-smooth);
  }

  .msg-menu-item:hover,
  .msg-menu-item:focus-visible {
    background: var(--bg-well);
    outline: none;
  }

  .msg-menu-item.danger {
    color: var(--danger);
  }

  .menu-spacer {
    height: 6px;
  }
</style>
