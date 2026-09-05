<script lang="ts">
  import { ArrowLeft, ArrowRight, Search, X } from "@lucide/svelte";
</script>

<form id="search-form" class="tool-panel search-panel">
  <header class="panel-heading">
    <button class="icon-button panel-back" type="button" data-close-reader-tools aria-label="返回阅读" title="返回阅读"><ArrowLeft aria-hidden="true" /></button>
    <h2>搜索本书</h2>
  </header>
  <div class="reader-search-field">
    <label>
      <span class="visually-hidden">书内文字</span>
      <input
        id="search-query"
        type="search"
        maxlength="128"
        autocomplete="off"
        placeholder="搜索本书内容"
      />
    </label>
    <button class="icon-button" type="submit" aria-label="搜索" title="搜索"><Search aria-hidden="true" /></button>
    <button id="cancel-search" class="icon-button" type="button" aria-label="取消搜索" title="取消搜索"><X aria-hidden="true" /></button>
  </div>
  <output id="search-status" aria-live="polite"></output>
  <label>
    <span class="visually-hidden">搜索结果</span>
    <select id="search-results" size="8" aria-label="搜索结果"></select>
  </label>
  <button id="go-search-result" class="reader-primary-command" type="button"><span>跳转到结果</span><ArrowRight aria-hidden="true" /></button>
</form>
