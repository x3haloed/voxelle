const MENTION_BOUNDARY = /[\s.,!?;:()[\]{}'"-]/;

export const MESSAGE_MAX_CHARACTERS = 4000;

/** @param {string} text */
export function unicodeCharacterCount(text) {
  return [...text].length;
}

/** @param {string} text */
export function messageDraftGuidance(text) {
  if (!text.trim()) return "";
  if (text !== text.trim()) {
    return "Remove spaces or blank lines at the beginning or end.";
  }
  if (text.includes("\0")) return "Remove the unsupported null character.";
  if (unicodeCharacterCount(text) > MESSAGE_MAX_CHARACTERS) {
    return `Shorten this message to ${MESSAGE_MAX_CHARACTERS.toLocaleString()} characters or fewer.`;
  }
  return "";
}

/** @param {string} text */
export function messageDraftCanSend(text) {
  return Boolean(text.trim()) && !messageDraftGuidance(text);
}

/**
 * Resolve visible @names to the stable peer IDs carried by the command.
 * @param {string} text
 * @param {Array<{peer_id: string, display_name: string}>} profiles
 * @param {Iterable<string>} [selectedPeerIds]
 */
export function mentionedPeerIds(text, profiles, selectedPeerIds = []) {
  const lower = text.toLocaleLowerCase();
  const selected = new Set(selectedPeerIds);
  const nameCounts = new Map();
  for (const profile of profiles) {
    const name = profile.display_name.trim().toLocaleLowerCase();
    if (name) nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  }
  const ids = [];
  for (const profile of profiles) {
    const name = profile.display_name.trim();
    if (!name) continue;
    if (!selected.has(profile.peer_id)
        && nameCounts.get(name.toLocaleLowerCase()) !== 1) continue;
    const token = `@${name.toLocaleLowerCase()}`;
    let index = lower.indexOf(token);
    while (index !== -1) {
      const next = lower[index + token.length];
      if (next === undefined || MENTION_BOUNDARY.test(next)) {
        ids.push(profile.peer_id);
        break;
      }
      index = lower.indexOf(token, index + token.length);
    }
  }
  return ids;
}

/**
 * @param {string} text
 * @param {number | null} selectionStart
 * @param {number | null} selectionEnd
 * @param {string} displayName
 */
export function insertMentionText(text, selectionStart, selectionEnd, displayName) {
  const start = selectionStart ?? text.length;
  const end = selectionEnd ?? start;
  const before = text.slice(0, start);
  const after = text.slice(end);
  const leading = before && !/\s$/.test(before) ? " " : "";
  const trailing = after && !/^\s/.test(after) ? " " : "";
  const inserted = `${leading}@${displayName.trim()}${trailing || " "}`;
  return {
    text: `${before}${inserted}${after}`,
    caret: before.length + inserted.length,
  };
}

/**
 * Keep ordinary names primary while making duplicate-name choices stable.
 * @param {{peer_id: string, display_name: string}} profile
 * @param {Array<{peer_id: string, display_name: string}>} profiles
 */
export function disambiguatedMemberLabel(profile, profiles) {
  const name = profile.display_name.trim().toLocaleLowerCase();
  const duplicates = profiles.filter((candidate) => (
    candidate.display_name.trim().toLocaleLowerCase() === name
  ));
  const stableId = memberStableId(profile.peer_id);
  let markerLength = Math.min(12, stableId.length);
  while (
    markerLength < stableId.length
    && duplicates.some((candidate) => (
      candidate.peer_id !== profile.peer_id
      && memberStableId(candidate.peer_id).startsWith(stableId.slice(0, markerLength))
    ))
  ) markerLength += 1;
  return duplicates.length > 1
    ? `${profile.display_name} · member ${stableId.slice(0, markerLength)}`
    : profile.display_name;
}

function memberStableId(peerId) {
  return peerId.startsWith("ed25519:") ? peerId.slice(8) : peerId;
}

/**
 * Keep ordinary role names primary while making duplicate-name authority stable.
 * @param {{role_id: string, name: string}} role
 * @param {Array<{role_id: string, name: string}>} roles
 */
export function disambiguatedRoleLabel(role, roles) {
  const name = role.name.trim().toLocaleLowerCase();
  const duplicates = roles.filter((candidate) => (
    candidate.name.trim().toLocaleLowerCase() === name
  ));
  const stableId = role.role_id.startsWith("role:") ? role.role_id.slice(5) : role.role_id;
  let markerLength = Math.min(8, stableId.length);
  while (
    markerLength < stableId.length
    && duplicates.some((candidate) => {
      const candidateId = candidate.role_id.startsWith("role:")
        ? candidate.role_id.slice(5)
        : candidate.role_id;
      return candidate.role_id !== role.role_id
        && candidateId.endsWith(stableId.slice(-markerLength));
    })
  ) markerLength += 1;
  return duplicates.length > 1
    ? `${role.name} · role ${stableId.slice(-markerLength)}`
    : role.name;
}

/**
 * Keep ordinary channel names primary while making duplicate-name rooms stable.
 * @param {{room_id: string, name: string}} channel
 * @param {Array<{room_id: string, name: string}>} channels
 */
export function disambiguatedChannelLabel(channel, channels) {
  const name = channel.name.trim().toLocaleLowerCase();
  const duplicates = channels.filter((candidate) => (
    candidate.name.trim().toLocaleLowerCase() === name
  ));
  const stableId = channel.room_id.slice(channel.room_id.lastIndexOf(":") + 1);
  let markerLength = Math.min(8, stableId.length);
  while (
    markerLength < stableId.length
    && duplicates.some((candidate) => (
      candidate.room_id !== channel.room_id
      && candidate.room_id.slice(candidate.room_id.lastIndexOf(":") + 1)
        .endsWith(stableId.slice(-markerLength))
    ))
  ) markerLength += 1;
  return duplicates.length > 1
    ? `${channel.name} · channel ${stableId.slice(-markerLength)}`
    : channel.name;
}

/**
 * Make same-expiry bearer invites stable without cluttering ordinary rows.
 * @param {{invite_id: string, display_expiry: string}} invite
 * @param {Array<{invite_id: string, display_expiry: string}>} invites
 */
export function disambiguatedInviteLabel(invite, invites) {
  const duplicates = invites.filter((candidate) => (
    candidate.display_expiry === invite.display_expiry
  ));
  const stableId = invite.invite_id.slice(invite.invite_id.lastIndexOf(":") + 1);
  let markerLength = Math.min(8, stableId.length);
  while (
    markerLength < stableId.length
    && duplicates.some((candidate) => (
      candidate.invite_id !== invite.invite_id
      && candidate.invite_id.slice(candidate.invite_id.lastIndexOf(":") + 1)
        .endsWith(stableId.slice(-markerLength))
    ))
  ) markerLength += 1;
  return duplicates.length > 1
    ? `${invite.display_expiry} · invite ${stableId.slice(-markerLength)}`
    : invite.display_expiry;
}

/**
 * Make identical visible message contexts stable without cluttering ordinary posts.
 * @param {{event_id: string, display_context: string}} message
 * @param {Array<{event_id: string, display_context: string}>} messages
 */
export function disambiguatedMessageLabel(message, messages) {
  const duplicates = messages.filter((candidate) => (
    candidate.display_context === message.display_context
  ));
  const stableId = message.event_id.slice(message.event_id.lastIndexOf(":") + 1);
  let markerLength = Math.min(8, stableId.length);
  while (
    markerLength < stableId.length
    && duplicates.some((candidate) => (
      candidate.event_id !== message.event_id
      && candidate.event_id.slice(candidate.event_id.lastIndexOf(":") + 1)
        .endsWith(stableId.slice(-markerLength))
    ))
  ) markerLength += 1;
  return duplicates.length > 1
    ? `${message.display_context} · message ${stableId.slice(-markerLength)}`
    : message.display_context;
}
