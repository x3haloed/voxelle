import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("duplicate member names keep a bounded stable marker on authority-changing controls", () => {
  assert.match(source, /memberSummary\.setAttribute\("aria-label", `Actions for member \$\{memberLabel\}`\)/);
  assert.match(source, /memberBanConfirmation\(profile, memberLabel\)/);
  assert.match(source, /roleAssignmentConfirmation\(role, profile, memberLabel, draft\.grant\)/);
  assert.match(source, /`Mention \$\{disambiguatedMemberLabel\(profile, currentProfiles\)\}`/);
});
