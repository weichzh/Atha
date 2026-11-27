type DocRecord = Record<string, unknown>;

function escapeHtml(value: string): string {
  return value.replace(/[<>&]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;" }[c] || c));
}

function asDocRecord(value: unknown): DocRecord | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as DocRecord)
    : null;
}

/**
 * Extract the inner Tiptap JSON document from either a raw `{ type: "doc" }` payload
 * (demo fixtures) or the Rust backend envelope `{ schema: 1, kind: "text", text, richText: { schema: 1, document } }`.
 */
export function extractMessageDocJson(rawJson: string): DocRecord | null {
  if (!rawJson) return null;
  try {
    const parsed = asDocRecord(JSON.parse(rawJson));
    if (!parsed) return null;
    if (parsed.type === "doc") return parsed;
    const rich = asDocRecord(parsed.richText);
    const richDoc = asDocRecord(rich?.document);
    if (richDoc?.type === "doc") return richDoc;
    const nestedDoc = asDocRecord(parsed.document);
    if (nestedDoc?.type === "doc") return nestedDoc;
    return null;
  } catch {
    return null;
  }
}

export function extractMessageFallbackText(rawJson: string, fallbackText: string): string {
  if (fallbackText.trim()) return fallbackText;
  if (!rawJson) return "";
  try {
    const parsed = asDocRecord(JSON.parse(rawJson));
    return typeof parsed?.text === "string" ? parsed.text : "";
  } catch {
    return "";
  }
}

/**
 * Returns true when the Tiptap document contains formatting, structural blocks, or multiple
 * paragraphs beyond a single plain paragraph — used to decide whether to include formatting in the initial revision.
 */
export function docHasRichFormatting(doc: DocRecord | null | undefined): boolean {
  if (!doc || !Array.isArray(doc.content)) return false;
  if (doc.content.length > 1) return true;
  for (const rawBlock of doc.content) {
    const block = asDocRecord(rawBlock);
    if (!block || block.type !== "paragraph") return true;
    const children = Array.isArray(block.content) ? block.content : [];
    for (const rawChild of children) {
      const child = asDocRecord(rawChild);
      if (!child) continue;
      if (child.type !== "text") return true;
      if (Array.isArray(child.marks) && child.marks.length > 0) return true;
    }
  }
  return false;
}

function renderSafeNode(node: DocRecord): string {
  if (node.type === "text") {
    let txt = escapeHtml(String(node.text ?? ""));
    const marks = Array.isArray(node.marks) ? node.marks : [];
    for (const mark of marks as { type: string; attrs?: { href?: string } }[]) {
      if (mark.type === "bold") txt = `<strong>${txt}</strong>`;
      else if (mark.type === "italic") txt = `<em>${txt}</em>`;
      else if (mark.type === "link" && mark.attrs?.href && /^https?:\/\//i.test(mark.attrs.href)) {
        const safeHref = mark.attrs.href.replace(/"/g, "&quot;");
        txt = `<a href="${safeHref}" target="_blank" rel="noopener noreferrer">${txt}</a>`;
      }
    }
    return txt;
  }
  if (node.type === "hardBreak") return "<br/>";
  const children = Array.isArray(node.content)
    ? node.content
        .map((c) => asDocRecord(c))
        .filter((c): c is DocRecord => c !== null)
        .map(renderSafeNode)
        .join("")
    : "";
  switch (node.type) {
    case "doc":
      return children;
    case "paragraph":
      return `<p>${children}</p>`;
    case "blockquote":
      return `<blockquote>${children}</blockquote>`;
    case "bulletList":
      return `<ul>${children}</ul>`;
    case "orderedList":
      return `<ol>${children}</ol>`;
    case "listItem":
      return `<li>${children}</li>`;
    case "heading": {
      const lvl = Math.min(3, Math.max(1, Number((node.attrs as { level?: number })?.level ?? 2)));
      return `<h${lvl}>${children}</h${lvl}>`;
    }
    default:
      return children;
  }
}

export function renderSafeMessageHtml(rawJson: string, fallbackText: string): string {
  const doc = extractMessageDocJson(rawJson);
  if (doc) {
    const rendered = renderSafeNode(doc);
    if (rendered.trim()) return rendered;
  }
  const text = extractMessageFallbackText(rawJson, fallbackText);
  if (!text) return "";
  return text
    .split(/\r?\n/)
    .map((line) => `<p>${escapeHtml(line)}</p>`)
    .join("");
}
