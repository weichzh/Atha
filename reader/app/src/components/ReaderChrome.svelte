<script lang="ts">
  import {
    BookOpenText,
    Copy,
    Highlighter,
    NotebookPen,
    RefreshCw,
    Trash2,
  } from "@lucide/svelte";

  import BottomToolbar from "./chrome/BottomToolbar.svelte";
  import ConversationOverlay from "./ConversationOverlay.svelte";
  import TopToolbar from "./chrome/TopToolbar.svelte";
</script>

<section class="reader-controls" aria-label="阅读工具">
  <TopToolbar />
  <BottomToolbar />
</section>

<div id="selection-actions" class="selection-actions" role="toolbar" aria-label="选中文字操作" hidden>
  <button id="lookup-selection" type="button" aria-label="查词" title="查词"
    ><BookOpenText aria-hidden="true" /><span class="visually-hidden">查词</span></button
  >
  <button id="copy-selection" type="button" aria-label="复制" title="复制"
    ><Copy aria-hidden="true" /><span class="visually-hidden">复制</span></button
  >
  <button id="highlight-selection" type="button" aria-label="标注" title="标注"
    ><Highlighter aria-hidden="true" /><span class="visually-hidden">标注</span></button
  >
  <button id="update-selection" type="button" aria-label="更新选区" title="更新选区" hidden
    ><RefreshCw aria-hidden="true" /><span class="visually-hidden">更新</span></button
  >
  <button id="note-selection" type="button" aria-label="笔记" title="笔记"
    ><NotebookPen aria-hidden="true" /><span class="visually-hidden">笔记</span></button
  >
  <button id="delete-selection" type="button" aria-label="删除标注" title="删除标注" hidden
    ><Trash2 aria-hidden="true" /><span class="visually-hidden">删除</span></button
  >
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
