<script lang="ts">
  import { Editor, type JSONContent } from "@tiptap/core";
  import {
    ArrowUp,
    Bold,
    Code2,
    Heading2,
    Italic,
    Link2,
    List,
    Maximize2,
    Minimize2,
    Quote,
    Type,
    X,
  } from "@lucide/svelte";
  import { Dialog } from "bits-ui";
  import { onDestroy, onMount, untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { isSafeMessageLink, messageExtensions } from "../../message-editor.ts";
  import { createMessageMarkdownCodec } from "../../message-markdown.ts";
  import type { RichTextInput } from "../../messages.ts";
  import { extractMessageDocJson } from "../data/message-doc.ts";
  import type { ComposerSubmitPayload } from "../types.ts";

  interface Props {
    placeholder?: string;
    draftKey: string;
    sealDraftOnSubmit?: boolean;
    initialText?: string;
    initialContentJson?: string;
    replyChipLabel?: string | null;
    referenceChipLabels?: { id: string; label: string }[];
    editingLabel?: string | null;
    submitLabel?: string;
    onClearReply?: () => void;
    onRemoveReference?: (id: string) => void;
    onCancelEdit?: () => void;
    onSubmit: (payload: ComposerSubmitPayload) => Promise<boolean> | boolean;
  }

  let {
    draftKey,
    sealDraftOnSubmit = false,
    placeholder = "写下笔记或回复…",
    initialText = "",
    initialContentJson = "",
    replyChipLabel = null,
    referenceChipLabels = [],
    editingLabel = null,
    submitLabel = "发送",
    onClearReply,
    onRemoveReference,
    onCancelEdit,
    onSubmit,
  }: Props = $props();

  let editorEl = $state<HTMLDivElement | null>(null);
  let editor = $state<Editor | null>(null);
  let codec: ReturnType<typeof createMessageMarkdownCodec> | null = null;

  let mode = $state<"visual" | "markdown">("visual");
  let markdownSource = $state("");
  let markdownError = $state<string | null>(null);
  let expanded = $state(false);
  let showFormatBar = $state(false);
  const formatOptions = { heading: { level: 2 }, bold: undefined, italic: undefined,
    blockquote: undefined, bulletList: undefined, link: undefined };
  let activeFormats = $state<string[]>([]);
  let submitting = $state(false);
  let editingLocked = $state(false);
  let hasContent = $state(false);

  // Safe Link Dialog (bits-ui Dialog, zero window.prompt)
  let linkDialogOpen = $state(false);
  let linkUrl = $state("https://");
  let linkError = $state<string | null>(null);

  function parseInitialDoc(): JSONContent | string {
    const parsed = extractMessageDocJson(initialContentJson);
    if (parsed) return parsed as JSONContent;
    return initialText
      ? {
          type: "doc",
          content: [{ type: "paragraph", content: [{ type: "text", text: initialText }] }],
        }
      : "";
  }

  function pruneEmptyParagraphs(doc: JSONContent): JSONContent {
    const content = (doc.content ?? []).filter(
      (node) => !(node.type === "paragraph" && (!node.content || node.content.length === 0)),
    );
    if (content.length === 0) {
      return { type: "doc", content: [{ type: "paragraph" }] };
    }
    return { ...doc, content };
  }

  function extractPlainTextFromDoc(doc: JSONContent): string {
    const lines: string[] = [];
    for (const node of doc.content ?? []) {
      const parts: string[] = [];
      const walk = (n: JSONContent) => {
        if (n.text) parts.push(n.text);
        if (n.type === "hardBreak") parts.push("\n");
        if (Array.isArray(n.content)) n.content.forEach(walk);
      };
      walk(node);
      if (parts.length) lines.push(parts.join(""));
    }
    return lines.join("\n").trim();
  }

  function updateHasContent() {
    if (mode === "markdown") {
      hasContent = markdownSource.trim().length > 0;
    } else if (editor) {
      hasContent = editor.getText().trim().length > 0;
    } else {
      hasContent = false;
    }
  }

  function handleSubmitKey(event: KeyboardEvent, composing = false) {
    if (event.key !== "Enter" || event.isComposing || composing || event.keyCode === 229) return false;
    const shortcut = event.metaKey || event.ctrlKey;
    const quickSend = mode === "visual" && !expanded && !event.shiftKey && !event.altKey &&
      window.matchMedia("(pointer: fine)").matches;
    if (!shortcut && !quickSend) return false;
    event.preventDefault();
    if (!event.repeat) void handleSend();
    return true;
  }

  function formatActive(type: keyof typeof formatOptions) {
    return mode === "visual" && activeFormats.includes(type);
  }

  onMount(() => {
    if (!editorEl) return;
    const instance = new Editor({
      element: editorEl,
      extensions: messageExtensions,
      content: parseInitialDoc(),
      editorProps: {
        attributes: {
          class: "atha-tiptap-body",
          role: "textbox",
          "aria-label": placeholder,
          "aria-multiline": "true",
        },
        handleKeyDown: (view, event) => handleSubmitKey(event, view.composing),
      },
      onTransaction: ({ editor }) => {
        activeFormats = Object.entries(formatOptions)
          .filter(([name, attrs]) => editor.isActive(name, attrs))
          .map(([name]) => name);
      },
      onUpdate: () => {
        updateHasContent();
      },
    });
    editor = instance;
    codec = createMessageMarkdownCodec(instance.schema);
    updateHasContent();
  });

  let lastPropSig = "";
  $effect(() => {
    const sig = `${draftKey}:${editingLabel ?? ""}:${initialText}:${initialContentJson}`;
    const ed = editor;
    if (!ed || sig === lastPropSig) return;
    lastPropSig = sig;
    untrack(() => {
      mode = "visual";
      markdownSource = "";
      markdownError = null;
      expanded = false;
      showFormatBar = false;
      ed.commands.setContent(parseInitialDoc());
      updateHasContent();
    });
  });

  onDestroy(() => {
    editor?.destroy();
  });

  function toggleMode() {
    if (!editor || !codec) return;
    markdownError = null;
    if (mode === "visual") {
      try {
        const cleaned = pruneEmptyParagraphs(editor.getJSON());
        markdownSource = codec.serialize(cleaned);
        mode = "markdown";
      } catch {
        markdownError = "当前内容包含无法映射的段落结构，请检查后重试。";
      }
    } else {
      try {
        const parsed = codec.parse(markdownSource);
        editor.commands.setContent(parsed);
        mode = "visual";
        updateHasContent();
      } catch {
        markdownError = "仅支持段落、三级以内标题、列表、引用、粗体、斜体和 http(s):// 安全链接。";
      }
    }
  }

  function openLinkModal() {
    linkError = null;
    const existingHref = editor?.getAttributes("link")?.href;
    linkUrl = typeof existingHref === "string" && existingHref ? existingHref : "https://";
    linkDialogOpen = true;
  }

  function submitLinkModal(event: SubmitEvent) {
    event.preventDefault();
    const trimmed = linkUrl.trim();
    if (!isSafeMessageLink(trimmed)) {
      linkError = "链接必须以 http:// 或 https:// 开头。";
      return;
    }
    if (mode === "markdown") {
      markdownSource = `${markdownSource}${markdownSource ? " " : ""}[${trimmed}](${trimmed})`;
      updateHasContent();
      linkDialogOpen = false;
      return;
    }
    if (!editor) return;
    const { from, to } = editor.state.selection;
    if (from === to) {
      editor
        .chain()
        .focus()
        .insertContent({
          type: "text",
          text: trimmed,
          marks: [{ type: "link", attrs: { href: trimmed } }],
        })
        .run();
    } else {
      editor.chain().focus().extendMarkRange("link").setLink({ href: trimmed }).run();
    }
    linkDialogOpen = false;
    updateHasContent();
  }

  async function handleSend() {
    if (submitting || !editor || !codec) return;
    markdownError = null;

    let documentJson: JSONContent;
    let plainText: string;

    if (mode === "markdown") {
      try {
        documentJson = codec.parse(markdownSource);
        plainText = extractPlainTextFromDoc(documentJson);
      } catch {
        markdownError = "Markdown 中包含不支持的语法或不安全链接（仅支持 http(s)://）。";
        return;
      }
    } else {
      documentJson = pruneEmptyParagraphs(editor.getJSON());
      plainText = editor.getText().trim();
    }

    if (!plainText) return;
    const sent = { editor, doc: editor.state.doc, mode, markdown: markdownSource, key: draftKey };
    submitting = true;
    editingLocked = sealDraftOnSubmit;
    if (editingLocked) sent.editor.setEditable(false, false);
    try {
      const richText: RichTextInput = {
        schema: 1,
        document: documentJson as Record<string, unknown>,
      };
      if (!(await onSubmit({ text: plainText, richText }))) return;
      if (editor !== sent.editor || editor.isDestroyed || draftKey !== sent.key || mode !== sent.mode ||
        markdownSource !== sent.markdown || !editor.state.doc.eq(sent.doc)) return;
      editor.commands.clearContent();
      markdownSource = "";
      mode = "visual";
      expanded = false;
      showFormatBar = false;
      updateHasContent();
    } catch {
      if (draftKey === sent.key) markdownError = "发送失败，草稿已保留";
    } finally {
      if (editingLocked && !sent.editor.isDestroyed) sent.editor.setEditable(true, false);
      editingLocked = false;
      submitting = false;
    }
  }
</script>

<div class="composer-card" class:expanded aria-busy={submitting}>
  {#if editingLabel || replyChipLabel || referenceChipLabels.length > 0}
    <div class="composer-chips">
      {#if editingLabel}
        <span class="context-chip edit-chip">
          <span>正在编辑 · {editingLabel}</span>
          {#if onCancelEdit}
            <button type="button" aria-label="取消编辑" onclick={onCancelEdit}>
              <X size={12} />
            </button>
          {/if}
        </span>
      {/if}
      {#if replyChipLabel}
        <span class="context-chip">
          <span>回复 · {replyChipLabel}</span>
          {#if onClearReply}
            <button type="button" aria-label="取消回复" onclick={onClearReply}>
              <X size={12} />
            </button>
          {/if}
        </span>
      {/if}
      {#each referenceChipLabels as ref (ref.id)}
        <span class="context-chip">
          <span>引用 · {ref.label}</span>
          {#if onRemoveReference}
            <button type="button" aria-label="移除引用" onclick={() => onRemoveReference(ref.id)}>
              <X size={12} />
            </button>
          {/if}
        </span>
      {/each}
    </div>
  {/if}

  {#if markdownError}
    <div class="composer-error" role="alert">{markdownError}</div>
  {/if}

  <div class="composer-pill">
    <button
      type="button"
      class="format-toggle"
      aria-label="排版工具"
      aria-controls="message-format-tools"
      aria-expanded={showFormatBar}
      disabled={editingLocked}
      onclick={() => (showFormatBar = !showFormatBar)}
    >
      <Type size={19} />
    </button>
    <div class="editor-area">
      {#if mode === "visual" && !hasContent}
        <span class="composer-placeholder" aria-hidden="true">{placeholder}</span>
      {/if}
      <div
        bind:this={editorEl}
        class="tiptap-mount"
        hidden={mode === "markdown"}
      ></div>
      {#if mode === "markdown"}
        <textarea
          class="markdown-textarea"
          aria-label="Markdown 源码"
          readonly={editingLocked}
          bind:value={markdownSource}
          oninput={updateHasContent}
          onkeydown={(event) => handleSubmitKey(event)}
          placeholder="支持 **粗体**、*斜体*、> 引用、## 标题、[链接](https://...)…"
        ></textarea>
      {/if}
    </div>
    <button
      type="button"
      class="send-btn"
      aria-label={submitLabel}
      title={submitting ? "发送中…" : submitLabel}
      disabled={!hasContent || submitting}
      onclick={() => void handleSend()}
    >
      <ArrowUp size={20} />
    </button>
  </div>

  <div id="message-format-tools" class="composer-toolbar" inert={editingLocked} hidden={!showFormatBar}>
    <div class="format-tools">
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("heading")}
        aria-pressed={formatActive("heading")}
        disabled={mode === "markdown"}
        aria-label="标题"
        title="标题"
        onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
      >
        <Heading2 size={15} />
      </button>
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("bold")}
        aria-pressed={formatActive("bold")}
        disabled={mode === "markdown"}
        aria-label="粗体"
        title="粗体"
        onclick={() => editor?.chain().focus().toggleBold().run()}
      >
        <Bold size={15} />
      </button>
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("italic")}
        aria-pressed={formatActive("italic")}
        disabled={mode === "markdown"}
        aria-label="斜体"
        title="斜体"
        onclick={() => editor?.chain().focus().toggleItalic().run()}
      >
        <Italic size={15} />
      </button>
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("blockquote")}
        aria-pressed={formatActive("blockquote")}
        disabled={mode === "markdown"}
        aria-label="引用"
        title="引用"
        onclick={() => editor?.chain().focus().toggleBlockquote().run()}
      >
        <Quote size={15} />
      </button>
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("bulletList")}
        aria-pressed={formatActive("bulletList")}
        disabled={mode === "markdown"}
        aria-label="列表"
        title="无序列表"
        onclick={() => editor?.chain().focus().toggleBulletList().run()}
      >
        <List size={15} />
      </button>
      <button
        type="button"
        class="tool-btn"
        class:active={formatActive("link")}
        aria-pressed={formatActive("link")}
        aria-label="插入链接"
        title="插入安全链接"
        onclick={openLinkModal}
      >
        <Link2 size={15} />
      </button>
    </div>

    <div class="right-tools">
      <button
        type="button"
        class="mode-btn"
        class:active={mode === "markdown"}
        aria-pressed={mode === "markdown"}
        aria-label="切换 Markdown 源码模式"
        onclick={toggleMode}
      >
        <Code2 size={14} />
        <span>{mode === "markdown" ? "可视化" : "Markdown"}</span>
      </button>
      <button
        type="button"
        class="tool-btn"
        aria-label={expanded ? "收起编辑器" : "展开编辑器"}
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        {#if expanded}
          <Minimize2 size={14} />
        {:else}
          <Maximize2 size={14} />
        {/if}
      </button>
    </div>
  </div>
</div>

<Dialog.Root bind:open={linkDialogOpen}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 120 }}></div>
        {/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount preventBodyPointerEvents={false}>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content link-dialog"
            transition:scale={{ start: 0.96, duration: 140, easing: cubicOut }}
          >
            <Dialog.Title class="dialog-heading">插入安全链接</Dialog.Title>
            <Dialog.Description class="dialog-sub">
              为防止脚本注入，仅接收以 http:// 或 https:// 开头的外部链接。
            </Dialog.Description>
            <form class="link-form" onsubmit={submitLinkModal}>
              <div class="well-input link-input-wrap">
                <input
                  type="text"
                  aria-label="链接地址"
                  bind:value={linkUrl}
                  placeholder="https://example.com"
                />
              </div>
              {#if linkError}
                <p class="link-err" role="alert">{linkError}</p>
              {/if}
              <div class="link-actions">
                <button type="button" class="pill-btn" onclick={() => (linkDialogOpen = false)}>
                  取消
                </button>
                <button type="submit" class="accent-btn">确定插入</button>
              </div>
            </form>
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  .composer-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .composer-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    max-height: 5.5rem;
    overflow: auto;
  }

  .context-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: var(--radius-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 500;
  }

  .context-chip button {
    display: inline-flex;
    align-items: center;
    color: inherit;
    opacity: 0.75;
  }

  .composer-toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 8px;
    padding: 4px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .composer-toolbar[hidden] {
    display: none;
  }

  .format-tools,
  .right-tools {
    display: inline-flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
  }

  .tool-btn {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-xs);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-secondary);
    transition: background-color 140ms var(--ease-smooth), color 140ms var(--ease-smooth);
  }

  .tool-btn:hover:not(:disabled) {
    background: var(--bg-well);
    color: var(--ink-primary);
  }

  .tool-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .mode-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 36px;
    padding: 0 10px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    color: var(--ink-secondary);
    font-size: 12px;
    font-weight: 500;
  }

  .mode-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .composer-error {
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 12.5px;
  }

  .composer-pill {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 4px;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    box-shadow: var(--shadow-sm);
  }

  .composer-pill:focus-within {
    outline: 1px solid var(--ring);
  }

  .format-toggle,
  .send-btn {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    color: var(--ink-secondary);
  }

  .format-toggle:hover:not(:disabled) {
    background: var(--bg-well-hover);
  }

  .format-toggle[aria-expanded="true"] {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .send-btn:not(:disabled) {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .send-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .editor-area {
    position: relative;
    flex: 1;
    min-width: 0;
    max-height: min(8.5rem, 24dvh);
    overflow: auto;
    font-size: 14px;
    line-height: 1.6;
    overflow-wrap: anywhere;
  }

  .editor-area :global(.ProseMirror) {
    min-height: 44px;
    padding: 11px 0;
  }

  .composer-placeholder {
    position: absolute;
    inset: 11px 0 auto;
    color: var(--ink-muted);
    pointer-events: none;
  }

  .expanded .editor-area {
    max-height: min(17rem, 38dvh);
  }

  .expanded .editor-area :global(.ProseMirror),
  .expanded .markdown-textarea {
    min-height: min(11rem, 32dvh);
  }

  .markdown-textarea {
    display: block;
    width: 100%;
    min-height: 4.5rem;
    padding: 10px 0;
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.55;
    resize: vertical;
  }

  @media (pointer: coarse) {
    .tool-btn,
    .mode-btn {
      min-height: 44px;
    }

    .tool-btn {
      min-width: 44px;
    }
  }

  .link-dialog {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
    max-width: 420px;
  }

  .link-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .link-input-wrap {
    height: 42px;
    padding: 0 14px;
    display: flex;
    align-items: center;
  }

  .link-input-wrap input {
    width: 100%;
    font-size: 14px;
  }

  .link-err {
    margin: 0;
    font-size: 12.5px;
    color: var(--danger);
  }

  .link-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
