import assert from "node:assert/strict";
import test from "node:test";
import { createShellClient } from "./shell-client.js";

test("standalone preview never claims a product command succeeded", async () => {
  globalThis.window = {};
  const client = createShellClient();
  const before = await client.execute("shell.refresh");
  const messageCount = before.home.room.messages.length;

  await assert.rejects(
    client.execute("message.send", { text: "not really sent", room: null }),
    /Preview only; launch the desktop app/,
  );

  const after = await client.execute("shell.refresh");
  assert.equal(client.mode, "preview");
  assert.equal(after.home.room.messages.length, messageCount);
  assert.equal(
    after.home.room.messages.some(({ text }) => text === "not really sent"),
    false,
  );
});

test("native shell subscribes to Rust snapshot invalidations", async () => {
  let eventName = "";
  let listener = null;
  globalThis.window = {
    __TAURI__: {
      core: {
        invoke: async () => ({ home: null }),
      },
      event: {
        listen: async (name, callback) => {
          eventName = name;
          listener = callback;
          return () => {};
        },
      },
    },
  };
  const client = createShellClient();
  let invalidated = false;
  await client.onSnapshotInvalidated(() => {
    invalidated = true;
  });

  assert.equal(client.mode, "tauri");
  assert.equal(eventName, "voxelle://snapshot-invalidated");
  listener();
  assert.equal(invalidated, true);
});
