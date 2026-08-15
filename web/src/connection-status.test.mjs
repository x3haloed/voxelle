import assert from "node:assert/strict";
import test from "node:test";

import {
  connectionHeaderState,
  connectionHealthLabel,
  peerActivityEvidence,
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
