import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  FocusSurfaceCoordinator,
  focusableElements,
  trapModalTab,
} from "./focus-management.mjs";

const focusSource = readFileSync(new URL("./focus-management.mjs", import.meta.url), "utf8");

function fixture() {
  const document = { activeElement: null };
  const make = (name, {
    hidden = false,
    rendered = true,
    closedDetails = false,
    summary = false,
  } = {}) => {
    const details = closedDetails ? {} : null;
    return {
      name,
      tagName: summary ? "SUMMARY" : "BUTTON",
      parentElement: summary ? details : null,
      getAttribute(attribute) {
        return attribute === "aria-hidden" && hidden ? "true" : null;
      },
      closest(selector) {
        return selector === "details:not([open])" ? details : null;
      },
      getClientRects() {
        return rendered ? [{}] : [];
      },
      focus() {
        document.activeElement = this;
      },
    };
  };
  const first = make("first");
  const hidden = make("hidden", { hidden: true });
  const collapsed = make("collapsed", { closedDetails: true });
  const summary = make("summary", { closedDetails: true, summary: true });
  const last = make("last");
  const container = {
    ownerDocument: document,
    querySelectorAll() {
      return [first, hidden, collapsed, summary, last];
    },
    contains(element) {
      return [first, hidden, collapsed, summary, last].includes(element);
    },
    focus() {
      document.activeElement = this;
    },
  };
  return { document, first, summary, last, container };
}

test("focusable elements omit accessibility-hidden controls", () => {
  const { first, summary, last, container } = fixture();
  assert.deepEqual(focusableElements(container), [first, summary, last]);
});

test("native disclosure summaries remain in the modal focus order", () => {
  assert.match(focusSource, /"summary",/);
  assert.match(focusSource, /details:not\(\[open\]\)/);
});

test("Tab wraps forward and backward inside a modal", () => {
  const { document, first, last, container } = fixture();
  let prevented = false;
  document.activeElement = last;
  assert.equal(trapModalTab({
    key: "Tab",
    shiftKey: false,
    preventDefault() { prevented = true; },
  }, container), true);
  assert.equal(prevented, true);
  assert.equal(document.activeElement, first);

  document.activeElement = first;
  assert.equal(trapModalTab({
    key: "Tab",
    shiftKey: true,
    preventDefault() {},
  }, container), true);
  assert.equal(document.activeElement, last);
});

test("Tab remains native while focus moves within the modal", () => {
  const { document, first, container } = fixture();
  document.activeElement = first;
  assert.equal(trapModalTab({
    key: "Tab",
    shiftKey: false,
    preventDefault() { throw new Error("must not prevent ordinary movement"); },
  }, container), false);
});

test("focus enters once and returns to the invoking control after close", () => {
  const frames = [];
  const origin = { isConnected: true, focusCount: 0, focus() { this.focusCount += 1; } };
  const initial = { focusCount: 0, focus() { this.focusCount += 1; } };
  const document = { activeElement: origin };
  const coordinator = new FocusSurfaceCoordinator(document, (callback) => frames.push(callback));
  coordinator.rememberReturnElement();
  coordinator.synchronize("palette", () => initial);
  frames.shift()();
  assert.equal(initial.focusCount, 1);
  coordinator.synchronize("palette", () => initial);
  assert.equal(frames.length, 0);
  coordinator.synchronize("", () => null);
  frames.shift()();
  assert.equal(origin.focusCount, 1);
});

test("switching surfaces does not restore focus behind the new surface", () => {
  const frames = [];
  const origin = { isConnected: true, focusCount: 0, focus() { this.focusCount += 1; } };
  const next = { focusCount: 0, focus() { this.focusCount += 1; } };
  const coordinator = new FocusSurfaceCoordinator(
    { activeElement: origin },
    (callback) => frames.push(callback),
  );
  coordinator.rememberReturnElement();
  coordinator.synchronize("palette", () => null);
  frames.shift()();
  coordinator.synchronize("", () => null);
  coordinator.synchronize("utility", () => next);
  for (const frame of frames.splice(0)) frame();
  assert.equal(origin.focusCount, 0);
  assert.equal(next.focusCount, 1);
});

test("command completion restores its surviving origin unless a surface opened", () => {
  const frames = [];
  const origin = { isConnected: true, focusCount: 0, focus() { this.focusCount += 1; } };
  const coordinator = new FocusSurfaceCoordinator(
    { activeElement: origin },
    (callback) => frames.push(callback),
  );
  assert.equal(coordinator.currentElement(), origin);
  coordinator.restoreWhenNoSurface(origin);
  frames.shift()();
  assert.equal(origin.focusCount, 1);

  coordinator.synchronize("utility", () => null);
  frames.shift()();
  coordinator.restoreWhenNoSurface(origin);
  frames.shift()();
  assert.equal(origin.focusCount, 1);

  coordinator.synchronize("", () => null);
  origin.isConnected = false;
  coordinator.restoreWhenNoSurface(origin);
  frames.shift()();
  assert.equal(origin.focusCount, 1);
});

test("command completion uses a stable fallback when its origin disappeared", () => {
  const frames = [];
  const origin = { isConnected: false, focusCount: 0, focus() { this.focusCount += 1; } };
  const fallback = { focusCount: 0, focus() { this.focusCount += 1; } };
  const coordinator = new FocusSurfaceCoordinator(
    { activeElement: origin },
    (callback) => frames.push(callback),
  );
  coordinator.restoreWhenNoSurface(origin, () => fallback);
  frames.shift()();
  assert.equal(origin.focusCount, 0);
  assert.equal(fallback.focusCount, 1);

  coordinator.synchronize("utility", () => null);
  frames.shift()();
  coordinator.restoreWhenNoSurface(origin, () => fallback);
  frames.shift()();
  assert.equal(fallback.focusCount, 1);
});

test("document roots are not treated as useful command origins", () => {
  const body = { focus() {} };
  const documentElement = { focus() {} };
  const document = { activeElement: body, body, documentElement };
  const coordinator = new FocusSurfaceCoordinator(document, () => {});
  assert.equal(coordinator.currentElement(), null);
  document.activeElement = documentElement;
  assert.equal(coordinator.currentElement(), null);

  const button = { focus() {} };
  document.activeElement = button;
  assert.equal(coordinator.currentElement(), button);
});
