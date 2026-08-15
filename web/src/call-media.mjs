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

export function participantConnectionRecovery(connectionState) {
  if (connectionState === "disconnected") {
    return "Voxelle is trying to restore this direct connection. If it does not recover, leave and rejoin the call.";
  }
  if (connectionState === "failed") {
    return "Leave and rejoin the call to try this direct connection again.";
  }
  return null;
}

export function participantMediaPresentation(video, connectionState) {
  const connected = connectionState === "connected";
  const connectionLabel = participantConnectionLabel(connectionState);
  return {
    mediaLabel: video ? "Camera on" : "Voice only",
    connectionLabel,
    recoveryLabel: participantConnectionRecovery(connectionState),
    placeholderLabel: connected
      ? "Voice only"
      : connectionLabel,
    showVideo: video && connected,
  };
}

export function localMicrophoneEnabled(stream) {
  return Boolean(stream?.getAudioTracks().some((track) => track.enabled));
}

export function toggleLocalMicrophone(stream) {
  const tracks = stream?.getAudioTracks() ?? [];
  if (tracks.length === 0) return { changed: false, enabled: false };
  const enabled = !tracks.some((track) => track.enabled);
  for (const track of tracks) track.enabled = enabled;
  return { changed: true, enabled };
}

export function localCameraEnabled(stream) {
  return Boolean(stream?.getVideoTracks().some((track) => track.enabled));
}

export function setLocalCameraEnabled(stream, enabled) {
  const tracks = stream?.getVideoTracks() ?? [];
  if (tracks.length === 0) return { changed: false, enabled: false };
  for (const track of tracks) track.enabled = enabled;
  return { changed: true, enabled };
}

export function toggleLocalCamera(stream) {
  return setLocalCameraEnabled(stream, !localCameraEnabled(stream));
}

export async function toggleCameraIntent(stream, publish) {
  const camera = toggleLocalCamera(stream);
  if (!camera.changed) return camera;
  try {
    await publish(camera.enabled);
    return camera;
  } catch (error) {
    setLocalCameraEnabled(stream, !camera.enabled);
    throw error;
  }
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
