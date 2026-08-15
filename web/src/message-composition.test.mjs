import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  disambiguatedChannelLabel,
  disambiguatedMemberLabel,
  disambiguatedRoleLabel,
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

test("duplicate member names receive stable bounded disambiguators", () => {
  const duplicates = [
    { peer_id: "ed25519:aaaaaaaaaaaa1111", display_name: "Alice" },
    { peer_id: "ed25519:bbbbbbbbbbbb2222", display_name: "alice" },
  ];
  assert.equal(disambiguatedMemberLabel(duplicates[0], duplicates), "Alice · member aaaaaaaaaaaa");
  assert.equal(disambiguatedMemberLabel(duplicates[1], duplicates), "alice · member bbbbbbbbbbbb");
  assert.equal(disambiguatedMemberLabel(profiles[0], profiles), "Alice");
  const collidingPrefixes = [
    { peer_id: "ed25519:aaaaaaaaaaaa1-first", display_name: "Sam" },
    { peer_id: "ed25519:aaaaaaaaaaaa2-second", display_name: "SAM" },
  ];
  assert.equal(
    disambiguatedMemberLabel(collidingPrefixes[0], collidingPrefixes),
    "Sam · member aaaaaaaaaaaa1",
  );
  assert.equal(
    disambiguatedMemberLabel(collidingPrefixes[1], collidingPrefixes),
    "SAM · member aaaaaaaaaaaa2",
  );
});

test("duplicate role names receive stable bounded disambiguators", () => {
  const roles = [
    { role_id: "role:moderator-aaa11111", name: "Moderator" },
    { role_id: "role:moderator-bbb22222", name: "moderator" },
  ];
  assert.equal(disambiguatedRoleLabel(roles[0], roles), "Moderator · role aaa11111");
  assert.equal(disambiguatedRoleLabel(roles[1], roles), "moderator · role bbb22222");
  assert.equal(disambiguatedRoleLabel(roles[0], [roles[0]]), "Moderator");
  const collidingSuffixes = [
    { role_id: "role:first-a12345678", name: "Helper" },
    { role_id: "role:second-b12345678", name: "helper" },
  ];
  assert.equal(
    disambiguatedRoleLabel(collidingSuffixes[0], collidingSuffixes),
    "Helper · role a12345678",
  );
  assert.equal(
    disambiguatedRoleLabel(collidingSuffixes[1], collidingSuffixes),
    "helper · role b12345678",
  );
});

test("duplicate channel names receive stable bounded disambiguators", () => {
  const channels = [
    { room_id: "space:channel:general-aaa11111", name: "General" },
    { room_id: "space:channel:general-bbb22222", name: "general" },
  ];
  assert.equal(disambiguatedChannelLabel(channels[0], channels), "General · channel aaa11111");
  assert.equal(disambiguatedChannelLabel(channels[1], channels), "general · channel bbb22222");
  assert.equal(disambiguatedChannelLabel(channels[0], [channels[0]]), "General");
  const collidingSuffixes = [
    { room_id: "space:channel:first-a12345678", name: "Plans" },
    { room_id: "space:channel:second-b12345678", name: "plans" },
  ];
  assert.equal(
    disambiguatedChannelLabel(collidingSuffixes[0], collidingSuffixes),
    "Plans · channel a12345678",
  );
  assert.equal(
    disambiguatedChannelLabel(collidingSuffixes[1], collidingSuffixes),
    "plans · channel b12345678",
  );
});
