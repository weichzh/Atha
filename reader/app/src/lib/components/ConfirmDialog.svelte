<script lang="ts">
  import { AlertDialog } from "bits-ui";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  interface Props {
    open: boolean;
    title: string;
    description: string;
    confirmLabel?: string;
    cancelLabel?: string;
    destructive?: boolean;
    onConfirm: () => void | Promise<void>;
    onCancel?: () => void;
  }

  let {
    open = $bindable(false),
    title,
    description,
    confirmLabel = "确认",
    cancelLabel = "取消",
    destructive = false,
    onConfirm,
    onCancel,
  }: Props = $props();

  let busy = $state(false);

  async function handleConfirm() {
    if (busy) return;
    busy = true;
    try {
      await onConfirm();
      open = false;
    } finally {
      busy = false;
    }
  }

  function handleCancel() {
    open = false;
    onCancel?.();
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Portal>
    <AlertDialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-overlay"
            transition:fade={{ duration: 160 }}
          ></div>
        {/if}
      {/snippet}
    </AlertDialog.Overlay>
    <AlertDialog.Content forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content confirm-dialog"
            transition:scale={{ start: 0.96, duration: 200, easing: cubicOut }}
          >
            <AlertDialog.Title class="confirm-title">{title}</AlertDialog.Title>
            <AlertDialog.Description class="confirm-desc">{description}</AlertDialog.Description>
            <div class="confirm-actions">
              <AlertDialog.Cancel class="pill-btn" onclick={handleCancel} disabled={busy}>
                {cancelLabel}
              </AlertDialog.Cancel>
              <AlertDialog.Action
                class={destructive ? "danger-btn" : "accent-btn"}
                onclick={handleConfirm}
                disabled={busy}
              >
                {busy ? "处理中…" : confirmLabel}
              </AlertDialog.Action>
            </div>
          </div>
        {/if}
      {/snippet}
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>

<style>
  .confirm-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
    max-width: 420px;
  }

  :global(.confirm-title) {
    margin: 0 0 10px;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--ink-primary);
  }

  :global(.confirm-desc) {
    margin: 0 0 24px;
    font-size: 14px;
    line-height: 1.65;
    color: var(--ink-secondary);
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
