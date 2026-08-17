<script lang="ts">
  import { ArrowLeft, Bookmark } from "@lucide/svelte";
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
  </div>
</div>
