import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("correctable form errors identify their failing controls", () => {
  assert.match(source, /setUserError\(nameError, "profile-name"\)/);
  assert.match(source, /setUserError\(nameError, "channel-name"\)/);
  assert.match(source, /setUserError\(nameError, "role-name"\)/);
  assert.match(source, /setUserError\(aboutError, "profile-about"\)/);
  assert.match(source, /setUserError\(topicError, "channel-topic"\)/);
  assert.match(source, /setUserError\("Choose at least one permission for this role\.", "role-permissions"\)/);
});

test("invalid controls share an inline description and receive focus", () => {
  assert.match(source, /control\.setAttribute\("aria-invalid", "true"\)/);
  assert.match(source, /control\.setAttribute\("aria-describedby", `validation-\$\{target\}`\)/);
  assert.match(source, /control\.querySelector\("input"\)\?\.focus\(\)/);
});

test("editing the failing control clears only its stale presentation error", () => {
  assert.match(source, /clearCorrectedValidation\(validationTarget\)/);
  assert.match(source, /uiState\.validationTarget !== target/);
  assert.match(source, /clearValidation\(\);\n  const presentation = presentShellError/);
});

test("retained search cannot submit an empty local query", () => {
  assert.match(source, /search\.disabled = Boolean\(uiState\.busyCommand\) \|\| !uiState\.searchDraft\.trim\(\)/);
  assert.match(source, /if \(!uiState\.searchDraft\.trim\(\)\) return;\s*runCommand\("message\.search"/);
});
