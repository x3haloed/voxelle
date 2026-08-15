import assert from "node:assert/strict";
import test from "node:test";

import { shortTextDraftError } from "./form-draft.mjs";

const options = {
  fieldName: "Display name",
  emptyMessage: "Enter a display name.",
};

test("short human names expose Rust-shaped local corrections", () => {
  assert.equal(shortTextDraftError("", options), "Enter a display name.");
  assert.equal(
    shortTextDraftError(" Alice", options),
    "Display name cannot start or end with spaces.",
  );
  assert.equal(
    shortTextDraftError("Alice\u0000", options),
    "Display name cannot contain control characters.",
  );
  assert.equal(
    shortTextDraftError("a".repeat(81), options),
    "Display name must be 80 characters or fewer.",
  );
  assert.equal(shortTextDraftError("😀".repeat(80), options), "");
  assert.equal(shortTextDraftError("Alice", options), "");
});
