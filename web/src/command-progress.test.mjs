import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { commandProgress } from "./command-progress.mjs";

const commands = [
  { id: "space.join", label: "Join Space" },
  { id: "identity.recovery.restore", label: "Recover My Identity" },
];

test("progress uses the shared semantic command label", () => {
  assert.deepEqual(commandProgress("identity.recovery.restore", commands), {
    buttonLabel: "Recover My Identity…",
    announcement:
      "Recover My Identity is in progress. Voxelle will update this window when it finishes.",
  });
});

test("progress is absent while idle and remains intelligible for an unknown command", () => {
  assert.equal(commandProgress("", commands), null);
  assert.equal(commandProgress("extension.command", commands)?.buttonLabel, "extension.command…");
});

test("the product surface exposes command progress visibly and to assistive technology", () => {
  const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");
  assert.match(source, /container\.setAttribute\("aria-busy", String\(Boolean\(uiState\.busyCommand\)\)\)/);
  assert.doesNotMatch(source, /app\.setAttribute\("aria-busy"/);
  assert.match(source, /banner\.setAttribute\("role", "status"\)/);
  assert.match(source, /banner\.setAttribute\("aria-atomic", "true"\)/);
});
