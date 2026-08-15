import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("the admitted selected channel is exposed as the current location", () => {
  assert.match(source, /if \(channel\.selected\) row\.setAttribute\("aria-current", "page"\)/);
  assert.match(source, /row\.dataset\.renderKey = `channel:\$\{channel\.room_id\}`/);
});

test("channel selection moves focus from its removed button to the selected row", () => {
  assert.match(
    source,
    /case "channel\.select":\s*currentSnapshot = await shell\.execute\(command, payload\);\s*focusChannelRow\(payload\.room_id\);/,
  );
  assert.match(source, /row\.dataset\.renderKey === `channel:\$\{roomId\}`/);
});
