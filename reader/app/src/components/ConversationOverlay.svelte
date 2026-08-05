<script lang="ts">
  import { ArrowLeft, ChevronRight, Ellipsis, Send, X } from "@lucide/svelte";
</script>

<div
  id="message-conversation"
  class="message-conversation"
  data-message-theme="atha"
  role="dialog"
  aria-labelledby="message-conversation-title"
  hidden
>
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
    <details class="message-header-menu">
      <summary class="message-icon-button" aria-label="更多操作" title="更多操作">
        <Ellipsis aria-hidden="true" />
      </summary>
      <div class="message-menu" role="menu">
        <button id="message-conversation-export" type="button" role="menuitem">导出对话</button>
        <button
          id="message-conversation-collapse"
          type="button"
          role="menuitem"
          aria-expanded="true"
        >收起对话</button>
      </div>
    </details>
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

      <label class="visually-hidden" for="message-composer-text">消息</label>
      <div class="message-composer-row">
        <textarea
          id="message-composer-text"
          rows="1"
          maxlength="8000"
          placeholder="写下想法…"
          required
        ></textarea>
        <button class="message-send-button" type="submit" aria-label="发送" title="发送">
          <Send aria-hidden="true" />
        </button>
      </div>
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
