import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("going online focuses locally invalid IPv6 socket text before invocation", () => {
  assert.match(source, /optionalIpv6SocketDraftError\(uiState\.bindDraft, "Bind address"\)/);
  assert.match(source, /optionalIpv6SocketDraftError\(uiState\.advertiseDraft, "Advertised address"\)/);
  assert.match(source, /setUserError\(invalidAddress\[0\], invalidAddress\[1\]\)/);
  assert.match(source, /uiState\.connectionOpen = true;\s*setUserError/);
  assert.match(source, /"service-bind"/);
  assert.match(source, /"service-advertise"/);
});
