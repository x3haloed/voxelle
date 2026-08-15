import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("every inline consequential review uses one modal interaction boundary", () => {
  for (const className of [
    "invite-revoke-confirmation",
    "channel-key-confirmation",
    "member-ban-confirmation",
    "role-assignment-confirmation",
    "message-delete-confirmation",
    "attachment-review",
  ]) {
    assert.match(source, new RegExp(`consequentialAlertDialog\\([\\s\\S]*?"${className}"`));
  }
  assert.match(source, /consequential-review-backdrop/);
  assert.match(source, /dialog\.setAttribute\("aria-modal", "true"\)/);
});

test("active consequential reviews contain focus and cancel with Escape", () => {
  assert.match(source, /const consequentialReview = activeConsequentialReview\(\)/);
  assert.match(source, /trapModalTab\(event, confirmation\)/);
  assert.match(source, /event\.key === "Escape"[\s\S]*?consequentialReview\.cancel\(\)/);
  assert.match(source, /consequential-review:\$\{consequentialReview\.key\}/);
  assert.match(source, /consequentialReview\?\.selector/);
});

test("each consequential review exposes an explicit initial action", () => {
  const initialFocusAssignments = source.match(/\.dataset\.dialogInitialFocus = "true"/g) ?? [];
  assert.ok(initialFocusAssignments.length >= 9);
});
