import assert from "node:assert/strict";
import test from "node:test";
import { ProductComponentHost } from "./product-component-host.mjs";

function fakeDocument() {
  const children = [];
  return {
    children,
    head: {
      append(node) {
        if (!children.includes(node)) children.push(node);
        node.connected = true;
      },
    },
    createElement() {
      const node = {
        dataset: {},
        textContent: "",
        connected: false,
        remove() {
          const index = children.indexOf(node);
          if (index >= 0) children.splice(index, 1);
          node.connected = false;
        },
      };
      return node;
    },
  };
}

function component(digest, source, styles = `.${digest} { color: red; }`) {
  return { api_version: 1, digest, source, styles };
}

test("activation replaces executable behavior and styles and disposes the old generation", async () => {
  const events = [];
  const documentObject = fakeDocument();
  const host = new ProductComponentHost({ events }, documentObject);
  await host.activate(component("a", 'api.events.push("mount-a"); return async () => api.events.push("dispose-a");'));
  await host.activate(component("b", 'api.events.push("mount-b"); return async () => api.events.push("dispose-b");'));

  assert.deepEqual(events, ["mount-a", "dispose-a", "mount-b"]);
  assert.equal(host.activeDigest, "b");
  assert.equal(documentObject.children.length, 1);
  assert.equal(documentObject.children[0].dataset.voxelleProductComponent, "b");
});

test("syntax failure leaves the running generation untouched", async () => {
  const events = [];
  const documentObject = fakeDocument();
  const host = new ProductComponentHost({ events }, documentObject);
  await host.activate(component("a", 'api.events.push("mount-a"); return async () => api.events.push("dispose-a");'));

  await assert.rejects(host.activate(component("broken", "return }")), SyntaxError);
  assert.deepEqual(events, ["mount-a"]);
  assert.equal(host.activeDigest, "a");
  assert.equal(documentObject.children[0].dataset.voxelleProductComponent, "a");
});

test("mount failure remounts the previous executable generation", async () => {
  const events = [];
  const documentObject = fakeDocument();
  const host = new ProductComponentHost({ events }, documentObject);
  await host.activate(component("a", 'api.events.push("mount-a"); return async () => api.events.push("dispose-a");'));

  await assert.rejects(
    host.activate(component("bad", 'api.events.push("mount-bad"); throw new Error("bad mount");')),
    /bad mount/,
  );
  assert.deepEqual(events, ["mount-a", "dispose-a", "mount-bad", "mount-a"]);
  assert.equal(host.activeDigest, "a");
  assert.equal(documentObject.children[0].dataset.voxelleProductComponent, "a");
});
