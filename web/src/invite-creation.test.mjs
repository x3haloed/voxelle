import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("invite creation reviews the selected authority window before signing", () => {
  assert.match(source, /expiryReview\.setAttribute\("aria-live", "polite"\)/);
  assert.match(source, /This signed bearer capability can authorize joins for/);
  assert.match(source, /after creation unless members revoke it/);
  assert.match(source, /create\.textContent = "Create signed invite"/);
});

test("ordinary conversation view exposes invitation without guessing through People", () => {
  assert.match(source, /actionButton\("Invite someone", openInviteUtility\)/);
  assert.match(source, /openPeopleUtility\("\.invite-flow \.command-button"\)/);
  assert.match(source, /querySelector\("\.invite-flow"\)\?\.scrollIntoView\(\{ block: "start" \}\)/);
});
