import { fixtureSnapshot } from "./fixture.js";

const app = document.querySelector("#app");

if (!(app instanceof HTMLElement)) {
  throw new Error("missing #app");
}

renderShell(app, fixtureSnapshot);

/**
 * @param {HTMLElement} root
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function renderShell(root, snapshot) {
  root.replaceChildren(
    header(snapshot),
    networkHealth(snapshot),
    secondarySurface(snapshot),
  );
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function header(snapshot) {
  const headerEl = element("header", "app-header");
  const titleGroup = element("div", "title-group");
  titleGroup.append(element("h1", "", "Voxelle"));
  titleGroup.append(element("p", "path", snapshot.home_root));

  const runtime = snapshot.home?.runtime.state ?? "offline";
  const state = element("div", `runtime-state ${runtime}`, runtime);

  headerEl.append(titleGroup, state);
  return headerEl;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function networkHealth(snapshot) {
  const section = element("section", "network-health");
  section.append(sectionHeader("Network Health"));

  const rows = element("ol", "health-list");
  for (const row of snapshot.network_health.rows) {
    rows.append(healthRow(row, snapshot));
  }

  section.append(rows);
  return section;
}

/**
 * @param {import("./shell-contract").NetworkHealthRow} row
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function healthRow(row, snapshot) {
  const item = element("li", "health-row");
  item.dataset.status = row.status;

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

  const action = primaryAction(row, snapshot);
  item.append(indicator, body);
  if (action) {
    item.append(action);
  }
  return item;
}

/**
 * @param {import("./shell-contract").NetworkHealthRow} row
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function primaryAction(row, snapshot) {
  if (!row.primary_action) {
    return null;
  }
  const command = snapshot.ui_ontology.commands.find((item) => item.id === row.primary_action);
  const button = element("button", "primary-action", command?.label ?? row.primary_action);
  button.type = "button";
  button.dataset.command = row.primary_action;
  button.addEventListener("click", () => {
    appendActivity(snapshot, `queued ${row.primary_action}`);
    renderShell(app, snapshot);
  });
  return button;
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

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function secondarySurface(snapshot) {
  const grid = element("section", "secondary-grid");
  grid.append(activityPanel(snapshot), peerPanel(snapshot), invitePanel(snapshot), roomPanel(snapshot));
  return grid;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function activityPanel(snapshot) {
  const section = element("section", "panel");
  section.append(sectionHeader("Activity"));

  const list = element("ol", "activity-list");
  for (const activity of [...snapshot.service_activity].reverse()) {
    const row = element("li", "");
    row.dataset.level = activity.level;
    row.append(element("span", "activity-id", String(activity.id)), element("span", "", activity.summary));
    list.append(row);
  }
  section.append(list);
  return section;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function peerPanel(snapshot) {
  const section = element("section", "panel");
  section.append(sectionHeader("Peers"));

  const peers = snapshot.home?.peers ?? [];
  const list = element("ol", "peer-list");
  for (const peer of peers) {
    const row = element("li", "peer-row");
    row.append(element("strong", "", peer.label));
    row.append(element("span", "mono", peer.addr));
    row.append(element("span", "muted", shortId(peer.peer_id)));
    list.append(row);
  }
  section.append(list);
  return section;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function invitePanel(snapshot) {
  const section = element("section", "panel invite-panel");
  section.append(sectionHeader("Invite"));

  const invite = snapshot.home?.invite?.peer_record_json ?? "";
  const pre = element("pre", "invite-json", invite);
  section.append(pre);
  return section;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roomPanel(snapshot) {
  const section = element("section", "panel room-panel");
  section.append(sectionHeader(snapshot.home?.room.room_id ?? "Room"));

  const messages = snapshot.home?.room.messages ?? [];
  const list = element("ol", "message-list");
  for (const message of messages) {
    const row = element("li", "");
    row.append(element("span", "muted", shortId(message.author_peer_id)));
    row.append(element("p", "", message.text));
    list.append(row);
  }
  section.append(list);
  return section;
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

/** @param {string} title */
function sectionHeader(title) {
  const headerEl = element("div", "section-header");
  headerEl.append(element("h2", "", title));
  return headerEl;
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
