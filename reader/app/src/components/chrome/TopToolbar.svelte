<script lang="ts">
  import { ArrowLeft, Bookmark, Settings } from "@lucide/svelte";

  import PreferencesPanel from "../panels/PreferencesPanel.svelte";
</script>

<div class="top-toolbar">
  <button id="reader-back" class="icon-button" type="button" aria-label="返回" title="返回">
    <ArrowLeft aria-hidden="true" />
  </button>
  <div class="top-toolbar-actions">
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
        <Settings aria-hidden="true" />
      </summary>
      <PreferencesPanel />
    </details>
  </div>
</div>
