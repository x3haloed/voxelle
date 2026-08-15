import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("duplicate member and role names keep bounded stable markers on authority-changing controls", () => {
  assert.match(source, /memberSummary\.setAttribute\("aria-label", `Actions for member \$\{memberLabel\}`\)/);
  assert.match(source, /memberBanConfirmation\(profile, memberLabel\)/);
  assert.match(source, /const roleLabel = disambiguatedRoleLabel\(role, roles\)/);
  assert.match(source, /Manage members for role \$\{roleLabel\}/);
  assert.match(source, /roleAssignmentConfirmation\(\s*role,\s*roleLabel,\s*profile,\s*memberLabel,/);
  assert.match(source, /`Mention \$\{disambiguatedMemberLabel\(profile, currentProfiles\)\}`/);
});

test("duplicate channel names retain one stable label across causal surfaces", () => {
  assert.match(source, /const channelLabel = disambiguatedChannelLabel\(channel, channels\)/);
  assert.match(source, /channelKeyRotationConfirmation\(channel, channelLabel\)/);
  assert.match(source, /Message \$\{channel \? `#\$\{channelLabel\}` : "this room"\}/);
  assert.match(source, /disambiguatedChannelLabel\(channel, snapshot\.home\?\.channels \?\? \[\]\)/);
});

test("same-expiry invites retain one stable label through revocation review", () => {
  assert.match(source, /const inviteLabel = disambiguatedInviteLabel\(invite, activeInvites\)/);
  assert.match(source, /Revoke invite expiring \$\{inviteLabel\} confirmation/);
  assert.match(source, /confirm\.textContent = `Revoke invite expiring \$\{inviteLabel\}`/);
});
