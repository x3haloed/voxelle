import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("compact utility and connection panels expose matching modal behavior", () => {
  assert.match(source, /aside\.setAttribute\("aria-modal", String\(compactTransientModal\(\)\)\)/);
  assert.match(source, /event\.key === "Tab"\s*&& compactTransientModal\(\)/);
  assert.match(source, /trapModalTab\(event, transient\)/);
  assert.match(source, /addEventListener\?\.\("change", handleCompactTransientChange\)/);
  assert.match(source, /removeEventListener\?\.\("change", handleCompactTransientChange\)/);
});
