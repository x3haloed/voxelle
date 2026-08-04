/** @param {import("./shell-contract").UiOntologyView} ontology */
export function ontologyPresentation(ontology) {
  const styles = {};
  for (const token of ontology.semantic_tokens) {
    styles[cssProperty(token.id)] = token.current_value;
  }
  for (const metric of ontology.metrics) {
    styles[cssProperty(metric.id)] = metric.unit === "count"
      ? String(metric.current_value)
      : `${metric.current_value}${metric.unit}`;
  }

  return {
    styles,
    timestampsVisible: boolBehavior(ontology, "timestamps.visible", true),
    timestampStyle: textBehavior(ontology, "timestamps.style", "relative"),
    activityAutoScroll: boolBehavior(ontology, "activity.autoScroll", true),
    peerListCompact: boolBehavior(ontology, "peerList.compact", false),
    syncAutoAfterImport: boolBehavior(ontology, "sync.autoAfterImport", false),
    startOnlineOnLaunch: boolBehavior(
      ontology,
      "runtime.startOnlineOnLaunch",
      false,
    ),
    activityMaxItems: Math.max(
      0,
      Math.floor(metricValue(ontology, "activity.maxItems", 30)),
    ),
  };
}

/**
 * @param {HTMLElement} root
 * @param {import("./shell-contract").UiOntologyView} ontology
 */
export function applyOntology(root, ontology) {
  const presentation = ontologyPresentation(ontology);
  for (const [property, value] of Object.entries(presentation.styles)) {
    root.style.setProperty(property, value);
  }
  root.dataset.peerListCompact = String(presentation.peerListCompact);
  return presentation;
}

/**
 * @param {{ created_ms: number }} message
 * @param {import("./shell-contract").UiOntologyView} ontology
 * @param {number} [nowMs]
 * @param {string} [locale]
 */
export function messageTimestamp(message, ontology, nowMs = Date.now(), locale) {
  const presentation = ontologyPresentation(ontology);
  if (!presentation.timestampsVisible) {
    return null;
  }
  if (presentation.timestampStyle === "absolute") {
    return new Date(message.created_ms).toLocaleString(locale);
  }

  const seconds = Math.round((message.created_ms - nowMs) / 1000);
  const magnitude = Math.abs(seconds);
  const [value, unit] = magnitude < 60
    ? [seconds, "second"]
    : magnitude < 3_600
      ? [Math.round(seconds / 60), "minute"]
      : magnitude < 86_400
        ? [Math.round(seconds / 3_600), "hour"]
        : [Math.round(seconds / 86_400), "day"];
  return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(value, unit);
}

export function safeDateTime(timestampMs) {
  const date = new Date(timestampMs);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

/**
 * @param {Array<import("./shell-contract").ServiceActivityItem>} items
 * @param {import("./shell-contract").UiOntologyView} ontology
 */
export function visibleActivity(items, ontology) {
  const limit = ontologyPresentation(ontology).activityMaxItems;
  return items.slice(-limit || items.length).reverse();
}

function metricValue(ontology, id, fallback) {
  return ontology.metrics.find((metric) => metric.id === id)?.current_value ?? fallback;
}

function boolBehavior(ontology, id, fallback) {
  const value = ontology.behaviors.find((behavior) => behavior.id === id)?.current_value;
  return value?.type === "bool" ? value.value : fallback;
}

function textBehavior(ontology, id, fallback) {
  const value = ontology.behaviors.find((behavior) => behavior.id === id)?.current_value;
  return value?.type === "text" ? value.value : fallback;
}

function cssProperty(id) {
  return `--${id
    .replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
    .replaceAll(".", "-")}`;
}
