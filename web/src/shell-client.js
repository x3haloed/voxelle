import { fixtureSnapshot } from "./fixture.js";

/**
 * @typedef {import("./shell-contract").ShellSnapshotView} ShellSnapshotView
 * @typedef {import("./shell-contract").InitHomeRequest} InitHomeRequest
 * @typedef {import("./shell-contract").StartServiceRequest} StartServiceRequest
 * @typedef {import("./shell-contract").SendMessageRequest} SendMessageRequest
 * @typedef {import("./shell-contract").ImportPeerRecordRequest} ImportPeerRecordRequest
 * @typedef {import("./shell-contract").PeerCommandRequest} PeerCommandRequest
 */

export function createShellClient() {
  const invoke = tauriInvoke();
  if (invoke) {
    return new TauriShellClient(invoke, "tauri");
  }
  return new FixtureShellClient(structuredClone(fixtureSnapshot), "fixture");
}

class TauriShellClient {
  /**
   * @param {(command: string, args?: Record<string, unknown>) => Promise<unknown>} invoke
   * @param {string} mode
   */
  constructor(invoke, mode) {
    this.invoke = invoke;
    this.mode = mode;
  }

  /** @returns {Promise<ShellSnapshotView>} */
  snapshot() {
    return this.call("snapshot");
  }

  /** @param {InitHomeRequest} request */
  initHome(request) {
    return this.call("init_home", { request });
  }

  /** @param {StartServiceRequest} request */
  startService(request) {
    return this.call("start_service", { request });
  }

  stopService() {
    return this.call("stop_service");
  }

  /** @param {SendMessageRequest} request */
  sendMessage(request) {
    return this.call("send_message", { request });
  }

  /** @param {ImportPeerRecordRequest} request */
  importPeerRecord(request) {
    return this.call("import_peer_record", { request });
  }

  /** @param {PeerCommandRequest} request */
  diagnosePeer(request) {
    return this.call("diagnose_peer", { request });
  }

  /** @param {PeerCommandRequest} request */
  syncPeer(request) {
    return this.call("sync_peer", { request });
  }

  /**
   * @param {string} command
   * @param {Record<string, unknown>} [args]
   * @returns {Promise<ShellSnapshotView>}
   */
  async call(command, args) {
    return /** @type {ShellSnapshotView} */ (await this.invoke(command, args));
  }
}

class FixtureShellClient {
  /**
   * @param {ShellSnapshotView} snapshot
   * @param {string} mode
   */
  constructor(snapshot, mode) {
    this.current = snapshot;
    this.mode = mode;
  }

  async snapshot() {
    return this.current;
  }

  /** @param {InitHomeRequest} _request */
  async initHome(_request) {
    this.appendActivity("fixture init_home");
    return this.current;
  }

  /** @param {StartServiceRequest} _request */
  async startService(_request) {
    this.current.home && (this.current.home.runtime.state = "online");
    this.setHealth(
      "service",
      "working",
      "Resident service is online in fixture mode.",
    );
    this.appendActivity("fixture start_service");
    return this.current;
  }

  async stopService() {
    this.current.home && (this.current.home.runtime.state = "offline");
    this.setHealth(
      "service",
      "needs_attention",
      "Go online to accept peer diagnostics and sync requests.",
    );
    this.appendActivity("fixture stop_service");
    return this.current;
  }

  /** @param {SendMessageRequest} request */
  async sendMessage(request) {
    this.current.home?.room.messages.push({
      event_id: `fixture_${Date.now()}`,
      created_ms: Date.now(),
      author_peer_id: this.current.home.profile.peer_id,
      text: request.text,
    });
    this.appendActivity("fixture send_message");
    return this.current;
  }

  /** @param {ImportPeerRecordRequest} _request */
  async importPeerRecord(_request) {
    this.setHealth("peers", "working", "1 known peer record(s).");
    this.appendActivity("fixture import_peer_record");
    return this.current;
  }

  /** @param {PeerCommandRequest} _request */
  async diagnosePeer(_request) {
    this.appendActivity("fixture diagnostic reached peer");
    return this.current;
  }

  /** @param {PeerCommandRequest} _request */
  async syncPeer(_request) {
    this.appendActivity("fixture sync completed");
    return this.current;
  }

  /**
   * @param {string} id
   * @param {import("./shell-contract").NetworkHealthStatus} status
   * @param {string} summary
   */
  setHealth(id, status, summary) {
    const row = this.current.network_health.rows.find((item) => item.id === id);
    if (row) {
      row.status = status;
      row.summary = summary;
    }
  }

  /** @param {string} summary */
  appendActivity(summary) {
    const id = this.current.service_activity.at(-1)?.id ?? 0;
    this.current.service_activity.push({ id: id + 1, level: "info", summary });
  }
}

function tauriInvoke() {
  const maybeWindow =
    /** @type {Window & { __TAURI__?: { core?: { invoke?: unknown } }, __TAURI_INTERNALS__?: { invoke?: unknown } }} */ (window);
  const publicInvoke = maybeWindow.__TAURI__?.core?.invoke;
  if (typeof publicInvoke === "function") {
    return publicInvoke;
  }
  const internalInvoke = maybeWindow.__TAURI_INTERNALS__?.invoke;
  return typeof internalInvoke === "function" ? internalInvoke : null;
}
