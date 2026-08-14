import { createShellClient } from "./shell-client.js";
import { ProductComponentHost } from "./product-component-host.mjs";

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

app.textContent = "Loading your local Voxelle home…";
const initialSnapshot = await shell.execute("shell.refresh");
if (!initialSnapshot.product_component && shell.mode === "preview") {
  const moduleSources = await Promise.all([
    "./src/call-media.mjs",
    "./src/dom-reconcile.mjs",
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
