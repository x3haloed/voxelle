import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("product update surfaces share snapshot-derived command availability", () => {
  assert.match(source, /availabilityButton\("product\.update\.check", snapshot\)/);
  assert.match(source, /paletteAvailability\("product\.update\.install", snapshot\)/);
  assert.match(source, /paletteAvailability\("product\.update\.rotateTrust", snapshot\)/);
  assert.match(source, /updateAuthenticationAvailable: snapshot\.product_generation\.update_authentication_available/);
  assert.match(source, /hasAvailableUpdate: Boolean/);
  assert.match(source, /hasStagedUpdate: Boolean/);
  assert.match(source, /hasPreviousGeneration: snapshot\.product_generation\.previous_available/);
});
