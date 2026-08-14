/**
 * @typedef {import("./shell-contract").ShellSnapshotView} ShellSnapshotView
 */

export async function createShellClient() {
  const invoke = tauriInvoke();
  if (invoke) {
    return new TauriShellClient(invoke, "tauri");
  }
  const fixtureUrl = new URL("./fixture.js", import.meta.url);
  fixtureUrl.searchParams.set("preview", String(Date.now()));
  const { fixtureSnapshot } = await import(fixtureUrl.href);
  const snapshot = structuredClone(fixtureSnapshot);
  if (new URLSearchParams(window.location?.search ?? "").get("preview") === "fresh") {
    snapshot.home = null;
    snapshot.home_error = "This device does not have a Voxelle identity yet.";
  }
  return new PreviewShellClient(snapshot, "preview");
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

  async onSnapshotInvalidated(callback) {
    const listen = tauriListen();
    return listen ? listen("voxelle://snapshot-invalidated", callback) : () => {};
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

  async chooseRecoveryKitPath(mode) {
    return await this.invoke("choose_recovery_kit_path", { mode });
  }
}

class PreviewShellClient {
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
   * @returns {Promise<ShellSnapshotView>}
   */
  async execute(command) {
    if (command !== "shell.refresh") {
      throw new Error(
        `Preview only; launch the desktop app to run ${command}.`,
      );
    }
    return this.current;
  }

  async onSnapshotInvalidated() {
    return () => {};
  }

  async chooseRecoveryKitPath() {
    return null;
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

function tauriListen() {
  const maybeWindow =
    /** @type {Window & { __TAURI__?: { event?: { listen?: unknown } } }} */ (window);
  const listen = maybeWindow.__TAURI__?.event?.listen;
  return typeof listen === "function" ? listen : null;
}
