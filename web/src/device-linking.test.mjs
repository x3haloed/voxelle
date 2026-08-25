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
  assert.match(source, /Use my existing identity on this device/);
  assert.match(source, /1\. Save request on this device/);
  assert.match(source, /Your devices → Approve a new device request/);
  assert.match(source, /3\. Open authorization package here/);
  assert.match(source, /This device is now you\. Your other authorized devices still work\./);
});

test("fresh setup separates device linking from destructive recovery", () => {
  assert.match(source, /Lost access to every authorized device\?/);
  assert.match(source, /This is not device linking/);
  assert.match(source, /recovery rotates authority to this device and revokes the devices that were previously authorized/);
});

test("ordinary conversation exposes devices without hiding them under recovery", () => {
  assert.match(source, /utilityButton\("devices", `Your devices ·/);
  assert.match(source, /Use Voxelle on another device/);
  assert.match(source, /Approve a new device request…/);
  assert.match(source, /Recovery kits are not used for linking/);
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
