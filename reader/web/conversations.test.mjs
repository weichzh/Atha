import assert from "node:assert/strict";

import { isSnapshotCssSafe, parseSnapshotPresentation } from "./conversations.mjs";

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
