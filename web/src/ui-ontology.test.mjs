import assert from "node:assert/strict";
import test from "node:test";
import { defaultUiOntology } from "./ui-ontology.fixture.mjs";
import {
  messageTimestamp,
  safeDateTime,
  ontologyPresentation,
  visibleActivity,
} from "./ui-ontology.mjs";

function ontologyWith(...changes) {
  const ontology = structuredClone(defaultUiOntology);
  for (const [collection, id, value] of changes) {
    const primitive = ontology[collection].find((item) => item.id === id);
    primitive.current_value = value;
  }
  return ontology;
}

test("saved appearance values become concrete rendered values", () => {
  const ontology = ontologyWith(
    ["semantic_tokens", "app.background", "#102030"],
    ["metrics", "sidebar.width", 444],
    ["metrics", "panel.padding", 21],
  );

  assert.deepEqual(ontologyPresentation(ontology).styles, {
    "--app-background": "#102030",
    "--panel-background": "Canvas",
    "--panel-border": "ButtonBorder",
    "--text-primary": "CanvasText",
    "--text-secondary": "GrayText",
    "--runtime-online": "#18794e",
    "--runtime-offline": "GrayText",
    "--peer-reachable": "#18794e",
    "--peer-unreachable": "#b42318",
    "--message-own-background": "#e8f1ff",
    "--message-remote-background": "#f2f2f2",
    "--activity-info": "LinkText",
    "--activity-error": "#b42318",
    "--sidebar-width": "444px",
    "--panel-padding": "21px",
    "--panel-gap": "8px",
    "--message-gap": "8px",
    "--message-max-width": "720px",
    "--avatar-size": "32px",
    "--activity-max-items": "30",
  });
});

test("invalid retained timestamps do not throw during rendering", () => {
  assert.equal(safeDateTime(Number.MIN_SAFE_INTEGER), null);
  assert.equal(safeDateTime(Date.UTC(2026, 0, 1)), "2026-01-01T00:00:00.000Z");
});

test("timestamp preferences change what a person sees", () => {
  const message = { created_ms: Date.UTC(2026, 0, 2, 3, 4, 5) };
  const hidden = ontologyWith([
    "behaviors",
    "timestamps.visible",
    { type: "bool", value: false },
  ]);
  assert.equal(messageTimestamp(message, hidden, message.created_ms, "en"), null);

  const relative = ontologyWith([
    "behaviors",
    "timestamps.style",
    { type: "text", value: "relative" },
  ]);
  assert.equal(messageTimestamp(message, relative, message.created_ms + 120_000, "en"), "2 minutes ago");

  const absolute = ontologyWith([
    "behaviors",
    "timestamps.style",
    { type: "text", value: "absolute" },
  ]);
  assert.match(messageTimestamp(message, absolute, 0, "en-US"), /2026/);
});

test("activity limit controls the visible newest entries", () => {
  const ontology = ontologyWith(["metrics", "activity.maxItems", 2]);
  const activities = [1, 2, 3].map((id) => ({ id, level: "info", summary: String(id) }));

  assert.deepEqual(visibleActivity(activities, ontology).map((item) => item.id), [3, 2]);
});
