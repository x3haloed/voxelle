export async function captureCallMedia(mediaDevices, videoRequested) {
  if (!mediaDevices?.getUserMedia) {
    throw new Error("This WebView does not provide media capture");
  }
  if (!videoRequested) {
    return {
      stream: await mediaDevices.getUserMedia({ audio: true, video: false }),
      video: false,
      notice: null,
    };
  }
  try {
    return {
      stream: await mediaDevices.getUserMedia({ audio: true, video: {} }),
      video: true,
      notice: null,
    };
  } catch (error) {
    if (!isCameraUnavailable(error)) throw error;
    return {
      stream: await mediaDevices.getUserMedia({ audio: true, video: false }),
      video: false,
      notice: "Camera unavailable; joined with voice only.",
    };
  }
}

export function isCameraUnavailable(error) {
  const name = error instanceof Error ? error.name : "";
  const message = error instanceof Error ? error.message : String(error);
  return name === "NotFoundError"
    || name === "OverconstrainedError"
    || /invalid constraint/i.test(message);
}

export function mediaCaptureErrorMessage(error, videoRequested) {
  const name = error instanceof Error ? error.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") {
    return `Voxelle could not use your ${videoRequested ? "camera and microphone" : "microphone"}. Allow access in system settings, then try again.`;
  }
  if (name === "NotFoundError" || name === "OverconstrainedError") {
    return videoRequested
      ? "No usable microphone was found after the camera fallback. Connect or enable a microphone, then try again."
      : "No usable microphone was found. Connect or enable one, then try again.";
  }
  if (name === "NotReadableError" || name === "AbortError") {
    return `Your ${videoRequested ? "camera or microphone is" : "microphone is"} unavailable or already in use. Close other media apps, then try again.`;
  }
  return `Voxelle could not start ${videoRequested ? "video" : "voice"}. Check your media devices, then try again.`;
}

export function participantConnectionLabel(connectionState) {
  if (connectionState === "connected") return "Connected directly";
  if (connectionState === "failed" || connectionState === "disconnected") {
    return "Direct connection unavailable";
  }
  return "Connecting directly";
}

export async function leaveCall(executeLeave, stopMedia) {
  try {
    return await executeLeave();
  } finally {
    stopMedia();
  }
}

export async function consumeRetainedSignal(signal, seenEventIds, apply) {
  if (seenEventIds.has(signal.event_id)) return false;
  try {
    await apply(signal);
  } finally {
    seenEventIds.add(signal.event_id);
  }
  return true;
}

export function disconnectedParticipantIds(activeParticipantIds, connectedPeerIds) {
  const active = new Set(activeParticipantIds);
  return [...connectedPeerIds].filter((peerId) => !active.has(peerId));
}
