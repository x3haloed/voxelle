import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(new URL("./styles.css", import.meta.url), "utf8");

test("compact workbench overrides the higher-specificity two-column layout", () => {
  const compact = css.slice(css.indexOf("@media (max-width: 760px)"));
  assert.match(compact, /\.workbench,\s*\.workbench\.without-inspector\s*\{/);
  assert.match(compact, /grid-template-columns:\s*minmax\(0, 1fr\)/);
});

test("compact header actions use two bounded columns instead of one row per action", () => {
  const compact = css.slice(css.indexOf("@media (max-width: 760px)"));
  const actionsRule = compact.match(/\.header-actions \{[\s\S]*?\n  \}/)?.[0] ?? "";
  const childRule = compact.match(/\.header-actions > \* \{[\s\S]*?\n  \}/)?.[0] ?? "";
  assert.match(actionsRule, /grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/);
  assert.match(childRule, /min-width:\s*0/);
  assert.match(childRule, /width:\s*100%/);
});

test("compact composer controls use bounded columns", () => {
  const compact = css.slice(css.indexOf("@media (max-width: 760px)"));
  const controlsRule = compact.match(/\.composer-controls \{[\s\S]*?\n  \}/)?.[0] ?? "";
  assert.match(controlsRule, /grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/);
});

test("right-anchored dialogs size against scrollbar-safe containing width", () => {
  const connectionRule = css.match(/\.connection-center \{[\s\S]*?\n\}/)?.[0] ?? "";
  const utilityRule = css.match(/\.utility-center \{[\s\S]*?\n\}/)?.[0] ?? "";
  assert.match(connectionRule, /calc\(100% - 2 \* var\(--panel-gap\)\)/);
  assert.match(utilityRule, /calc\(100% - 2 \* var\(--panel-gap\)\)/);
  assert.doesNotMatch(`${connectionRule}${utilityRule}`, /100vw/);
});

test("compact transient panels become viewport-contained modal surfaces", () => {
  const compact = css.slice(css.indexOf("@media (max-width: 760px)"));
  const panelRule = compact.match(/\.connection-center \{[\s\S]*?\n  \}/)?.[0] ?? "";
  assert.match(panelRule, /inset:\s*var\(--panel-gap\)/);
  assert.match(panelRule, /width:\s*auto/);
  assert.match(panelRule, /max-height:\s*none/);
});
