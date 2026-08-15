/**
 * Keep operator-oriented setup checks available without presenting them as
 * failures in the ordinary conversation header.
 *
 * @param {{home?: {runtime?: {state?: string}} | null, network_health: {rows: Array<{status: string}>}}} snapshot
 */
export function connectionHeaderState(snapshot) {
  const rows = snapshot.network_health?.rows ?? [];
  const brokenCount = rows.filter((row) => row.status === "broken").length;
  const checkCount = rows.filter((row) => row.status === "needs_attention").length;
  const online = snapshot.home?.runtime?.state === "online";
  const stateLabel = online ? "Online" : "Offline";

  if (brokenCount > 0) {
    return {
      label: `${stateLabel} · ${brokenCount} problem${brokenCount === 1 ? "" : "s"}`,
      help: `${brokenCount} connection problem${brokenCount === 1 ? "" : "s"} need attention`,
      tone: "attention",
    };
  }

  return {
    label: stateLabel,
    help: checkCount > 0
      ? `${stateLabel}. ${checkCount} setup or verification check${checkCount === 1 ? "" : "s"} available`
      : "Review connection and synchronization health",
    tone: "working",
  };
}

export function connectionHealthLabel(status) {
  switch (status) {
    case "working":
      return "working";
    case "needs_attention":
      return "check";
    case "broken":
      return "broken";
    case "unknown":
      return "unknown";
  }
}

/**
 * @param {{peer_id: string, device_id: string}} peer
 */
export function peerTargetKey(peer) {
  return JSON.stringify([peer.peer_id, peer.device_id]);
}

/**
 * Keep a deliberate target selected across snapshot refreshes, while falling
 * back to the first currently known peer when the previous record disappears.
 *
 * @param {Array<{peer_id: string, device_id: string}>} peers
 * @param {string} requestedKey
 */
export function resolvePeerTarget(peers, requestedKey) {
  return peers.find((peer) => peerTargetKey(peer) === requestedKey) ?? peers[0] ?? null;
}

/**
 * @param {Array<{summary: string}>} activity
 * @param {{label: string} | null} target
 */
export function peerActivityEvidence(activity, target) {
  if (!target) return { diagnosticReached: false, synchronized: false };
  return {
    diagnosticReached: activity.some((item) => item.summary === `diagnostic reached ${target.label}`),
    synchronized: activity.some((item) => item.summary.startsWith(`synced ${target.label}:`)),
  };
}

const MAX_PEER_RECORD_PREVIEW_BYTES = 128 * 1024;

export function peerRecordClaimPreview(value) {
  if (!value.trim()) return { state: "empty" };
  if (new TextEncoder().encode(value).length > MAX_PEER_RECORD_PREVIEW_BYTES) {
    return {
      state: "unavailable",
      reason: "Peer availability text is too large to review safely.",
    };
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch {
    return {
      state: "unavailable",
      reason: "Peer availability text is not complete JSON.",
    };
  }
  const endpoint = parsed && typeof parsed === "object" ? parsed.endpoint : null;
  if (!endpoint || typeof endpoint !== "object") {
    return {
      state: "unavailable",
      reason: "Peer availability text does not contain an endpoint record.",
    };
  }
  const stringClaim = (candidate) => typeof candidate === "string" ? candidate : "";
  const preview = {
    state: "claims",
    version: Number.isInteger(parsed.v) ? parsed.v : null,
    label: stringClaim(parsed.label) || "Unnamed peer",
    spaceId: stringClaim(parsed.space_id),
    defaultRoom: stringClaim(parsed.default_room),
    address: stringClaim(endpoint.addr),
    peerId: stringClaim(endpoint.peer_id),
    deviceId: stringClaim(endpoint.device_id),
  };
  preview.recognized = preview.version === 1
    && endpoint.v === 1
    && preview.spaceId !== ""
    && stringClaim(parsed.governance_room_id) !== ""
    && preview.defaultRoom !== ""
    && stringClaim(parsed.authority_peer_id) !== ""
    && preview.address !== ""
    && preview.peerId !== ""
    && preview.deviceId !== ""
    && stringClaim(endpoint.quic_cert_der_b64) !== ""
    && stringClaim(endpoint.quic_cert_fingerprint) !== "";
  return preview;
}
