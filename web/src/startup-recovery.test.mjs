import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { loadInitialSnapshotWithRetry } from "./startup-recovery.mjs";

const mainSource = readFileSync(new URL("./main.js", import.meta.url), "utf8");

test("initial snapshot loading waits for an explicit retry and returns the first success", async () => {
  let attempts = 0;
  const failures = [];
  const snapshot = await loadInitialSnapshotWithRetry(
    async () => {
      attempts += 1;
      if (attempts < 3) throw new Error(`failure ${attempts}`);
      return { ready: true };
    },
    async (error) => {
      failures.push(error.message);
    },
  );

  assert.deepEqual(snapshot, { ready: true });
  assert.equal(attempts, 3);
  assert.deepEqual(failures, ["failure 1", "failure 2"]);
});

test("startup failure renders a focused non-destructive retry action", () => {
  assert.match(mainSource, /retry\.textContent = "Try Again"/);
  assert.match(mainSource, /Trying again does not delete, archive, or replace local state/);
  assert.match(mainSource, /window\.requestAnimationFrame\(\(\) => retry\.focus\(\)\)/);
  assert.match(mainSource, /summary\.setAttribute\("role", "button"\)/);
  assert.match(mainSource, /summary\.setAttribute\("aria-expanded", String\(details\.open\)\)/);
  assert.match(mainSource, /loadInitialSnapshotWithRetry/);
  assert.doesNotMatch(mainSource, /catch \(error\) \{[\s\S]*throw error;[\s\S]*\} finally/);
});
