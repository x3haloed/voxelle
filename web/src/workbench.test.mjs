import test from "node:test";
import assert from "node:assert/strict";

import {
  filterPaletteCommands,
  moveView,
  paletteCommandAvailability,
  placementsFromViews,
  setViewVisible,
  shiftView,
  shortcutMatches,
} from "./workbench.mjs";

const views = [
  { id: "a", place_id: "left", order: 0, visible: true },
  { id: "b", place_id: "left", order: 1, visible: true },
  { id: "c", place_id: "main", order: 0, visible: false },
];

test("every view remains placed exactly once while docking, ordering, and hiding", () => {
  const docked = moveView(views, "a", "main");
  assert.deepEqual(
    docked.filter((placement) => placement.place_id === "main").map((placement) => placement.view_id),
    ["a", "c"],
  );
  assert.deepEqual(
    docked.filter((placement) => placement.place_id === "main").map((placement) => placement.order),
    [1, 0],
  );
  assert.equal(new Set(docked.map((placement) => placement.view_id)).size, views.length);

  const shifted = shiftView(views, "b", -1);
  assert.equal(shifted.find((placement) => placement.view_id === "b").order, 0);
  assert.equal(shifted.find((placement) => placement.view_id === "a").order, 1);

  const hidden = setViewVisible(views, "b", false);
  assert.equal(hidden.find((placement) => placement.view_id === "b").visible, false);
  assert.equal(placementsFromViews(views).length, 3);
});

test("palette search and registry shortcuts use command metadata", () => {
  const commands = [
    { id: "peer.sync", label: "Sync Peer", description: "Exchange events", palette: true },
    { id: "layout.save", label: "Save Layout", description: "Internal", palette: false },
  ];
  assert.deepEqual(filterPaletteCommands(commands, "peer events").map((command) => command.id), ["peer.sync"]);
  assert.equal(filterPaletteCommands(commands, "layout").length, 0);
  assert(shortcutMatches(
    { key: "P", metaKey: true, ctrlKey: false, shiftKey: true, altKey: false },
    "Mod+Shift+P",
  ));
  assert(shortcutMatches(
    { key: "p", metaKey: false, ctrlKey: true, shiftKey: true, altKey: false },
    "Mod+Shift+P",
  ));
});

test("palette availability explains causal prerequisites without changing command ids", () => {
  const fresh = {
    hasHome: false,
    hasHomeError: false,
    runtimeOnline: false,
    hasInvite: false,
    joinedCall: false,
  };
  assert.deepEqual(paletteCommandAvailability("channel.create", fresh), {
    available: false,
    reason: "Create, join, or recover a space first",
  });
  assert.equal(paletteCommandAvailability("space.join", fresh).available, true);

  const active = {
    hasHome: true,
    hasHomeError: false,
    runtimeOnline: true,
    hasInvite: false,
    joinedCall: false,
  };
  assert.equal(paletteCommandAvailability("space.join", active).available, false);
  assert.equal(paletteCommandAvailability("identity.recovery.restore", active).available, false);
  assert.equal(paletteCommandAvailability("invite.copy", active).reason, "Create a signed invite first");
  assert.equal(paletteCommandAvailability("channel.create", active).available, true);
  assert.equal(
    paletteCommandAvailability("call.microphone.toggle", active).reason,
    "Join this room's call first",
  );
  assert.equal(
    paletteCommandAvailability("call.microphone.toggle", { ...active, joinedCall: true }).available,
    true,
  );
  assert.equal(
    paletteCommandAvailability("call.camera.toggle", active).reason,
    "Join this room's call first",
  );
  assert.equal(
    paletteCommandAvailability("call.camera.toggle", { ...active, joinedCall: true }).available,
    true,
  );
});
