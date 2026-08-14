/** @param {Node | null | undefined} node */
function renderKey(node) {
  return node?.nodeType === 1
    ? /** @type {Element} */ (node).getAttribute("data-render-key")
    : null;
}

/** @param {Node | null | undefined} node */
function actionKey(node) {
  return node?.nodeType === 1
    ? /** @type {Element} */ (node).getAttribute("data-action-key")
    : null;
}

/** @param {Node | undefined} current @param {Node} desired */
function compatible(current, desired) {
  if (!current || current.nodeType !== desired.nodeType) return false;
  if (
    current.nodeType === 1
    && /** @type {Element} */ (current).tagName !== /** @type {Element} */ (desired).tagName
  ) return false;
  const currentKey = renderKey(current);
  const desiredKey = renderKey(desired);
  if (currentKey || desiredKey) return currentKey === desiredKey;
  const currentAction = actionKey(current);
  const desiredAction = actionKey(desired);
  return currentAction || desiredAction ? currentAction === desiredAction : true;
}

/** @param {Element} current @param {Element} desired */
function syncAttributes(current, desired) {
  const preserveOpen = current.tagName === "DETAILS";
  const desiredNames = new Set();
  for (const attribute of desired.attributes) {
    desiredNames.add(attribute.name);
    if (preserveOpen && attribute.name === "open") continue;
    if (current.getAttribute(attribute.name) !== attribute.value) {
      current.setAttribute(attribute.name, attribute.value);
    }
  }
  for (const attribute of [...current.attributes]) {
    if (preserveOpen && attribute.name === "open") continue;
    if (!desiredNames.has(attribute.name)) current.removeAttribute(attribute.name);
  }
}

/** @param {Element} current @param {Element} desired */
function syncControlState(current, desired) {
  if (!["INPUT", "TEXTAREA", "SELECT"].includes(current.tagName)) return;
  const syncFocusedValue = desired.getAttribute("data-sync-focused-value") === "true";
  if (current.ownerDocument?.activeElement === current && !syncFocusedValue) return;
  const currentControl = /** @type {HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement} */ (current);
  const desiredControl = /** @type {HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement} */ (desired);
  if (currentControl.value !== desiredControl.value) currentControl.value = desiredControl.value;
  if (current.tagName === "INPUT") {
    /** @type {HTMLInputElement} */ (currentControl).checked =
      /** @type {HTMLInputElement} */ (desiredControl).checked;
  }
  if (current.tagName === "SELECT") {
    /** @type {HTMLSelectElement} */ (currentControl).selectedIndex =
      /** @type {HTMLSelectElement} */ (desiredControl).selectedIndex;
  }
}

/** @param {Node} current @param {Node} desired */
function reconcileNode(current, desired) {
  if (current.nodeType === 3) {
    const currentText = /** @type {Text} */ (current);
    const desiredText = /** @type {Text} */ (desired);
    if (currentText.data !== desiredText.data) currentText.data = desiredText.data;
    return;
  }
  if (current.nodeType !== 1) return;
  const currentElement = /** @type {Element} */ (current);
  const desiredElement = /** @type {Element} */ (desired);
  syncAttributes(currentElement, desiredElement);
  syncControlState(currentElement, desiredElement);
  reconcileChildren(currentElement, desiredElement);
}

/** @param {ParentNode} parent @param {Node} desired @param {number} start */
function keyedCandidate(parent, desired, start) {
  const key = renderKey(desired);
  if (!key) return null;
  for (let index = start; index < parent.childNodes.length; index += 1) {
    const candidate = parent.childNodes[index];
    if (renderKey(candidate) === key && compatible(candidate, desired)) return candidate;
  }
  return null;
}

/** @param {Node & ParentNode} parent @param {ParentNode} desiredParent */
export function reconcileChildren(parent, desiredParent) {
  const desiredChildren = [...desiredParent.childNodes];
  for (let index = 0; index < desiredChildren.length; index += 1) {
    const desired = desiredChildren[index];
    let current = keyedCandidate(parent, desired, index) ?? parent.childNodes[index];
    if (!compatible(current, desired)) {
      parent.insertBefore(desired, current ?? null);
      current = desired;
    } else {
      if (parent.childNodes[index] !== current) {
        parent.insertBefore(current, parent.childNodes[index] ?? null);
      }
      reconcileNode(current, desired);
    }
  }
  while (parent.childNodes.length > desiredChildren.length) {
    const last = parent.lastChild;
    if (last) parent.removeChild(last);
  }
}
