import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("blocking reviews own structured command failures", () => {
  assert.match(source, /const modalOwnsError = blockingModalOwnsError\(\)/);
  assert.match(source, /modalOwnsError \? \[\] : \[globalErrorBanner\(\)\]/);
  assert.match(source, /function blockingModalOwnsError\(\)[\s\S]*?activeConsequentialReview\(\)/);
  assert.match(source, /function consequentialAlertDialog[\s\S]*?dialog\.append\(globalErrorBanner\(\)\)/);
  assert.match(source, /home-recovery-confirmation-title[\s\S]*?title,\s*globalErrorBanner\(\)/);
  assert.match(source, /customization-reset-title[\s\S]*?title,\s*globalErrorBanner\(\)/);
  assert.match(source, /product-confirmation-title[\s\S]*?title,\s*globalErrorBanner\(\)/);
  assert.match(
    source,
    /const focusModalDismissal = blockingModalOwnsError\(\)[\s\S]*?data-dismiss-notice=\\?"error\\?"/,
  );
});

test("dismissing a modal failure keeps focus within its active review", () => {
  for (const selector of [
    ".home-recovery-confirmation",
    ".invite-revoke-confirmation",
    ".channel-key-confirmation",
    ".member-ban-confirmation",
    ".role-assignment-confirmation",
    ".message-delete-confirmation",
    ".attachment-review",
  ]) {
    assert.match(source, new RegExp(selector.replaceAll(".", "\\.")));
  }
  assert.match(source, /activeSurface\.contains\(returnElement\)/);
});
