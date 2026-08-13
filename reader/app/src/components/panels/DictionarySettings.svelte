<script lang="ts">
  import { Plus, Trash2 } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    dictionaryAvailable,
    dictionaryErrorMessage,
    dictionaryFontScales,
    importDictionary,
    listDictionaries,
    readDictionaryPreferences,
    removeDictionary,
    writeDictionaryPreferences,
    type DictionaryFontScale,
    type LocalDictionary,
  } from "../../dictionary";

  export let disabled = false;

  const savedPreferences = readDictionaryPreferences();
  let dictionaries: LocalDictionary[] = [];
  let selectedId = savedPreferences.dictionaryId;
  let fontScale: DictionaryFontScale = savedPreferences.fontScale;
  let status = "";
  let settingsStatus = "";
  let busy = false;
  let currentDictionary: LocalDictionary | undefined;

  $: currentDictionary = dictionaries.find((dictionary) => dictionary.id === selectedId);

  onMount(() => void refresh());

  function savePreferences() {
    settingsStatus = writeDictionaryPreferences({ dictionaryId: selectedId, fontScale })
      ? ""
      : "设置无法保存，将仅在本次使用中生效。";
  }

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
    } catch (error) {
      status = dictionaryErrorMessage(error);
    }
  }

  async function chooseDictionary() {
    if (busy || disabled || !dictionaryAvailable) return;
    busy = true;
    status = "正在导入词典…";
    try {
      const imported = await importDictionary();
      if (!imported) {
        status = "";
        return;
      }
      dictionaries = imported;
      selectedId = dictionaries[0]?.id ?? "";
      savePreferences();
      status = dictionaries.length ? "词典已导入。" : "";
    } catch (error) {
      status = dictionaryErrorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function deleteDictionary() {
    const dictionary = dictionaries.find((item) => item.id === selectedId);
    if (
      busy ||
      disabled ||
      !dictionary ||
      !confirm(`移除词典“${dictionary.title}”？原文件不会删除。`)
    ) {
      return;
    }
    busy = true;
    try {
      dictionaries = await removeDictionary(dictionary.id);
      selectedId = dictionaries[0]?.id ?? "";
      savePreferences();
      status = dictionaries.length ? "词典已移除。" : "请先导入一个离线词典。";
    } catch (error) {
      status = dictionaryErrorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section class="library-settings-group dictionary-settings-page" aria-labelledby="dictionary-settings-heading">
  <h2 id="dictionary-settings-heading">离线词典</h2>

  <label class="library-setting-row library-setting-row-stacked">
    <span>当前词典</span>
    <select bind:value={selectedId} onchange={savePreferences} disabled={busy || disabled || !dictionaries.length}>
      {#if dictionaries.length}
        {#each dictionaries as dictionary}
          <option value={dictionary.id}>{dictionary.title}</option>
        {/each}
      {:else}
        <option value="">未导入词典</option>
      {/if}
    </select>
    {#if currentDictionary}
      <small>
        {currentDictionary.format === "mdict" ? "MDict" : "Kindle MOBI6"}
        · {currentDictionary.entryCount.toLocaleString("zh-CN")} 词条
      </small>
    {/if}
  </label>

  <label class="library-setting-row">
    <span>释义字号</span>
    <select bind:value={fontScale} onchange={savePreferences} aria-label="词典释义字号" disabled={busy || disabled}>
      {#each dictionaryFontScales as scale}
        <option value={scale}>{Math.round(scale * 100)}%</option>
      {/each}
    </select>
  </label>

  <div class="dictionary-settings-actions">
    <button type="button" disabled={busy || disabled} onclick={chooseDictionary}>
      <Plus aria-hidden="true" />
      <span>导入词典</span>
    </button>
    <button type="button" class="danger" disabled={busy || disabled || !selectedId} onclick={deleteDictionary}>
      <Trash2 aria-hidden="true" />
      <span>移除当前词典</span>
    </button>
  </div>

  <output class="dictionary-settings-status" aria-live="polite">{status}</output>
  {#if settingsStatus}
    <p class="dictionary-settings-status">{settingsStatus}</p>
  {/if}
</section>
