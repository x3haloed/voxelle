import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  captureCallMedia,
  consumeRetainedSignal,
  disconnectedParticipantIds,
  isCameraUnavailable,
  leaveCall,
  localCameraEnabled,
  localMicrophoneEnabled,
  mediaCaptureErrorMessage,
  participantConnectionLabel,
  participantMediaPresentation,
  setLocalCameraEnabled,
  toggleCameraIntent,
  toggleLocalCamera,
  toggleLocalMicrophone,
} from "./call-media.mjs";

const productSource = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

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
  assert.equal(
    mediaCaptureErrorMessage(denied, true),
    "Voxelle could not use your camera and microphone. Allow access in system settings, then try again.",
  );
});

test("capture failures give a local recovery action without leaking technical errors", () => {
  const busy = new Error("Could not start video source");
  busy.name = "NotReadableError";
  assert.equal(
    mediaCaptureErrorMessage(busy, false),
    "Your microphone is unavailable or already in use. Close other media apps, then try again.",
  );
  assert.equal(
    mediaCaptureErrorMessage(new Error("opaque platform failure"), true),
    "Voxelle could not start video. Check your media devices, then try again.",
  );
});

test("participant connection states remain direct and human readable", () => {
  assert.equal(participantConnectionLabel("new"), "Connecting directly");
  assert.equal(participantConnectionLabel("connected"), "Connected directly");
  assert.equal(participantConnectionLabel("failed"), "Direct connection unavailable");
});

test("remote voice-only participation never renders as an empty video", () => {
  assert.deepEqual(participantMediaPresentation(false, "connected"), {
    mediaLabel: "Voice only",
    connectionLabel: "Connected directly",
    showVideo: false,
  });
  assert.deepEqual(participantMediaPresentation(true, "new"), {
    mediaLabel: "Camera on",
    connectionLabel: "Connecting directly",
    showVideo: true,
  });
});

test("microphone toggling controls every local audio track and reports missing capture", () => {
  const audioTracks = [{ enabled: true }, { enabled: true }];
  const stream = { getAudioTracks: () => audioTracks };

  assert.equal(localMicrophoneEnabled(stream), true);
  assert.deepEqual(toggleLocalMicrophone(stream), { changed: true, enabled: false });
  assert.deepEqual(audioTracks.map((track) => track.enabled), [false, false]);
  assert.equal(localMicrophoneEnabled(stream), false);
  assert.deepEqual(toggleLocalMicrophone(stream), { changed: true, enabled: true });
  assert.deepEqual(toggleLocalMicrophone({ getAudioTracks: () => [] }), {
    changed: false,
    enabled: false,
  });
});

test("camera toggling controls every captured video track and reports missing capture", () => {
  const videoTracks = [{ enabled: true }, { enabled: true }];
  const stream = { getVideoTracks: () => videoTracks };

  assert.equal(localCameraEnabled(stream), true);
  assert.deepEqual(toggleLocalCamera(stream), { changed: true, enabled: false });
  assert.deepEqual(videoTracks.map((track) => track.enabled), [false, false]);
  assert.deepEqual(setLocalCameraEnabled(stream, true), { changed: true, enabled: true });
  assert.equal(localCameraEnabled(stream), true);
  assert.deepEqual(toggleLocalCamera({ getVideoTracks: () => [] }), {
    changed: false,
    enabled: false,
  });
});

test("camera intent rolls local tracks back when the admitted update fails", async () => {
  const videoTracks = [{ enabled: true }];
  const stream = { getVideoTracks: () => videoTracks };
  const published = [];

  assert.deepEqual(await toggleCameraIntent(stream, async (enabled) => published.push(enabled)), {
    changed: true,
    enabled: false,
  });
  assert.deepEqual(published, [false]);
  assert.equal(videoTracks[0].enabled, false);

  await assert.rejects(
    toggleCameraIntent(stream, async () => { throw new Error("admission failed"); }),
    /admission failed/,
  );
  assert.equal(videoTracks[0].enabled, false);
});

test("the active call and palette share the microphone semantic command", () => {
  assert.match(productSource, /commandButton\("call\.microphone\.toggle"\)/);
  assert.match(productSource, /case "call\.microphone\.toggle":/);
  assert.match(productSource, /No microphone track is available\. Leave and rejoin/);
});

test("the active call and palette share the camera semantic command", () => {
  assert.match(productSource, /commandButton\("call\.camera\.toggle"\)/);
  assert.match(productSource, /case "call\.camera\.toggle":/);
  assert.match(productSource, /shell\.execute\("call\.media"/);
  assert.match(productSource, /No camera track is available\. Leave and rejoin with camera/);
});

test("a full direct call disables both visible join choices", () => {
  assert.match(productSource, /const callFull = !joined && \(call\?\.participants\.length \?\? 0\) >= 4/);
  assert.match(productSource, /joinWithMicrophone\.disabled = callFull/);
  assert.match(productSource, /joinWithCamera\.disabled = callFull/);
  assert.match(productSource, /This direct call is full \(4 of 4\)\. Wait for someone to leave/);
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
