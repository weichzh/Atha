<script lang="ts">
  import { ArrowLeft, ChevronRight, Maximize2, Minimize2, X } from "@lucide/svelte";
  import { onMount, tick } from "svelte";

  let Composer = $state<typeof import("./MessageComposer.svelte").default>();

  onMount(() => {
    let mounted = true;
    window.athaEnsureMessageComposer = async () => {
      if (!Composer) {
        const module = await import("./MessageComposer.svelte");
        if (!mounted) return;
        Composer = module.default;
        await tick();
      }
    };
    return () => {
      mounted = false;
      delete window.athaEnsureMessageComposer;
    };
  });
</script>

<div
  id="message-conversation"
  class="message-conversation"
  data-message-theme="atha"
  role="dialog"
  aria-labelledby="message-conversation-title"
  hidden
>
  <button
    id="message-conversation-handle"
    class="message-conversation-handle"
    type="button"
    aria-label="拖动调整对话高度，轻点全屏"
    title="拖动调整高度，轻点全屏"
  ><span aria-hidden="true"></span></button>
  <header class="message-conversation-heading">
    <button
      id="message-conversation-close"
      class="message-icon-button"
      type="button"
      aria-label="返回阅读"
      title="返回阅读"
    >
      <ArrowLeft aria-hidden="true" />
    </button>
    <h2 id="message-conversation-title">阅读对话</h2>
    <button
      id="message-conversation-fullscreen"
      class="message-icon-button"
      type="button"
      aria-label="全屏对话"
      title="全屏对话"
      aria-pressed="false"
    >
      <Maximize2 class="message-expand-icon" aria-hidden="true" />
      <Minimize2 class="message-restore-icon" aria-hidden="true" />
    </button>
  </header>

  <div class="message-source-context">
    <div>
      <span>原文引用</span>
      <p id="message-conversation-source"></p>
    </div>
    <button id="message-conversation-source-jump" type="button">
      返回原文
      <ChevronRight aria-hidden="true" />
    </button>
  </div>

  <div id="message-conversation-content" class="message-conversation-content">
    <section
      id="message-conversation-list"
      class="message-conversation-list"
      aria-live="polite"
    ></section>

    <form id="message-composer" class="message-composer">
      <div id="message-composer-context" class="message-composer-context" hidden>
        <span class="message-quote-mark" aria-hidden="true">“</span>
        <p id="message-composer-context-text"></p>
        <button
          id="message-composer-cancel"
          class="message-icon-button"
          type="button"
          aria-label="取消当前操作"
          title="取消当前操作"
        >
          <X aria-hidden="true" />
        </button>
      </div>

      {#if Composer}<Composer />{/if}
      <output id="message-conversation-status" aria-live="polite"></output>
    </form>
  </div>
</div>

<dialog id="message-history-dialog" class="message-detail-dialog" data-message-theme="atha" aria-labelledby="message-history-title">
  <header class="message-detail-heading">
    <h2 id="message-history-title">修订历史</h2>
    <form method="dialog">
      <button class="message-icon-button" aria-label="关闭" title="关闭"><X aria-hidden="true" /></button>
    </form>
  </header>
  <section id="message-history-content"></section>
</dialog>

<dialog id="message-snapshot-dialog" class="message-detail-dialog" data-message-theme="atha" aria-labelledby="message-snapshot-title">
  <header class="message-detail-heading">
    <h2 id="message-snapshot-title">历史引用</h2>
    <form method="dialog">
      <button class="message-icon-button" aria-label="关闭" title="关闭"><X aria-hidden="true" /></button>
    </form>
  </header>
  <nav id="message-snapshot-versions" aria-label="引用版本"></nav>
  <section id="message-snapshot-content"></section>
</dialog>
