import test from "node:test";
import assert from "node:assert/strict";
import { signedArtifactPreview } from "./signed-artifact-preview.mjs";

test("product package preview exposes bounded untrusted release claims", () => {
  assert.deepEqual(signedArtifactPreview(JSON.stringify({
    format: "voxelle-product-update/v1",
    release_id: "v0.1.0-beta.4",
    sequence: 4,
    channel: "beta",
    min_kernel_version: "0.1.0",
    signer_key_id: "release-primary",
  }), "package"), {
    state: "claims",
    recognizedFormat: true,
    releaseId: "v0.1.0-beta.4",
    sequence: 4,
    channel: "beta",
    minKernelVersion: "0.1.0",
    signerKeyId: "release-primary",
  });
});

test("trust transition preview exposes sequence and key-set changes", () => {
  assert.deepEqual(signedArtifactPreview(JSON.stringify({
    format: "voxelle-release-trust-transition/v1",
    sequence: 3,
    signer_key_id: "release-primary",
    add: [{ key_id: "release-next" }],
    remove_key_ids: ["release-old"],
  }), "trust"), {
    state: "claims",
    recognizedFormat: true,
    sequence: 3,
    signerKeyId: "release-primary",
    addCount: 1,
    removeCount: 1,
  });
});

test("unknown formats remain visible while malformed and oversized input stays bounded", () => {
  assert.equal(signedArtifactPreview("{", "package").state, "unavailable");
  assert.equal(signedArtifactPreview(JSON.stringify({ format: "other" }), "package").recognizedFormat, false);
  assert.match(signedArtifactPreview("x".repeat(64 * 1024 + 1), "trust").reason, /too large/);
});
