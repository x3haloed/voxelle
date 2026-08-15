import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  insertMentionText,
  MESSAGE_MAX_CHARACTERS,
  mentionedPeerIds,
  messageDraftCanSend,
  messageDraftGuidance,
  unicodeCharacterCount,
} from "./message-composition.mjs";

const profiles = [
  { peer_id: "peer:alice", display_name: "Alice" },
  { peer_id: "peer:bob", display_name: "Bob Stone" },
];
const productSource = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("message submission requires visible content", () => {
  assert.equal(messageDraftCanSend(""), false);
  assert.equal(messageDraftCanSend("  \n\t"), false);
  assert.equal(messageDraftCanSend("hello"), true);
  assert.equal(messageDraftCanSend("  hello  "), false);
  assert.equal(messageDraftCanSend("hello\n"), false);
  assert.equal(messageDraftCanSend("hello\0"), false);
  assert.equal(messageDraftCanSend("a".repeat(MESSAGE_MAX_CHARACTERS + 1)), false);
  assert.equal(messageDraftCanSend("😀".repeat(MESSAGE_MAX_CHARACTERS)), true);
  assert.equal(unicodeCharacterCount("😀😀"), 2);
  assert.equal(
    messageDraftGuidance(" hello"),
    "Remove spaces or blank lines at the beginning or end.",
  );
  assert.equal(
    messageDraftGuidance("a".repeat(MESSAGE_MAX_CHARACTERS + 1)),
    "Shorten this message to 4,000 characters or fewer.",
  );
});

test("composer and inline editor share the visible-content predicate", () => {
  assert.match(productSource, /send\.disabled = Boolean\(uiState\.busyCommand\) \|\| !messageDraftCanSend\(input\.value\)/);
  assert.match(productSource, /save\.disabled = Boolean\(uiState\.busyCommand\) \|\| !messageDraftCanSend\(input\.value\)/);
});

test("visible names resolve to stable peer IDs without prefix false positives", () => {
  assert.deepEqual(
    mentionedPeerIds("Thanks @Alice, please ask @Bob Stone!", profiles),
    ["peer:alice", "peer:bob"],
  );
  assert.deepEqual(mentionedPeerIds("@Aliceville is elsewhere", profiles), []);
  assert.deepEqual(mentionedPeerIds("@alice and @ALICE", profiles), ["peer:alice"]);
});

test("duplicate visible names require an explicit picker selection", () => {
  const duplicates = [
    { peer_id: "peer:alice-1", display_name: "Alice" },
    { peer_id: "peer:alice-2", display_name: "Alice" },
  ];
  assert.deepEqual(mentionedPeerIds("Hello @Alice", duplicates), []);
  assert.deepEqual(
    mentionedPeerIds("Hello @Alice", duplicates, ["peer:alice-2"]),
    ["peer:alice-2"],
  );
});

test("mention insertion preserves surrounding text and returns the next caret", () => {
  assert.deepEqual(insertMentionText("hello world", 6, 11, "Bob Stone"), {
    text: "hello @Bob Stone ",
    caret: 17,
  });
  assert.deepEqual(insertMentionText("hello", 5, 5, "Alice"), {
    text: "hello @Alice ",
    caret: 13,
  });
});
