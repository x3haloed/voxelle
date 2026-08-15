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
