import assert from "node:assert/strict";

import {
  formatMessageTime,
  isSnapshotCssSafe,
  linkedMessagePreviews,
  parseSnapshotPresentation,
} from "./conversations.mjs";

const captured = parseSnapshotPresentation(
  '{"schema":1,"theme":"paper","brightness":90,"fontSize":40,"fontFamily":"serif","density":"comfortable"}',
);
assert.deepEqual(captured, {
  theme: "paper",
  brightness: 90,
  fontSize: 40,
  fontFamily: "serif",
  lineHeightPx: 72,
});
assert.equal(
  parseSnapshotPresentation(
    '{"schema":1,"theme":"system","brightness":100,"fontSize":32,"fontFamily":"book","density":"standard"}',
    true,
  ).theme,
  "dark",
);
assert.throws(() => parseSnapshotPresentation('{"schema":1,"theme":"dark"}'));
assert.equal(isSnapshotCssSafe("p { color: #222; }"), true);
for (const css of [
  "p { background: url(a.png); }",
  "p { background: src('a.png'); }",
  "p { background: image('a.png'); }",
  "p { background: image-set('a.png' 1x); }",
  String.raw`p { background: u\72l('a.png'); }`,
]) {
  assert.equal(isSnapshotCssSafe(css), false);
}

const rootMessage = {
  id: "root",
  text: "根消息",
  source: null,
  deleted: false,
  referencePreviews: [{ id: "nested", text: "不应展开的间接引用", deleted: false }],
};
const replyMessage = {
  id: "reply",
  text: "回复正文",
  source: null,
  deleted: false,
  replyToMessageId: "root",
  referencePreviews: [{ id: "external", text: "外部引用", deleted: false }],
};
assert.deepEqual(linkedMessagePreviews(replyMessage, [rootMessage, replyMessage]), [
  { id: "root", text: "根消息", local: true },
  { id: "external", text: "外部引用", local: false },
]);
assert.equal(formatMessageTime(new Date(2026, 0, 1, 14, 5).valueOf()), "14:05");
