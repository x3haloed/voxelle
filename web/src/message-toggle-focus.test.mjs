import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("reaction and pin controls expose target-specific names and stable focus keys", () => {
  assert.match(source, /button\.dataset\.messageFocusKey = `reaction-chip:\$\{reaction\.emoji\}`/);
  assert.match(source, /thumb\.dataset\.messageFocusKey = "reaction-action:thumb"/);
  assert.match(source, /pin\.dataset\.messageFocusKey = "pin-action"/);
  assert.match(source, /reaction on \$\{messageContextLabel\(message, author\.display_name\)\}/);
  assert.match(source, /\$\{message\.pinned \? "Unpin" : "Pin"\} \$\{messageContextLabel/);
});

test("accepted message toggles reacquire the replacement control or message row", () => {
  assert.match(source, /const messageFocusKey = commandReturnElement\?\.dataset\?\.messageFocusKey \?\? ""/);
  assert.match(
    source,
    /case "pin\.remove":\s*currentSnapshot = await shell\.execute\(command, payload\);\s*focusMessageControl\(payload\.target_event_id, messageFocusKey\);/,
  );
  assert.match(source, /candidate\.dataset\.messageFocusKey === focusKey/);
  assert.match(source, /\(replacement \?\? row\)\?\.focus\(\)/);
});
