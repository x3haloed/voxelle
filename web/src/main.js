import { createShellClient } from "./shell-client.js";
import {
  captureCallMedia,
  consumeRetainedSignal,
  disconnectedParticipantIds,
  leaveCall,
} from "./call-media.mjs";
import { reconcileChildren } from "./dom-reconcile.mjs";
import {
  applyOntology,
  messageTimestamp,
  ontologyPresentation,
  safeDateTime,
  visibleActivity,
} from "./ui-ontology.mjs";
import {
  filterPaletteCommands,
  moveView,
  setViewVisible,
  shiftView,
  shortcutMatches,
} from "./workbench.mjs";

const app = document.querySelector("#app");
const shell = createShellClient();

if (!(app instanceof HTMLElement)) {
  throw new Error("missing #app");
}

const uiState = {
  busyCommand: "",
  error: "",
  peerRecordDraft: "",
  spaceInviteDraft: "",
  messageDraft: "",
  channelNameDraft: "",
  channelTopicDraft: "",
  channelMembersDraft: "",
  profileNameDraft: "",
  profileAboutDraft: "",
  searchDraft: "",
  bindDraft: "",
  advertiseDraft: "",
  draggedViewId: "",
  paletteOpen: false,
  paletteQuery: "",
  localMediaStream: null,
  remoteMediaStreams: new Map(),
  peerConnections: new Map(),
  pendingIce: new Map(),
  seenCallSignals: new Set(),
  processingCallSignals: false,
  mediaNotice: null,
  lastCallHeartbeatMs: 0,
};

const viewRenderers = {
  "profile.summary": profileSummaryView,
  "runtime.status": runtimeStatusView,
  "network.health": networkHealthView,
  "field.test": fieldTestView,
  "invite.exchange": inviteExchangeView,
  "peer.list": peerListView,
  "channel.list": channelListView,
  "member.profiles": memberProfilesView,
  "role.list": roleListView,
  "message.search": messageSearchView,
  "notification.center": notificationCenterView,
  "room.timeline": roomTimelineView,
  "message.composer": messageComposerView,
  "call.mesh": callMeshView,
  "service.activity": activityView,
};

let currentSnapshot = await shell.execute("shell.refresh");
let refreshInFlight = false;
let refreshQueued = false;
if (
  ontologyPresentation(currentSnapshot.ui_ontology).startOnlineOnLaunch
  && currentSnapshot.home?.runtime.state === "offline"
) {
  currentSnapshot = await shell.execute("runtime.goOnline", {
    bind: null,
    advertise: null,
  });
}
render();

await shell.onSnapshotInvalidated(() => {
  publishRefresh().catch(reportError);
});

async function publishRefresh() {
  if (refreshInFlight || uiState.busyCommand) {
    refreshQueued = true;
    return;
  }
  refreshInFlight = true;
  try {
    do {
      refreshQueued = false;
      await refresh();
    } while (refreshQueued);
    render();
  } finally {
    refreshInFlight = false;
  }
}

window.setInterval(async () => {
  const localPeerId = currentSnapshot.home?.profile.peer_id;
  const call = currentSnapshot.home?.call;
  if (
    refreshInFlight
    || uiState.busyCommand
    || !uiState.localMediaStream
    || !localPeerId
    || !call?.participants.includes(localPeerId)
    || Date.now() - uiState.lastCallHeartbeatMs < 20_000
  ) return;
  try {
    currentSnapshot = await shell.execute("call.heartbeat", {
      room: currentSnapshot.home?.room.room_id ?? null,
      call_id: call.call_id,
    });
    uiState.lastCallHeartbeatMs = Date.now();
    render();
  } catch (error) {
    reportError(error);
  }
}, 1_000);

async function refresh() {
  currentSnapshot = await shell.execute("shell.refresh");
  return currentSnapshot;
}

function render() {
  const localPeerId = currentSnapshot.home?.profile.peer_id;
  if (
    uiState.localMediaStream
    && localPeerId
    && !currentSnapshot.home?.call.participants.includes(localPeerId)
  ) {
    stopLocalMedia();
    uiState.mediaNotice = "Call session ended or the four-peer mesh was full.";
  }
  for (const peerId of disconnectedParticipantIds(
    currentSnapshot.home?.call.participants ?? [],
    uiState.peerConnections.keys(),
  )) {
    closePeerConnection(peerId);
  }
  const presentation = applyOntology(
    document.documentElement,
    currentSnapshot.ui_ontology,
  );
  const desired = document.createElement("div");
  desired.append(
    header(currentSnapshot),
    workbenchShell(currentSnapshot),
    ...(uiState.paletteOpen ? [commandPalette(currentSnapshot)] : []),
  );
  reconcileChildren(app, desired);
  if (presentation.activityAutoScroll) {
    const activity = app.querySelector(".activity-list");
    activity?.scrollTo?.({ top: activity.scrollHeight });
  }
  if (uiState.paletteOpen) {
    window.requestAnimationFrame(() => {
      const input = app.querySelector(".command-palette-input");
      input?.focus();
      input?.setSelectionRange?.(input.value.length, input.value.length);
    });
  }
  attachCallMedia();
  processCallSignals().catch(reportError);
}

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && uiState.paletteOpen) {
    event.preventDefault();
    uiState.paletteOpen = false;
    render();
    return;
  }
  const command = currentSnapshot.ui_ontology.commands.find((candidate) =>
    shortcutMatches(event, candidate.shortcut)
  );
  if (!command) return;
  event.preventDefault();
  runCommand(command.id).catch(reportError);
});

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function header(snapshot) {
  const headerEl = element("header", "app-header");
  const titleGroup = element("div", "title-group");
  titleGroup.append(element("h1", "", "Voxelle"));
  titleGroup.append(element("p", "path", snapshot.home_root));

  const actions = element("div", "header-actions");
  actions.append(
    customizationEditor(snapshot),
    shellMode(),
    runtimeState(snapshot),
    commandButton("workbench.commandPalette.open"),
    commandButton("shell.refresh"),
  );

  headerEl.append(titleGroup, actions);
  return headerEl;
}

function shellMode() {
  const mode = shell.mode ?? "unknown";
  return element(
    "div",
    `shell-mode ${mode}`,
    mode === "tauri" ? "Tauri" : "Preview only · no peer service",
  );
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function runtimeState(snapshot) {
  const runtime = snapshot.home?.runtime.state ?? "offline";
  return element("div", `runtime-state ${runtime}`, runtime);
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function customizationEditor(snapshot) {
  const details = element("details", "customization");
  details.append(element("summary", "command-button", "Customize"));
  const editor = element("div", "customization-editor");
  editor.append(
    preferenceGroup(
      "Appearance",
      snapshot.ui_ontology.semantic_tokens,
      semanticTokenEditor,
    ),
    preferenceGroup("Layout", snapshot.ui_ontology.metrics, metricEditor),
    preferenceGroup("Behavior", snapshot.ui_ontology.behaviors, behaviorEditor),
  );
  details.append(editor);
  return details;
}

function preferenceGroup(title, preferences, renderer) {
  const group = element("section", "preference-group");
  group.append(element("h3", "", title));
  for (const preference of preferences.filter((item) => item.editable)) {
    group.append(renderer(preference));
  }
  return group;
}

/** @param {import("./shell-contract").SemanticToken} token */
function semanticTokenEditor(token) {
  const input = preferenceInput(token, "text", token.current_value);
  return preferenceForm(token, input, () => ({
    kind: "semantic_token",
    id: token.id,
    value: input.value,
  }));
}

/** @param {import("./shell-contract").UiMetric} metric */
function metricEditor(metric) {
  const input = preferenceInput(metric, "number", String(metric.current_value));
  input.min = "0";
  input.step = metric.unit === "count" ? "1" : "0.5";
  return preferenceForm(metric, input, () => ({
    kind: "metric",
    id: metric.id,
    value: input.valueAsNumber,
  }));
}

/** @param {import("./shell-contract").UiBehavior} behavior */
function behaviorEditor(behavior) {
  const value = behavior.current_value;
  const input = preferenceInput(
    behavior,
    value.type === "bool" ? "checkbox" : "text",
    value.type === "text" ? value.value : "",
  );
  if (value.type === "bool") {
    input.checked = value.value;
  }
  return preferenceForm(behavior, input, () => ({
    kind: "behavior",
    id: behavior.id,
    value: value.type === "bool"
      ? { type: "bool", value: input.checked }
      : { type: "text", value: input.value },
  }));
}

function preferenceInput(preference, type, value) {
  const input = element("input", "preference-input");
  input.type = type;
  input.value = value;
  input.dataset.preferenceId = preference.id;
  return input;
}

function preferenceForm(preference, input, request) {
  const form = element("form", "preference-form");
  form.dataset.preferenceId = preference.id;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("ui.preference.set", request()).catch(reportError);
  });
  const label = element("label", "preference-label");
  label.append(
    element("span", "", preference.label),
    element("small", "view-id", preference.id),
    input,
  );
  form.append(label, submitButton("ui.preference.set"));
  return form;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function workbenchShell(snapshot) {
  const container = element("section", "workbench-container");
  const hidden = snapshot.ui_ontology.views.filter((view) => !view.visible);
  if (hidden.length > 0) {
    const shelf = element("nav", "hidden-view-shelf");
    shelf.dataset.renderKey = "hidden-view-shelf";
    shelf.setAttribute("aria-label", "Hidden workbench views");
    shelf.append(element("span", "muted", "Hidden views"));
    for (const view of hidden) {
      shelf.append(actionButton(`Show ${view.label}`, () => {
        saveLayout(setViewVisible(currentSnapshot.ui_ontology.views, view.id, true)).catch(reportError);
      }));
    }
    shelf.append(commandButton("workbench.layout.reset"));
    container.append(shelf);
  }

  const shellEl = element("section", "workbench");
  shellEl.dataset.renderKey = "workbench";
  for (const place of snapshot.ui_ontology.places) {
    shellEl.append(dockZone(place, snapshot));
  }
  container.append(shellEl);
  return container;
}

function dockZone(place, snapshot) {
  const zone = element("section", `dock-zone dock-${place.id}`);
  zone.dataset.renderKey = `dock:${place.id}`;
  zone.dataset.placeId = place.id;
  zone.setAttribute("aria-label", `${place.label} dock`);
  const heading = element("div", "dock-zone-header");
  heading.append(
    element("span", "dock-zone-label", place.label),
    element("span", "view-id", place.id),
  );
  zone.append(heading);
  zone.addEventListener("dragover", (event) => {
    event.preventDefault();
    zone.dataset.dragOver = "true";
  });
  zone.addEventListener("dragleave", () => delete zone.dataset.dragOver);
  zone.addEventListener("drop", (event) => {
    event.preventDefault();
    delete zone.dataset.dragOver;
    const viewId = event.dataTransfer?.getData("text/x-voxelle-view")
      || uiState.draggedViewId;
    if (viewId) {
      saveLayout(moveView(currentSnapshot.ui_ontology.views, viewId, place.id)).catch(reportError);
    }
  });

  const views = snapshot.ui_ontology.views
    .filter((view) => view.visible && view.place_id === place.id)
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));
  if (views.length === 0) {
    zone.append(element("p", "dock-empty", "Drop a view here"));
  } else {
    for (const view of views) {
      zone.append(workbenchPanel(view, snapshot));
    }
  }
  return zone;
}

/**
 * @param {import("./shell-contract").UiView} viewDefinition
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function workbenchPanel(viewDefinition, snapshot) {
  const section = element("section", "panel");
  section.dataset.renderKey = `view:${viewDefinition.id}`;
  section.dataset.panelId = `panel.${viewDefinition.id}`;
  section.dataset.viewId = viewDefinition.id;
  section.dataset.placeId = viewDefinition.place_id;
  section.append(panelHeader(viewDefinition, snapshot));

  const view = element("div", "panel-view");
  const renderer = viewRenderers[viewDefinition.id] ?? unknownView;
  view.append(renderer(snapshot));
  section.append(view);
  return section;
}

function panelHeader(viewDefinition, snapshot) {
  const headerEl = element("div", "panel-header");
  headerEl.draggable = true;
  headerEl.addEventListener("dragstart", (event) => {
    uiState.draggedViewId = viewDefinition.id;
    event.dataTransfer?.setData("text/x-voxelle-view", viewDefinition.id);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  });
  headerEl.addEventListener("dragend", () => {
    uiState.draggedViewId = "";
  });
  const titleGroup = element("div", "panel-title");
  titleGroup.append(element("h2", "", viewDefinition.label));
  titleGroup.append(element("span", "view-id", viewDefinition.id));
  const controls = element("div", "panel-controls");
  const placeSelect = element("select", "dock-select");
  placeSelect.setAttribute("aria-label", `Dock ${viewDefinition.label}`);
  for (const place of snapshot.ui_ontology.places) {
    const option = element("option", "", place.label);
    option.value = place.id;
    option.selected = place.id === viewDefinition.place_id;
    placeSelect.append(option);
  }
  placeSelect.addEventListener("pointerdown", (event) => event.stopPropagation());
  placeSelect.addEventListener("change", () => {
    saveLayout(moveView(currentSnapshot.ui_ontology.views, viewDefinition.id, placeSelect.value))
      .catch(reportError);
  });
  controls.append(
    actionButton("↑", () => {
      saveLayout(shiftView(currentSnapshot.ui_ontology.views, viewDefinition.id, -1)).catch(reportError);
    }, `Move ${viewDefinition.label} earlier`),
    actionButton("↓", () => {
      saveLayout(shiftView(currentSnapshot.ui_ontology.views, viewDefinition.id, 1)).catch(reportError);
    }, `Move ${viewDefinition.label} later`),
    placeSelect,
    actionButton("×", () => {
      saveLayout(setViewVisible(currentSnapshot.ui_ontology.views, viewDefinition.id, false))
        .catch(reportError);
    }, `Hide ${viewDefinition.label}`),
  );
  headerEl.append(titleGroup, controls);
  return headerEl;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function profileSummaryView(snapshot) {
  const fragment = document.createDocumentFragment();
  fragment.append(errorBanner());

  if (!snapshot.home) {
    const empty = element("div", "empty-state");
    empty.append(
      element("h3", "", "No initialized home"),
      element("p", "summary", snapshot.home_error ?? "Home state is not available."),
      commandButton("home.init"),
    );
    const joinForm = element("form", "field-stack");
    joinForm.addEventListener("submit", (event) => {
      event.preventDefault();
      runCommand("space.join").catch(reportError);
    });
    const inviteInput = element("textarea", "peer-record-input");
    inviteInput.placeholder = "Paste signed .voxinvite JSON";
    inviteInput.value = uiState.spaceInviteDraft;
    inviteInput.addEventListener("input", () => {
      uiState.spaceInviteDraft = inviteInput.value;
    });
    joinForm.append(
      element("h3", "", "Or join a space"),
      element("p", "summary", "Paste the signed invite; Voxelle will create your identity, join, sync, and go online."),
      inviteInput,
      submitButton("space.join"),
    );
    empty.append(joinForm);
    fragment.append(empty);
    return fragment;
  }

  const profile = snapshot.home.profile;
  const rows = [
    ["Home root", snapshot.home_root],
    ["Peer", profile.peer_id],
    ["Device", profile.device_id],
    ["Default room", profile.default_room],
    ["Authority", profile.authority_peer_id],
    ["Known peers", String(snapshot.home.peers.length)],
    ["Messages", String(snapshot.home.room.messages.length)],
  ];

  fragment.append(definitionGrid(rows));

  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function runtimeStatusView(snapshot) {
  const runtime = snapshot.home?.runtime;
  const rows = [
    ["Runtime", runtime?.state ?? "offline"],
    ["Listen", runtime?.listen_addr ?? "not listening"],
    ["Advertise", runtime?.advertised_addr ?? "not advertising"],
  ];
  for (const [index, note] of (runtime?.reachability_notes ?? []).entries()) {
    rows.push([`Reachability ${index + 1}`, note]);
  }
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(
    commandButton("home.init"),
    commandButton("runtime.goOnline"),
    commandButton("runtime.goOffline"),
  );
  fragment.append(errorBanner(), definitionGrid(rows), serviceOptions(), controls);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function networkHealthView(snapshot) {
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(
    commandButton("home.init"),
    commandButton("runtime.goOnline"),
    commandButton("runtime.goOffline"),
  );
  fragment.append(errorBanner(), controls);

  const rows = element("ol", "health-list");
  for (const row of snapshot.network_health.rows) {
    rows.append(healthRow(row));
  }
  fragment.append(rows);
  return fragment;
}

function serviceOptions() {
  const options = element("div", "service-options");
  options.append(
    labeledInput("Bind", "Optional local bind, e.g. [::]:0", uiState.bindDraft, (value) => {
      uiState.bindDraft = value;
    }),
    labeledInput(
      "Advertise",
      "Optional advertised IPv6 address",
      uiState.advertiseDraft,
      (value) => {
        uiState.advertiseDraft = value;
      },
    ),
  );
  return options;
}

/**
 * @param {import("./shell-contract").NetworkHealthRow} row
 */
function healthRow(row) {
  const item = element("li", "health-row");
  item.dataset.renderKey = `health:${row.id}`;
  item.dataset.status = row.status;
  item.dataset.healthRowId = row.id;

  const indicator = element("span", "status-indicator", statusLabel(row.status));
  const body = element("div", "health-body");
  body.append(element("h3", "", row.label));
  body.append(element("p", "summary", row.summary));

  if (row.details.length > 0) {
    const details = element("ul", "details");
    for (const detail of row.details) {
      details.append(element("li", "", detail));
    }
    body.append(details);
  }

  item.append(indicator, body);
  if (row.primary_action) {
    item.append(commandButton(row.primary_action));
  }
  return item;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function activityView(snapshot) {
  const fragment = document.createDocumentFragment();
  fragment.append(errorBanner());

  const actions = element("div", "control-row");
  actions.append(commandButton("shell.refresh"));
  fragment.append(actions);

  const list = element("ol", "activity-list");
  for (const activity of visibleActivity(
    snapshot.service_activity,
    snapshot.ui_ontology,
  )) {
    const row = element("li", "");
    row.dataset.renderKey = `activity:${activity.id}`;
    row.dataset.level = activity.level;
    row.append(
      element("span", "activity-id", String(activity.id)),
      element("span", "", activity.summary),
    );
    list.append(row);
  }
  fragment.append(list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function inviteExchangeView(snapshot) {
  const fragment = document.createDocumentFragment();
  const invite = snapshot.home?.invite?.space_invite_json ?? "";
  const inviteGroup = element("div", "field-stack");
  inviteGroup.append(element("h3", "", "Signed Space Invite"));
  inviteGroup.append(element("p", "summary", "Create an expiring invite after going online. It grants membership; bootstrap addresses inside it are signed availability hints."));
  inviteGroup.append(element("pre", "invite-json", invite));
  inviteGroup.append(
    commandButton("space.invite.create"),
    commandButton("invite.copy"),
  );

  const importGroup = element("form", "field-stack");
  importGroup.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("peer.import").catch(reportError);
  });
  const textarea = element("textarea", "peer-record-input");
  textarea.placeholder = "Paste peer record JSON";
  textarea.value = uiState.peerRecordDraft;
  textarea.addEventListener("input", () => {
    uiState.peerRecordDraft = textarea.value;
  });
  importGroup.append(
    element("h3", "", commandLabel("peer.import")),
    textarea,
    submitButton("peer.import"),
  );

  fragment.append(inviteGroup, importGroup);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function fieldTestView(snapshot) {
  const fragment = document.createDocumentFragment();
  const rows = [
    {
      label: "Home initialized",
      status: snapshot.home ? "working" : "needs_attention",
      command: snapshot.home ? null : "home.init",
      detail: snapshot.home ? snapshot.home.profile.default_room : snapshot.home_error,
    },
    {
      label: "Resident service online",
      status: snapshot.home?.runtime.state === "online" ? "working" : "needs_attention",
      command: snapshot.home?.runtime.state === "online" ? "runtime.goOffline" : "runtime.goOnline",
      detail: snapshot.home?.runtime.advertised_addr ?? "offline",
    },
    {
      label: "Invite available",
      status: snapshot.home?.invite?.space_invite_json ? "working" : "unknown",
      command: snapshot.home?.invite?.space_invite_json
        ? "invite.copy"
        : snapshot.home?.invite
          ? "space.invite.create"
          : "runtime.goOnline",
      detail: snapshot.home?.invite?.space_invite_json
        ? "signed membership capability ready"
        : "go online, then create a signed invite",
    },
    {
      label: "Peer imported",
      status: (snapshot.home?.peers.length ?? 0) > 0 ? "working" : "needs_attention",
      command: "peer.import",
      detail: `${snapshot.home?.peers.length ?? 0} known peer(s)`,
    },
    {
      label: "Peer diagnostic",
      status: activityIncludes(snapshot, "diagnostic reached") ? "working" : "needs_attention",
      command: (snapshot.home?.peers.length ?? 0) > 0 ? "peer.diagnose" : "peer.import",
      detail: activityIncludes(snapshot, "diagnostic reached")
        ? "latest diagnostic reached a peer"
        : "run against an imported peer",
    },
    {
      label: "Room sync",
      status: activityIncludes(snapshot, "sync") ? "working" : "needs_attention",
      command: (snapshot.home?.peers.length ?? 0) > 0 ? "peer.sync" : "peer.import",
      detail: `${snapshot.home?.room.messages.length ?? 0} visible message(s)`,
    },
  ];

  const list = element("ol", "workflow-list");
  for (const row of rows) {
    const item = element("li", "workflow-row");
    item.dataset.renderKey = `workflow:${row.label}`;
    item.dataset.status = row.status;
    const body = element("div", "workflow-body");
    body.append(
      element("span", "status-indicator", statusLabel(row.status)),
      element("h3", "", row.label),
      element("p", "summary", row.detail ?? ""),
    );
    item.append(body);
    if (row.command) {
      item.append(commandButton(row.command));
    }
    list.append(item);
  }

  fragment.append(list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function peerListView(snapshot) {
  const peers = snapshot.home?.peers ?? [];
  const list = element("ol", "peer-list");
  for (const peer of peers) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `peer:${peer.peer_id}:${peer.device_id}`;
    const body = element("div", "peer-body");
    body.append(element("strong", "", peer.label));
    body.append(element("span", "mono", peer.addr));
    body.append(element("span", "muted", shortId(peer.peer_id)));

    const actions = element("div", "row-actions");
    actions.append(
      commandButton("peer.diagnose", peerRequest(peer)),
      commandButton("peer.sync", peerRequest(peer)),
    );
    row.append(body, actions);
    list.append(row);
  }
  return list;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function channelListView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "peer-list");
  for (const channel of snapshot.home?.channels ?? []) {
    const row = element("li", channel.selected ? "peer-row selected" : "peer-row");
    row.dataset.renderKey = `channel:${channel.room_id}`;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", `${channel.visibility === "private" ? "🔒" : "#"} ${channel.name}${channel.unread_count > 0 ? ` (${channel.unread_count})` : ""}`),
      element("span", "muted", channel.topic),
    );
    const actions = element("div", "row-actions");
    actions.append(commandButton("channel.select", { room_id: channel.room_id }));
    if (channel.visibility === "private") {
      actions.append(commandButton("channel.rotateKey", { room_id: channel.room_id }));
    }
    row.append(body, actions);
    list.append(row);
  }
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("channel.create").catch(reportError);
  });
  form.append(
    labeledInput("Name", "new-channel", uiState.channelNameDraft, (value) => { uiState.channelNameDraft = value; }),
    labeledInput("Topic", "What belongs here?", uiState.channelTopicDraft, (value) => { uiState.channelTopicDraft = value; }),
    labeledInput("Private members", "Peer IDs, comma-separated; blank means public", uiState.channelMembersDraft, (value) => { uiState.channelMembersDraft = value; }),
    submitButton("channel.create"),
  );
  fragment.append(list, form);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function memberProfilesView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "peer-list");
  for (const profile of snapshot.home?.profiles ?? []) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `profile:${profile.peer_id}`;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", profile.display_name),
      element("span", "muted", profile.about),
      element("span", "mono", shortId(profile.peer_id)),
    );
    row.append(body);
    list.append(row);
  }
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("profile.update").catch(reportError);
  });
  form.append(
    labeledInput("Display name", "Your name", uiState.profileNameDraft, (value) => { uiState.profileNameDraft = value; }),
    labeledInput("About", "A short profile", uiState.profileAboutDraft, (value) => { uiState.profileAboutDraft = value; }),
    submitButton("profile.update"),
  );
  fragment.append(list, form);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roleListView(snapshot) {
  const list = element("ol", "peer-list");
  for (const role of snapshot.home?.roles ?? []) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `role:${role.role_id}`;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", role.name),
      element("span", "muted", `${role.member_count} member(s)`),
      element("span", "mono", role.permissions.join(", ") || "no permissions"),
    );
    row.append(body);
    list.append(row);
  }
  const controls = element("div", "control-row");
  controls.append(commandButton("role.create"), commandButton("role.grant"));
  list.append(controls);
  return list;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function messageSearchView(snapshot) {
  const fragment = document.createDocumentFragment();
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("message.search").catch(reportError);
  });
  form.append(labeledInput("Search", "Words in messages or attachment names", uiState.searchDraft, (value) => { uiState.searchDraft = value; }), submitButton("message.search"));
  const results = element("ol", "message-list");
  for (const result of snapshot.search_results) {
    const row = element("li", "message remote");
    row.dataset.renderKey = `search:${result.message.event_id}`;
    row.append(
      element("span", "mono", result.room_id),
      element("span", "muted", shortId(result.message.author_peer_id)),
      element("p", "", result.message.text),
    );
    results.append(row);
  }
  fragment.append(form, results);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function notificationCenterView(snapshot) {
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(commandButton("channel.markRead"));
  const list = element("ol", "activity-list");
  for (const notification of snapshot.home?.notifications ?? []) {
    const row = element("li", "");
    row.dataset.renderKey = `notification:${notification.event_id}`;
    row.append(
      element("strong", "", `@ ${shortId(notification.author_peer_id)}`),
      element("span", "mono", notification.room_id),
      element("span", "", notification.summary),
    );
    list.append(row);
  }
  fragment.append(controls, list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roomTimelineView(snapshot) {
  const messages = snapshot.home?.room.messages ?? [];
  const list = element("ol", "message-list");
  for (const message of messages) {
    const own = message.author_peer_id === snapshot.home?.profile.peer_id;
    const row = element("li", own ? "message own" : "message remote");
    row.dataset.renderKey = `message:${message.event_id}`;
    row.append(element("span", "muted", shortId(message.author_peer_id)));
    const timestamp = messageTimestamp(message, snapshot.ui_ontology);
    if (timestamp !== null) {
      const time = element("time", "message-time", timestamp);
      time.dateTime = safeDateTime(message.created_ms) ?? "";
      row.append(time);
    }
    row.append(element("p", message.redacted ? "muted" : "", message.text));
    if (message.edited_ms !== null) row.append(element("small", "muted", "edited"));
    if (message.pinned) row.append(element("small", "muted", "pinned"));
    if (message.reply_count > 0) row.append(element("small", "muted", `${message.reply_count} repl${message.reply_count === 1 ? "y" : "ies"}`));
    for (const reaction of message.reactions) {
      row.append(commandButton("reaction.add", { target_event_id: message.event_id, emoji: reaction.emoji, room: snapshot.home?.room.room_id ?? null }));
    }
    for (const attachment of message.attachments) {
      const link = element("a", "mono", `${attachment.filename} · ${attachment.sha256}`);
      link.href = `data:${attachment.mime};base64,${attachment.data_b64}`;
      link.download = attachment.filename;
      row.append(link);
    }
    const actions = element("div", "row-actions");
    actions.append(
      commandButton("reaction.add", { target_event_id: message.event_id, emoji: "👍", room: snapshot.home?.room.room_id ?? null }),
      commandButton("pin.add", { target_event_id: message.event_id, room: snapshot.home?.room.room_id ?? null }),
    );
    if (own) {
      actions.append(
        commandButton("message.edit", { target_event_id: message.event_id, room: snapshot.home?.room.room_id ?? null }),
        commandButton("message.redact", { target_event_id: message.event_id, room: snapshot.home?.room.room_id ?? null }),
      );
    }
    row.append(actions);
    list.append(row);
  }

  return list;
}

function messageComposerView() {
  const form = element("form", "message-form");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("message.send").catch(reportError);
  });
  const input = element("input", "message-input");
  input.placeholder = "Message";
  input.value = uiState.messageDraft;
  input.addEventListener("input", () => {
    uiState.messageDraft = input.value;
  });
  const fileInput = element("input", "");
  fileInput.type = "file";
  fileInput.setAttribute("aria-label", "Attach file");
  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    if (file.size > 256 * 1024) {
      reportError(new Error("attachments are limited to 256 KiB"));
      return;
    }
    const data_b64 = await fileAsBase64(file);
    await runCommand("attachment.add", {
      filename: file.name,
      mime: file.type || "application/octet-stream",
      data_b64,
      room: null,
    });
  });
  form.append(input, submitButton("message.send"), fileInput);

  return form;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function callMeshView(snapshot) {
  const fragment = document.createDocumentFragment();
  const call = snapshot.home?.call;
  const localPeerId = snapshot.home?.profile.peer_id;
  const joined = Boolean(localPeerId && call?.participants.includes(localPeerId));
  const controls = element("div", "control-row");
  controls.append(
    commandButton("call.join", { video: false }),
    commandButton("call.join", { video: true }),
    commandButton("call.leave"),
  );
  controls.children[0].textContent = "Join voice";
  controls.children[1].textContent = "Join video";
  const status = element(
    "p",
    "summary",
    joined
      ? `${call?.participants.length ?? 0} of 4 peers in direct mesh`
      : "Media stays in direct WebRTC connections; replicated signed events carry signaling only.",
  );
  const videos = element("div", "call-grid");
  if (joined) {
    const localVideo = element("video", "call-video");
    localVideo.autoplay = true;
    localVideo.muted = true;
    localVideo.playsInline = true;
    localVideo.dataset.peerId = "local";
    videos.append(callTile("You", localVideo));
    for (const peerId of call?.participants ?? []) {
      if (peerId === localPeerId) continue;
      const video = element("video", "call-video");
      video.autoplay = true;
      video.playsInline = true;
      video.dataset.peerId = peerId;
      videos.append(callTile(shortId(peerId), video));
    }
  }
  fragment.append(status);
  if (uiState.mediaNotice) {
    fragment.append(element("p", "call-warning", uiState.mediaNotice));
  }
  fragment.append(controls, videos);
  return fragment;
}

function callTile(label, video) {
  const tile = element("figure", "call-tile");
  tile.append(video, element("figcaption", "mono", label));
  return tile;
}

function attachCallMedia() {
  const localVideo = app.querySelector('video[data-peer-id="local"]');
  if (localVideo && localVideo.srcObject !== uiState.localMediaStream) {
    localVideo.srcObject = uiState.localMediaStream;
  }
  for (const [peerId, stream] of uiState.remoteMediaStreams) {
    const video = [...app.querySelectorAll("video[data-peer-id]")]
      .find((candidate) => candidate.dataset.peerId === peerId);
    if (video && video.srcObject !== stream) video.srcObject = stream;
  }
}

async function processCallSignals() {
  if (uiState.processingCallSignals || !uiState.localMediaStream || !currentSnapshot.home) return;
  uiState.processingCallSignals = true;
  try {
    const localPeerId = currentSnapshot.home.profile.peer_id;
    const call = currentSnapshot.home.call;
    const retainedSignalIds = new Set(call.signals.map((signal) => signal.event_id));
    for (const eventId of uiState.seenCallSignals) {
      if (!retainedSignalIds.has(eventId)) uiState.seenCallSignals.delete(eventId);
    }
    for (const signal of call.signals) {
      try {
        await consumeRetainedSignal(signal, uiState.seenCallSignals, async () => {
          if (signal.kind === "CALL_JOIN" && signal.author_peer_id !== localPeerId) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            if (localPeerId.localeCompare(signal.author_peer_id) < 0 && pc.signalingState === "stable") {
              const offer = await pc.createOffer();
              await pc.setLocalDescription(offer);
              await sendCallSignal("offer", signal.author_peer_id, JSON.stringify(offer), null);
            }
          } else if (signal.kind === "CALL_LEAVE" && signal.author_peer_id !== localPeerId) {
            closePeerConnection(signal.author_peer_id);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_OFFER" && signal.sdp) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            await pc.setRemoteDescription(JSON.parse(signal.sdp));
            await flushPendingIce(signal.author_peer_id, pc);
            const answer = await pc.createAnswer();
            await pc.setLocalDescription(answer);
            await sendCallSignal("answer", signal.author_peer_id, JSON.stringify(answer), null);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_ANSWER" && signal.sdp) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            await pc.setRemoteDescription(JSON.parse(signal.sdp));
            await flushPendingIce(signal.author_peer_id, pc);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_ICE" && signal.candidate) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            const candidate = JSON.parse(signal.candidate);
            if (pc.remoteDescription) await pc.addIceCandidate(candidate);
            else {
              const pending = uiState.pendingIce.get(signal.author_peer_id) ?? [];
              if (pending.length >= 64) pending.shift();
              pending.push(candidate);
              uiState.pendingIce.set(signal.author_peer_id, pending);
            }
          }
        });
      } catch (error) {
        const detail = errorMessage(error);
        uiState.mediaNotice = `Ignored invalid call signal from ${shortId(signal.author_peer_id)}.`;
        appendActivity(currentSnapshot, `invalid call signal ${signal.event_id}: ${detail}`);
      }
    }
  } finally {
    uiState.processingCallSignals = false;
    attachCallMedia();
  }
}

function ensurePeerConnection(peerId, callId) {
  const existing = uiState.peerConnections.get(peerId);
  if (existing) return existing;
  const pc = new RTCPeerConnection({ iceServers: [] });
  for (const track of uiState.localMediaStream?.getTracks() ?? []) {
    pc.addTrack(track, uiState.localMediaStream);
  }
  pc.addEventListener("icecandidate", (event) => {
    if (event.candidate) {
      sendCallSignal("ice", peerId, null, JSON.stringify(event.candidate.toJSON())).catch(reportError);
    }
  });
  pc.addEventListener("track", (event) => {
    let stream = uiState.remoteMediaStreams.get(peerId);
    if (!stream) {
      stream = new MediaStream();
      uiState.remoteMediaStreams.set(peerId, stream);
    }
    stream.addTrack(event.track);
    attachCallMedia();
  });
  pc.addEventListener("connectionstatechange", () => {
    if (["failed", "closed"].includes(pc.connectionState)) closePeerConnection(peerId);
  });
  pc.__voxelleCallId = callId;
  uiState.peerConnections.set(peerId, pc);
  return pc;
}

async function flushPendingIce(peerId, pc) {
  for (const candidate of uiState.pendingIce.get(peerId) ?? []) {
    await pc.addIceCandidate(candidate);
  }
  uiState.pendingIce.delete(peerId);
}

async function sendCallSignal(signal_type, target_peer_id, sdp, candidate) {
  currentSnapshot = await shell.execute("call.signal", {
    room: currentSnapshot.home?.room.room_id ?? null,
    call_id: currentSnapshot.home?.call.call_id ?? "",
    target_peer_id,
    signal_type,
    sdp,
    candidate,
  });
  render();
}

function closePeerConnection(peerId) {
  uiState.peerConnections.get(peerId)?.close();
  uiState.peerConnections.delete(peerId);
  uiState.remoteMediaStreams.delete(peerId);
  uiState.pendingIce.delete(peerId);
}

function stopLocalMedia() {
  for (const track of uiState.localMediaStream?.getTracks() ?? []) track.stop();
  uiState.localMediaStream = null;
  for (const peerId of [...uiState.peerConnections.keys()]) closePeerConnection(peerId);
  uiState.seenCallSignals.clear();
}

function unknownView() {
  return element("p", "summary", "Unknown view");
}

function commandPalette(snapshot) {
  const backdrop = element("div", "command-palette-backdrop");
  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      uiState.paletteOpen = false;
      render();
    }
  });
  const dialog = element("section", "command-palette");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", "Command Palette");
  const form = element("form", "command-palette-form");
  const input = element("input", "command-palette-input");
  input.type = "search";
  input.placeholder = "Type a command";
  input.value = uiState.paletteQuery;
  input.setAttribute("aria-label", "Search commands");
  const list = element("ol", "command-palette-list");
  const updateResults = () => {
    uiState.paletteQuery = input.value;
    populatePaletteResults(list, snapshot);
  };
  input.addEventListener("input", updateResults);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const first = filterPaletteCommands(
      snapshot.ui_ontology.commands,
      uiState.paletteQuery,
    )[0];
    if (first) executePaletteCommand(first.id);
  });
  form.append(input);
  dialog.append(form);
  populatePaletteResults(list, snapshot);
  dialog.append(list);
  backdrop.append(dialog);
  return backdrop;
}

function populatePaletteResults(list, snapshot) {
  list.replaceChildren();
  const commands = filterPaletteCommands(
    snapshot.ui_ontology.commands,
    uiState.paletteQuery,
  );
  if (commands.length === 0) {
    list.append(element("li", "command-palette-empty", "No matching commands"));
  }
  for (const command of commands) {
    const item = element("li", "");
    const button = element("button", "command-palette-result");
    button.type = "button";
    const copy = element("span", "command-palette-copy");
    copy.append(
      element("strong", "", command.label),
      element("small", "muted", command.description),
    );
    button.append(copy);
    if (command.shortcut) {
      button.append(element("kbd", "", command.shortcut.replace("Mod", navigator.platform.includes("Mac") ? "⌘" : "Ctrl")));
    }
    button.addEventListener("click", () => executePaletteCommand(command.id));
    item.append(button);
    list.append(item);
  }
}

function executePaletteCommand(commandId) {
  uiState.paletteOpen = false;
  uiState.paletteQuery = "";
  runCommand(commandId).catch(reportError);
}

/**
 * @param {Array<[string, string]>} rows
 */
function definitionGrid(rows) {
  const grid = element("dl", "definition-grid");
  for (const [term, value] of rows) {
    grid.append(element("dt", "", term), element("dd", "mono", value));
  }
  return grid;
}

/**
 * @param {string} command
 */
function commandLabel(command) {
  return currentSnapshot.ui_ontology.commands.find((item) => item.id === command)?.label
    ?? command;
}

/**
 * @param {string} command
 * @param {unknown} [payload]
 */
function commandButton(command, payload) {
  const button = element("button", "command-button", commandLabel(command));
  button.type = "button";
  button.dataset.command = command;
  const definition = currentSnapshot.ui_ontology.commands.find((item) => item.id === command);
  if (definition?.shortcut) {
    button.title = `${definition.description} (${definition.shortcut})`;
  }
  button.disabled = uiState.busyCommand !== "";
  if (uiState.busyCommand === command) {
    button.textContent = "Working";
  }
  button.addEventListener("click", () => {
    runCommand(command, payload).catch(reportError);
  });
  return button;
}

function actionButton(label, action, title = label) {
  const button = element("button", "command-button", label);
  button.type = "button";
  button.title = title;
  button.disabled = uiState.busyCommand !== "";
  button.addEventListener("pointerdown", (event) => event.stopPropagation());
  button.addEventListener("click", action);
  return button;
}

async function saveLayout(placements) {
  await runCommand("workbench.layout.save", { placements });
}

/** @param {string} command */
function submitButton(command) {
  const button = element("button", "command-button", commandLabel(command));
  button.type = "submit";
  button.disabled = uiState.busyCommand !== "";
  return button;
}

/**
 * @param {string} command
 * @param {unknown} [payload]
 */
async function runCommand(command, payload) {
  uiState.busyCommand = command;
  uiState.error = "";
  render();
  try {
    switch (command) {
      case "workbench.commandPalette.open":
        uiState.paletteOpen = true;
        uiState.paletteQuery = "";
        return;
      case "message.composer.focus":
        uiState.paletteOpen = false;
        window.requestAnimationFrame(() => {
          app.querySelector(".message-input")?.focus();
        });
        return;
      case "shell.refresh":
        await refresh();
        return;
      case "home.init":
        currentSnapshot = await shell.execute(command, { default_room: null });
        return;
      case "runtime.goOnline":
        currentSnapshot = await shell.execute(command, {
          bind: blankToNull(uiState.bindDraft),
          advertise: blankToNull(uiState.advertiseDraft),
        });
        return;
      case "runtime.goOffline":
        currentSnapshot = await shell.execute(command);
        return;
      case "space.invite.create":
        currentSnapshot = await shell.execute(command, { expires_minutes: 1440 });
        return;
      case "space.join":
        currentSnapshot = await shell.execute(command, {
          space_invite_json: uiState.spaceInviteDraft,
          max_events: 4096,
        });
        uiState.spaceInviteDraft = "";
        return;
      case "peer.import":
        currentSnapshot = await shell.execute(command, {
          peer_record_json: uiState.peerRecordDraft,
        });
        uiState.peerRecordDraft = "";
        if (ontologyPresentation(currentSnapshot.ui_ontology).syncAutoAfterImport) {
          currentSnapshot = await shell.execute("peer.sync", firstPeerRequest());
        }
        return;
      case "peer.diagnose":
      case "peer.sync":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").PeerCommandRequest} */ (
            payload ?? firstPeerRequest()
          ),
        );
        return;
      case "message.send":
        currentSnapshot = await shell.execute(command, {
          text: payload?.text ?? uiState.messageDraft,
          room: payload?.room ?? null,
          mentions: payload?.mentions ?? [],
          thread_root_event_id: payload?.thread_root_event_id ?? null,
        });
        uiState.messageDraft = "";
        return;
      case "channel.select":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "channel.markRead":
        currentSnapshot = await shell.execute(command, payload ?? { room_id: null });
        return;
      case "channel.rotateKey":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "channel.create":
        currentSnapshot = await shell.execute(command, payload ?? {
          name: uiState.channelNameDraft,
          topic: uiState.channelTopicDraft,
          private_members: uiState.channelMembersDraft.split(",").map((value) => value.trim()).filter(Boolean),
        });
        uiState.channelNameDraft = "";
        uiState.channelTopicDraft = "";
        uiState.channelMembersDraft = "";
        return;
      case "message.edit": {
        const text = payload?.text ?? window.prompt("New message text");
        if (text === null) return;
        currentSnapshot = await shell.execute(command, { ...payload, text, mentions: payload?.mentions ?? [] });
        return;
      }
      case "message.redact":
      case "reaction.add":
      case "reaction.remove":
      case "pin.add":
      case "pin.remove":
      case "attachment.add":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "profile.update":
        currentSnapshot = await shell.execute(command, payload ?? {
          display_name: uiState.profileNameDraft,
          about: uiState.profileAboutDraft,
        });
        uiState.profileNameDraft = "";
        uiState.profileAboutDraft = "";
        return;
      case "message.search":
        currentSnapshot = await shell.execute(command, payload ?? {
          query: uiState.searchDraft,
          room: null,
          limit: 50,
        });
        return;
      case "call.join": {
        if (!navigator.mediaDevices?.getUserMedia || typeof RTCPeerConnection === "undefined") {
          throw new Error("This WebView does not provide WebRTC media capture");
        }
        stopLocalMedia();
        const capture = await captureCallMedia(navigator.mediaDevices, Boolean(payload?.video));
        uiState.localMediaStream = capture.stream;
        uiState.mediaNotice = capture.notice;
        try {
          currentSnapshot = await shell.execute(command, {
            room: currentSnapshot.home?.room.room_id ?? null,
            video: capture.video,
          });
          uiState.lastCallHeartbeatMs = Date.now();
        } catch (error) {
          stopLocalMedia();
          throw error;
        }
        return;
      }
      case "call.leave":
        currentSnapshot = await leaveCall(
          () => shell.execute(command, {
            room: currentSnapshot.home?.room.room_id ?? null,
            call_id: currentSnapshot.home?.call.call_id ?? "",
          }),
          () => {
            stopLocalMedia();
            uiState.mediaNotice = null;
            uiState.lastCallHeartbeatMs = 0;
          },
        );
        return;
      case "call.heartbeat":
        currentSnapshot = await shell.execute(command, payload);
        uiState.lastCallHeartbeatMs = Date.now();
        return;
      case "call.signal":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "role.create": {
        const name = payload?.name ?? window.prompt("Role name");
        if (!name) return;
        const permissionsText = payload?.permissions?.join(",") ?? window.prompt("Permissions (comma-separated)", "message:moderate,message:pin") ?? "";
        currentSnapshot = await shell.execute(command, {
          name,
          permissions: payload?.permissions ?? permissionsText.split(",").map((value) => value.trim()).filter(Boolean),
        });
        return;
      }
      case "role.grant":
      case "role.revoke": {
        const peer_id = payload?.peer_id ?? window.prompt("Member peer ID");
        const role_id = payload?.role_id ?? window.prompt("Role ID");
        if (!peer_id || !role_id) return;
        currentSnapshot = await shell.execute(command, { peer_id, role_id });
        return;
      }
      case "member.ban":
      case "member.unban": {
        const peer_id = payload?.peer_id ?? window.prompt("Member peer ID");
        if (!peer_id) return;
        currentSnapshot = await shell.execute(command, { peer_id, reason: payload?.reason ?? "" });
        return;
      }
      case "invite.copy":
        if (shell.mode === "preview") {
          throw new Error(
            "Preview only; launch the desktop app to copy a usable invite.",
          );
        }
        await navigator.clipboard?.writeText(
          currentSnapshot.home?.invite?.space_invite_json ?? "",
        );
        appendActivity(currentSnapshot, "copied invite");
        return;
      case "ui.preference.set":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").SetUiPreferenceRequest} */ (payload),
        );
        return;
      case "workbench.layout.save":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").SetWorkbenchLayoutRequest} */ (payload),
        );
        return;
      case "workbench.layout.reset":
        currentSnapshot = await shell.execute(command, {});
        return;
      default:
        throw new Error(`No command handler is registered for ${command}`);
    }
  } finally {
    uiState.busyCommand = "";
    render();
    if (refreshQueued) queueMicrotask(() => publishRefresh().catch(reportError));
  }
}

/** @param {unknown} error */
function reportError(error) {
  uiState.busyCommand = "";
  uiState.error = errorMessage(error);
  appendActivity(currentSnapshot, `error: ${uiState.error}`);
  render();
}

/** @param {unknown} error */
function errorMessage(error) {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return String(error);
}

function errorBanner() {
  if (!uiState.error) {
    return document.createDocumentFragment();
  }
  return element("p", "error-banner", uiState.error);
}

function firstPeerRequest() {
  const peer = currentSnapshot.home?.peers[0];
  if (!peer) {
    throw new Error("no peer available");
  }
  return peerRequest(peer);
}

/** @param {import("./shell-contract").PeerListItemView} peer */
function peerRequest(peer) {
  return {
    peer_id: peer.peer_id,
    device_id: peer.device_id,
    max_events: 64,
  };
}

/** @param {import("./shell-contract").NetworkHealthStatus} status */
function statusLabel(status) {
  switch (status) {
    case "working":
      return "working";
    case "needs_attention":
      return "attention";
    case "broken":
      return "broken";
    case "unknown":
      return "unknown";
  }
}

/**
 * @param {string} label
 * @param {string} placeholder
 * @param {string} value
 * @param {(value: string) => void} onInput
 */
function labeledInput(label, placeholder, value, onInput) {
  const field = element("label", "field");
  const input = element("input", "");
  input.placeholder = placeholder;
  input.value = value;
  input.addEventListener("input", () => {
    onInput(input.value);
  });
  field.append(element("span", "", label), input);
  return field;
}

/** @param {string} value */
function blankToNull(value) {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

/** @param {File} file */
async function fileAsBase64(file) {
  const dataUrl = await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => resolve(String(reader.result)));
    reader.addEventListener("error", () => reject(reader.error ?? new Error("file read failed")));
    reader.readAsDataURL(file);
  });
  return String(dataUrl).split(",", 2)[1] ?? "";
}

/**
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 * @param {string} text
 */
function activityIncludes(snapshot, text) {
  return snapshot.service_activity.some((item) => item.summary.includes(text));
}

/**
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 * @param {string} summary
 */
function appendActivity(snapshot, summary) {
  const id = snapshot.service_activity.at(-1)?.id ?? 0;
  snapshot.service_activity.push({ id: id + 1, level: "info", summary });
}

/** @param {string} text */
function shortId(text) {
  const value = text.startsWith("ed25519:") ? text.slice(8) : text;
  return value.length > 12 ? `${value.slice(0, 12)}` : value;
}

/**
 * @param {keyof HTMLElementTagNameMap} tag
 * @param {string} className
 * @param {string} [text]
 */
function element(tag, className, text) {
  const el = document.createElement(tag);
  if (className) {
    el.className = className;
  }
  if (text !== undefined) {
    el.textContent = text;
  }
  return el;
}
