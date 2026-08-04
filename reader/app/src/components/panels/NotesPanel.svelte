<section class="tool-panel notes-panel" aria-labelledby="annotation-heading">
  <header class="panel-heading">
    <button class="icon-button panel-back" type="button" data-close-reader-tools aria-label="返回阅读">
      <span class="fluent-icon" aria-hidden="true">&#xE72B;</span>
    </button>
    <h2 id="annotation-heading">笔记</h2>
    <button id="message-export-all" type="button">导出</button>
  </header>
  <div class="annotation-filters">
    <input
      id="annotation-filter-query"
      type="search"
      maxlength="128"
      autocomplete="off"
      placeholder="搜索笔记与标注"
      aria-label="搜索笔记与标注"
    />
    <select id="annotation-filter-section" aria-label="按章节筛选">
      <option value="">全部章节</option>
    </select>
  </div>
  <nav id="annotations" class="annotation-list" aria-label="笔记与标注"></nav>
  <output id="annotations-status" aria-live="polite"></output>
</section>
