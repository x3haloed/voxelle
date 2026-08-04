import { createShellClient } from "./shell-client.js";
import {
  applyOntology,
  messageTimestamp,
  ontologyPresentation,
  visibleActivity,
} from "./ui-ontology.mjs";

const app = document.querySelector("#app");
const shell = createShellClient();

if (!(app instanceof HTMLElement)) {
  throw new Error("missing #app");
}

const uiState = {
  busyCommand: "",
  error: "",
  peerRecordDraft: "",
  messageDraft: "",
  bindDraft: "",
  advertiseDraft: "",
};

const viewRenderers = {
  "profile.summary": profileSummaryView,
  "runtime.status": runtimeStatusView,
  "network.health": networkHealthView,
  "field.test": fieldTestView,
  "invite.exchange": inviteExchangeView,
  "peer.list": peerListView,
  "room.timeline": roomTimelineView,
  "message.composer": messageComposerView,
  "service.activity": activityView,
};

let currentSnapshot = await shell.execute("shell.refresh");
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

async function refresh() {
  currentSnapshot = await shell.execute("shell.refresh");
  return currentSnapshot;
}

function render() {
  const presentation = applyOntology(
    document.documentElement,
    currentSnapshot.ui_ontology,
  );
  app.replaceChildren(
    header(currentSnapshot),
    workbenchShell(currentSnapshot),
  );
  if (presentation.activityAutoScroll) {
    const activity = app.querySelector(".activity-list");
    activity?.scrollTo?.({ top: activity.scrollHeight });
  }
}

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
  const shellEl = element("section", "workbench");
  const mainRegion = element("div", "workbench-region main-region");
  const sideRegion = element("aside", "workbench-region side-region");

  for (const view of snapshot.ui_ontology.views) {
    const panelEl = workbenchPanel(view, snapshot);
    if (sidePlace(view.place_id)) {
      sideRegion.append(panelEl);
    } else {
      mainRegion.append(panelEl);
    }
  }

  shellEl.append(mainRegion, sideRegion);
  return shellEl;
}

function sidePlace(placeId) {
  return placeId === "sidebar" || placeId === "activity" || placeId === "inspector";
}

/**
 * @param {import("./shell-contract").UiView} viewDefinition
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function workbenchPanel(viewDefinition, snapshot) {
  const section = element("section", "panel");
  section.dataset.panelId = `panel.${viewDefinition.id}`;
  section.dataset.viewId = viewDefinition.id;
  section.dataset.placeId = viewDefinition.place_id;
  section.append(panelHeader(viewDefinition));

  const view = element("div", "panel-view");
  const renderer = viewRenderers[viewDefinition.id] ?? unknownView;
  view.append(renderer(snapshot));
  section.append(view);
  return section;
}

/** @param {import("./shell-contract").UiView} viewDefinition */
function panelHeader(viewDefinition) {
  const headerEl = element("div", "panel-header");
  const titleGroup = element("div", "panel-title");
  titleGroup.append(element("h2", "", viewDefinition.label));
  titleGroup.append(element("span", "view-id", viewDefinition.id));
  headerEl.append(titleGroup);
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
  const invite = snapshot.home?.invite?.peer_record_json ?? "";
  const inviteGroup = element("div", "field-stack");
  inviteGroup.append(element("h3", "", "Local Invite"));
  inviteGroup.append(element("p", "summary", "After this peer is online, copy this JSON into another peer's Import Peer field."));
  inviteGroup.append(element("pre", "invite-json", invite));
  inviteGroup.append(commandButton("invite.copy"));

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
      status: snapshot.home?.invite ? "working" : "unknown",
      command: snapshot.home?.invite ? "invite.copy" : "runtime.goOnline",
      detail: snapshot.home?.invite?.peer_record.endpoint.addr ?? "go online to create an invite",
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
function roomTimelineView(snapshot) {
  const messages = snapshot.home?.room.messages ?? [];
  const list = element("ol", "message-list");
  for (const message of messages) {
    const own = message.author_peer_id === snapshot.home?.profile.peer_id;
    const row = element("li", own ? "message own" : "message remote");
    row.append(element("span", "muted", shortId(message.author_peer_id)));
    const timestamp = messageTimestamp(message, snapshot.ui_ontology);
    if (timestamp !== null) {
      const time = element("time", "message-time", timestamp);
      time.dateTime = new Date(message.created_ms).toISOString();
      row.append(time);
    }
    row.append(element("p", "", message.text));
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
  form.append(input, submitButton("message.send"));

  return form;
}

function unknownView() {
  return element("p", "summary", "Unknown view");
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
  button.disabled = uiState.busyCommand !== "";
  if (uiState.busyCommand === command) {
    button.textContent = "Working";
  }
  button.addEventListener("click", () => {
    runCommand(command, payload).catch(reportError);
  });
  return button;
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
          text: uiState.messageDraft,
          room: null,
        });
        uiState.messageDraft = "";
        return;
      case "invite.copy":
        if (shell.mode === "preview") {
          throw new Error(
            "Preview only; launch the desktop app to copy a usable invite.",
          );
        }
        await navigator.clipboard?.writeText(
          currentSnapshot.home?.invite?.peer_record_json ?? "",
        );
        appendActivity(currentSnapshot, "copied invite");
        return;
      case "ui.preference.set":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").SetUiPreferenceRequest} */ (payload),
        );
        return;
      default:
        appendActivity(currentSnapshot, `unhandled ${command}`);
    }
  } finally {
    uiState.busyCommand = "";
    render();
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
