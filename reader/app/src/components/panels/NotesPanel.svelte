<script lang="ts">
  import { ArrowLeft, Download, Ellipsis } from "@lucide/svelte";
</script>

<section class="tool-panel notes-panel" aria-labelledby="annotation-heading">
  <header class="panel-heading">
    <button class="icon-button panel-back" type="button" data-close-reader-tools aria-label="返回阅读">
      <ArrowLeft aria-hidden="true" />
    </button>
    <h2 id="annotation-heading">笔记</h2>
    <details class="notes-actions">
      <summary class="icon-button" aria-label="更多操作" title="更多操作">
        <Ellipsis aria-hidden="true" />
      </summary>
      <div class="notes-actions-menu">
        <button id="message-export-all" type="button">
          <Download aria-hidden="true" />
          <span data-export-label>导出笔记</span>
        </button>
      </div>
    </details>
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
