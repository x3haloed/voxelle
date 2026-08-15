import assert from "node:assert/strict";
import test from "node:test";
import { createShellClient } from "./shell-client.js";

test("standalone preview never claims a product command succeeded", async () => {
  globalThis.window = {};
  const client = await createShellClient();
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

test("fresh preview exposes the uninitialized human path without simulating authority", async () => {
  globalThis.window = { location: { search: "?preview=fresh" } };
  const client = await createShellClient();
  const snapshot = await client.execute("shell.refresh");

  assert.equal(snapshot.home, null);
  assert.equal(snapshot.home_error, null);
  await assert.rejects(client.execute("home.init"), /Preview only/);
});

test("damaged preview exposes structured recovery without simulating archival", async () => {
  globalThis.window = { location: { search: "?preview=damaged" } };
  const client = await createShellClient();
  const snapshot = await client.execute("shell.refresh");

  assert.equal(snapshot.home, null);
  assert.equal(snapshot.home_error.message, "The encrypted local identity could not be opened.");
  assert.match(snapshot.home_error.recovery_message, /offline recovery kit/);
  assert.match(snapshot.home_error.detail, /bounded preview/);
  await assert.rejects(client.execute("home.archiveForRecovery"), /Preview only/);
});

test("native shell subscribes to Rust snapshot invalidations", async () => {
  let eventName = "";
  let listener = null;
  const invokes = [];
  globalThis.window = {
    __TAURI__: {
      core: {
        invoke: async (command, args) => {
          invokes.push([command, args]);
          return command === "choose_recovery_kit_path"
            ? "/offline/identity.voxrecover"
            : { home: null };
        },
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
  const client = await createShellClient();
  let invalidated = false;
  await client.onSnapshotInvalidated(() => {
    invalidated = true;
  });

  assert.equal(client.mode, "tauri");
  assert.equal(eventName, "voxelle://snapshot-invalidated");
  listener();
  assert.equal(invalidated, true);
  assert.equal(
    await client.chooseRecoveryKitPath("save"),
    "/offline/identity.voxrecover",
  );
  assert.deepEqual(invokes.at(-1), [
    "choose_recovery_kit_path",
    { mode: "save" },
  ]);
});
