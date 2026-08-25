import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("successful identity recovery announces its authority result and next capability step", () => {
  assert.match(
    source,
    /Identity recovered on this device\. Authority from previous devices was revoked\. Save a fresh offline recovery kit now\./,
  );
  assert.match(
    source,
    /command === "space\.join"\s*\|\| command === "identity\.recovery\.restore"[\s\S]*?\.recovery-setup-prompt \.command-button/,
  );
  assert.match(
    source,
    /app\.querySelector\("\.recovery-setup-prompt \.command-button"\)\s*\?\? app\.querySelector\("\.message-input"\)/,
  );
});

test("saved recovery capability exposes recency and a renewal action", () => {
  assert.match(source, /\["Last saved", formatRecoveryKitSavedTime\(recovery\.last_exported_ms\)\]/);
  assert.match(source, /save\.textContent = "Save a fresh recovery kit"/);
  assert.match(source, /Number\.isFinite\(value\)/);
  assert.match(source, /Number\.isNaN\(date\.getTime\(\)\)/);
});
