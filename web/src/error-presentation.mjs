/** @param {unknown} error */
export function presentShellError(error) {
  if (error && typeof error === "object" && "message" in error) {
    const structured = /** @type {{message?: unknown, recovery?: unknown, recovery_message?: unknown, detail?: unknown}} */ (error);
    return {
      message: String(structured.message ?? "Voxelle could not complete that action."),
      recovery: typeof structured.recovery === "string" ? structured.recovery : "internal_error",
      recoveryMessage: typeof structured.recovery_message === "string"
        ? structured.recovery_message
        : "Try once more. If it repeats, retain the technical details for a bug report.",
      detail: typeof structured.detail === "string" ? structured.detail : "",
    };
  }
  const message = error instanceof Error ? error.message : String(error);
  return {
    message,
    recovery: "internal_error",
    recoveryMessage: "Try once more. If it repeats, retain the technical details for a bug report.",
    detail: "",
  };
}
