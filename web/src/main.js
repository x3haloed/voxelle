import { createShellClient } from "./shell-client.js";
import { presentShellError } from "./error-presentation.mjs";
import { ProductComponentHost } from "./product-component-host.mjs";
import { loadInitialSnapshotWithRetry } from "./startup-recovery.mjs";

const app = document.querySelector("#app");
if (!(app instanceof HTMLElement)) throw new Error("missing #app");

app.textContent = "Connecting to the local Voxelle runtime…";
const shell = await createShellClient();
const componentApi = Object.freeze({
  shell,
  app,
});

const componentHost = new ProductComponentHost(componentApi);
let transition = Promise.resolve();

async function activate(snapshot) {
  await componentHost.activate(snapshot.product_component);
}

function scheduleActivation(snapshot) {
  transition = transition.then(() => activate(snapshot)).catch(async (error) => {
    if (!componentHost.disposeActive) {
      app.textContent = `Voxelle product component failed: ${error?.message ?? String(error)}`;
    }
    if (snapshot.product_generation.previous_available && snapshot.product_generation.active_sequence > 0) {
      await shell.execute("product.update.rollback").catch(() => {});
    }
  });
  return transition;
}

async function executeInitialSnapshotLoad() {
  app.textContent = "Loading your local Voxelle home…";
  const credentialWaitNotice = window.setTimeout(() => {
    app.textContent = "Opening your local Voxelle identity… If your operating system asks whether Voxelle may access its saved credential, approve it to continue.";
  }, 1200);
  try {
    return await shell.execute("shell.refresh");
  } finally {
    window.clearTimeout(credentialWaitNotice);
  }
}

function waitForStartupRetry(error) {
  const presentation = presentShellError(error);
  app.dataset.fatalHandled = "true";
  const alert = document.createElement("section");
  alert.className = "startup-error";
  alert.setAttribute("role", "alert");
  const heading = document.createElement("h1");
  heading.textContent = presentation.message;
  const recovery = document.createElement("p");
  recovery.textContent = presentation.recoveryMessage;
  const safety = document.createElement("p");
  safety.textContent = "Trying again does not delete, archive, or replace local state.";
  const retry = document.createElement("button");
  retry.type = "button";
  retry.textContent = "Try Again";
  retry.setAttribute("aria-description", presentation.recoveryMessage);
  alert.append(heading, recovery, safety);
  if (presentation.detail) {
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.textContent = "Technical details";
    summary.setAttribute("role", "button");
    summary.setAttribute("aria-expanded", "false");
    summary.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      details.open = !details.open;
    });
    details.addEventListener("toggle", () => {
      summary.setAttribute("aria-expanded", String(details.open));
    });
    const detail = document.createElement("pre");
    detail.textContent = presentation.detail;
    details.append(summary, detail);
    alert.append(details);
  }
  alert.append(retry);
  app.replaceChildren(alert);
  window.requestAnimationFrame(() => retry.focus());
  return new Promise((resolve) => {
    retry.addEventListener("click", resolve, { once: true });
  });
}

const initialSnapshot = await loadInitialSnapshotWithRetry(
  executeInitialSnapshotLoad,
  waitForStartupRetry,
);
delete app.dataset.fatalHandled;
if (!initialSnapshot.product_component && shell.mode === "preview") {
  const moduleSources = await Promise.all([
    "./src/call-media.mjs",
    "./src/clipboard.mjs",
    "./src/command-progress.mjs",
    "./src/connection-status.mjs",
    "./src/dom-reconcile.mjs",
    "./src/error-presentation.mjs",
    "./src/focus-management.mjs",
    "./src/form-draft.mjs",
    "./src/invite-preview.mjs",
    "./src/message-composition.mjs",
    "./src/product-update-confirmation.mjs",
    "./src/signed-artifact-preview.mjs",
    "./src/ui-ontology.mjs",
    "./src/workbench.mjs",
  ].map((url) => fetch(url, { cache: "no-store" }).then((response) => response.text())));
  const productSource = await fetch("./src/product-component.js", { cache: "no-store" })
    .then((response) => response.text());
  initialSnapshot.product_component = {
    api_version: 1,
    digest: "preview-builtin",
    source: `${moduleSources.map((source) => source.replaceAll("export ", "")).join("\n")}\n${productSource}`,
    styles: await fetch("./src/styles.css", { cache: "no-store" }).then((response) => response.text()),
  };
}
app.textContent = "Preparing your workspace…";
await scheduleActivation(initialSnapshot);
await shell.onSnapshotInvalidated(async () => {
  const snapshot = await shell.execute("shell.refresh");
  if (snapshot.product_component.digest !== componentHost.activeDigest) {
    await scheduleActivation(snapshot);
  }
});
