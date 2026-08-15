import assert from "node:assert/strict";
import test from "node:test";

import {
  connectionHeaderState,
  connectionHealthLabel,
  peerActivityEvidence,
  peerRecordClaimPreview,
  peerTargetKey,
  resolvePeerTarget,
} from "./connection-status.mjs";

function snapshot(state, statuses) {
  return {
    home: { runtime: { state } },
    network_health: { rows: statuses.map((status) => ({ status })) },
  };
}

test("setup and verification checks do not make an online peer look broken", () => {
  assert.deepEqual(
    connectionHeaderState(snapshot("online", ["working", "needs_attention", "needs_attention"])),
    {
      label: "Online",
      help: "Online. 2 setup or verification checks available",
      tone: "working",
    },
  );
});

test("actual broken health rows remain prominent", () => {
  assert.deepEqual(
    connectionHeaderState(snapshot("online", ["needs_attention", "broken", "broken"])),
    {
      label: "Online · 2 problems",
      help: "2 connection problems need attention",
      tone: "attention",
    },
  );
});

test("intentional offline state is not mislabeled as an issue", () => {
  assert.equal(
    connectionHeaderState(snapshot("offline", ["needs_attention", "unknown"])).label,
    "Offline",
  );
});

test("operator checks and actual failures use different human labels", () => {
  assert.equal(connectionHealthLabel("needs_attention"), "check");
  assert.equal(connectionHealthLabel("broken"), "broken");
});

test("manual peer checks retain the exact principal and device target", () => {
  const peers = [
    { peer_id: "principal:b", device_id: "device:b1" },
    { peer_id: "principal:b", device_id: "device:b2" },
  ];
  const requested = peerTargetKey(peers[1]);

  assert.equal(resolvePeerTarget(peers, requested), peers[1]);
  assert.equal(resolvePeerTarget(peers, "missing"), peers[0]);
  assert.equal(resolvePeerTarget([], requested), null);
});

test("field evidence is attributed to the selected peer rather than a label prefix", () => {
  const activity = [
    { summary: "diagnostic reached Carol laptop" },
    { summary: "synced Carol laptop: governance accepted 1, room accepted 2" },
  ];

  assert.deepEqual(peerActivityEvidence(activity, { label: "Carol" }), {
    diagnosticReached: false,
    synchronized: false,
  });
  assert.deepEqual(peerActivityEvidence(activity, { label: "Carol laptop" }), {
    diagnosticReached: true,
    synchronized: true,
  });
});

test("peer availability claims are bounded and remain untrusted preview data", () => {
  const record = JSON.stringify({
    v: 1,
    label: "Carol",
    space_id: "s:friends",
    governance_room_id: "s:friends:governance",
    default_room: "s:friends:channel:general",
    authority_peer_id: "p:alice",
    endpoint: {
      v: 1,
      addr: "[fd00::23]:49154",
      peer_id: "p:carol",
      device_id: "d:carol-laptop",
      quic_cert_der_b64: "certificate",
      quic_cert_fingerprint: "sha256:fingerprint",
    },
  });

  assert.deepEqual(peerRecordClaimPreview(record), {
    state: "claims",
    version: 1,
    label: "Carol",
    spaceId: "s:friends",
    defaultRoom: "s:friends:channel:general",
    address: "[fd00::23]:49154",
    peerId: "p:carol",
    deviceId: "d:carol-laptop",
    recognized: true,
  });
  assert.equal(peerRecordClaimPreview("{").state, "unavailable");
  assert.equal(peerRecordClaimPreview("x".repeat(128 * 1024 + 1)).state, "unavailable");
});
