<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";
</script>

<div class="tool-panel directory-panel">
  <header class="panel-heading">
    <button class="icon-button panel-back" type="button" data-close-reader-tools aria-label="返回阅读">
      <ArrowLeft aria-hidden="true" />
    </button>
    <h2>目录</h2>
  </header>
  <label class="visually-hidden" aria-hidden="true" hidden>
    <span class="visually-hidden">章节和书签</span>
    <select id="toc" size="10" tabindex="-1" aria-label="章节和书签"></select>
  </label>
  <nav id="directory-list" class="directory-list" aria-label="章节和书签"></nav>
  <output id="bookmarks-status" class="visually-hidden" aria-live="polite"></output>
</div>
