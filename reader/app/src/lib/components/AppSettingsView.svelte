<script lang="ts">
  import {
    BookMarked,
    Database,
    Download,
    Monitor,
    Moon,
    Plus,
    RotateCcw,
    Sun,
    Trash2,
    Upload,
  } from "@lucide/svelte";
  import { onMount } from "svelte";
  import type { DictionaryFontScale, LocalDictionary } from "../../dictionary.ts";
  import type { AppTheme, StorageUsage } from "../../library.ts";
  import {
    dictionaryFontScales,
    exportFullLocalDataBackup,
    getAppTheme,
    getDictionaryPreferences,
    importOfflineDictionary,
    listOfflineDictionaries,
    readLocalStorageUsage,
    removeOfflineDictionary,
    resetGlobalReaderDefaults,
    restoreFullLocalDataBackup,
    saveDictionaryFontScale,
    setAppTheme,
  } from "../data/library-adapter.ts";
  import { isTauriRuntime } from "../data/runtime.ts";
  import { uiPrefs, type ShelfDesktopDensity, type ShelfMobileColumns } from "../data/ui-prefs.svelte.ts";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let appTheme = $state<AppTheme>(getAppTheme());
  let dictionaries = $state<LocalDictionary[]>([]);
  let fontScale = $state<DictionaryFontScale>(getDictionaryPreferences().fontScale);
  let usage = $state<StorageUsage | null>(null);
  let statusMessage = $state<string | null>(null);
  let restoreInputEl = $state<HTMLInputElement | null>(null);

  let confirmResetOpen = $state(false);
  let confirmRemoveDict = $state<LocalDictionary | null>(null);
  let confirmRestoreOpen = $state(false);
  let pendingRestoreFile = $state<File | null>(null);

  const themeOptions: { value: AppTheme; label: string; icon: typeof Sun }[] = [
    { value: "system", label: "跟随系统", icon: Monitor },
    { value: "light", label: "浅色纸感", icon: Sun },
    { value: "dark", label: "黑曜石暗色", icon: Moon },
  ];

  let statusTimer: ReturnType<typeof setTimeout> | null = null;
  function notify(msg: string) {
    statusMessage = msg;
    if (statusTimer) clearTimeout(statusTimer);
    statusTimer = setTimeout(() => {
      statusMessage = null;
    }, 2800);
  }

  async function refreshAll() {
    dictionaries = await listOfflineDictionaries();
    usage = await readLocalStorageUsage();
  }

  onMount(() => {
    void refreshAll();
  });

  function selectTheme(next: AppTheme) {
    appTheme = next;
    setAppTheme(next);
    notify("已切换应用外观");
  }

  function selectScale(scale: DictionaryFontScale) {
    fontScale = scale;
    saveDictionaryFontScale(scale);
    notify(`词典字号缩放已设为 ${Math.round(scale * 100)}%`);
  }

  async function handleImportDictionary() {
    const result = await importOfflineDictionary();
    if (result) {
      dictionaries = result;
      usage = await readLocalStorageUsage();
      notify("已导入离线词典索引");
    }
  }

  async function executeRemoveDictionary() {
    if (!confirmRemoveDict) return;
    dictionaries = await removeOfflineDictionary(confirmRemoveDict.id);
    usage = await readLocalStorageUsage();
    notify(`已移除词典《${confirmRemoveDict.title}》`);
    confirmRemoveDict = null;
  }

  function executeResetDefaults() {
    const ok = resetGlobalReaderDefaults();
    notify(ok ? "已恢复全书阅读默认排版" : "恢复默认失败");
  }

  async function handleBackup() {
    const res = await exportFullLocalDataBackup();
    notify(res.label);
  }

  function handleRestoreTrigger() {
    if (isTauriRuntime()) {
      pendingRestoreFile = null;
      confirmRestoreOpen = true;
    } else {
      restoreInputEl?.click();
    }
  }

  function handleFileChosen(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0] ?? null;
    input.value = "";
    if (!file) return;
    pendingRestoreFile = file;
    confirmRestoreOpen = true;
  }

  async function executeRestore() {
    const res = await restoreFullLocalDataBackup(pendingRestoreFile);
    pendingRestoreFile = null;
    notify(res.label);
    if (res.ok) {
      await refreshAll();
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  const usageSegments = $derived.by(() => {
    if (!usage) return [];
    const total = Math.max(1, usage.totalBytes);
    return [
      { label: "藏书文件", bytes: usage.booksBytes, pct: (usage.booksBytes / total) * 100, tone: "tone-1" },
      { label: "离线词典", bytes: usage.dictionariesBytes, pct: (usage.dictionariesBytes / total) * 100, tone: "tone-2" },
      { label: "排版缓存", bytes: usage.cacheBytes, pct: (usage.cacheBytes / total) * 100, tone: "tone-3" },
      { label: "笔记与偏好", bytes: usage.messagesBytes + usage.preferencesBytes, pct: ((usage.messagesBytes + usage.preferencesBytes) / total) * 100, tone: "tone-4" },
    ];
  });
</script>

<section class="settings-shell" aria-label="设置">
  <header class="settings-header">
    <h1>设置</h1>
    <p class="subtitle">应用外观、全书默认重置、离线词典与本地数据备份各自独立，互不越界。</p>
  </header>

  {#if statusMessage}
    <div class="toast-banner glass-island" role="status">{statusMessage}</div>
  {/if}

  <div class="sections-stack">
    <!-- Scope 1: 应用外观 -->
    <section class="settings-block">
      <div class="block-head">
        <h2>应用外观</h2>
        <span class="scope-tag">作用域：书架、阅读记忆与设置界面</span>
      </div>
      <p class="block-desc">决定书房工具界面的明暗呈现，不会覆盖书籍正文内的日间 / 纸色 / 夜间主题。</p>

      <div class="segmented-row" role="radiogroup" aria-label="应用外观模式">
        {#each themeOptions as opt (opt.value)}
          {@const Icon = opt.icon}
          <button
            type="button"
            role="radio"
            aria-checked={appTheme === opt.value}
            class="theme-pill"
            class:active={appTheme === opt.value}
            onclick={() => selectTheme(opt.value)}
          >
            <Icon size={16} />
            <span>{opt.label}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="settings-block" aria-labelledby="shelf-display-heading">
      <div class="block-head">
        <h2 id="shelf-display-heading">书架布局</h2>
        <span class="scope-tag">仅此设备</span>
      </div>
      <div class="shelf-display-settings">
        <label>
          <span>手机封面列数</span>
          <select
            value={uiPrefs.state.shelfMobileColumns}
            onchange={(event) => uiPrefs.update({ shelfMobileColumns: Number(event.currentTarget.value) as ShelfMobileColumns })}
          >
            <option value={2}>2 列</option>
            <option value={3}>3 列</option>
            <option value={4}>4 列</option>
          </select>
        </label>
        <label>
          <span>桌面封面密度</span>
          <select
            value={uiPrefs.state.shelfDesktopDensity}
            onchange={(event) => uiPrefs.update({ shelfDesktopDensity: event.currentTarget.value as ShelfDesktopDensity })}
          >
            <option value="comfortable">宽松</option>
            <option value="standard">标准</option>
            <option value="compact">紧凑</option>
          </select>
        </label>
        <label>
          <span>封面下显示书名和作者</span>
          <input
            type="checkbox"
            checked={uiPrefs.state.shelfShowMeta}
            onchange={(event) => uiPrefs.update({ shelfShowMeta: event.currentTarget.checked })}
          />
        </label>
        <label>
          <span>继续阅读默认收起</span>
          <input
            type="checkbox"
            checked={uiPrefs.state.shelfDockCollapsed}
            onchange={(event) => uiPrefs.update({ shelfDockCollapsed: event.currentTarget.checked })}
          />
        </label>
      </div>
    </section>

    <!-- Scope 2: 全书阅读默认恢复 -->
    <section class="settings-block">
      <div class="block-head">
        <h2>全书阅读默认</h2>
        <span class="scope-tag">作用域：所有未单独覆盖的书籍</span>
      </div>
      <div class="action-surface">
        <div class="action-info">
          <div class="action-title">恢复全书阅读默认排版</div>
          <div class="action-sub">将阅读主题、亮度、字号（19px）、字体与行距还原至出厂基线；各书的独立覆盖不受影响。</div>
        </div>
        <button type="button" class="pill-btn" onclick={() => (confirmResetOpen = true)}>
          <RotateCcw size={15} />
          <span>恢复默认</span>
        </button>
      </div>
    </section>

    <!-- Scope 3: 离线词典管理 -->
    <section class="settings-block">
      <div class="block-head">
        <h2>离线词典管理</h2>
        <span class="scope-tag">支持 MDX / MDD 与经典 Kindle MOBI</span>
      </div>

      <div class="surface-panel">
        <div class="panel-top">
          <span class="panel-label">词典卡片字号缩放</span>
          <div class="scale-pills" role="radiogroup" aria-label="词典字号缩放">
            {#each dictionaryFontScales as scale (scale)}
              <button
                type="button"
                role="radio"
                aria-checked={fontScale === scale}
                class="scale-pill tabular"
                class:active={fontScale === scale}
                onclick={() => selectScale(scale)}
              >
                {Math.round(scale * 100)}%
              </button>
            {/each}
          </div>
        </div>

        <div class="dict-list">
          {#if dictionaries.length === 0}
            <div class="dict-empty">尚未导入本地离线词典。</div>
          {:else}
            {#each dictionaries as dict (dict.id)}
              <div class="dict-row">
                <div class="dict-icon">
                  <BookMarked size={17} />
                </div>
                <div class="dict-main">
                  <div class="dict-title">{dict.title}</div>
                  <div class="dict-meta tabular">
                    {dict.format === "mdict" ? "MDict MDX" : "Kindle MOBI"} · {dict.entryCount.toLocaleString()} 词条
                  </div>
                </div>
                <button
                  type="button"
                  class="icon-btn danger-hover"
                  aria-label={`移除词典：${dict.title}`}
                  onclick={() => (confirmRemoveDict = dict)}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            {/each}
          {/if}
        </div>

        <div class="panel-foot">
          <button type="button" class="pill-btn" onclick={handleImportDictionary}>
            <Plus size={15} />
            <span>导入本地词典</span>
          </button>
        </div>
      </div>
    </section>

    <!-- Scope 4: 本地资料与备份 -->
    <section class="settings-block">
      <div class="block-head">
        <h2>本地资料与完整备份</h2>
        <span class="scope-tag">作用域：全部本地藏书、笔记、快照与设置</span>
      </div>

      <div class="surface-panel">
        {#if usage}
          <div class="storage-summary">
            <div class="storage-head">
              <span class="storage-label">
                <Database size={15} />
                <span>本机存储占用</span>
              </span>
              <span class="storage-total tabular">{formatBytes(usage.totalBytes)}</span>
            </div>

            <div class="storage-bar" aria-hidden="true">
              {#each usageSegments as seg (seg.label)}
                {#if seg.pct > 0.5}
                  <div class="bar-seg {seg.tone}" style:width={`${seg.pct}%`}></div>
                {/if}
              {/each}
            </div>

            <div class="storage-legend">
              {#each usageSegments as seg (seg.label)}
                <div class="legend-item">
                  <span class="legend-dot {seg.tone}"></span>
                  <span class="legend-name">{seg.label}</span>
                  <span class="legend-val tabular">{formatBytes(seg.bytes)}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <div class="backup-actions">
          <button type="button" class="accent-btn" onclick={handleBackup}>
            <Download size={15} />
            <span>导出 .atha-data 完整备份</span>
          </button>
          <button type="button" class="pill-btn" onclick={handleRestoreTrigger}>
            <Upload size={15} />
            <span>从备份文件恢复</span>
          </button>
          <input
            bind:this={restoreInputEl}
            type="file"
            accept=".json,.atha-data"
            hidden
            onchange={handleFileChosen}
          />
        </div>
      </div>
    </section>
  </div>
</section>

<ConfirmDialog
  bind:open={confirmResetOpen}
  title="恢复全书阅读默认？"
  description="将把全局阅读主题、亮度、字号、字体与行距还原为默认基线，各书籍的独立设置不受影响。"
  confirmLabel="确认恢复"
  onConfirm={executeResetDefaults}
/>

<ConfirmDialog
  open={Boolean(confirmRemoveDict)}
  onCancel={() => (confirmRemoveDict = null)}
  title="移除离线词典？"
  description={`将从本地索引移除《${confirmRemoveDict?.title ?? ""}》，已产生的笔记内容不会受影响。`}
  confirmLabel="移除词典"
  destructive
  onConfirm={executeRemoveDictionary}
/>

<ConfirmDialog
  bind:open={confirmRestoreOpen}
  title="从 .atha-data 备份完整恢复？"
  description="完整恢复将整体替换当前本地藏书、笔记消息、阅读进度与偏好设置（校验失败时自动回滚）。确定继续吗？"
  confirmLabel="开始恢复"
  destructive
  onConfirm={executeRestore}
/>

<style>
  .settings-shell {
    padding: 36px 40px 96px 248px;
    max-width: 920px;
    margin: 0 auto;
    min-height: 100vh;
  }

  .settings-header h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }

  .subtitle {
    margin: 8px 0 32px;
    font-size: 14px;
    color: var(--ink-secondary);
  }

  .sections-stack {
    display: flex;
    flex-direction: column;
    gap: 38px;
  }

  .settings-block {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .shelf-display-settings {
    display: grid;
    gap: 8px;
    max-width: 500px;
  }

  .shelf-display-settings label {
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    font-size: 14px;
  }

  .shelf-display-settings select {
    min-height: 44px;
    padding: 0 12px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    color: var(--ink-primary);
  }

  .shelf-display-settings input {
    width: 22px;
    height: 22px;
    margin: 0 10px;
    accent-color: var(--accent);
  }

  .block-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .block-head h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .scope-tag {
    font-size: 12px;
    color: var(--accent);
    font-weight: 500;
  }

  .block-desc {
    margin: 0;
    font-size: 13.5px;
    color: var(--ink-secondary);
  }

  .segmented-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px;
    border-radius: var(--radius-lg);
    background: var(--bg-well);
    width: fit-content;
  }

  .theme-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 40px;
    padding: 0 16px;
    border-radius: var(--radius-md);
    font-size: 13.5px;
    font-weight: 500;
    color: var(--ink-secondary);
    transition: background-color 180ms var(--ease-smooth), color 180ms var(--ease-smooth), box-shadow 180ms var(--ease-smooth);
  }

  .theme-pill.active {
    background: var(--bg-pill-active);
    color: var(--ink-primary);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .action-surface,
  .surface-panel {
    padding: 20px 22px;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .action-surface {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .action-title {
    font-size: 14.5px;
    font-weight: 600;
  }

  .action-sub {
    font-size: 13px;
    color: var(--ink-secondary);
    margin-top: 3px;
  }

  .panel-top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .panel-label {
    font-size: 13.5px;
    font-weight: 600;
  }

  .scale-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .scale-pill {
    min-height: 32px;
    padding: 0 11px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    color: var(--ink-secondary);
    font-size: 12.5px;
    font-weight: 500;
  }

  .scale-pill.active {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }

  .dict-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dict-empty {
    padding: 18px 0;
    color: var(--ink-muted);
    font-size: 13.5px;
  }

  .dict-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .dict-icon {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-surface);
    color: var(--accent);
    flex: none;
  }

  .dict-main {
    flex: 1;
    min-width: 0;
  }

  .dict-title {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dict-meta {
    font-size: 12px;
    color: var(--ink-muted);
  }

  :global(.danger-hover:hover) {
    color: var(--danger) !important;
    background: var(--danger-soft) !important;
  }

  .panel-foot {
    display: flex;
    justify-content: flex-start;
  }

  .storage-summary {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .storage-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .storage-label {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 14px;
    font-weight: 600;
  }

  .storage-total {
    font-size: 14.5px;
    font-weight: 700;
    color: var(--accent);
  }

  .storage-bar {
    height: 10px;
    border-radius: 999px;
    background: var(--bg-well);
    overflow: hidden;
    display: flex;
    gap: 2px;
  }

  .bar-seg {
    height: 100%;
    border-radius: 999px;
  }

  .tone-1 { background: var(--accent); }
  .tone-2 { background: #4B7B6B; }
  .tone-3 { background: #C29148; }
  .tone-4 { background: #7D8590; }

  .storage-legend {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 22px;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--ink-secondary);
  }

  .legend-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
  }

  .legend-val {
    color: var(--ink-muted);
  }

  .backup-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .toast-banner {
    position: fixed;
    top: 20px;
    left: 50%;
    translate: -50% 0;
    z-index: 60;
    padding: 10px 20px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 500;
  }

  @media (max-width: 899px) {
    .settings-shell {
      padding: 22px 16px 104px;
    }

    .settings-header h1 {
      font-size: 26px;
    }

    .action-surface {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
