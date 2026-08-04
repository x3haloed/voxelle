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
