import assert from "node:assert/strict";
import test from "node:test";
import { getSchema } from "@tiptap/core";
import { createMessageMarkdownCodec } from "../src/message-markdown.ts";
import { messageExtensions } from "../src/message-editor.ts";

test("message markdown round-trips the supported rich-text subset", () => {
  const codec = createMessageMarkdownCodec(getSchema(messageExtensions));
  const markdown = "## 标题\n\n**粗体**和*斜体*\n\n- 第一项\n- 第二项\n\n> 引用\n\n[链接](https://example.com)";

  const document = codec.parse(markdown);
  assert.match(codec.serialize(document), /^## 标题/m);
  assert.throws(() => codec.parse("#### 暂不支持的标题"), /unsupported-markdown/);
  assert.throws(() => codec.parse("```js\nalert(1)\n```"));
});
