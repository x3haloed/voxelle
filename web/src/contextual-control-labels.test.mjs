import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile(new URL("./product-component.js", import.meta.url), "utf8");

test("channel actions expose their visible channel context", () => {
  assert.match(source, /Select \$\{channel\.visibility === "private" \? "private " : ""\}channel \$\{channelLabel\}/);
  assert.match(source, /Rotate encryption key for private channel \$\{channelLabel\}/);
});

test("message action disclosures expose author and bounded content context", () => {
  assert.match(source, /actionSummary\.setAttribute\("aria-label", messageActionsLabel\(messageLabel\)\)/);
  assert.match(source, /reaction on \$\{messageLabel\}/);
  assert.match(source, /text\.length > 48 \? `\$\{text\.slice\(0, 47\)\}…` : text/);
});

test("governance row controls expose their visible member, role, and invite targets", () => {
  assert.match(source, /Actions for member \$\{memberLabel\}/);
  assert.match(source, /Manage members for role \$\{roleLabel\}/);
  assert.match(source, /Revoke invite expiring \$\{inviteLabel\}/);
});
