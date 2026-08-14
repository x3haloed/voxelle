import assert from "node:assert/strict";
import test from "node:test";

import { connectionHeaderState, connectionHealthLabel } from "./connection-status.mjs";

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
