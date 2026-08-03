import { fixtureSnapshot } from "./fixture.js";

/**
 * @typedef {import("./shell-contract").ShellSnapshotView} ShellSnapshotView
 * @typedef {import("./shell-contract").SendMessageRequest} SendMessageRequest
 * @typedef {import("./shell-contract").SetUiPreferenceRequest} SetUiPreferenceRequest
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

  /**
   * @param {string} command
   * @param {unknown} [payload]
   * @returns {Promise<ShellSnapshotView>}
   */
  async execute(command, payload = {}) {
    return /** @type {ShellSnapshotView} */ (
      await this.invoke("execute_shell_command", { commandId: command, payload })
    );
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

  /**
   * @param {string} command
   * @param {unknown} [payload]
   * @returns {Promise<ShellSnapshotView>}
   */
  async execute(command, payload = {}) {
    switch (command) {
      case "snapshot":
        break;
      case "init_home":
        this.appendActivity("fixture init_home");
        break;
      case "start_service":
        this.current.home && (this.current.home.runtime.state = "online");
        this.setHealth(
          "service",
          "working",
          "Resident service is online in fixture mode.",
        );
        this.appendActivity("fixture start_service");
        break;
      case "stop_service":
        this.current.home && (this.current.home.runtime.state = "offline");
        this.setHealth(
          "service",
          "needs_attention",
          "Go online to accept peer diagnostics and sync requests.",
        );
        this.appendActivity("fixture stop_service");
        break;
      case "send_message": {
        const request = /** @type {SendMessageRequest} */ (payload);
        this.current.home?.room.messages.push({
          event_id: `fixture_${Date.now()}`,
          created_ms: Date.now(),
          author_peer_id: this.current.home.profile.peer_id,
          text: request.text,
        });
        this.appendActivity("fixture send_message");
        break;
      }
      case "import_peer_record":
        this.setHealth("peers", "working", "1 known peer record(s).");
        this.appendActivity("fixture import_peer_record");
        break;
      case "diagnose_peer":
        this.appendActivity("fixture diagnostic reached peer");
        break;
      case "sync_peer":
        this.appendActivity("fixture sync completed");
        break;
      case "set_ui_preference": {
        const request = /** @type {SetUiPreferenceRequest} */ (payload);
        const { semantic_tokens: tokens, metrics, behaviors } = this.current.ui_ontology;
        const collection = request.kind === "semantic_token"
          ? tokens
          : request.kind === "metric"
            ? metrics
            : behaviors;
        const preference = collection.find((item) => item.id === request.id);
        if (!preference) {
          throw new Error(`unknown UI preference ${request.id}`);
        }
        preference.current_value = request.value;
        this.appendActivity(`updated UI preference ${request.id}`);
        break;
      }
      default:
        throw new Error(`unknown shell command ${command}`);
    }
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
