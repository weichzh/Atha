<script lang="ts">
  import { Clock3, List, Search, Settings, SquarePen } from "@lucide/svelte";

  import DirectoryPanel from "../panels/DirectoryPanel.svelte";
  import NotesPanel from "../panels/NotesPanel.svelte";
  import PreferencesPanel from "../panels/PreferencesPanel.svelte";
  import ProgressPanel from "../panels/ProgressPanel.svelte";
  import SearchPanel from "../panels/SearchPanel.svelte";
</script>

<nav class="toolbar" aria-label="阅读控制">
  <details class="reader-tool directory" name="reader-panel">
    <summary class="icon-button" aria-label="目录" title="目录">
      <List aria-hidden="true" />
    </summary>
    <DirectoryPanel />
  </details>
  <details class="reader-tool search" name="reader-panel">
    <summary class="icon-button" aria-label="搜索" title="搜索">
      <Search aria-hidden="true" />
    </summary>
    <SearchPanel />
  </details>
  <details class="reader-tool notes" name="reader-panel">
    <summary class="icon-button" aria-label="笔记" title="笔记">
      <SquarePen aria-hidden="true" />
    </summary>
    <NotesPanel />
  </details>
  <details class="reader-tool progress" name="reader-panel">
    <summary class="icon-button" aria-label="进度" title="进度">
      <Clock3 aria-hidden="true" />
    </summary>
    <ProgressPanel />
  </details>
  <details class="reader-tool preferences" name="reader-panel">
    <summary class="icon-button" aria-label="阅读设置" title="阅读设置">
      <Settings aria-hidden="true" />
    </summary>
    <PreferencesPanel />
  </details>
</nav>
