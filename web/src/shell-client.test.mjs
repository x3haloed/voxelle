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
