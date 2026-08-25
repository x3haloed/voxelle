import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("preference forms save only changed drafts", () => {
  assert.match(source, /function preferenceForm\(preference, input, request, isChanged, readDraft, showId\)/);
  assert.match(source, /if \(!isChanged\(\)\) return;/);
  assert.match(source, /uiState\.preferenceDrafts\.set\(preference\.id, readDraft\(\)\)/);
  assert.match(source, /uiState\.preferenceDrafts\.delete\(payload\.id\)/);
  assert.match(source, /save\.disabled = uiState\.busyCommand !== "" \|\| !changed/);
  assert.match(source, /has unsaved changes/);
});

test("customization and layout resets require distinct focused cancellable reviews", () => {
  assert.match(source, /role", "alertdialog"/);
  assert.match(source, /Reset all customization\?/);
  assert.match(source, /Reset workbench layout\?/);
  assert.match(source, /Reset appearance, behavior, and layout/);
  assert.match(source, /Reset dock placement and visibility/);
  assert.match(source, /Keep my customization/);
  assert.match(source, /Keep my layout/);
  assert.match(source, /command === "ui\.preferences\.reset" \|\| command === "workbench\.layout\.reset"/);
  assert.match(source, /uiState\.utilityOpen = "settings"/);
  assert.match(source, /uiState\.layoutEditing = true/);
  assert.match(source, /app\.querySelector\("\.header-more > summary"\)/);
  assert.match(source, /trapModalTab\(event, confirmation\)/);
  assert.match(source, /cancelCustomizationResetConfirmation\(\)/);
});
