<script lang="ts">
  import { X } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    dictionaryAvailable,
    dictionaryErrorMessage,
    listDictionaries,
    lookupDictionary,
    readDictionaryPreferences,
    writeDictionaryPreferences,
    type DictionaryLookup,
    type LocalDictionary,
  } from "../../dictionary";

  const savedPreferences = readDictionaryPreferences();
  let dictionaries: LocalDictionary[] = [];
  let selectedId = savedPreferences.dictionaryId;
  let fontScale = savedPreferences.fontScale;
  let query = "";
  let result: DictionaryLookup | null = null;
  let status = "";
  let lookupVersion = 0;
  let root: HTMLElement | undefined;
  let resultHeading: HTMLHeadingElement | undefined;
  let currentDictionary: LocalDictionary | undefined;

  $: currentDictionary = dictionaries.find((dictionary) => dictionary.id === selectedId);

  function closePanel() {
    const owner = root?.closest("details");
    owner?.removeAttribute("open");
    document.documentElement.removeAttribute("data-reader-tools");
    document.querySelector<HTMLElement>(".reader")?.focus({ preventScroll: true });
  }

  function onPanelKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    closePanel();
  }

  onMount(() => {
    void refresh();
    const owner = root?.closest<HTMLDetailsElement>("details");
    const lookup = (event: Event) => {
      const value = (event as CustomEvent<{ query?: unknown }>).detail?.query;
      if (typeof value !== "string") return;
      query = value.trim();
      result = null;
      document.documentElement.setAttribute("data-reader-tools", "");
      owner?.setAttribute("open", "");
      void search();
    };
    globalThis.addEventListener("atha:dictionary-lookup", lookup);
    return () => {
      globalThis.removeEventListener("atha:dictionary-lookup", lookup);
    };
  });

  async function refresh() {
    if (!dictionaryAvailable) {
      status = "请在 Atha 应用中管理本地词典。";
      return;
    }
    try {
      dictionaries = await listDictionaries();
      if (!dictionaries.some((dictionary) => dictionary.id === selectedId)) {
        selectedId = dictionaries[0]?.id ?? "";
        writeDictionaryPreferences({ dictionaryId: selectedId, fontScale });
      }
      status = dictionaries.length ? "" : "请先导入一个离线词典。";
      if (query && selectedId) await search();
    } catch (error) {
      status = dictionaryErrorMessage(error);
    }
  }

  async function search() {
    if (!query) return;
    if (!selectedId) {
      status = "请先导入一个离线词典。";
      return;
    }
    const version = ++lookupVersion;
    const dictionaryId = selectedId;
    const lookupQuery = query;
    status = "正在查词…";
    result = null;
    try {
      const next = await lookupDictionary(dictionaryId, lookupQuery);
      if (version !== lookupVersion) return;
      result = next;
      status = next ? "" : "未找到这个词。";
    } catch (error) {
      if (version !== lookupVersion) return;
      status = dictionaryErrorMessage(error);
    }
  }
</script>

<button
  class="dictionary-backdrop"
  type="button"
  aria-label="关闭词典"
  tabindex="-1"
  onclick={closePanel}
></button>
<div
  bind:this={root}
  class="tool-panel dictionary-panel"
  style={`--dictionary-font-size: ${fontScale}rem`}
  role="dialog"
  aria-labelledby="dictionary-heading"
  tabindex="-1"
  onkeydown={onPanelKeydown}
>
  <header class="panel-heading">
    <button
      class="icon-button panel-close"
      type="button"
      aria-label="关闭词典"
      title="关闭词典"
      onclick={closePanel}
    >
      <X aria-hidden="true" />
    </button>
    <h2 bind:this={resultHeading} id="dictionary-heading" tabindex="-1">词典</h2>
  </header>

  <div class="dictionary-content">
    {#if result}
      <article class="dictionary-result" aria-live="polite">
        <h3 class="dictionary-headword">{result.headword}</h3>
        <div class="dictionary-definition">
          {#if result.definitionHtml}
            <!-- The backend strips active elements and every source attribute before this boundary. -->
            {@html result.definitionHtml}
          {:else}
            <p>{result.definition}</p>
          {/if}
        </div>
        {#if currentDictionary}
          <p class="dictionary-result-source">{currentDictionary.title}</p>
        {/if}
      </article>
    {/if}
    <output class="dictionary-status" aria-live="polite">{status}</output>
  </div>
</div>
