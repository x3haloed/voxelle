export function coordinationFrontierStatements(item, localPeerId, displayName, formatTime) {
  const statements = [];
  if (item.relevance.includes("mention_without_local_disposition")) {
    statements.push("Mentioned you; no response is recorded.");
  }
  if (item.relevance.includes("reply_after_local_disposition")) {
    statements.push("A newer reply may change what happens next.");
  }
  for (const acknowledgement of item.acknowledgements) {
    const who = acknowledgement.peer_id === localPeerId
      ? "You"
      : displayName(acknowledgement.peer_id);
    statements.push(`${who} marked this ${acknowledgement.state === "handled" ? "handled" : "seen"}.`);
    if (acknowledgement.result_event_ids.length > 0) {
      statements.push(`${who} linked ${acknowledgement.result_event_ids.length === 1 ? "a result" : "results"}.`);
    }
    if (acknowledgement.result_conflict) statements.push(`${who} linked conflicting results.`);
  }
  for (const continuation of item.continuations) {
    const who = continuation.peer_id === localPeerId
      ? "You"
      : displayName(continuation.peer_id);
    switch (continuation.state) {
      case "continuing":
        statements.push(`${who} said ${who === "You" ? "you’re" : "they’re"} continuing until ${formatTime(continuation.expires_ms)}.`);
        break;
      case "released":
        statements.push(`${who} released this exchange.`);
        break;
      case "declined":
        statements.push(`${who} declined to continue.`);
        break;
      case "conflict":
        statements.push(`${who} has conflicting continuation updates.`);
        break;
      default:
        if (continuation.overdue) statements.push(`No recent continuation update from ${who.toLowerCase() === "you" ? "you" : who}.`);
    }
  }
  return [...new Set(statements)];
}
