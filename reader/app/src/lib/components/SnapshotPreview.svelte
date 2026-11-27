<script lang="ts">
  import type { SourceCaptureView } from "../../messages.ts";

  interface Props {
    capture: SourceCaptureView;
  }

  let { capture }: Props = $props();
  let hostEl = $state<HTMLDivElement | null>(null);

  function sanitizeCss(raw: string): string {
    if (!raw) return "";
    if (/@import|(?:url|src|image|image-set)\s*\(/i.test(raw) || raw.includes("\\")) {
      return "";
    }
    return raw.replace(/:host(?:-context)?\b|::part\b|::slotted\b/gi, ".book");
  }

  function sanitizeHtml(raw: string): DocumentFragment {
    const parser = new DOMParser();
    const doc = parser.parseFromString(`<body>${raw}</body>`, "text/html");
    doc.querySelectorAll("script, iframe, object, embed, form, input, button, select, textarea, style, link, meta, base").forEach((el) => el.remove());
    for (const el of doc.body.querySelectorAll("*")) {
      for (const attr of [...el.attributes]) {
        const name = attr.name.toLowerCase();
        if (name.startsWith("on") || name === "srcset" || (name === "href" && !attr.value.startsWith("#"))) {
          el.removeAttribute(attr.name);
        }
      }
    }
    const frag = document.createDocumentFragment();
    while (doc.body.firstChild) {
      frag.appendChild(doc.body.firstChild);
    }
    return frag;
  }

  $effect(() => {
    if (!hostEl) return;
    const shadow = hostEl.shadowRoot ?? hostEl.attachShadow({ mode: "open" });
    let presentation = { theme: "paper", fontSize: 18, fontFamily: "book", density: "standard", brightness: 100 };
    try {
      const parsed = JSON.parse(capture.snapshot.presentationJson || "{}");
      presentation = { ...presentation, ...parsed };
    } catch {
      // keep defaults
    }

    const themeBg: Record<string, string> = {
      light: "#FAF9F5",
      paper: "#F4EFE4",
      dark: "#181715",
    };
    const themeFg: Record<string, string> = {
      light: "#1A1916",
      paper: "#26211A",
      dark: "#E8E3D9",
    };
    const bg = themeBg[presentation.theme] ?? themeBg.paper;
    const fg = themeFg[presentation.theme] ?? themeFg.paper;

    const baseCss = `
      :host {
        display: block;
        border-radius: 14px;
        overflow: hidden;
        background: ${bg};
        color: ${fg};
      }
      .book {
        padding: 20px 24px;
        font-size: ${Math.min(22, Math.max(15, Number(presentation.fontSize) || 18))}px;
        line-height: 1.75;
        filter: brightness(${(Number(presentation.brightness) || 100) / 100});
        font-family: 'Palatino Linotype', 'Songti SC', 'Noto Serif SC', Georgia, serif;
      }
      .book p { margin: 0 0 0.7em; }
      .book p:last-child { margin-bottom: 0; }
      .book blockquote { margin: 0; padding-left: 14px; opacity: 0.92; }
    `;

    const styleBase = document.createElement("style");
    styleBase.textContent = baseCss;
    const styleReader = document.createElement("style");
    styleReader.textContent = sanitizeCss(capture.snapshot.readerCss);
    const styleBook = document.createElement("style");
    styleBook.textContent = sanitizeCss(capture.snapshot.bookCss);
    const styleUser = document.createElement("style");
    styleUser.textContent = sanitizeCss(capture.snapshot.userCss);

    const article = document.createElement("article");
    article.className = "book";
    article.dataset.theme = presentation.theme;
    article.dataset.fontFamily = presentation.fontFamily;
    article.dataset.density = presentation.density;
    article.appendChild(sanitizeHtml(capture.snapshot.fragmentHtml));

    shadow.replaceChildren(styleBase, styleReader, styleBook, styleUser, article);
  });
</script>

<div bind:this={hostEl} class="snapshot-shadow-host"></div>

<style>
  .snapshot-shadow-host {
    display: block;
    width: 100%;
    border-radius: var(--radius-md);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
  }
</style>
