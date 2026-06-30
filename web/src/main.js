import { createShellClient } from "./shell-client.js";

const app = document.querySelector("#app");
const shell = createShellClient();

if (!(app instanceof HTMLElement)) {
  throw new Error("missing #app");
}

const workbench = {
  panels: [
    {
      id: "panel.home.profile",
      title: "Home",
      viewId: "home.profile",
      region: "side",
    },
    {
      id: "panel.network.health",
      title: "Network Health",
      viewId: "network.health",
      region: "main",
    },
    {
      id: "panel.network.log",
      title: "Network Log",
      viewId: "service.activity",
      region: "side",
    },
    {
      id: "panel.peer.exchange",
      title: "Peer Exchange",
      viewId: "peer.exchange",
      region: "side",
    },
    {
      id: "panel.field.test",
      title: "Field Test",
      viewId: "field.test",
      region: "side",
    },
    {
      id: "panel.room.timeline",
      title: "Room",
      viewId: "room.timeline",
      region: "main",
    },
  ],
};

const uiState = {
  busyCommand: "",
  error: "",
  peerRecordDraft: "",
  messageDraft: "",
  bindDraft: "",
  advertiseDraft: "",
};

const viewRenderers = {
  "home.profile": homeProfileView,
  "network.health": networkHealthView,
  "service.activity": activityView,
  "peer.exchange": peerExchangeView,
  "field.test": fieldTestView,
  "room.timeline": roomTimelineView,
};

let currentSnapshot = await shell.snapshot();
render();

async function refresh() {
  currentSnapshot = await shell.snapshot();
  return currentSnapshot;
}

function render() {
  app.replaceChildren(
    header(currentSnapshot),
    workbenchShell(currentSnapshot),
  );
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function header(snapshot) {
  const headerEl = element("header", "app-header");
  const titleGroup = element("div", "title-group");
  titleGroup.append(element("h1", "", "Voxelle"));
  titleGroup.append(element("p", "path", snapshot.home_root));

  const actions = element("div", "header-actions");
  actions.append(runtimeState(snapshot), commandButton("Refresh", "shell.refresh"));

  headerEl.append(titleGroup, actions);
  return headerEl;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function runtimeState(snapshot) {
  const runtime = snapshot.home?.runtime.state ?? "offline";
  return element("div", `runtime-state ${runtime}`, runtime);
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function workbenchShell(snapshot) {
  const shellEl = element("section", "workbench");
  const mainRegion = element("div", "workbench-region main-region");
  const sideRegion = element("aside", "workbench-region side-region");

  for (const panel of workbench.panels) {
    const panelEl = workbenchPanel(panel, snapshot);
    if (panel.region === "side") {
      sideRegion.append(panelEl);
    } else {
      mainRegion.append(panelEl);
    }
  }

  shellEl.append(mainRegion, sideRegion);
  return shellEl;
}

/**
 * @param {{ id: string, title: string, viewId: string }} panel
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function workbenchPanel(panel, snapshot) {
  const section = element("section", "panel");
  section.dataset.panelId = panel.id;
  section.dataset.viewId = panel.viewId;
  section.append(panelHeader(panel));

  const view = element("div", "panel-view");
  const renderer = viewRenderers[panel.viewId] ?? unknownView;
  view.append(renderer(snapshot));
  section.append(view);
  return section;
}

/** @param {{ id: string, title: string, viewId: string }} panel */
function panelHeader(panel) {
  const headerEl = element("div", "panel-header");
  const titleGroup = element("div", "panel-title");
  titleGroup.append(element("h2", "", panel.title));
  titleGroup.append(element("span", "view-id", panel.viewId));
  headerEl.append(titleGroup);
  return headerEl;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function homeProfileView(snapshot) {
  const fragment = document.createDocumentFragment();
  fragment.append(errorBanner());

  if (!snapshot.home) {
    const empty = element("div", "empty-state");
    empty.append(
      element("h3", "", "No initialized home"),
      element("p", "summary", snapshot.home_error ?? "Home state is not available."),
      commandButton("Initialize Home", "home.init"),
    );
    fragment.append(empty);
    return fragment;
  }

  const profile = snapshot.home.profile;
  const runtime = snapshot.home.runtime;
  const rows = [
    ["Home root", snapshot.home_root],
    ["Peer", profile.peer_id],
    ["Device", profile.device_id],
    ["Default room", profile.default_room],
    ["Authority", profile.authority_peer_id],
    ["Runtime", runtime.state],
    ["Listen", runtime.listen_addr ?? "not listening"],
    ["Advertise", runtime.advertised_addr ?? "not advertising"],
    ["Known peers", String(snapshot.home.peers.length)],
    ["Messages", String(snapshot.home.room.messages.length)],
  ];

  fragment.append(definitionGrid(rows));

  const controls = element("div", "control-row");
  controls.append(
    commandButton("Initialize", "home.init"),
    commandButton("Go Online", "runtime.goOnline"),
    commandButton("Go Offline", "runtime.goOffline"),
  );
  fragment.append(controls);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function networkHealthView(snapshot) {
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(
    commandButton("Initialize", "home.init"),
    commandButton("Go Online", "runtime.goOnline"),
    commandButton("Go Offline", "runtime.goOffline"),
  );
  fragment.append(errorBanner(), serviceOptions(), controls);

  const rows = element("ol", "health-list");
  for (const row of snapshot.network_health.rows) {
    rows.append(healthRow(row, snapshot));
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
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function healthRow(row, snapshot) {
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
    item.append(primaryAction(row.primary_action, snapshot));
  }
  return item;
}

/**
 * @param {string} command
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function primaryAction(command, snapshot) {
  const knownCommand = snapshot.ui_ontology.commands.find((item) => item.id === command);
  return commandButton(knownCommand?.label ?? command, command);
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function activityView(snapshot) {
  const fragment = document.createDocumentFragment();
  fragment.append(errorBanner());

  const actions = element("div", "control-row");
  actions.append(commandButton("Refresh", "shell.refresh"));
  fragment.append(actions);

  const list = element("ol", "activity-list");
  for (const activity of [...snapshot.service_activity].reverse()) {
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
function peerExchangeView(snapshot) {
  const fragment = document.createDocumentFragment();
  const invite = snapshot.home?.invite?.peer_record_json ?? "";
  const inviteGroup = element("div", "field-stack");
  inviteGroup.append(element("h3", "", "Local Invite"));
  inviteGroup.append(element("p", "summary", "After this peer is online, copy this JSON into another peer's Import Peer field."));
  inviteGroup.append(element("pre", "invite-json", invite));
  inviteGroup.append(commandButton("Copy Invite", "invite.copy"));

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
  importGroup.append(element("h3", "", "Import Peer"), textarea, submitButton("Import"));

  fragment.append(inviteGroup, importGroup, peerList(snapshot));
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
      item.append(commandButton(commandLabel(row.command, snapshot), row.command));
    }
    list.append(item);
  }

  fragment.append(list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function peerList(snapshot) {
  const peers = snapshot.home?.peers ?? [];
  const group = element("div", "field-stack");
  group.append(element("h3", "", "Known Peers"));

  const list = element("ol", "peer-list");
  for (const peer of peers) {
    const row = element("li", "peer-row");
    const body = element("div", "peer-body");
    body.append(element("strong", "", peer.label));
    body.append(element("span", "mono", peer.addr));
    body.append(element("span", "muted", shortId(peer.peer_id)));

    const actions = element("div", "row-actions");
    actions.append(
      commandButton("Diagnose", "peer.diagnose", peerRequest(peer)),
      commandButton("Sync", "peer.sync", peerRequest(peer)),
    );
    row.append(body, actions);
    list.append(row);
  }
  group.append(list);
  return group;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roomTimelineView(snapshot) {
  const fragment = document.createDocumentFragment();
  const messages = snapshot.home?.room.messages ?? [];
  const list = element("ol", "message-list");
  for (const message of messages) {
    const row = element("li", "");
    row.append(element("span", "muted", shortId(message.author_peer_id)));
    row.append(element("p", "", message.text));
    list.append(row);
  }

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
  form.append(input, submitButton("Send"));

  fragment.append(list, form);
  return fragment;
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
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function commandLabel(command, snapshot) {
  const fallback = {
    "home.init": "Initialize Home",
    "runtime.goOnline": "Go Online",
    "runtime.goOffline": "Go Offline",
    "peer.import": "Import Peer",
    "peer.diagnose": "Diagnose Peer",
    "peer.sync": "Sync Peer",
    "invite.copy": "Copy Invite",
    "message.send": "Send Message",
    "shell.refresh": "Refresh",
  };
  return snapshot.ui_ontology.commands.find((item) => item.id === command)?.label
    ?? fallback[command]
    ?? command;
}

/**
 * @param {string} label
 * @param {string} command
 * @param {unknown} [payload]
 */
function commandButton(label, command, payload) {
  const button = element("button", "command-button", label);
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

/** @param {string} label */
function submitButton(label) {
  const button = element("button", "command-button", label);
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
        currentSnapshot = await shell.initHome({ default_room: null });
        return;
      case "runtime.goOnline":
        currentSnapshot = await shell.startService({
          bind: blankToNull(uiState.bindDraft),
          advertise: blankToNull(uiState.advertiseDraft),
        });
        return;
      case "runtime.goOffline":
        currentSnapshot = await shell.stopService();
        return;
      case "peer.import":
        currentSnapshot = await shell.importPeerRecord({
          peer_record_json: uiState.peerRecordDraft,
        });
        uiState.peerRecordDraft = "";
        return;
      case "peer.diagnose":
        currentSnapshot = await shell.diagnosePeer(
          /** @type {import("./shell-contract").PeerCommandRequest} */ (
            payload ?? firstPeerRequest()
          ),
        );
        return;
      case "peer.sync":
        currentSnapshot = await shell.syncPeer(
          /** @type {import("./shell-contract").PeerCommandRequest} */ (
            payload ?? firstPeerRequest()
          ),
        );
        return;
      case "message.send":
        currentSnapshot = await shell.sendMessage({
          text: uiState.messageDraft,
          room: null,
        });
        uiState.messageDraft = "";
        return;
      case "invite.copy":
        await navigator.clipboard?.writeText(
          currentSnapshot.home?.invite?.peer_record_json ?? "",
        );
        appendActivity(currentSnapshot, "copied invite");
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
