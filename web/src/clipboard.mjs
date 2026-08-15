export async function copyTextToClipboard(clipboard, text) {
  if (!clipboard || typeof clipboard.writeText !== "function") {
    throw {
      message: "Voxelle cannot access the clipboard in this installation.",
      recovery: "needs_human",
      recovery_message: "Open Signed invite details, select the complete JSON, and copy it manually.",
      detail: "Clipboard API writeText is unavailable.",
    };
  }
  try {
    await clipboard.writeText(text);
  } catch (error) {
    throw {
      message: "Voxelle could not copy the signed invite.",
      recovery: "needs_human",
      recovery_message: "Allow clipboard access if your system asks, or copy the complete JSON from Signed invite details.",
      detail: `Clipboard write failed: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}
