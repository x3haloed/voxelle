import assert from "node:assert/strict";
import test from "node:test";

import { optionalTextDraftError, searchDraftError, shortTextDraftError } from "./form-draft.mjs";

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
  assert.equal(
    shortTextDraftError("f".repeat(256), { ...options, fieldName: "File name", maxCharacters: 255 }),
    "File name must be 255 characters or fewer.",
  );
});

test("optional human text exposes bounded local corrections", () => {
  assert.equal(optionalTextDraftError("", { fieldName: "About", maxCharacters: 512 }), "");
  assert.equal(
    optionalTextDraftError("hello\n", { fieldName: "About", maxCharacters: 512 }),
    "About cannot contain control characters.",
  );
  assert.equal(
    optionalTextDraftError("😀".repeat(513), { fieldName: "About", maxCharacters: 512 }),
    "About must be 512 characters or fewer.",
  );
});

test("retained search exposes Rust-shaped local corrections", () => {
  assert.equal(searchDraftError("   "), "Enter one or more words to search for.");
  assert.equal(
    searchDraftError("hello\nworld"),
    "Search terms cannot contain control characters.",
  );
  assert.equal(
    searchDraftError("😀".repeat(1025)),
    "Search terms must be 1,024 characters or fewer.",
  );
  assert.equal(searchDraftError("  hello world  "), "");
});
