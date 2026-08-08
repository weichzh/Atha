<script lang="ts">
  import { Plus, Trash2 } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    dictionaryAvailable,
    dictionaryErrorMessage,
    importDictionary,
    listDictionaries,
    lookupDictionary,
    removeDictionary,
    type DictionaryLookup,
    type LocalDictionary,
  } from "../../dictionary";

  let dictionaries: LocalDictionary[] = [];
  let selectedId = "";
  let query = "";
  let result: DictionaryLookup | null = null;
  let status = "";
  let busy = false;
  let lookupVersion = 0;
  let root: HTMLElement | undefined;

  onMount(() => {
    void refresh();
    const lookup = (event: Event) => {
      const value = (event as CustomEvent<{ query?: unknown }>).detail?.query;
      if (typeof value !== "string") return;
      query = value.trim();
      result = null;
      document.documentElement.setAttribute("data-reader-tools", "");
      root?.closest("details")?.setAttribute("open", "");
      void search();
    };
    globalThis.addEventListener("atha:dictionary-lookup", lookup);
    return () => globalThis.removeEventListener("atha:dictionary-lookup", lookup);
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
      }
      status = dictionaries.length ? "" : "请先导入一个离线词典。";
      if (query && selectedId) await search();
    } catch (error) {
      status = dictionaryErrorMessage(error);
    }
  }

  async function chooseDictionary() {
    if (busy || !dictionaryAvailable) return;
    busy = true;
    status = "正在导入词典…";
    try {
      const imported = await importDictionary();
      if (imported) {
        dictionaries = imported;
        selectedId = dictionaries[0]?.id ?? "";
        status = dictionaries.length ? "词典已导入。" : "";
        if (query) await search();
      } else {
        status = "";
      }
    } catch (error) {
      status = dictionaryErrorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function deleteDictionary() {
    const dictionary = dictionaries.find((item) => item.id === selectedId);
    if (busy || !dictionary || !confirm(`移除词典“${dictionary.title}”？原文件不会删除。`)) {
      return;
    }
    busy = true;
    try {
      lookupVersion += 1;
      dictionaries = await removeDictionary(dictionary.id);
      selectedId = dictionaries[0]?.id ?? "";
      result = null;
      status = dictionaries.length ? "词典已移除。" : "请先导入一个离线词典。";
    } catch (error) {
      status = dictionaryErrorMessage(error);
    } finally {
      busy = false;
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

<div bind:this={root} class="tool-panel dictionary-panel">
  <header class="panel-heading">
    <h2>词典</h2>
    <div class="panel-heading-actions">
      <button
        class="icon-button"
        type="button"
        aria-label="导入词典"
        title="导入词典"
        disabled={busy}
        onclick={chooseDictionary}
      >
        <Plus aria-hidden="true" />
      </button>
      <button
        class="icon-button danger-button"
        type="button"
        aria-label="移除当前词典"
        title="移除当前词典"
        disabled={busy || !selectedId}
        onclick={deleteDictionary}
      >
        <Trash2 aria-hidden="true" />
      </button>
    </div>
  </header>

  {#if dictionaries.length}
    <label class="dictionary-source">
      <span class="visually-hidden">当前词典</span>
      <select bind:value={selectedId} onchange={() => query && search()} disabled={busy}>
        {#each dictionaries as dictionary}
          <option value={dictionary.id}>{dictionary.title}</option>
        {/each}
      </select>
    </label>
  {/if}

  {#if query}
    <p class="dictionary-query">{query}</p>
  {/if}
  {#if result}
    <article class="dictionary-result" aria-live="polite">
      <h3>{result.headword}</h3>
      <p>{result.definition}</p>
    </article>
  {/if}
  <output class="dictionary-status" aria-live="polite">{status}</output>
</div>
