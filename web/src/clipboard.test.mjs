import test from "node:test";
import assert from "node:assert/strict";
import { copyTextToClipboard } from "./clipboard.mjs";

test("clipboard success waits for the native write", async () => {
  let copied = "";
  await copyTextToClipboard({
    async writeText(value) {
      copied = value;
    },
  }, "signed invite");
  assert.equal(copied, "signed invite");
});

test("missing clipboard never masquerades as success", async () => {
  await assert.rejects(
    copyTextToClipboard(undefined, "signed invite"),
    (error) => error.recovery === "needs_human" && /manually/.test(error.recovery_message),
  );
});

test("clipboard rejection preserves human recovery and technical detail", async () => {
  await assert.rejects(
    copyTextToClipboard({
      async writeText() {
        throw new Error("permission denied");
      },
    }, "signed invite"),
    (error) => (
      error.message === "Voxelle could not copy the signed invite."
      && /Signed invite details/.test(error.recovery_message)
      && /permission denied/.test(error.detail)
    ),
  );
});
