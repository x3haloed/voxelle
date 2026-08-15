import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("correctable form errors identify their failing controls", () => {
  assert.match(source, /setUserError\("Enter a channel name\.", "channel-name"\)/);
  assert.match(source, /setUserError\("Enter a role name\.", "role-name"\)/);
  assert.match(source, /setUserError\("Choose at least one permission for this role\.", "role-permissions"\)/);
});

test("invalid controls share an inline description and receive focus", () => {
  assert.match(source, /control\.setAttribute\("aria-invalid", "true"\)/);
  assert.match(source, /control\.setAttribute\("aria-describedby", `validation-\$\{target\}`\)/);
  assert.match(source, /control\.querySelector\("input"\)\?\.focus\(\)/);
});
