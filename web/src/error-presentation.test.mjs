import assert from "node:assert/strict";
import test from "node:test";

import { presentShellError } from "./error-presentation.mjs";

test("structured shell errors keep technical detail out of the human summary", () => {
  const presentation = presentShellError({
    message: "Voxelle could not join with that invite.",
    recovery: "needs_reachability",
    recovery_message: "Check the invite and try its included peers again.",
    detail: "parse /private/path/invite.json: invalid signature",
  });
  assert.equal(presentation.message, "Voxelle could not join with that invite.");
  assert.equal(presentation.recovery, "needs_reachability");
  assert.match(presentation.recoveryMessage, /included peers/);
  assert.match(presentation.detail, /private\/path/);
  assert.doesNotMatch(presentation.message, /private\/path/);
});

test("ordinary JavaScript errors retain useful fallback guidance", () => {
  const presentation = presentShellError(new Error("Preview action unavailable"));
  assert.equal(presentation.message, "Preview action unavailable");
  assert.equal(presentation.recovery, "internal_error");
  assert.equal(presentation.detail, "");
});

test("correctable input remains distinct from internal failures", () => {
  const presentation = presentShellError({
    message: "Enter something to search for.",
    recovery: "needs_input",
    recovery_message: "Type one or more words, then search again.",
    detail: "search query is empty",
  });
  assert.equal(presentation.recovery, "needs_input");
  assert.match(presentation.recoveryMessage, /one or more words/);
  assert.equal(presentation.detail, "search query is empty");
});
