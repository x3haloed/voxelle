import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(new URL("./styles.css", import.meta.url), "utf8");

test("compact workbench overrides the higher-specificity two-column layout", () => {
  const compact = css.slice(css.indexOf("@media (max-width: 760px)"));
  assert.match(compact, /\.workbench,\s*\.workbench\.without-inspector\s*\{/);
  assert.match(compact, /grid-template-columns:\s*minmax\(0, 1fr\)/);
});

test("right-anchored dialogs size against scrollbar-safe containing width", () => {
  const connectionRule = css.match(/\.connection-center \{[\s\S]*?\n\}/)?.[0] ?? "";
  const utilityRule = css.match(/\.utility-center \{[\s\S]*?\n\}/)?.[0] ?? "";
  assert.match(connectionRule, /calc\(100% - 2 \* var\(--panel-gap\)\)/);
  assert.match(utilityRule, /calc\(100% - 2 \* var\(--panel-gap\)\)/);
  assert.doesNotMatch(`${connectionRule}${utilityRule}`, /100vw/);
});
