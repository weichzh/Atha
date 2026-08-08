<script lang="ts">
  import { ArrowLeft, Bookmark, BookOpenText, Ellipsis } from "@lucide/svelte";

  import DictionaryPanel from "../panels/DictionaryPanel.svelte";
  import PreferencesPanel from "../panels/PreferencesPanel.svelte";
</script>

<div class="top-toolbar">
  <button id="reader-back" class="icon-button" type="button" aria-label="返回" title="返回">
    <ArrowLeft aria-hidden="true" />
  </button>
  <div class="top-toolbar-actions">
    <details class="reader-tool dictionary" name="reader-panel">
      <summary class="icon-button" aria-label="词典" title="词典">
        <BookOpenText aria-hidden="true" />
      </summary>
      <DictionaryPanel />
    </details>
    <button
      id="add-bookmark"
      class="icon-button"
      type="button"
      aria-label="添加或取消书签"
      title="添加或取消书签"
      aria-pressed="false"
    >
      <Bookmark aria-hidden="true" />
    </button>
    <details class="reader-tool preferences" name="reader-panel">
      <summary class="icon-button" aria-label="阅读设置" title="阅读设置">
        <Ellipsis aria-hidden="true" />
      </summary>
      <PreferencesPanel />
    </details>
  </div>
</div>
