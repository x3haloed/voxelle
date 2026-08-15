import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const productSource = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");
const shellSource = readFileSync(new URL("./shell-client.js", import.meta.url), "utf8");

test("damaged-home archival review owns keyboard focus and remains cancellable", () => {
  assert.match(productSource, /uiState\.preparingHomeRecovery[\s\S]*?trapModalTab\(event, confirmation\)/);
  assert.match(productSource, /event\.key === "Escape" && uiState\.preparingHomeRecovery/);
  assert.match(productSource, /cancelDamagedHomeRecoveryPreparation\(\)/);
  assert.match(productSource, /confirmation\.setAttribute\("aria-modal", "true"\)/);
  assert.match(productSource, /confirmation\.setAttribute\("aria-labelledby", "home-recovery-confirmation-title"\)/);
  assert.match(productSource, /rememberFocusReturn\(\)[\s\S]*?uiState\.preparingHomeRecovery = true/);
  assert.match(productSource, /surface = uiState\.preparingHomeRecovery[\s\S]*?"home-recovery"/);
  assert.match(
    productSource,
    /cancelDamagedHomeRecoveryPreparation\(\)[\s\S]*?\.damaged-home-panel > \.command-button/,
  );
});

test("damaged preview exposes recovery UX without changing a real home", () => {
  assert.match(shellSource, /preview === "damaged"/);
  assert.match(shellSource, /snapshot\.home = null/);
  assert.match(shellSource, /bounded preview: encrypted identity unavailable/);
  assert.doesNotMatch(shellSource, /PreviewShellClient[\s\S]*?home\.archiveForRecovery[\s\S]*?return this\.current/);
});
