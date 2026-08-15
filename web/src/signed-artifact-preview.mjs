const UPDATE_FORMAT = "voxelle-product-update/v1";
const TRUST_FORMAT = "voxelle-release-trust-transition/v1";

export function signedArtifactPreview(text, kind) {
  if (typeof text !== "string" || !text.trim()) return { state: "empty" };
  const maxCharacters = kind === "trust" ? 64 * 1024 : 1024 * 1024;
  if (text.length > maxCharacters) {
    return { state: "unavailable", reason: `Signed ${kind === "trust" ? "trust transition" : "update package"} text is too large to preview safely.` };
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    return { state: "unavailable", reason: "The selected artifact is not complete JSON yet." };
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { state: "unavailable", reason: "The selected artifact does not contain a JSON object." };
  }
  const stringClaim = (candidate, fallback) => (
    typeof candidate === "string" && candidate.trim() && candidate.length <= 256
      ? candidate
      : fallback
  );
  const sequence = Number.isSafeInteger(value.sequence) && value.sequence >= 0
    ? value.sequence
    : null;
  if (kind === "trust") {
    return {
      state: "claims",
      recognizedFormat: value.format === TRUST_FORMAT,
      sequence,
      signerKeyId: stringClaim(value.signer_key_id, "Unrecognized signer"),
      addCount: Array.isArray(value.add) ? value.add.length : null,
      removeCount: Array.isArray(value.remove_key_ids) ? value.remove_key_ids.length : null,
    };
  }
  return {
    state: "claims",
    recognizedFormat: value.format === UPDATE_FORMAT,
    releaseId: stringClaim(value.release_id, "Unrecognized release"),
    sequence,
    channel: stringClaim(value.channel, "Unrecognized channel"),
    minKernelVersion: stringClaim(value.min_kernel_version, "Unrecognized kernel requirement"),
    signerKeyId: stringClaim(value.signer_key_id, "Unrecognized signer"),
  };
}
