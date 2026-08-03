<section class="tool-panel notes-panel annotation-editor" aria-labelledby="annotation-heading">
  <h2 id="annotation-heading">笔记</h2>
  <label>
    笔记（可选）
    <textarea id="annotation-note" rows="3" maxlength="2000"></textarea>
  </label>
  <button id="add-annotation" type="button">保存当前选择</button>
  <label>
    已存标注
    <select id="annotations" size="5" aria-label="已存标注"></select>
  </label>
  <div class="annotation-actions">
    <button id="go-annotation" type="button">跳转</button>
    <button id="save-annotation-note" type="button">保存笔记</button>
    <button id="delete-annotation" type="button">删除</button>
  </div>
  <output id="annotations-status" aria-live="polite"></output>
</section>
