import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("notice dismissal preserves validation and semantic command origins", () => {
  assert.match(source, /const validationTarget = kind === "error" \? uiState\.validationTarget : ""/);
  assert.match(source, /rememberNoticeReturn\(commandReturnElement\)/);
  assert.match(source, /focusAfterNoticeDismissal\(returnElement, returnActionKey, validationTarget\)/);
  assert.match(source, /candidate\.dataset\.actionKey === returnActionKey/);
  assert.match(source, /button\.dataset\.actionKey = `action:dismiss-\$\{kind\}`/);
  assert.match(source, /button\.dataset\.dismissNotice = kind/);
  assert.match(source, /event\.detail === 0\s*\? document\.activeElement/);
  assert.match(source, /document\.addEventListener\("click", handleNoticeDismissalClick, true\)/);
  assert.match(source, /event\.stopImmediatePropagation\(\)/);
  assert.match(source, /document\.removeEventListener\("click", handleNoticeDismissalClick, true\)/);
});

test("notice focus stays in active transient surfaces and has stable fallbacks", () => {
  assert.match(source, /activeSurface\.contains\(returnElement\)/);
  assert.match(source, /activeSurface\?\.querySelector\("\[data-dialog-initial-focus='true'\]"\)/);
  assert.match(source, /app\.querySelector\("\.message-input"\)/);
});

test("successful commands without notices do not leave a stale origin", () => {
  assert.match(source, /if \(!commandFailed && !uiState\.status\)/);
  assert.match(source, /uiState\.noticeReturnElement = null/);
});
