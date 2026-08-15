export function placementsFromViews(views) {
  return normalizePlacements(
    views.map((view) => ({
      view_id: view.id,
      place_id: view.place_id,
      order: view.order,
      visible: view.visible,
    })),
  );
}

export function moveView(views, viewId, placeId, targetIndex = Number.MAX_SAFE_INTEGER) {
  const placements = placementsFromViews(views);
  const moved = placements.find((placement) => placement.view_id === viewId);
  if (!moved) {
    throw new Error(`unknown view ${viewId}`);
  }
  moved.place_id = placeId;
  moved.visible = true;
  moved.order = targetIndex;
  return normalizePlacements(placements, viewId);
}

export function shiftView(views, viewId, delta) {
  const placements = placementsFromViews(views);
  const moved = placements.find((placement) => placement.view_id === viewId);
  if (!moved) {
    throw new Error(`unknown view ${viewId}`);
  }
  const peers = placements
    .filter((placement) => placement.place_id === moved.place_id)
    .sort(comparePlacement);
  const index = peers.findIndex((placement) => placement.view_id === viewId);
  const target = Math.max(0, Math.min(peers.length - 1, index + delta));
  if (target === index) {
    return placements;
  }
  const other = peers[target];
  [moved.order, other.order] = [other.order, moved.order];
  return normalizePlacements(placements);
}

export function setViewVisible(views, viewId, visible) {
  const placements = placementsFromViews(views);
  const placement = placements.find((candidate) => candidate.view_id === viewId);
  if (!placement) {
    throw new Error(`unknown view ${viewId}`);
  }
  placement.visible = visible;
  return placements;
}

export function filterPaletteCommands(commands, query) {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return commands
    .filter((command) => command.palette)
    .filter((command) => {
      const haystack = `${command.label} ${command.id} ${command.description}`.toLocaleLowerCase();
      return terms.every((term) => haystack.includes(term));
    });
}

const HOME_COMMANDS = new Set([
  "runtime.goOnline",
  "runtime.goOffline",
  "space.invite.create",
  "invite.copy",
  "identity.recovery.export",
  "channel.create",
  "channel.markRead",
  "profile.update",
  "role.create",
  "message.search",
  "message.composer.focus",
  "call.join",
  "call.leave",
  "call.microphone.toggle",
  "call.camera.toggle",
  "peer.import",
  "peer.diagnose",
  "peer.sync",
]);

export function paletteCommandAvailability(commandId, context) {
  if (HOME_COMMANDS.has(commandId) && !context.hasHome) {
    return { available: false, reason: "Create, join, or recover a space first" };
  }
  if (commandId === "home.init" && context.hasHome) {
    return { available: false, reason: "This device already has an active home" };
  }
  if (commandId === "home.init" && context.hasHomeError) {
    return { available: false, reason: "Resolve or archive the damaged home first" };
  }
  if (commandId === "space.join" && context.hasHome) {
    return { available: false, reason: "Joining requires a fresh Voxelle home" };
  }
  if (commandId === "space.join" && context.hasHomeError) {
    return { available: false, reason: "Archive the damaged home before joining" };
  }
  if (commandId === "identity.recovery.restore" && context.hasHome) {
    return { available: false, reason: "Recovery requires a fresh Voxelle home" };
  }
  if (commandId === "identity.recovery.restore" && context.hasHomeError) {
    return { available: false, reason: "Prepare the damaged home for recovery first" };
  }
  if (commandId === "runtime.goOffline" && !context.runtimeOnline) {
    return { available: false, reason: "The peer service is already offline" };
  }
  if (commandId === "runtime.goOnline" && context.runtimeOnline) {
    return {
      available: false,
      reason: "The peer service is already online; use Connection & sync to reconfigure it",
    };
  }
  if (commandId === "invite.copy" && !context.hasInvite) {
    return { available: false, reason: "Create a signed invite first" };
  }
  if (
    ["peer.diagnose", "peer.sync"].includes(commandId)
    && !context.hasKnownPeer
  ) {
    return {
      available: false,
      reason: "Join with an invite or import peer availability first",
    };
  }
  if (commandId === "call.join" && context.joinedCall) {
    return { available: false, reason: "You are already in this room's call" };
  }
  if (commandId === "call.join" && context.callFull) {
    return { available: false, reason: "This room's direct call is full" };
  }
  if (commandId === "call.leave" && !context.joinedCall) {
    return { available: false, reason: "You are not in this room's call" };
  }
  if (commandId === "call.microphone.toggle" && !context.joinedCall) {
    return { available: false, reason: "Join this room's call first" };
  }
  if (commandId === "call.camera.toggle" && !context.joinedCall) {
    return { available: false, reason: "Join this room's call first" };
  }
  if (
    [
      "product.update.check",
      "product.update.stageAvailable",
      "product.update.install",
      "product.update.rotateTrust",
    ].includes(commandId)
    && !context.updateAuthenticationAvailable
  ) {
    return { available: false, reason: "No trusted release root is available" };
  }
  if (commandId === "product.update.stageAvailable" && !context.hasAvailableUpdate) {
    return { available: false, reason: "Check for a signed update first" };
  }
  if (
    ["product.update.activateStaged", "product.update.discardStaged"].includes(commandId)
    && !context.hasStagedUpdate
  ) {
    return { available: false, reason: "Download and stage a signed update first" };
  }
  if (commandId === "product.update.rollback" && !context.hasPreviousGeneration) {
    return { available: false, reason: "No previous verified product generation is available" };
  }
  return { available: true, reason: "" };
}

export function shortcutMatches(event, shortcut) {
  if (!shortcut) {
    return false;
  }
  const parts = shortcut.split("+");
  const key = parts.at(-1)?.toLocaleLowerCase();
  const wantsMod = parts.includes("Mod");
  const wantsCtrl = parts.includes("Ctrl");
  const primaryModifierMatches = wantsMod
    ? event.metaKey || event.ctrlKey
    : wantsCtrl
      ? event.ctrlKey && !event.metaKey
      : !event.metaKey && !event.ctrlKey;
  return Boolean(
    key === event.key.toLocaleLowerCase()
      && primaryModifierMatches
      && event.shiftKey === parts.includes("Shift")
      && event.altKey === parts.includes("Alt"),
  );
}

function normalizePlacements(placements, movedViewId = "") {
  const byPlace = new Map();
  for (const placement of placements) {
    const list = byPlace.get(placement.place_id) ?? [];
    list.push({ ...placement });
    byPlace.set(placement.place_id, list);
  }
  const normalized = [];
  for (const list of byPlace.values()) {
    list.sort((left, right) => {
      if (left.view_id === movedViewId) return 1;
      if (right.view_id === movedViewId) return -1;
      return comparePlacement(left, right);
    });
    list.forEach((placement, order) => normalized.push({ ...placement, order }));
  }
  return normalized.sort((left, right) => left.view_id.localeCompare(right.view_id));
}

function comparePlacement(left, right) {
  return left.order - right.order || left.view_id.localeCompare(right.view_id);
}
