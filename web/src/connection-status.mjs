/**
 * Keep operator-oriented setup checks available without presenting them as
 * failures in the ordinary conversation header.
 *
 * @param {{home?: {runtime?: {state?: string}} | null, network_health: {rows: Array<{status: string}>}}} snapshot
 */
export function connectionHeaderState(snapshot) {
  const rows = snapshot.network_health?.rows ?? [];
  const brokenCount = rows.filter((row) => row.status === "broken").length;
  const checkCount = rows.filter((row) => row.status === "needs_attention").length;
  const online = snapshot.home?.runtime?.state === "online";
  const stateLabel = online ? "Online" : "Offline";

  if (brokenCount > 0) {
    return {
      label: `${stateLabel} · ${brokenCount} problem${brokenCount === 1 ? "" : "s"}`,
      help: `${brokenCount} connection problem${brokenCount === 1 ? "" : "s"} need attention`,
      tone: "attention",
    };
  }

  return {
    label: stateLabel,
    help: checkCount > 0
      ? `${stateLabel}. ${checkCount} setup or verification check${checkCount === 1 ? "" : "s"} available`
      : "Review connection and synchronization health",
    tone: "working",
  };
}

export function connectionHealthLabel(status) {
  switch (status) {
    case "working":
      return "working";
    case "needs_attention":
      return "check";
    case "broken":
      return "broken";
    case "unknown":
      return "unknown";
  }
}
