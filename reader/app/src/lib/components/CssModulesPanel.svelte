<script lang="ts">
  import type { EditorView as CmEditorView } from "codemirror";
  import { AlertCircle, CheckCircle2, Download, Plus, Trash2, Upload } from "@lucide/svelte";
  import { Switch } from "bits-ui";
  import { onDestroy, onMount } from "svelte";
  import type { ReaderEngine } from "../engine/reader-engine.svelte.ts";
  import { stylePackageCodec } from "../engine/preferences.ts";
  import type { StyleModule } from "../types.ts";

  interface Props {
    engine: ReaderEngine;
  }

  let { engine }: Props = $props();

  let selectedId = $state<string | null>(null);
  let cmMountEl = $state<HTMLDivElement | null>(null);
  let importInputEl = $state<HTMLInputElement | null>(null);
  let validationError = $state<string | null>(null);
  let packageMessage = $state<string | null>(null);

  let editorView = $state<CmEditorView | null>(null);
  let ignoreCmChange = false;
  let cmSetupPromise: Promise<{
    basicSetup: typeof import("codemirror")["basicSetup"];
    EditorView: typeof import("codemirror")["EditorView"];
    css: typeof import("@codemirror/lang-css")["css"];
  }> | null = null;

  const modules = $derived(engine.preferences.book.styleModules);
  const userStylesEnabled = $derived(engine.preferences.book.userStylesEnabled);
  const activeModule = $derived(modules.find((m) => m.id === selectedId) ?? modules[0] ?? null);

  function loadCodeMirrorModules() {
    cmSetupPromise ??= Promise.all([
      import("codemirror"),
      import("@codemirror/lang-css"),
    ]).then(([cm, langCss]) => ({
      basicSetup: cm.basicSetup,
      EditorView: cm.EditorView,
      css: langCss.css,
    }));
    return cmSetupPromise;
  }

  onMount(() => {
    if (!selectedId && modules[0]) {
      selectedId = modules[0].id;
    }
    void loadCodeMirrorModules();
  });

  $effect(() => {
    const mountNode = cmMountEl;
    if (!mountNode) {
      editorView = null;
      return;
    }
    let cancelled = false;
    let localView: CmEditorView | null = null;
    void loadCodeMirrorModules().then(({ basicSetup, EditorView, css }) => {
      if (cancelled || !mountNode.isConnected) return;
      localView = new EditorView({
        doc:
          activeModule?.css ??
          "/* 为当前书籍编写自定义样式 */\n.book p {\n  letter-spacing: 0.01em;\n}",
        extensions: [
          basicSetup,
          css(),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged || ignoreCmChange || !activeModule) return;
            const nextCss = update.state.doc.toString();
            const check = engine.validateCss(nextCss);
            validationError = check.error;
            if (check.valid) {
              void saveModulePatch(activeModule.id, { css: nextCss });
            }
          }),
        ],
        parent: mountNode,
      });
      editorView = localView;
    });
    return () => {
      cancelled = true;
      localView?.destroy();
      editorView = null;
    };
  });

  $effect(() => {
    const cssText = activeModule?.css ?? "";
    const view = editorView;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== cssText) {
      ignoreCmChange = true;
      view.dispatch({
        changes: { from: 0, to: current.length, insert: cssText },
      });
      ignoreCmChange = false;
      validationError = engine.validateCss(cssText).error;
    }
  });

  onDestroy(() => {
    editorView?.destroy();
  });

  async function saveModulePatch(id: string, patch: Partial<StyleModule>) {
    const next = modules.map((m) => (m.id === id ? { ...m, ...patch } : m));
    await engine.updatePreferences("book", { styleModules: next });
  }

  async function addModule() {
    const id = `mod-${Date.now().toString(36)}`;
    const nextMod: StyleModule = {
      id,
      name: `样式模块 ${modules.length + 1}`,
      group: "阅读微调",
      enabled: true,
      css: "/* 仅作用于当前书籍 */\n.book blockquote {\n  font-size: 0.96em;\n}",
    };
    const next = [...modules, nextMod];
    await engine.updatePreferences("book", { styleModules: next });
    selectedId = id;
    packageMessage = "已创建新样式模块";
  }

  async function removeModule(id: string) {
    const next = modules.filter((m) => m.id !== id);
    await engine.updatePreferences("book", { styleModules: next });
    selectedId = next[0]?.id ?? null;
  }

  function exportModules() {
    try {
      const json = stylePackageCodec.stringify(modules);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "atha-style-modules.json";
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      packageMessage = "已导出 schema v1 样式模块包";
    } catch {
      packageMessage = "导出校验失败";
    }
  }

  async function handleImportFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    try {
      const parsed = stylePackageCodec.parse(await file.text());
      await engine.updatePreferences("book", { styleModules: [...parsed] });
      selectedId = parsed[0]?.id ?? null;
      packageMessage = `已导入 ${parsed.length} 个样式模块`;
    } catch {
      packageMessage = "样式模块包校验失败（拒绝不安全规则或格式错误）";
    }
  }
</script>

<div class="css-panel">
  <div class="master-row">
    <div>
      <div class="master-title">启用用户 CSS 模块</div>
      <div class="master-sub">所有样式均受安全边界校验，禁止 @import、url() 与 :host。</div>
    </div>
    <Switch.Root
      checked={userStylesEnabled}
      onCheckedChange={(checked) => void engine.updatePreferences("book", { userStylesEnabled: checked })}
      class="atha-switch"
    >
      <Switch.Thumb class="atha-switch-thumb" />
    </Switch.Root>
  </div>

  <div class="module-actions-row">
    <button type="button" class="pill-btn" onclick={() => void addModule()}>
      <Plus size={14} />
      <span>新建模块</span>
    </button>
    <button type="button" class="pill-btn" onclick={exportModules} disabled={modules.length === 0}>
      <Download size={14} />
      <span>导出 JSON</span>
    </button>
    <button type="button" class="pill-btn" onclick={() => importInputEl?.click()}>
      <Upload size={14} />
      <span>导入 JSON</span>
    </button>
    <input bind:this={importInputEl} type="file" accept=".json" hidden onchange={handleImportFile} />
  </div>

  {#if packageMessage}
    <div class="pkg-toast">{packageMessage}</div>
  {/if}

  {#if modules.length === 0}
    <div class="empty-modules">
      还没有自定义样式模块。点击上方“新建模块”开始用 CSS 精修本书排版。
    </div>
  {:else}
    <div class="module-pills">
      {#each modules as mod (mod.id)}
        <div class="module-chip" class:active={activeModule?.id === mod.id}>
          <button
            type="button"
            class="chip-select"
            onclick={() => (selectedId = mod.id)}
          >
            {mod.name}
          </button>
          <Switch.Root
            checked={mod.enabled}
            onCheckedChange={(checked) => void saveModulePatch(mod.id, { enabled: checked })}
            class="atha-switch mini-switch"
            aria-label={`启用 ${mod.name}`}
          >
            <Switch.Thumb class="atha-switch-thumb" />
          </Switch.Root>
        </div>
      {/each}
    </div>

    {#if activeModule}
      <div class="active-module-editor">
        <div class="mod-meta-row">
          <div class="well-input mod-name-input">
            <input
              type="text"
              aria-label="模块名称"
              value={activeModule.name}
              onchange={(e) => {
                const name = e.currentTarget.value.trim();
                if (name) void saveModulePatch(activeModule.id, { name });
              }}
            />
          </div>
          <button
            type="button"
            class="icon-btn"
            aria-label="删除模块"
            onclick={() => void removeModule(activeModule.id)}
          >
            <Trash2 size={15} />
          </button>
        </div>

        <div class="cm-shell" bind:this={cmMountEl}></div>

        <div class="validation-status" class:invalid={Boolean(validationError)}>
          {#if validationError}
            <AlertCircle size={14} />
            <span>{validationError}</span>
          {:else}
            <CheckCircle2 size={14} />
            <span>CSS 校验通过并已实时生效</span>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .css-panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .master-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .master-title {
    font-size: 13.5px;
    font-weight: 600;
  }

  .master-sub {
    font-size: 12px;
    color: var(--ink-muted);
    margin-top: 2px;
  }

  .module-actions-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .pkg-toast {
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12.5px;
  }

  .empty-modules {
    padding: 28px 16px;
    text-align: center;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    color: var(--ink-muted);
    font-size: 13px;
  }

  .module-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .module-chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 12px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
  }

  .module-chip.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .chip-select {
    font-size: 12.5px;
    font-weight: 600;
  }

  .active-module-editor {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .mod-meta-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mod-name-input {
    flex: 1;
    height: 36px;
    padding: 0 12px;
    display: flex;
    align-items: center;
  }

  .mod-name-input input {
    width: 100%;
    font-size: 13.5px;
    font-weight: 500;
  }

  .cm-shell {
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs);
    min-height: 180px;
    max-height: 260px;
  }

  .cm-shell :global(.cm-editor) {
    font-family: var(--font-mono);
    font-size: 12.5px;
    max-height: 260px;
  }

  .validation-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--success);
  }

  .validation-status.invalid {
    color: var(--danger);
  }
</style>
