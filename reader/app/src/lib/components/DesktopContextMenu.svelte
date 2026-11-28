<script lang="ts">
  import { DropdownMenu } from "bits-ui";
  import { editMenuFor, type DesktopMenuItem } from "../data/desktop-edit-menu.ts";

  let open = $state(false);
  let point = $state({ x: 0, y: 0 });
  let anchor = $state<HTMLSpanElement | null>(null);
  let portalTarget = $state<HTMLElement>();
  let items = $state.raw<DesktopMenuItem[]>([]);
  let message = $state("");
  let restoreFocus: () => void = () => {};
  let messageTimer: ReturnType<typeof setTimeout>;

  function openAt(event: MouseEvent | KeyboardEvent, target: HTMLElement, actions: DesktopMenuItem[], restore: () => void) {
    event.preventDefault();
    if (event instanceof KeyboardEvent || (event.clientX === 0 && event.clientY === 0)) {
      const rect = target.getBoundingClientRect();
      point = { x: Math.max(0, rect.left + 8), y: Math.min(window.innerHeight, rect.bottom) };
    } else {
      point = { x: event.clientX, y: event.clientY };
    }
    items = actions;
    portalTarget = target.closest<HTMLElement>("[role=dialog]") ?? undefined;
    restoreFocus = restore;
    open = true;
  }

  export function show(event: MouseEvent | KeyboardEvent, actions: DesktopMenuItem[]) {
    const trigger = event.currentTarget as HTMLElement;
    openAt(event, trigger, actions, () => {
      if (trigger.isConnected) trigger.focus({ preventScroll: true });
    });
  }

  function maybeShowEditor(event: MouseEvent | KeyboardEvent) {
    const edit = editMenuFor(event.target);
    if (!edit) return false;
    openAt(event, edit.editor, edit.items, edit.restore);
    return true;
  }

  function handleContextMenu(event: MouseEvent) {
    if (event.defaultPrevented) return; // The book card or reader already owns this target.
    if (!maybeShowEditor(event)) {
      event.preventDefault();
      open = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented ||
      !(event.key === "ContextMenu" || (event.shiftKey && event.key === "F10"))) return;
    maybeShowEditor(event);
  }

  async function select(item: DesktopMenuItem) {
    restoreFocus();
    try {
      await item.select();
    } catch {
      clearTimeout(messageTimer);
      message = `未能${item.label}，请重试`;
      messageTimer = setTimeout(() => { message = ""; }, 3000);
    }
  }

  $effect(() => () => clearTimeout(messageTimer));
</script>

<svelte:window oncontextmenu={handleContextMenu} onkeydown={handleKeydown} />

<span class="menu-anchor" aria-hidden="true" bind:this={anchor} style:left={`${point.x}px`} style:top={`${point.y}px`}></span>
<DropdownMenu.Root bind:open>
  <DropdownMenu.Portal to={portalTarget}>
    <DropdownMenu.Content
      class="desktop-context-menu"
      aria-label="快捷操作"
      customAnchor={anchor}
      align="start"
      sideOffset={3}
      collisionPadding={8}
      onCloseAutoFocus={(event) => event.preventDefault()}
      onEscapeKeydown={() => restoreFocus()}
    >
      {#each items as item (item.label)}
        {#if item.separate}
          <DropdownMenu.Separator class="menu-divider" />
        {/if}
        <DropdownMenu.Item class="menu-item" disabled={item.disabled} onSelect={() => void select(item)}>
          {item.label}
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>

<div class="menu-status" role="status">{message}</div>

<style>
  .menu-anchor {
    position: fixed;
    width: 1px;
    height: 1px;
    pointer-events: none;
  }

  :global(.desktop-context-menu) {
    z-index: 200;
    min-inline-size: 9rem;
    padding: 4px;
    border-radius: 10px;
    background: var(--bg-surface);
    color: var(--ink-primary);
    box-shadow: var(--shadow-popover);
    outline: 1px solid var(--bg-well-active);
    font-size: 0.875rem;
    user-select: none;
  }

  :global(.desktop-context-menu .menu-item) {
    display: flex;
    align-items: center;
    min-block-size: 2rem;
    padding-inline: 0.75rem;
    border-radius: 6px;
    white-space: nowrap;
    outline: none;
    cursor: default;
  }

  :global(.desktop-context-menu .menu-item[data-highlighted]) {
    background: var(--accent-soft);
  }

  :global(.desktop-context-menu .menu-item[data-disabled]) {
    color: var(--ink-muted);
  }

  :global(.desktop-context-menu .menu-divider) {
    margin: 4px 6px;
    border-block-start: 1px solid var(--bg-well-active);
  }

  .menu-status:not(:empty) {
    position: fixed;
    z-index: 201;
    inset-inline: max(1rem, env(safe-area-inset-left)) max(1rem, env(safe-area-inset-right));
    inset-block-end: 2rem;
    justify-self: center;
    width: fit-content;
    padding: 0.625rem 0.875rem;
    border-radius: 10px;
    background: var(--bg-surface);
    color: var(--ink-primary);
    box-shadow: var(--shadow-popover);
    pointer-events: none;
  }
</style>
