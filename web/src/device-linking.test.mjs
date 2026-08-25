import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { paletteCommandAvailability } from "./workbench.mjs";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

const baseContext = {
  hasHome: false,
  hasHomeError: false,
  runtimeOnline: false,
  hasInvite: false,
  hasKnownPeer: false,
  joinedCall: false,
  callFull: false,
  updateAuthenticationAvailable: true,
  hasAvailableUpdate: false,
  hasStagedUpdate: false,
  hasPreviousGeneration: false,
};

test("fresh setup exposes the complete same-identity device handoff", () => {
  assert.match(source, /Use my existing identity/);
  assert.match(source, /1\. Save approval request/);
  assert.match(source, /Devices → Add another device/);
  assert.match(source, /3\. Open authorization package/);
  assert.match(source, /This device is now you\. Your other authorized devices still work\./);
});

test("ordinary conversation exposes devices without hiding them under recovery", () => {
  assert.match(source, /utilityButton\("devices", `Devices ·/);
  assert.match(source, /Devices that can be you/);
  assert.match(source, /does not copy your root or offline recovery capability/);
  assert.match(source, /Recovery stays separate from linked devices/);
});

test("device-link commands preserve fresh-home and active-home prerequisites", () => {
  assert.equal(
    paletteCommandAvailability("identity.device.request", baseContext).available,
    true,
  );
  assert.equal(
    paletteCommandAvailability("identity.device.accept", baseContext).available,
    true,
  );
  assert.equal(
    paletteCommandAvailability("identity.device.approve", baseContext).available,
    false,
  );
  assert.equal(
    paletteCommandAvailability("identity.device.approve", {
      ...baseContext,
      hasHome: true,
    }).available,
    true,
  );
});
