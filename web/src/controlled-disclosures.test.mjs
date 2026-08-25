import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("product-owned disclosure state is explicit at every controlled family", () => {
  for (const pattern of [
    /edit\.dataset\.syncOpen = "true";\s*edit\.open = uiState\.profileEditOpen/,
    /details\.dataset\.syncOpen = "true";\s*details\.open = uiState\.peerImportOpen/,
    /create\.dataset\.syncOpen = "true";\s*create\.open = uiState\.channelCreateOpen/,
    /privacy\.dataset\.syncOpen = "true";\s*privacy\.open = uiState\.channelPrivateDraft/,
    /const assignmentPending = uiState\.roleAssignmentDraft\?\.roleId === role\.role_id;\s*if \(assignmentPending\) members\.dataset\.syncOpen = "true";\s*members\.open = assignmentPending/,
    /create\.dataset\.syncOpen = "true";\s*create\.open = uiState\.roleCreateOpen/,
  ]) {
    assert.match(source, pattern);
  }
});
