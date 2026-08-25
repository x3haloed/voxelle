import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile(new URL("./product-component.js", import.meta.url), "utf8");

test("all product disclosures use the shared accessible summary", () => {
  assert.equal((source.match(/element\("summary"/g) ?? []).length, 1);
  assert.match(source, /function disclosureSummary\(label, className = ""\)/);
  assert.match(source, /summary\.setAttribute\("role", "button"\)/);
  assert.match(source, /summary\.setAttribute\("aria-expanded", String\(details\.open\)\)/);
});

test("Enter and Space preserve disclosure keyboard activation after role override", () => {
  assert.match(source, /if \(event\.key !== "Enter" && event\.key !== " "\) return/);
  assert.match(source, /event\.preventDefault\(\);\s*details\.open = !details\.open/);
});
