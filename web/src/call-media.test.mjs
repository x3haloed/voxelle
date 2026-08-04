import assert from "node:assert/strict";
import test from "node:test";

import {
  captureCallMedia,
  consumeRetainedSignal,
  disconnectedParticipantIds,
  isCameraUnavailable,
  leaveCall,
} from "./call-media.mjs";

test("camera capture requests audio and video together", async () => {
  const stream = { id: "camera" };
  const calls = [];
  const result = await captureCallMedia({
    async getUserMedia(constraints) {
      calls.push(constraints);
      return stream;
    },
  }, true);

  assert.deepEqual(calls, [{ audio: true, video: {} }]);
  assert.deepEqual(result, { stream, video: true, notice: null });
});

test("missing camera degrades explicitly to voice", async () => {
  const stream = { id: "microphone" };
  const calls = [];
  const result = await captureCallMedia({
    async getUserMedia(constraints) {
      calls.push(constraints);
      if (calls.length === 1) {
        const error = new Error("Invalid constraint");
        error.name = "OverconstrainedError";
        throw error;
      }
      return stream;
    },
  }, true);

  assert.deepEqual(calls, [
    { audio: true, video: {} },
    { audio: true, video: false },
  ]);
  assert.deepEqual(result, {
    stream,
    video: false,
    notice: "Camera unavailable; joined with voice only.",
  });
});

test("permission denial is not mislabeled as missing hardware", async () => {
  const denied = new Error("Permission denied");
  denied.name = "NotAllowedError";
  await assert.rejects(
    captureCallMedia({ getUserMedia: async () => { throw denied; } }, true),
    denied,
  );
  assert.equal(isCameraUnavailable(denied), false);
});

test("local media stops even when durable leave fails", async () => {
  let stopped = false;
  await assert.rejects(
    leaveCall(async () => { throw new Error("store unavailable"); }, () => { stopped = true; }),
    /store unavailable/,
  );
  assert.equal(stopped, true);
});

test("a malformed retained signal is consumed exactly once", async () => {
  const seen = new Set();
  const signal = { event_id: "bad-signal" };
  await assert.rejects(
    consumeRetainedSignal(signal, seen, async () => { throw new SyntaxError("bad JSON"); }),
    /bad JSON/,
  );
  assert.deepEqual([...seen], ["bad-signal"]);
  assert.equal(await consumeRetainedSignal(signal, seen, async () => {}), false);
});

test("connections for expired participants are selected for closure", () => {
  assert.deepEqual(
    disconnectedParticipantIds(["alice", "bob"], ["bob", "carol"]),
    ["carol"],
  );
});
