<script lang="ts">
  import BottomToolbar from "./chrome/BottomToolbar.svelte";
  import ConversationOverlay from "./ConversationOverlay.svelte";
  import TopToolbar from "./chrome/TopToolbar.svelte";
</script>

<section class="reader-controls" aria-label="阅读工具">
  <TopToolbar />
  <BottomToolbar />
</section>

<div id="selection-actions" class="selection-actions" role="toolbar" aria-label="选中文字操作" hidden>
  <button id="copy-selection" type="button">复制</button>
  <button id="highlight-selection" type="button">标注</button>
  <button id="update-selection" type="button" hidden>更新</button>
  <button id="note-selection" type="button">笔记</button>
  <button id="delete-selection" type="button" hidden>删除</button>
  <output id="selection-actions-status" class="visually-hidden" aria-live="polite"></output>
</div>

<dialog id="annotation-note-dialog" aria-labelledby="annotation-note-heading">
  <form id="annotation-note-form" method="dialog">
    <h2 id="annotation-note-heading">添加笔记</h2>
    <label for="annotation-note">笔记内容</label>
    <textarea id="annotation-note" rows="5" maxlength="2000" required></textarea>
    <div class="annotation-dialog-actions">
      <button id="cancel-annotation-note" type="button">取消</button>
      <button type="submit">保存</button>
    </div>
  </form>
</dialog>

<ConversationOverlay />
