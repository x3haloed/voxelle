import test from "node:test";
import assert from "node:assert/strict";
import { coordinationFrontierStatements } from "./coordination-frontier.mjs";

const base = {
  relevance: ["mention_without_local_disposition", "reply_after_local_disposition"],
  acknowledgements: [],
  continuations: [],
};

test("frontier language reports facts without inventing task authority", () => {
  const statements = coordinationFrontierStatements(base, "p:me", () => "Morgan", () => "3:42 PM");
  assert.deepEqual(statements, [
    "Mentioned you; no response is recorded.",
    "A newer reply may change what happens next.",
  ]);
  assert.doesNotMatch(statements.join(" "), /assigned|working|abandoned|completed|stalled/i);
});

test("continuation and handling remain separate literal evidence", () => {
  const statements = coordinationFrontierStatements({
    relevance: ["continuation_active", "handled"],
    acknowledgements: [{
      peer_id: "p:morgan",
      state: "handled",
      result_event_ids: ["e:result"],
      result_conflict: false,
    }],
    continuations: [{
      peer_id: "p:me",
      state: "continuing",
      expires_ms: 42,
      overdue: false,
    }],
  }, "p:me", () => "Morgan", () => "3:42 PM");
  assert.deepEqual(statements, [
    "Morgan marked this handled.",
    "Morgan linked a result.",
    "You said you’re continuing until 3:42 PM.",
  ]);
});
