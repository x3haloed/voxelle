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
