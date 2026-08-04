import assert from "node:assert/strict";
import test from "node:test";

import { reconcileChildren } from "./dom-reconcile.mjs";

class FakeText {
  constructor(data, ownerDocument) {
    this.nodeType = 3;
    this.data = data;
    this.ownerDocument = ownerDocument;
    this.parentNode = null;
  }
}

class FakeElement {
  constructor(tagName, ownerDocument) {
    this.nodeType = 1;
    this.tagName = tagName.toUpperCase();
    this.ownerDocument = ownerDocument;
    this.parentNode = null;
    this.childNodes = [];
    this.attributeMap = new Map();
    this.value = "";
    this.checked = false;
    this.selectedIndex = -1;
    this.open = false;
  }

  get attributes() {
    return [...this.attributeMap].map(([name, value]) => ({ name, value }));
  }

  get lastChild() {
    return this.childNodes.at(-1) ?? null;
  }

  getAttribute(name) {
    return this.attributeMap.get(name) ?? null;
  }

  setAttribute(name, value) {
    this.attributeMap.set(name, String(value));
    if (name === "open") this.open = true;
  }

  removeAttribute(name) {
    this.attributeMap.delete(name);
    if (name === "open") this.open = false;
  }

  append(...nodes) {
    for (const node of nodes) this.insertBefore(node, null);
  }

  insertBefore(node, reference) {
    if (node.parentNode) node.parentNode.removeChild(node);
    const index = reference ? this.childNodes.indexOf(reference) : this.childNodes.length;
    this.childNodes.splice(index < 0 ? this.childNodes.length : index, 0, node);
    node.parentNode = this;
    return node;
  }

  removeChild(node) {
    const index = this.childNodes.indexOf(node);
    if (index >= 0) this.childNodes.splice(index, 1);
    node.parentNode = null;
    return node;
  }
}

function element(document, tagName, key, ...children) {
  const node = new FakeElement(tagName, document);
  if (key) node.setAttribute("data-render-key", key);
  node.append(...children);
  return node;
}

function text(document, value) {
  return new FakeText(value, document);
}

test("snapshot publication preserves open details and the active draft control", () => {
  const document = { activeElement: null };
  const root = element(document, "main", "root");
  const details = element(document, "details", "customize");
  details.setAttribute("open", "");
  const input = element(document, "input", "theme-input");
  input.value = "unfinished user draft";
  input.selectionStart = 3;
  input.selectionEnd = 11;
  const status = element(document, "p", "status", text(document, "offline"));
  details.append(input, status);
  root.append(details);
  document.activeElement = input;

  const desired = element(document, "main", "desired-root");
  const desiredDetails = element(document, "details", "customize");
  const desiredInput = element(document, "input", "theme-input");
  desiredInput.value = "persisted value";
  desiredDetails.append(
    desiredInput,
    element(document, "p", "status", text(document, "online")),
  );
  desired.append(desiredDetails);

  reconcileChildren(root, desired);

  assert.equal(root.childNodes[0], details);
  assert.equal(details.childNodes[0], input);
  assert.equal(details.open, true);
  assert.equal(document.activeElement, input);
  assert.equal(input.value, "unfinished user draft");
  assert.equal(input.selectionStart, 3);
  assert.equal(input.selectionEnd, 11);
  assert.equal(details.childNodes[1].childNodes[0].data, "online");
});

test("keyed workbench views move without losing their node identity", () => {
  const document = { activeElement: null };
  const root = element(document, "main", "root");
  const first = element(document, "section", "view:first");
  const second = element(document, "section", "view:second");
  root.append(first, second);

  const desired = element(document, "main", "desired-root");
  desired.append(
    element(document, "section", "view:second"),
    element(document, "section", "view:first"),
  );

  reconcileChildren(root, desired);

  assert.deepEqual(root.childNodes, [second, first]);
});
