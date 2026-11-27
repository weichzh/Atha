import assert from "node:assert/strict";
import test from "node:test";
import {
  docHasRichFormatting,
  extractMessageDocJson,
  renderSafeMessageHtml,
} from "../src/lib/data/message-doc.ts";

test("extractMessageDocJson unwraps Rust backend richText envelopes", () => {
  const rustEnvelope = JSON.stringify({
    schema: 1,
    kind: "text",
    text: "加粗示例",
    richText: {
      schema: 1,
      document: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "加粗示例", marks: [{ type: "bold" }] }],
          },
        ],
      },
    },
  });

  const doc = extractMessageDocJson(rustEnvelope);
  assert.ok(doc && doc.type === "doc");
  assert.equal(renderSafeMessageHtml(rustEnvelope, "加粗示例"), "<p><strong>加粗示例</strong></p>");
});

test("renderSafeMessageHtml falls back cleanly on Rust plain content_json and bare Tiptap docs", () => {
  const plainRust = JSON.stringify({ schema: 1, kind: "text", text: "第一行\n第二行" });
  assert.equal(extractMessageDocJson(plainRust), null);
  assert.equal(
    renderSafeMessageHtml(plainRust, "第一行\n第二行"),
    "<p>第一行</p><p>第二行</p>",
  );

  const bareDoc = JSON.stringify({
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "<unsafe> ", marks: [{ type: "link", attrs: { href: "javascript:alert(1)" } }] },
          { type: "text", text: "ok", marks: [{ type: "link", attrs: { href: "https://example.com" } }] },
        ],
      },
    ],
  });
  assert.equal(
    renderSafeMessageHtml(bareDoc, ""),
    '<p>&lt;unsafe&gt; <a href="https://example.com" target="_blank" rel="noopener noreferrer">ok</a></p>',
  );
});

test("docHasRichFormatting detects single plain paragraphs vs rich formatting", () => {
  assert.equal(
    docHasRichFormatting({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "纯文本笔记" }] }],
    }),
    false,
  );
  assert.equal(
    docHasRichFormatting({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "重点", marks: [{ type: "bold" }] }],
        },
      ],
    }),
    true,
  );
  assert.equal(
    docHasRichFormatting({
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "A" }] },
        { type: "paragraph", content: [{ type: "text", text: "B" }] },
      ],
    }),
    true,
  );
});
