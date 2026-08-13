<script lang="ts">
  import { ArrowLeft, Plus, Settings, Trash2, X } from "@lucide/svelte";
  import { onMount, tick } from "svelte";

  import {
    dictionaryFontScales,
    dictionaryAvailable,
    dictionaryErrorMessage,
    importDictionary,
    listDictionaries,
    lookupDictionary,
    readDictionaryPreferences,
    removeDictionary,
    writeDictionaryPreferences,
    type DictionaryFontScale,
    type DictionaryLookup,
    type LocalDictionary,
  } from "../../dictionary";

  const savedPreferences = readDictionaryPreferences();
  let dictionaries: LocalDictionary[] = [];
  let selectedId = savedPreferences.dictionaryId;
  let fontScale: DictionaryFontScale = savedPreferences.fontScale;
  let query = "";
  let result: DictionaryLookup | null = null;
  let status = "";
  let settingsStatus = "";
  let busy = false;
  let lookupVersion = 0;
  let view: "result" | "settings" = "result";
  let root: HTMLElement | undefined;
  let resultHeading: HTMLHeadingElement | undefined;
  let settingsHeading: HTMLHeadingElement | undefined;
  let currentDictionary: LocalDictionary | undefined;

  $: currentDictionary = dictionaries.find((dictionary) => dictionary.id === selectedId);

  function closePanel() {
    view = "result";
    const owner = root?.closest("details");
    owner?.removeAttribute("open");
    owner?.querySelector<HTMLElement>(":scope > summary")?.focus();
  }

  async function showSettings() {
    view = "settings";
    await tick();
    settingsHeading?.focus();
  }

  async function showResult() {
    view = "result";
    await tick();
    resultHeading?.focus();
  }

  function onPanelKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    closePanel();
  }

  function savePreferences() {
    settingsStatus = writeDictionaryPreferences({ dictionaryId: selectedId, fontScale })
      ? ""
      : "设置无法保存，将仅在本次使用中生效。";
  }

  onMount(() => {
    void refresh();
    const owner = root?.closest<HTMLDetailsElement>("details");
    const resetView = () => {
      if (!owner?.open) view = "result";
    };
    const lookup = (event: Event) => {
      const value = (event as CustomEvent<{ query?: unknown }>).detail?.query;
      if (typeof value !== "string") return;
      query = value.trim();
      result = null;
      view = "result";
      document.documentElement.setAttribute("data-reader-tools", "");
      owner?.setAttribute("open", "");
      void search();
    };
    owner?.addEventListener("toggle", resetView);
    globalThis.addEventListener("atha:dictionary-lookup", lookup);
    return () => {
      owner?.removeEventListener("toggle", resetView);
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
        savePreferences();
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
        savePreferences();
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
      savePreferences();
      result = null;
      status = dictionaries.length ? "词典已移除。" : "请先导入一个离线词典。";
    } catch (error) {
      status = dictionaryErrorMessage(error);
    } finally {
      busy = false;
    }
  }

  function changeDictionary() {
    lookupVersion += 1;
    result = null;
    savePreferences();
    if (query) void search();
  }

  function changeFontScale() {
    savePreferences();
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
  data-dictionary-view={view}
  style={`--dictionary-font-size: ${fontScale}rem`}
  role="dialog"
  aria-labelledby="dictionary-heading"
  tabindex="-1"
  onkeydown={onPanelKeydown}
>
  {#if view === "settings"}
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        aria-label="返回词典"
        title="返回词典"
        onclick={showResult}
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 bind:this={settingsHeading} id="dictionary-heading" tabindex="-1">词典设置</h2>
    </header>

    <div class="dictionary-settings">
      <section class="dictionary-settings-section" aria-labelledby="dictionary-source-heading">
        <h3 id="dictionary-source-heading">词典来源</h3>
        <label class="setting-row setting-row-stacked">
          <span class="setting-label">当前词典</span>
          <select
            class="dictionary-settings-select"
            bind:value={selectedId}
            onchange={changeDictionary}
            disabled={busy || !dictionaries.length}
          >
            {#if dictionaries.length}
              {#each dictionaries as dictionary}
                <option value={dictionary.id}>{dictionary.title}</option>
              {/each}
            {:else}
              <option value="">未导入词典</option>
            {/if}
          </select>
          {#if currentDictionary}
            <span class="dictionary-settings-meta">
              {currentDictionary.format === "mdict" ? "MDict" : "Kindle MOBI6"}
              · {currentDictionary.entryCount.toLocaleString("zh-CN")} 词条
            </span>
          {/if}
        </label>
      </section>

      <section class="dictionary-settings-section" aria-labelledby="dictionary-appearance-heading">
        <h3 id="dictionary-appearance-heading">外观</h3>
        <label class="setting-row">
          <span class="setting-label">释义字号</span>
          <select
            class="dictionary-font-scale"
            bind:value={fontScale}
            onchange={changeFontScale}
            aria-label="词典释义字号"
          >
            {#each dictionaryFontScales as scale}
              <option value={scale}>{Math.round(scale * 100)}%</option>
            {/each}
          </select>
        </label>
      </section>

      <div class="dictionary-settings-actions">
        <button type="button" disabled={busy} onclick={chooseDictionary}>
          <Plus aria-hidden="true" />
          <span>导入词典</span>
        </button>
        <button
          class="danger-button"
          type="button"
          disabled={busy || !selectedId}
          onclick={deleteDictionary}
        >
          <Trash2 aria-hidden="true" />
          <span>移除当前词典</span>
        </button>
      </div>
      <div class="dictionary-settings-status">
        <output class="dictionary-status" aria-live="polite">{status}</output>
        {#if settingsStatus}
          <p class="dictionary-status">{settingsStatus}</p>
        {/if}
      </div>
    </div>
  {:else}
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
      <div class="panel-heading-actions">
        <button
          class="icon-button"
          type="button"
          aria-label="词典设置"
          title="词典设置"
          onclick={showSettings}
        >
          <Settings aria-hidden="true" />
        </button>
      </div>
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
  {/if}
</div>
