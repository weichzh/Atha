<form id="search-form" class="tool-panel search-panel">
  <h2>搜索本书</h2>
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
  <div class="search-actions">
    <button type="submit">搜索</button>
    <button id="cancel-search" type="button">取消</button>
  </div>
  <output id="search-status" aria-live="polite"></output>
  <label>
    <span class="visually-hidden">搜索结果</span>
    <select id="search-results" size="8" aria-label="搜索结果"></select>
  </label>
  <button id="go-search-result" type="button">跳转到结果</button>
</form>
