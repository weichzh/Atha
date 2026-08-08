<script lang="ts">
  import { onMount } from "svelte";

  let host: HTMLDivElement;
  let input: HTMLTextAreaElement;
  let ready = false;

  onMount(() => {
    let disposed = false;
    let editor: {
      contentDOM: HTMLElement;
      destroy(): void;
      dispatch(value: unknown): void;
      dom: HTMLElement;
      state: { doc: { toString(): string } };
    } | null = null;
    let syncing = false;
    const page = host.closest<HTMLElement>('[data-settings-page="modules"]');

    const initialize = async () => {
      if (disposed || editor || page?.hidden) return;
      const [{ basicSetup, EditorView }, { css }, { linter, lintGutter }, { syntaxTree }] = await Promise.all([
        import("codemirror"),
        import("@codemirror/lang-css"),
        import("@codemirror/lint"),
        import("@codemirror/language"),
      ]);
      if (disposed) return;
      editor = new EditorView({
        doc: input.value,
        parent: host,
        extensions: [
          basicSetup,
          css(),
          lintGutter(),
          linter((view) => {
            const diagnostics: Array<{
              from: number;
              to: number;
              severity: "error";
              message: string;
            }> = [];
            syntaxTree(view.state).iterate({
              enter(node) {
                if (node.type.isError) {
                  diagnostics.push({
                    from: node.from,
                    to: Math.max(node.from, node.to),
                    severity: "error",
                    message: "CSS 语法不完整",
                  });
                }
              },
            });
            return diagnostics;
          }, { delay: 100 }),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": "CSS 模块代码" }),
          EditorView.updateListener.of((update) => {
            if (!syncing && update.docChanged) {
              input.value = update.state.doc.toString();
              input.dispatchEvent(new Event("input", { bubbles: true }));
            }
          }),
          EditorView.theme({
            "&": { minHeight: "180px", fontSize: "14px" },
            ".cm-scroller": { minHeight: "180px", fontFamily: "Consolas, 'Cascadia Mono', monospace" },
            ".cm-content": { padding: "10px 0" },
          }),
        ],
      });
      editor.contentDOM.contentEditable = String(!input.disabled);
      editor.dom.setAttribute("aria-disabled", String(input.disabled));
      ready = true;
    };

    const sync = () => {
      if (!editor) return;
      editor.contentDOM.contentEditable = String(!input.disabled);
      editor.dom.setAttribute("aria-disabled", String(input.disabled));
      const current = editor.state.doc.toString();
      if (current === input.value) return;
      syncing = true;
      editor.dispatch({ changes: { from: 0, to: current.length, insert: input.value } });
      syncing = false;
    };
    input.addEventListener("atha-css-editor-sync", sync);
    const observer = new MutationObserver(initialize);
    if (page) observer.observe(page, { attributes: true, attributeFilter: ["hidden"] });
    void initialize();
    return () => {
      disposed = true;
      observer.disconnect();
      input.removeEventListener("atha-css-editor-sync", sync);
      editor?.destroy();
    };
  });
</script>

<div class="css-editor-shell" class:is-ready={ready}>
  <textarea
    bind:this={input}
    id="user-stylesheet"
    class="css-editor-input"
    rows="9"
    maxlength="32768"
    spellcheck="false"
    aria-label="CSS 模块代码"
    placeholder={"例如：p { letter-spacing: 1px; }"}
  ></textarea>
  <div bind:this={host} class="css-editor-host" hidden={!ready}></div>
</div>
