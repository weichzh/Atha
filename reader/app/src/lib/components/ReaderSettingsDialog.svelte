<script lang="ts">
  import { Check, Code2, RotateCcw, SlidersHorizontal, Sun, X } from "@lucide/svelte";
  import { Dialog, Slider, Switch } from "bits-ui";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { ReaderEngine } from "../engine/reader-engine.svelte.ts";
  import type {
    PageMargin,
    ParagraphIndent,
    ParagraphSpacing,
    ReaderDensity,
    ReaderFontFamily,
    ReaderTheme,
    ReadingMode,
  } from "../types.ts";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  interface Props {
    open: boolean;
    engine: ReaderEngine;
    bookTitle: string;
    onClosed?: () => void;
  }

  let { open = $bindable(false), engine, bookTitle, onClosed }: Props = $props();

  let activeTab = $state<"global" | "book" | "css">("global");
  let confirmResetBook = $state(false);

  const appPrefs = $derived(engine.preferences.application);
  const bookPrefs = $derived(engine.preferences.book);

  const themes: { value: ReaderTheme; label: string; swatch: string; ring: string }[] = [
    { value: "system", label: "系统", swatch: "linear-gradient(135deg, #FAF9F5 50%, #1C1B19 50%)", ring: "rgba(128,120,108,0.35)" },
    { value: "light", label: "浅白", swatch: "#FAF9F5", ring: "rgba(26,25,22,0.18)" },
    { value: "paper", label: "暖纸", swatch: "#F4EFE4", ring: "rgba(168,68,44,0.28)" },
    { value: "dark", label: "夜间", swatch: "#181715", ring: "rgba(255,255,255,0.22)" },
  ];

  async function setGlobal(patch: Record<string, unknown>) {
    await engine.updatePreferences("application", patch);
  }

  async function setBook(patch: Record<string, unknown>) {
    await engine.updatePreferences("book", patch);
  }

  async function executeResetBook() {
    await engine.resetPreferences("book");
  }
</script>

<Dialog.Root bind:open onOpenChange={(nextOpen) => { if (!nextOpen) onClosed?.(); }}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 150 }}></div>
        {/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content wide reader-settings-dialog"
            transition:scale={{ start: 0.96, duration: 190, easing: cubicOut }}
            onpointerdown={(e) => e.stopPropagation()}
          >
            <div class="rs-head">
              <div>
                <Dialog.Title class="dialog-heading">阅读排版设置</Dialog.Title>
                <Dialog.Description class="dialog-sub">
                  {#if activeTab === "global"}
                    作用域：全书阅读默认（影响所有未单独定制的书籍）
                  {:else if activeTab === "book"}
                    作用域：仅当前书籍《{bookTitle}》
                  {:else}
                    作用域：仅当前书籍的用户 CSS 模块（受安全边界校验）
                  {/if}
                </Dialog.Description>
              </div>
              <button type="button" class="icon-btn" aria-label="关闭设置" onclick={() => (open = false)}>
                <X size={18} />
              </button>
            </div>

            <div class="scope-tabs" role="tablist" aria-label="设置作用域">
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "global"}
                class="scope-tab"
                class:active={activeTab === "global"}
                onclick={() => (activeTab = "global")}
              >
                <Sun size={14} />
                <span>全书阅读默认</span>
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "book"}
                class="scope-tab"
                class:active={activeTab === "book"}
                onclick={() => (activeTab = "book")}
              >
                <SlidersHorizontal size={14} />
                <span>本书覆盖</span>
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "css"}
                class="scope-tab"
                class:active={activeTab === "css"}
                onclick={() => (activeTab = "css")}
              >
                <Code2 size={14} />
                <span>CSS 模块</span>
              </button>
            </div>

            {#if activeTab === "global"}
              <div class="settings-grid">
                <div class="setting-block">
                  <div class="setting-label">书页色调</div>
                  <div class="theme-dots" role="radiogroup" aria-label="书页色调">
                    {#each themes as t (t.value)}
                      <button
                        type="button"
                        role="radio"
                        aria-checked={appPrefs.theme === t.value}
                        class="theme-dot-btn"
                        class:active={appPrefs.theme === t.value}
                        onclick={() => void setGlobal({ theme: t.value })}
                      >
                        <span class="dot-circle" style:background={t.swatch} style:box-shadow={`0 2px 8px ${t.ring}`}>
                          {#if appPrefs.theme === t.value}
                            <Check size={14} class={t.value === "dark" ? "dot-check-light" : "dot-check-dark"} />
                          {/if}
                        </span>
                        <span class="dot-label">{t.label}</span>
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="setting-block">
                  <div class="setting-head">
                    <span class="setting-label">页面字号</span>
                    <span class="setting-val tabular">{appPrefs.fontSize} px</span>
                  </div>
                  <Slider.Root
                    type="single"
                    min={16}
                    max={40}
                    step={1}
                    value={appPrefs.fontSize}
                    onValueCommit={(val) => void setGlobal({ fontSize: val })}
                    class="atha-slider"
                    aria-label="页面字号"
                  >
                    <span class="atha-slider-track">
                      <Slider.Range class="atha-slider-range" />
                    </span>
                    <Slider.Thumb index={0} class="atha-slider-thumb" />
                  </Slider.Root>
                </div>

                <div class="setting-block">
                  <div class="setting-head">
                    <span class="setting-label">画面亮度</span>
                    <span class="setting-val tabular">{appPrefs.brightness}%</span>
                  </div>
                  <Slider.Root
                    type="single"
                    min={70}
                    max={120}
                    step={5}
                    value={appPrefs.brightness}
                    onValueCommit={(val) => void setGlobal({ brightness: val })}
                    class="atha-slider"
                    aria-label="画面亮度"
                  >
                    <span class="atha-slider-track">
                      <Slider.Range class="atha-slider-range" />
                    </span>
                    <Slider.Thumb index={0} class="atha-slider-thumb" />
                  </Slider.Root>
                </div>

                <div class="setting-block">
                  <div class="setting-label">正文字体</div>
                  <div class="pill-group">
                    {#each [
                      { value: "book", label: "书籍原设" },
                      { value: "serif", label: "人文衬线" },
                      { value: "sans", label: "现代无衬线" },
                    ] as f (f.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={appPrefs.fontFamily === f.value}
                        onclick={() => void setGlobal({ fontFamily: f.value as ReaderFontFamily })}
                      >
                        {f.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="setting-block">
                  <div class="setting-label">行间密度</div>
                  <div class="pill-group">
                    {#each [
                      { value: "compact", label: "紧凑" },
                      { value: "standard", label: "标准" },
                      { value: "comfortable", label: "舒展" },
                    ] as d (d.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={appPrefs.density === d.value}
                        onclick={() => void setGlobal({ density: d.value as ReaderDensity })}
                      >
                        {d.label}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>
            {:else if activeTab === "book"}
              <div class="settings-grid">
                <div class="setting-block">
                  <div class="setting-label">阅读方式</div>
                  <div class="pill-group">
                    {#each [
                      { value: "paged", label: "分页翻页" },
                      { value: "scroll", label: "连续滚动" },
                    ] as m (m.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={bookPrefs.readingMode === m.value}
                        onclick={() => void setBook({ readingMode: m.value as ReadingMode })}
                      >
                        {m.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="setting-block">
                  <div class="setting-label">视觉页边距</div>
                  <div class="pill-group">
                    {#each [
                      { value: "narrow", label: "窄 (24px)" },
                      { value: "standard", label: "标准 (32px)" },
                      { value: "wide", label: "宽 (48px)" },
                    ] as pm (pm.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={bookPrefs.pageMargin === pm.value}
                        onclick={() => void setBook({ pageMargin: pm.value as PageMargin })}
                      >
                        {pm.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="setting-block">
                  <div class="setting-label">首行缩进</div>
                  <div class="pill-group">
                    {#each [
                      { value: "none", label: "保持原样" },
                      { value: "two", label: "缩进两字" },
                    ] as ind (ind.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={bookPrefs.paragraphIndent === ind.value}
                        onclick={() => void setBook({ paragraphIndent: ind.value as ParagraphIndent })}
                      >
                        {ind.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="setting-block">
                  <div class="setting-label">段落间距</div>
                  <div class="pill-group">
                    {#each [
                      { value: "book", label: "原书" },
                      { value: "compact", label: "紧凑" },
                      { value: "comfortable", label: "宽松" },
                    ] as ps (ps.value)}
                      <button
                        type="button"
                        class="pill-btn"
                        class:active={bookPrefs.paragraphSpacing === ps.value}
                        onclick={() => void setBook({ paragraphSpacing: ps.value as ParagraphSpacing })}
                      >
                        {ps.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="switch-row">
                  <div>
                    <div class="setting-label">启用书源自带样式</div>
                    <div class="switch-sub">关闭后仅保留阅读器基线排版与你的 CSS 模块</div>
                  </div>
                  <Switch.Root
                    checked={bookPrefs.sourceStyles}
                    onCheckedChange={(checked) => void setBook({ sourceStyles: checked })}
                    class="atha-switch"
                  >
                    <Switch.Thumb class="atha-switch-thumb" />
                  </Switch.Root>
                </div>

                <div class="book-reset-row">
                  <button type="button" class="pill-btn" onclick={() => (confirmResetBook = true)}>
                    <RotateCcw size={14} />
                    <span>恢复本书默认覆盖</span>
                  </button>
                </div>
              </div>
            {:else}
              {#await import("./CssModulesPanel.svelte")}
                <div class="lazy-loading">正在加载 CSS 样式编辑器…</div>
              {:then { default: CssModulesPanel }}
                <CssModulesPanel {engine} />
              {/await}
            {/if}
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<ConfirmDialog
  bind:open={confirmResetBook}
  title="恢复本书覆盖设置？"
  description={`将重置《${bookTitle}》的翻页模式、页边距、段落间距及本书用户 CSS 模块，全局默认设置不受影响。`}
  confirmLabel="确认恢复"
  destructive
  onConfirm={executeResetBook}
/>

<style>
  .reader-settings-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
  }

  .rs-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 16px;
  }

  .scope-tabs {
    display: flex;
    gap: 6px;
    padding: 5px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    margin-bottom: 22px;
  }

  .scope-tab {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 12px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-secondary);
    transition: background-color 180ms var(--ease-smooth), color 180ms var(--ease-smooth);
  }

  .scope-tab.active {
    background: var(--bg-pill-active);
    color: var(--ink-primary);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .settings-grid {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .setting-block {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .setting-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .setting-label {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink-primary);
  }

  .setting-val {
    font-size: 13px;
    color: var(--accent);
    font-weight: 600;
  }

  .theme-dots {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }

  .theme-dot-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    padding: 10px 6px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    transition: background-color 160ms var(--ease-smooth);
  }

  .theme-dot-btn.active {
    background: var(--accent-soft);
  }

  .dot-circle {
    width: 34px;
    height: 34px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  :global(.dot-check-light) {
    color: #ffffff;
  }

  :global(.dot-check-dark) {
    color: var(--accent);
  }

  .dot-label {
    font-size: 12.5px;
    font-weight: 500;
  }

  .pill-group {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  :global(.atha-slider) {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    height: 28px;
    touch-action: none;
    user-select: none;
  }

  :global(.atha-slider-track) {
    position: relative;
    width: 100%;
    height: 6px;
    border-radius: 999px;
    background: var(--bg-well-active);
    overflow: hidden;
  }

  :global(.atha-slider-range) {
    position: absolute;
    height: 100%;
    background: var(--accent);
    border-radius: 999px;
  }

  :global(.atha-slider-thumb) {
    display: block;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    background: #ffffff;
    box-shadow: var(--shadow-sm);
    cursor: grab;
  }

  .switch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .switch-sub {
    font-size: 12px;
    color: var(--ink-muted);
    margin-top: 2px;
  }

  .book-reset-row {
    display: flex;
    justify-content: flex-start;
  }

  .lazy-loading {
    padding: 36px 16px;
    text-align: center;
    color: var(--ink-muted);
    font-size: 13.5px;
  }
</style>
