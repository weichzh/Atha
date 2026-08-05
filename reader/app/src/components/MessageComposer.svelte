<script lang="ts">
  import { Editor, generateHTML, type JSONContent } from "@tiptap/core";
  import {
    Bold as BoldIcon,
    FileText,
    Italic as ItalicIcon,
    Link2,
    List,
    ListOrdered,
    Maximize2,
    Minimize2,
    Quote,
    Redo2,
    Send,
    Undo2,
  } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { isSafeMessageLink, messageExtensions } from "../message-editor";

  const extensions = messageExtensions;

  let element: HTMLDivElement;
  let markdownElement: HTMLTextAreaElement;
  let editorState = $state<{ editor: Editor | null }>({ editor: null });
  let markdownCodec: ReturnType<
    typeof import("../message-markdown").createMessageMarkdownCodec
  >;
  let mode = $state<"visual" | "markdown">("visual");
  let markdown = $state("");
  let markdownError = $state("");
  let empty = $state(true);
  let expanded = $state(false);
  let tall = $state(false);
  let tooLong = $state(false);

  function plainDocument(text: string): JSONContent {
    return {
      type: "doc",
      content: (text || "").split("\n").map((line) => ({
        type: "paragraph",
        content: line ? [{ type: "text", text: line }] : undefined,
      })),
    };
  }

  function documentFrom(contentJson: string) {
    try {
      const richText = JSON.parse(contentJson)?.richText;
      return richText?.schema === 1 && richText.document?.type === "doc"
        ? (richText.document as JSONContent)
        : null;
    } catch {
      return null;
    }
  }

  function refresh(editor: Editor) {
    editorState = { editor };
    const text = editor.getText({ blockSeparator: "\n" }).trim();
    empty = !text;
    tooLong = [...text].length > 8_000;
    requestAnimationFrame(() => {
      tall = (element?.querySelector<HTMLElement>(".ProseMirror")?.scrollHeight ?? 0) > 118;
    });
  }

  function refreshMarkdown() {
    empty = !markdown.trim();
    tooLong = [...markdown].length > 8_000;
    markdownError = "";
  }

  function updateMarkdown(event: Event) {
    markdown = (event.currentTarget as HTMLTextAreaElement).value;
    refreshMarkdown();
  }

  async function showMarkdown() {
    if (mode === "markdown") return;
    const editor = editorState.editor;
    if (!editor) return;
    if (!markdownCodec) {
      const { createMessageMarkdownCodec } = await import("../message-markdown");
      markdownCodec = createMessageMarkdownCodec(editor.schema);
    }
    markdown = markdownCodec.serialize(editor.getJSON());
    mode = "markdown";
    refreshMarkdown();
    requestAnimationFrame(() => markdownElement.focus());
  }

  function showVisual() {
    if (mode === "visual") return true;
    const editor = editorState.editor;
    if (!editor) return false;
    try {
      editor.commands.setContent(markdownCodec.parse(markdown));
      mode = "visual";
      markdownError = "";
      refresh(editor);
      requestAnimationFrame(() => editor.commands.focus("end"));
      return true;
    } catch {
      markdownError = "这段 Markdown 含有当前不支持的格式";
      return false;
    }
  }

  function collapseEditor() {
    if (mode === "markdown" && !showVisual()) return;
    expanded = false;
  }

  function toggleLink() {
    const editor = editorState.editor;
    if (!editor) return;
    const current = editor.getAttributes("link").href || "";
    const entered = window.prompt("链接地址", current);
    if (entered === null) return;
    if (!entered.trim()) {
      editor.chain().focus().extendMarkRange("link").unsetLink().run();
      return;
    }
    const href = /^https?:\/\//i.test(entered.trim())
      ? entered.trim()
      : `https://${entered.trim()}`;
    if (!isSafeMessageLink(href)) return;
    editor.chain().focus().extendMarkRange("link").setLink({ href }).run();
  }

  onMount(() => {
    const editor = new Editor({
      element,
      extensions,
      content: plainDocument(""),
      editorProps: {
        attributes: { "aria-label": "消息内容", role: "textbox" },
        handleKeyDown: (_view, event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            element.closest("form")?.requestSubmit();
            return true;
          }
          return false;
        },
      },
      onTransaction: ({ editor }) => refresh(editor),
    });
    editorState = { editor };
    const controller: Window["athaMessageComposer"] = {
      clear() {
        editor.commands.setContent(plainDocument(""));
        mode = "visual";
        markdown = "";
        markdownError = "";
        expanded = false;
      },
      collapse() {
        mode = "visual";
        markdownError = "";
        expanded = false;
      },
      expand() {
        expanded = true;
        requestAnimationFrame(() => editor.commands.focus("end"));
      },
      focus() {
        requestAnimationFrame(() =>
          mode === "markdown" ? markdownElement.focus() : editor.commands.focus("end"),
        );
      },
      render(target, contentJson, fallback) {
        const document = documentFrom(contentJson);
        if (!document) {
          target.textContent = fallback;
          return;
        }
        try {
          target.innerHTML = generateHTML(document, extensions);
        } catch {
          target.textContent = fallback;
        }
      },
      setValue(contentJson, fallback) {
        editor.commands.setContent(documentFrom(contentJson) || plainDocument(fallback));
        mode = "visual";
        markdown = "";
        markdownError = "";
        expanded = false;
      },
      value() {
        let document: JSONContent;
        try {
          document = mode === "markdown" ? markdownCodec.parse(markdown) : editor.getJSON();
        } catch {
          markdownError = "这段 Markdown 含有当前不支持的格式";
          return null;
        }
        const node = editor.schema.nodeFromJSON(document);
        return {
          text: node.textBetween(0, node.content.size, "\n").trim(),
          richText: { schema: 1, document: document as Record<string, unknown> },
        };
      },
    };
    window.athaMessageComposer = controller;
    refresh(editor);
    return () => {
      if (window.athaMessageComposer === controller) delete window.athaMessageComposer;
      editor.destroy();
    };
  });
</script>

<div class="message-editor" data-expanded={expanded} data-tall={tall}>
  {#if expanded && editorState.editor}
    <div class="message-editor-toolbar" role="toolbar" aria-label="富文本工具">
      <div class="message-editor-toolbar-primary">
        <button class="message-editor-collapse" type="button" aria-label="收起编辑器" title="收起编辑器" onclick={collapseEditor}>
          <Minimize2 aria-hidden="true" />
        </button>
        {#if mode === "visual"}
          <div class="message-editor-tool-group">
            <button type="button" aria-label="撤销" title="撤销" onclick={() => editorState.editor?.chain().focus().undo().run()}>
              <Undo2 aria-hidden="true" />
            </button>
            <button type="button" aria-label="重做" title="重做" onclick={() => editorState.editor?.chain().focus().redo().run()}>
              <Redo2 aria-hidden="true" />
            </button>
          </div>
        {/if}
        <div class="message-editor-mode-switch" aria-label="输入模式">
          <button type="button" class:active={mode === "visual"} aria-pressed={mode === "visual"} onclick={showVisual}>可视</button>
          <button type="button" class:active={mode === "markdown"} aria-pressed={mode === "markdown"} onclick={showMarkdown}>
            <FileText aria-hidden="true" />
            Markdown
          </button>
        </div>
      </div>
      {#if mode === "visual"}
        <div class="message-editor-toolbar-secondary">
          <div class="message-editor-tool-group">
            <button
              type="button"
              class:active={editorState.editor.isActive("heading")}
              aria-label="标题"
              title="标题"
              onclick={() => editorState.editor?.chain().focus().toggleHeading({ level: 2 }).run()}
            >Aa</button>
            <button type="button" class:active={editorState.editor.isActive("bold")} aria-label="粗体" title="粗体" onclick={() => editorState.editor?.chain().focus().toggleBold().run()}><BoldIcon aria-hidden="true" /></button>
            <button type="button" class:active={editorState.editor.isActive("italic")} aria-label="斜体" title="斜体" onclick={() => editorState.editor?.chain().focus().toggleItalic().run()}><ItalicIcon aria-hidden="true" /></button>
            <button type="button" class:active={editorState.editor.isActive("bulletList")} aria-label="项目列表" title="项目列表" onclick={() => editorState.editor?.chain().focus().toggleBulletList().run()}><List aria-hidden="true" /></button>
            <button type="button" class:active={editorState.editor.isActive("orderedList")} aria-label="编号列表" title="编号列表" onclick={() => editorState.editor?.chain().focus().toggleOrderedList().run()}><ListOrdered aria-hidden="true" /></button>
            <button type="button" class:active={editorState.editor.isActive("blockquote")} aria-label="引用" title="引用" onclick={() => editorState.editor?.chain().focus().toggleBlockquote().run()}><Quote aria-hidden="true" /></button>
            <button type="button" class:active={editorState.editor.isActive("link")} aria-label="链接" title="链接" onclick={toggleLink}><Link2 aria-hidden="true" /></button>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <div class="message-editor-input">
    {#if empty}<span class="message-editor-placeholder">{mode === "markdown" ? "使用 Markdown 写下想法…" : "写下想法…"}</span>{/if}
    <div bind:this={element} hidden={mode === "markdown"}></div>
    <textarea
      bind:this={markdownElement}
      class="message-editor-markdown"
      value={markdown}
      hidden={mode !== "markdown"}
      aria-label="Markdown 消息内容"
      spellcheck="true"
      oninput={updateMarkdown}
    ></textarea>
    {#if markdownError}<output class="message-editor-error">{markdownError}</output>{/if}
  </div>

  <div class="message-editor-actions">
    {#if tall && !expanded}
      <button class="message-editor-expand" type="button" aria-label="全屏编辑" title="全屏编辑" onclick={() => (expanded = true)}>
        <Maximize2 aria-hidden="true" />
      </button>
    {/if}
    <button class="message-send-button" type="submit" aria-label="发送" title={tooLong ? "消息最多 8000 个字符" : "发送"} disabled={empty || tooLong || Boolean(markdownError)}>
      <Send aria-hidden="true" />
    </button>
  </div>
</div>
