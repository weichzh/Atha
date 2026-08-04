import assert from "node:assert/strict";

import { parseSnapshotPresentation } from "./conversations.mjs";

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
