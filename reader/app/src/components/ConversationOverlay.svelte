<div
  id="message-conversation"
  class="message-conversation"
  role="dialog"
  aria-labelledby="message-conversation-title"
  hidden
>
  <header class="message-conversation-heading">
    <div>
      <p id="message-conversation-source" class="message-conversation-source"></p>
      <h2 id="message-conversation-title">阅读对话</h2>
    </div>
    <div class="message-conversation-heading-actions">
      <button id="message-conversation-export" type="button">导出</button>
      <button id="message-conversation-collapse" type="button" aria-expanded="true">收起</button>
      <button id="message-conversation-close" type="button" aria-label="关闭阅读对话">关闭</button>
    </div>
  </header>
  <div id="message-conversation-content">
    <section id="message-conversation-list" class="message-conversation-list" aria-live="polite"></section>
    <form id="message-composer" class="message-composer">
      <p id="message-composer-context">回复原文</p>
      <label for="message-composer-text">消息</label>
      <textarea id="message-composer-text" rows="3" maxlength="8000" required></textarea>
      <fieldset id="message-composer-references">
        <legend>引用本对话中的消息（可选）</legend>
      </fieldset>
      <div class="message-composer-actions">
        <button id="message-composer-cancel" type="button" hidden>取消编辑</button>
        <button type="submit">发送</button>
      </div>
      <output id="message-conversation-status" aria-live="polite"></output>
    </form>
  </div>
</div>

<dialog id="message-history-dialog" class="message-detail-dialog" aria-labelledby="message-history-title">
  <form method="dialog"><button aria-label="关闭">关闭</button></form>
  <h2 id="message-history-title">修订历史</h2>
  <section id="message-history-content"></section>
</dialog>

<dialog id="message-snapshot-dialog" class="message-detail-dialog" aria-labelledby="message-snapshot-title">
  <form method="dialog"><button aria-label="关闭">关闭</button></form>
  <h2 id="message-snapshot-title">历史引用</h2>
  <nav id="message-snapshot-versions" aria-label="引用版本"></nav>
  <section id="message-snapshot-content"></section>
</dialog>
