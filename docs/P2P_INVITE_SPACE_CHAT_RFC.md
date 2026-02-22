# P2P Invite-Only Space Chat (Draft RFC v0)

This document specifies a **serverless**, **invite-only**, **Discord-like** communication system designed to work well for both humans and autonomous agents.

It is intentionally written so that **no central servers are required** for identity registration, membership, messaging, moderation, discovery, or synchronization. Optional third-party infrastructure (relays, caches, indexers) is permitted only when it is **untrusted** and **replaceable**.

Keywords **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, **MAY** are to be interpreted as described in RFC 2119.

## 0) Summary (Non-Normative)

- People/agents create a globally unique cryptographic **ID** by generating a keypair; there is no account registration service.
- Communities are **Spaces** (Discord “servers”). Everything (membership, roles, bans, channel creation, etc.) is driven by signed **events**.
- Access is **invite-only**: you need an invite capability to join a Space or even read it.
- Public channels are not end-to-end encrypted (Discord-style); DMs/private rooms are end-to-end encrypted.

## 1) Design Goals

1. **No central servers**: the system remains functional with only peers.
2. **Trustless identities**: anyone can create an ID without permission.
3. **Invite-only Spaces**: a Space is readable only to invited members.
4. **Authority-rooted governance**: each Space has explicit authority roots.
5. **Human usability**: passkeys, social recovery, and optional social-login *claims*.
6. **Agent-native**: least-privilege capabilities, auditability, higher security defaults.
7. **Partition tolerance**: offline-first; converges when peers reconnect.

## 2) Non-Goals

- A global directory of Spaces or members.
- Global uniqueness of human-readable handles.
- Strong anonymity against a global network observer.
- Perfect enforcement of “invite max uses” without online issuers (see §8.6).

## 3) Terminology

- **Principal**: a human or agent identity (root key).
- **Device**: a specific client instance (browser/phone/agent runtime).
- **Device key**: a key used for day-to-day signing; authorized by a Principal root key.
- **Space**: a community (Discord “server” / “guild”).
- **Room**: a channel/DM/group thread; belongs to a Space (except optional DMs).
- **Event**: an immutable, signed record appended to a Room’s event log.
- **Capability**: a portable token granting scoped permissions (e.g., an invite).
- **Relay**: an optional forwarder that provides reachability; always untrusted.

## 4) Cryptography (Required)

### 4.1 Algorithms

Implementations **MUST** support:

- Ed25519 signatures.
- SHA-256 hashing.
- Base64url (RFC 4648 URL-safe, **no padding**) for textual encodings unless specified otherwise.

Implementations **SHOULD** additionally support:

- BLAKE3 for fast local fingerprints (non-authoritative UI).
- MLS (Messaging Layer Security) for group end-to-end encryption in private rooms.

### 4.2 Public Key Encoding

To keep binary formats compatible across languages:

- Public keys **MUST** be represented as SPKI DER bytes and encoded as standard Base64 (padded `=` allowed) when placed in JSON.
- Private keys (when exported) **SHOULD** be PKCS#8 DER Base64.

### 4.3 Principal ID (Global Unique, Trustless)

A Principal’s canonical ID is derived from its public key:

```
principal_id = "ed25519:" + base64url( sha256(principal_pub_spki_der) )
```

Notes:
- The ID is globally unique with overwhelming probability.
- “Registration” is just generating the keypair.
- Verifiers can validate `principal_id` by recomputing it from the presented `principal_pub`.

### 4.4 Space ID (Authority Rooted)

A Space **MUST** have one or more authority roots (“space roots”).

For v0, a Space **MUST** have exactly one **Space Root** Ed25519 keypair.

The Space’s canonical ID is:

```
space_id = "ed25519:" + base64url( sha256(space_root_pub_spki_der) )
```

## 5) Identity, Devices, and Delegation (Required)

### 5.1 Root vs Device Keys

Human usability and safety require limiting exposure of the Principal root key.

- The **Principal root key** **MUST NOT** be used for routine chat posting.
- Devices **MUST** use **Device keys**, authorized by the root via delegation.

### 5.2 Delegation Certificates

A delegation certificate binds a Device public key to a Principal.

Delegation certificates are JSON objects with fields:

- `v` (number): MUST be `1`
- `principal_id` (string): the Principal ID
- `principal_pub` (string): SPKI DER Base64
- `device_pub` (string): SPKI DER Base64
- `device_id` (string): derived like `principal_id` from `device_pub` (same scheme)
- `not_before_ts` (number): Unix ms
- `expires_ts` (number): Unix ms
- `scopes` (array of strings): capability scopes (see below)
- `sig` (string): Base64 Ed25519 signature by the Principal root key

Delegation `scopes` are **device-local restrictions**: they limit what the Principal allows that Device to do.

Delegation `scopes` **MUST NOT** be interpreted as granting Space permissions. Space permissions are granted only by Space governance (§6).

Scope strings **MUST** be stable and namespaced. Recommended v0 scopes:

- `space:<space_id>:join`
- `space:<space_id>:post`
- `space:<space_id>:governance`
- `dm:read`
- `dm:post`

Devices **MUST** present a valid delegation certificate when emitting events.

Verifiers **MUST**:
- verify `sig` using `principal_pub`
- verify `principal_id` matches `principal_pub`
- verify `device_id` matches `device_pub`
- check `not_before_ts <= now <= expires_ts` (allow small clock skew, recommended ±10 minutes)
- enforce `scopes` as device-local restrictions for the requested action

### 5.3 Passkeys (Human Default)

Human-facing clients **SHOULD** store Device keys as passkeys (WebAuthn) when available.

WebAuthn credential sync (iCloud/Google/Microsoft) is permitted as a **convenience**, but **MUST NOT** be required for protocol correctness.

### 5.4 Social Recovery (Human Default)

Clients **SHOULD** support social recovery of the Principal root key via one of:

- **Threshold reconstruction** (e.g., Shamir): encrypt the root key and split into `n` shares, requiring `k` shares to recover.
- **Threshold signing** (preferred): guardians jointly sign a rotation/recovery without reconstructing the root secret.

The protocol **MUST NOT** require any centralized recovery service.

### 5.5 Social Login (Optional Claims, Non-Authoritative)

Clients **MAY** allow linking OAuth identities (Google, GitHub, etc.) as **claims**.

Claims:
- **MUST NOT** be required to use the network.
- **MUST NOT** be accepted as authentication for the Principal ID.
- **MAY** be used as UI hints (“this Principal also controls GitHub X”).

Claim format (recommended):
- `claim_type`: e.g. `oauth:github`
- `claim_value`: e.g. GitHub username or stable subject identifier
- `proof`: an opaque blob proving control (implementation-defined)
- `sig`: signature by the Principal root key over the claim fields

## 6) Spaces: Governance and Roles (Required)

### 6.1 Space Genesis

Space creation is the publication of a **Space Genesis** object, signed by the Space Root.

Genesis object fields:
- `v` (number): MUST be `1`
- `space_id` (string)
- `space_root_pub` (string): SPKI DER Base64
- `created_ts` (number): Unix ms
- `name` (string, optional): display name (non-authoritative)
- `sig` (string): signature by Space Root

Peers **MUST** validate `space_id` matches `space_root_pub` and the signature is valid.

### 6.2 Governance Events

All administrative actions are represented as signed events in a dedicated Room:

- `room_id = "governance"`

Governance events are authoritative for:
- roles and permissions
- bans/mutes
- room creation/deletion (and room metadata)
- policy changes (rate limits, agent rules, encryption requirements, etc.)

Governance event authorship:
- MUST be signed by a Device key that is validly delegated by its Principal (§5.2)
- MUST be authorized by Space policy/roles derived from governance state (§6.3, §6.4)

### 6.3 Role/Permission Model (v0 Recommendation)

v0 **SHOULD** implement a simple model:

- Roles are named strings.
- A member may hold multiple roles.
- Roles grant permission bits:
  - `READ`, `POST`, `MODERATE`, `ISSUE_INVITES`, `MANAGE_ROOMS`

Implementations **MAY** add per-room ACLs later, but **MUST** keep the model deterministic and auditable from events.

### 6.4 Governance State Evaluation (Required)

To make authorization deterministic in a partitioned DAG, each peer computes governance state by applying governance-room events in a canonical order:

1. Build the governance-room DAG (events where `room_id == "governance"`).
2. Perform a deterministic topological traversal:
   - primary: DAG order (parents before children)
   - tie-breakers (when multiple nodes are eligible): sort by (`ts`, `event_id`) ascending
3. Apply a pure state machine over the ordered events to derive:
   - membership admissions (from `MEMBER_JOIN`)
   - bans/unbans
   - role definitions and grants/revocations
   - invite revocations
   - room definitions/archives
   - policy blobs
   - device revocations

Peers **MUST** use this derived state when deciding whether a Principal is authorized to emit a given governance or room event.

## 7) Rooms and Events (Required)

### 7.1 Room IDs

Room IDs **MUST** be stable strings unique within a Space.

Recommended:
- public rooms: `room:<slug>` (e.g., `room:general`)
- private rooms: `room:private:<random>` (random 128 bits encoded)
- DMs (optional): `dm:<principal_id_A>:<principal_id_B>` with canonical ordering

### 7.2 Event Logs (Append-Only)

Each Room has an append-only log of Events.

Events **MUST** be immutable. Edits and deletes are new Events that reference prior ones.

### 7.3 Event Structure

An Event is a JSON object:

- `v` (number): MUST be `1`
- `space_id` (string)
- `room_id` (string)
- `event_id` (string): unique (see below)
- `author_principal_id` (string)
- `author_device_id` (string)
- `author_device_pub` (string): SPKI DER Base64
- `delegation` (object): delegation certificate (see §5.2)
- `ts` (number): Unix ms at author
- `kind` (string): event type (see below)
- `prev` (array of strings): zero or more parent event_ids (for partial order)
- `body` (object): kind-specific
- `sig` (string): Base64 signature by the Device key

`event_id` **MUST** be:

```
event_id = "e:" + base64url( sha256( signature_input_bytes ) )
```

Where `signature_input_bytes` is the deterministic encoding of all fields except `sig` and `event_id` itself.

Rationale:
- Event IDs become content-addressed; peers can request by hash.
- Storage, deduplication, and sync are simpler.

### 7.3.1 Deterministic Signature Encoding (Required)

All signatures in this RFC use the same deterministic encoding rule:

- Use **netstrings** to encode signature inputs:
  - `netstring(bytes) = <len-as-decimal-ascii> ":" <bytes> ","`
  - `<len>` is byte length (not Unicode codepoints).
- Concatenate netstrings in the exact field order defined for that object.
- Prefix with an ASCII domain separator including trailing newline.

Implementations **MUST** compute signatures over these bytes, not raw JSON bytes.

Common prefixes:
- Delegation certificate: `p2pspace/delegation/v0\n`
- Space genesis: `p2pspace/space-genesis/v0\n`
- Peer record: `p2pspace/peer/v0\n`
- Invite issuer certificate: `p2pspace/invite-issuer/v0\n`
- Invite: `p2pspace/invite/v0\n`
- Event: `p2pspace/event/v0\n`
- PoW: `p2pspace/pow/v0\n` (see §8.3)

#### Nested / Extensible Fields

When a signature must cover an extensible JSON sub-object (e.g., `body`, `constraints`, `bootstrap`), it **MUST** be included as UTF-8 bytes produced by the JSON Canonicalization Scheme (JCS, RFC 8785). Those bytes are then placed into the netstring stream as a single netstring.

This keeps signatures stable across languages and allows forward-compatible fields without re-specifying a field-by-field netstring order for every object.

### 7.3.2 Signature Inputs (Normative)

This section defines exact field order for signature inputs.

All integers are encoded as decimal ASCII with no leading `+` and no leading zeros (unless the number is exactly `0`).

**Delegation certificate signature input**

Prefix: `p2pspace/delegation/v0\n`

Netstrings, in order:
1. `v`
2. `principal_id`
3. `principal_pub`
4. `device_id`
5. `device_pub`
6. `not_before_ts`
7. `expires_ts`
8. `count(scopes)`
9. each scope string in `scopes`, in order

**Space genesis signature input**

Prefix: `p2pspace/space-genesis/v0\n`

Netstrings, in order:
1. `v`
2. `space_id`
3. `space_root_pub`
4. `created_ts`
5. `name` (empty string if missing)

**Invite issuer certificate (IIC) signature input**

Prefix: `p2pspace/invite-issuer/v0\n`

Netstrings, in order:
1. `v`
2. `space_id`
3. `space_root_pub`
4. `issuer_principal_id`
5. `issuer_principal_pub`
6. `not_before_ts`
7. `expires_ts`
8. `count(allowed_scopes)`
9. each scope string in `allowed_scopes`, in order

**Invite signature input**

Prefix: `p2pspace/invite/v0\n`

Netstrings, in order:
1. `v`
2. `space_id`
3. `invite_id`
4. `issued_ts`
5. `expires_ts`
6. `issuer_principal_id`
7. `issuer_device_id`
8. `issuer_device_pub`
9. `issuer_delegation.sig`
10. `invite_issuer.sig` (empty string if missing)
11. `constraints_jcs` (JCS bytes; empty object `{}` if missing)
12. `bootstrap_jcs` (JCS bytes)

**Event signature input**

Prefix: `p2pspace/event/v0\n`

Netstrings, in order:
1. `v`
2. `space_id`
3. `room_id`
4. `author_principal_id`
5. `author_device_id`
6. `author_device_pub`
7. `delegation.sig`
8. `ts`
9. `kind`
10. `count(prev)`
11. each `prev` entry, in order
12. `body_jcs` (JCS bytes; `{}` if missing)

**Peer record signature input**

Prefix: `p2pspace/peer/v0\n`

Netstrings, in order:
1. `v`
2. `principal_id`
3. `principal_pub`
4. `device_id`
5. `device_pub`
6. `delegation.sig`
7. `ts`
8. `expires_ts`
9. `addrs_jcs` (JCS bytes; `[]` if missing)

### 7.4 Event Kinds (v0)

Room message kinds:
- `MSG_POST` (post a message)
- `MSG_EDIT` (edit a prior message)
- `MSG_REDACT` (tombstone/hide a prior message)
- `REACTION_ADD`
- `REACTION_REMOVE`
- `PIN_ADD`
- `PIN_REMOVE`

Space/membership kinds (typically in `governance` room):
- `SPACE_POLICY_SET`
- `ROLE_DEFINE`
- `ROLE_GRANT`
- `ROLE_REVOKE`
- `MEMBER_BAN`
- `MEMBER_UNBAN`
- `INVITE_ISSUE`
- `INVITE_REVOKE`
- `MEMBER_JOIN`
- `ROOM_DEFINE`
- `ROOM_ARCHIVE`
- `DEVICE_REVOKE`

Implementations **MUST** ignore unknown `kind` values (forward compatibility), but **MUST** still validate signatures and store events.

### 7.4.1 Event Bodies (v0, Normative Minimum)

Unless specified, unknown fields in `body` **MUST** be ignored.

`MSG_POST` body:
- `msg_id` (string, optional): stable identifier for UI threading; if omitted, use `event_id`
- `text` (string): UTF-8 text
- `attachments` (array, optional): implementation-defined descriptors (hashes, sizes, mime)

`MSG_EDIT` body:
- `target_event_id` (string): event being edited
- `text` (string): replacement text

`MSG_REDACT` body:
- `target_event_id` (string): event being redacted
- `reason` (string, optional)

`REACTION_ADD` / `REACTION_REMOVE` body:
- `target_event_id` (string)
- `emoji` (string): Unicode emoji or short code

`PIN_ADD` / `PIN_REMOVE` body:
- `target_event_id` (string)

`ROOM_DEFINE` body (governance room):
- `room_id` (string)
- `name` (string, optional)
- `visibility` (string): `public` | `private`
- `encryption` (string, optional): `none` | `mls` | `x3dh+double-ratchet` (if `private`)

`ROOM_ARCHIVE` body (governance room):
- `room_id` (string)
- `archived` (boolean): true to archive, false to unarchive

`SPACE_POLICY_SET` body (governance room):
- `policy` (object): free-form policy blob (JCS-signed as part of the event body)

`ROLE_DEFINE` body (governance room):
- `role` (string)
- `permissions` (array of strings): subset of `READ`, `POST`, `ISSUE_INVITES`, `MANAGE_ROOMS`, `MODERATE`

`ROLE_GRANT` / `ROLE_REVOKE` body (governance room):
- `principal_id` (string)
- `role` (string)

`INVITE_ISSUE` body (governance room, optional audit trail):
- `invite_id` (string)
- `invite_sig` (string)
- `expires_ts` (number)

`INVITE_REVOKE` body (governance room):
- `invite_id` (string)
- `reason` (string, optional)

`MEMBER_JOIN` body (governance room):
- `principal_id` (string)
- `principal_pub` (string): SPKI DER Base64
- `invite` (object): the invite object
- `pow_nonce` (string, optional): required if invite constraints require PoW

`MEMBER_BAN` / `MEMBER_UNBAN` body (governance room):
- `principal_id` (string)
- `reason` (string, optional)

`DEVICE_REVOKE` body (governance room):
- `principal_id` (string)
- `device_id` (string)
- `reason` (string, optional)

### 7.5 Ordering and Convergence

There is no global ordering.

- Clients **MUST** treat the log as a DAG ordered by `prev` edges.
- For display ordering, clients **SHOULD** use a deterministic topological sort, tie-breaking by (`ts`, `event_id`) to ensure stable rendering.
- Clients **MUST** tolerate missing ancestors temporarily and fill them in as they arrive.

### 7.6 Validation Rules (Required)

**Event validation**

Upon receiving an Event, peers **MUST**:
1. Validate `space_id`, `room_id`, types, and size limits.
2. Validate the author Device key identity:
   - `author_device_id` matches `author_device_pub` using the ID derivation in §4.3.
3. Validate `delegation`:
   - `delegation.device_id == author_device_id`
   - `delegation.principal_id == author_principal_id`
   - delegation signature verifies (per §7.3.2) under `delegation.principal_pub`
   - delegation is within its validity window
4. Validate membership / permissions:
   - for any room other than `governance`, the `author_principal_id` **MUST** be an admitted member and **MUST NOT** be currently banned (per §6.4)
   - for `governance` room:
     - `MEMBER_JOIN` events are authorized if the embedded invite is valid (per §9.1)
     - all other governance kinds are authorized only if the governance state grants the Principal the required permission (e.g., `MODERATE`, `ISSUE_INVITES`, `MANAGE_ROOMS`)
   - delegation scopes **MUST** additionally permit the action as a device-local restriction:
     - `space:<space_id>:join` for `MEMBER_JOIN`
     - `space:<space_id>:post` for room message kinds
     - `space:<space_id>:governance` for governance kinds other than `MEMBER_JOIN`
   - the Device **MUST NOT** be revoked by governance (`DEVICE_REVOKE` for `author_principal_id` + `author_device_id`)
5. Validate the Event signature:
   - compute signature input (per §7.3.2) and verify `sig` under the Device public key corresponding to `author_device_pub`
6. Validate `event_id`:
   - recompute `event_id` from the signature input bytes and require it matches

Peers **MUST** store valid Events even if some referenced ancestors in `prev` are missing.

## 8) Invites (Required)

### 8.1 Invite as Capability

An invite is a bearer capability that allows a Principal to join a Space and read its Rooms.

Invites are portable: QR codes, links, files, etc.

### 8.2 Invite Object

Invite objects are JSON with:

- `v` (number): MUST be `1`
- `space_id` (string)
- `invite_id` (string): globally unique random (recommend 128 bits base64url)
- `issued_ts` (number): Unix ms
- `expires_ts` (number): Unix ms
- `issuer_principal_id` (string)
- `issuer_device_id` (string)
- `issuer_device_pub` (string): SPKI DER Base64
- `issuer_delegation` (object): delegation cert authorizing invite issuance
- `invite_issuer` (object, optional): invite issuer certificate (required unless issuer is Space Root)
- `scopes` (array of strings): Space permission scopes granted by this invite (MUST include `space:<space_id>:read`)
- `constraints` (object, optional): see below
- `bootstrap` (object): initial connectivity hints (see below)
- `sig` (string): signature by issuer Device key

Invite verifiers **MUST**:
- validate issuer Device identity (`issuer_device_id` matches `issuer_device_pub`)
- validate `issuer_delegation` and require:
  - `issuer_delegation.device_id == issuer_device_id`
  - `issuer_delegation.principal_id == issuer_principal_id`
  - `issuer_delegation` signature is valid (per §7.3.2)
- validate issuer authorization:
  - if `issuer_principal_id == space_id`, issuer is Space Root
  - else require a valid `invite_issuer` IIC (per §8.5) matching `issuer_principal_id`
- validate invite signature (per §7.3.2) under the issuer Device key

Invite scope mapping (v0):
- `space:<space_id>:read` grants `READ`
- `space:<space_id>:post` grants `POST`

### 8.3 Constraints (Optional)

`constraints` MAY include:

- `principal_type` (string): `human` | `agent` | `any` (default `any`)
- `requires_pow` (object): `{ "bits": number, "expires_ts": number }`
- `rate_limits` (object): implementation-defined defaults for the joining principal/device
- `bound_principal_id` (string, optional): if present, only that Principal may use the invite

If `requires_pow` is present, joiners **MUST** include a PoW solution in `MEMBER_JOIN`:

- `pow_nonce` (string): base64url random bytes (recommended 16–32 bytes)

The verifier computes:

```
digest = sha256(
  "p2pspace/pow/v0\n" ||
  utf8(invite_id) ||
  0x00 ||
  utf8(joiner_principal_id) ||
  0x00 ||
  base64url_decode(pow_nonce)
)
```

And checks `digest` has at least `bits` leading zero bits.

### 8.4 Bootstrap Hints (Required)

`bootstrap` provides decentralized join without directories.

`bootstrap` **MUST** include at least one of:

- `peers` (array of objects): signed peer records for members willing to be contacted out-of-band
- `relays` (array of objects): relay identities + addresses (untrusted)
- `rendezvous` (array of strings): opaque “meet here” hints (e.g., DHT keys, local network hints)

Notes:
- Relays **MUST** be treated as untrusted forwarders. They can drop/observe metadata, but cannot forge signed events.
- For end-to-end encrypted rooms, relays **MUST NOT** be able to read ciphertext payloads.

### 8.5 Issuance Authority (No Governance Replay Required)

To avoid ambiguous “authorization at time of issuance” in a partitioned event DAG, v0 defines a **portable issuer authorization**.

An invite is valid if and only if the issuer is either:

1) the **Space Root**, or
2) a delegated invite issuer holding a valid **Invite Issuer Certificate** (IIC) signed by the Space Root.

Invite Issuer Certificate (IIC) fields:
- `v` (number): MUST be `1`
- `space_id` (string)
- `space_root_pub` (string): SPKI DER Base64
- `issuer_principal_id` (string)
- `issuer_principal_pub` (string): SPKI DER Base64
- `allowed_scopes` (array of strings): e.g. `space:<space_id>:read`, `space:<space_id>:post`
- `not_before_ts` (number): Unix ms
- `expires_ts` (number): Unix ms
- `sig` (string): signature by the Space Root

Verification rules:
- `space_id` must match `space_root_pub`.
- `issuer_principal_id` must match `issuer_principal_pub`.
- If the invite contains `invite_issuer`, verifiers **MUST** check:
  - IIC signature is valid under `space_root_pub`
  - `now` is within `[not_before_ts, expires_ts]` (allow small skew)
  - invite `scopes` are a subset of IIC `allowed_scopes`

If `invite_issuer` is missing, verifiers **MUST** require that `issuer_principal_id == space_id` (issuer is Space Root).

### 8.6 “Max uses” and Offline Issuance (Important)

Without a central online service, strict enforcement of `max_uses` for a broadcast invite is not generally possible.

Therefore:
- v0 invites **SHOULD NOT** include `max_uses` as a security guarantee.
- If an implementation supports `max_uses`, it **MUST** be documented as **best-effort**, enforced only from locally observed redeems, and **MUST NOT** be relied upon for security.

For strict single-use behavior, use **bound invites** (`bound_principal_id`) or require an online admission signature (future extension).

## 9) Membership (Required)

### 9.1 Join Flow (v0)

To join a Space, a Principal:

1. Obtains an invite capability out-of-band.
2. Uses `bootstrap` hints to contact any reachable member (directly or via relays).
3. Syncs governance events and validates the invite issuer authorization.
4. Emits a `MEMBER_JOIN` event in the governance room:
   - includes the invite object
   - includes the joining Principal root pub (or a proof binding the join to that Principal)

Pre-join syncing:
- To avoid a “membership chicken-and-egg”, peers **MAY** serve governance/room metadata to a joiner who presents a valid invite, even before the joiner is admitted.
- Peers **MUST NOT** serve non-governance room content to non-members.

Peers accept membership if and only if:
- the invite signature is valid and unexpired
- constraints pass (e.g., PoW if required; bound principal if present)
- the invite issuer is authorized via §8.5
- the invite has not been revoked by a later `INVITE_REVOKE` event

On membership admission, peers **MUST** grant the joining Principal the permissions implied by invite `scopes` (at minimum `READ`, and optionally `POST`).

### 9.2 Membership Proof

Peers **MUST** be able to validate that future events are authored by members.

v0 recommendation:
- treat the first valid `MEMBER_JOIN` event for a Principal as membership admission
- treat `MEMBER_BAN` as the authoritative removal/denial

Membership is therefore explicit and auditable (join/bans are events), but does not require an online moderator at join time.

In v0, the “explicit state” is the `MEMBER_JOIN` event itself (plus any subsequent ban/unban events).

## 10) Transport and Sync (Required)

This RFC is transport-agnostic, but requires that peers can:

- discover other peers via invite bootstrap
- exchange event inventories and fetch missing events
- tolerate partial connectivity and relays

### 10.1 Minimal Transport Requirements

Implementations **MUST** support at least one:
- TCP
- QUIC
- WebRTC (data channels)

Implementations **MAY** support multiple and treat them as interchangeable.

### 10.2 Peer Records (Reachability)

Peers **SHOULD** use signed peer records containing reachable addresses and expiry (similar to a signed “contact card”).

Peer records **MUST** be authenticated by the Principal (or a device with appropriate delegation) so that addresses cannot be trivially spoofed.

#### 10.2.1 Peer Record Format (v0, Normative Minimum)

A peer record is a JSON object:

- `v` (number): MUST be `1`
- `principal_id` (string)
- `principal_pub` (string): SPKI DER Base64
- `device_id` (string)
- `device_pub` (string): SPKI DER Base64
- `delegation` (object): delegation certificate (§5.2)
- `ts` (number): Unix ms
- `expires_ts` (number): Unix ms
- `addrs` (array of strings): e.g. `["tcp://203.0.113.10:9001", "webrtc://..."]`
- `sig` (string): Base64 signature by the Device key

Validation:
- verify identities (`principal_id`/`principal_pub`, `device_id`/`device_pub`)
- verify delegation (and require it’s unexpired)
- verify signature input per §7.3.2 under `device_pub`
- reject records where `expires_ts` is in the past (allow small skew)

Bootstrap `peers` entries in invites (§8.4) **MUST** be peer records in this format.

### 10.3 Sync Protocol (Minimal Viable)

For each Space/Room, peers synchronize by content-addressed events.

Peers **MUST** support:
- `HEADS(room)` → returns the set of known “head” event_ids (events with no known children)
- `WANT(room, event_ids[])` → request the full Event objects for missing ids
- `HAVE(room, event_ids[])` → announce newly learned ids

Clients **SHOULD**:
- periodically gossip heads for rooms they participate in
- request missing ancestors when a DAG has gaps
- bound memory/bandwidth via configurable limits (e.g., max events per request, max room backlog)

### 10.4 Anti-Abuse at Transport Layer

Peers **MUST** implement:
- rate limiting per remote address and per Principal ID
- replay protection for Events (cache `event_id` for at least 5 minutes)
- size limits on frames and per-room sync windows

## 11) End-to-End Encryption (Discord-Style)

### 11.1 Public Rooms (Default)

Public rooms in a Space are **not** end-to-end encrypted by default.

Integrity and authentication **MUST** still be enforced via signatures.

### 11.2 DMs and Private Rooms

DMs and private rooms **MUST** be end-to-end encrypted.

v0 requirements:
- Encrypt message bodies; keep enough metadata to route and sync (space_id/room_id/event_id).
- Include an explicit `body.encryption` field indicating the scheme/version.

Group private rooms **SHOULD** use MLS to handle membership churn safely.

## 12) Agents: Capability-First Security (Required Guidance)

Spaces should assume agents can operate at higher scale than humans.

Therefore:
- Space policy **SHOULD** distinguish `principal_type=agent` vs `human`.
- Invites issued to agents **SHOULD** be narrower and shorter-lived.
- Agent device delegations **SHOULD** default to least privilege (room-scoped post only).
- Implementations **MAY** require PoW or other resource tickets for agent joins/posting even when humans are invite-gated.

## 13) Threat Model and Security Notes

### 13.1 What This Design Prevents

- Impersonation without key compromise (signatures + ID derivation).
- Undetectable message tampering by relays (content-addressed signed events).
- Accidental trust in network endpoints (relay-safe identity model).

### 13.2 What This Design Does Not Prevent

- Metadata leakage (who connects to whom, room participation timing), especially via relays.
- Spam by invited members (mitigated via roles, rate limits, and moderation).
- Key theft on compromised devices (mitigated via passkeys, short-lived delegations, and recovery/rotation).

### 13.3 Key Rotation and Revocation

Principals **MUST** be able to:
- revoke device delegations (publish `DEVICE_REVOKE` in governance)
- rotate root keys (publish a signed rotation event; requires careful design—future extension)

## 14) Open Questions / v1 Candidates

- Multi-root Spaces (threshold governance, quorum-signed moderation).
- Strictly enforced single-use/bounded-use invites without central counting.
- A standardized “name layer” for optional global handles (separate from the base protocol).
- Full wire protocol definitions and test vectors (if standardization is desired).

---

## Appendix A: Mermaid Overview (Non-Normative)

```mermaid
flowchart TD
  I["Invite Capability (QR/link)"] --> J["Joiner Device"]
  J -->|bootstrap peers/relays| P["Any reachable member peer"]
  P --> G["Governance Room DAG"]
  G -->|validates issuer+policy| A["Membership + Roles"]
  J -->|signed events (device key)| R["Room Event DAGs"]
  R --> P
```
