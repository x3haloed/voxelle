const MAX_PREVIEW_CHARACTERS = 1024 * 1024;

export function inviteClaimPreview(text, nowMs = Date.now()) {
  if (typeof text !== "string" || !text.trim()) return { state: "empty" };
  if (text.length > MAX_PREVIEW_CHARACTERS) {
    return { state: "unavailable", reason: "Invite text is too large to preview safely." };
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    return { state: "unavailable", reason: "Invite text is not complete JSON yet." };
  }
  if (!value || typeof value !== "object") {
    return { state: "unavailable", reason: "Invite JSON does not contain an object." };
  }
  const space = value.space;
  const event = value.invite_event;
  const body = event?.body;
  if (!space || typeof space !== "object" || !event || typeof event !== "object" || !body || typeof body !== "object") {
    return { state: "unavailable", reason: "Invite JSON does not contain recognizable space and invitation claims." };
  }
  const stringClaim = (candidate, fallback) => (
    typeof candidate === "string" && candidate.trim() && candidate.length <= 256
      ? candidate
      : fallback
  );
  const expiresMs = Number.isSafeInteger(body.expires_ms) ? body.expires_ms : null;
  const bootstrapCount = Array.isArray(body.bootstrap_peers) ? body.bootstrap_peers.length : null;
  const spaceId = stringClaim(space.space_id, "Unrecognized space ID");
  const authorityPeerId = stringClaim(space.authority_peer_id, "Unrecognized authority");
  const claimsConsistent = (
    body.space_id === space.space_id
    && event.author_peer_id === space.authority_peer_id
    && event.room_id === space.governance_room_id
  );
  return {
    state: "claims",
    spaceName: stringClaim(space.name, "Unnamed space"),
    spaceId,
    authorityPeerId,
    expiresMs,
    expiredClaim: expiresMs !== null && expiresMs < nowMs,
    bootstrapCount,
    claimsConsistent,
  };
}
