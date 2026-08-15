import test from "node:test";
import assert from "node:assert/strict";
import { inviteClaimPreview } from "./invite-preview.mjs";

function invite(overrides = {}) {
  return JSON.stringify({
    v: 1,
    space: {
      space_id: "s:space",
      name: "Friends",
      authority_peer_id: "p:authority",
      governance_room_id: "s:space:governance",
    },
    invite_event: {
      room_id: "s:space:governance",
      author_peer_id: "p:authority",
      body: {
        space_id: "s:space",
        expires_ms: 2_000,
        bootstrap_peers: [{}, {}],
      },
    },
    ...overrides,
  });
}

test("invite preview exposes bounded untrusted claims", () => {
  assert.deepEqual(inviteClaimPreview(invite(), 1_000), {
    state: "claims",
    spaceName: "Friends",
    spaceId: "s:space",
    authorityPeerId: "p:authority",
    expiresMs: 2_000,
    expiredClaim: false,
    bootstrapCount: 2,
    claimsConsistent: true,
  });
});

test("expired and conflicting claims are visible before Rust validation", () => {
  const value = JSON.parse(invite());
  value.invite_event.author_peer_id = "p:other";
  const preview = inviteClaimPreview(JSON.stringify(value), 3_000);
  assert.equal(preview.state, "claims");
  assert.equal(preview.expiredClaim, true);
  assert.equal(preview.claimsConsistent, false);
});

test("partial and oversized text cannot consume the review surface", () => {
  assert.equal(inviteClaimPreview("{").state, "unavailable");
  assert.match(inviteClaimPreview("x".repeat(1024 * 1024 + 1)).reason, /too large/);
});
