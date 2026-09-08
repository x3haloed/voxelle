use super::*;

pub const ENDPOINT_CLAIM_LIFETIME_MS: i64 = 15 * 60_000;
const MAX_CLAIMS: usize = 128;
const MAX_EXCHANGE_BYTES: usize = MAX_SYNC_BYTES;
const MAX_CLOCK_SKEW_MS: i64 = 30_000;

/// Disposable routing evidence; keys and membership come from admitted facts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EndpointClaimV1 {
    pub v: u8,
    pub governance_room_id: String,
    pub endpoint: PeerEndpoint,
    pub issued_ms: i64,
    pub expires_ms: i64,
    pub sig: String,
}

impl EndpointClaimV1 {
    pub fn create(
        identity: &PeerIdentity,
        endpoint: PeerEndpoint,
        governance_room_id: String,
        now_ms: i64,
    ) -> Result<Self> {
        if endpoint.peer_id != identity.peer_id || endpoint.device_id != identity.device.id {
            bail!("endpoint claim must describe the signing device");
        }
        endpoint.validate()?;
        let mut claim = Self {
            v: 1,
            governance_room_id,
            endpoint,
            issued_ms: now_ms,
            expires_ms: now_ms
                .checked_add(ENDPOINT_CLAIM_LIFETIME_MS)
                .context("endpoint expiry overflow")?,
            sig: String::new(),
        };
        claim.sig = identity.device.sign(&claim.signing_bytes()?);
        Ok(claim)
    }

    fn signing_bytes(&self) -> Result<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.sig.clear();
        let mut bytes = b"voxelle.endpoint-claim.v1\0".to_vec();
        bytes.extend(serde_json::to_vec(&unsigned)?);
        Ok(bytes)
    }

    pub fn validate(&self, store: &Store, context: &RoomContext, now_ms: i64) -> Result<()> {
        let governance = derive_governance_state(
            &store.room_events(&context.governance_room_id)?,
            context,
            now_ms,
        );
        self.validate_with_governance(store, context, now_ms, &governance)
    }

    fn validate_with_governance(
        &self,
        store: &Store,
        context: &RoomContext,
        now_ms: i64,
        governance: &voxelle_core::GovernanceState,
    ) -> Result<()> {
        if serde_json::to_vec(self)?.len() > 4096 {
            bail!("endpoint claim exceeds byte bound");
        }
        if self.v != 1 || self.governance_room_id != context.governance_room_id {
            bail!("endpoint claim is outside the configured protocol/space");
        }
        if self.issued_ms > now_ms.saturating_add(MAX_CLOCK_SKEW_MS)
            || self.expires_ms <= now_ms
            || self.expires_ms <= self.issued_ms
            || self.expires_ms.saturating_sub(self.issued_ms) > ENDPOINT_CLAIM_LIFETIME_MS
        {
            bail!("endpoint claim is expired or outside its time bound");
        }
        self.endpoint.validate()?;
        if self.endpoint.addr.port() == 0
            || self.endpoint.addr.ip().is_unspecified()
            || self.endpoint.addr.ip().is_multicast()
        {
            bail!("endpoint claim must name a concrete unicast listener");
        }
        if let SocketAddr::V6(address) = self.endpoint.addr {
            if address.scope_id() != 0 || (address.ip().segments()[0] & 0xffc0) == 0xfe80 {
                bail!("endpoint claims cannot forward interface-local addresses");
            }
        }
        let key = authorized_device_key_in(
            store,
            governance,
            &self.endpoint.peer_id,
            &self.endpoint.device_id,
            now_ms,
        )?;
        verify_signature_from_spki_b64(&key, &self.signing_bytes()?, &self.sig)
            .context("verify endpoint claim signature")
    }
}

fn authorized_device_key(
    store: &Store,
    context: &RoomContext,
    peer_id: &str,
    device_id: &str,
    now_ms: i64,
) -> Result<String> {
    let events = store.room_events(&context.governance_room_id)?;
    let governance = derive_governance_state(&events, context, now_ms);
    authorized_device_key_in(store, &governance, peer_id, device_id, now_ms)
}

fn authorized_device_key_in(
    store: &Store,
    governance: &voxelle_core::GovernanceState,
    peer_id: &str,
    device_id: &str,
    now_ms: i64,
) -> Result<String> {
    if !governance.members.contains(peer_id)
        || governance
            .revoked_devices
            .contains(&(peer_id.to_string(), device_id.to_string()))
    {
        bail!("endpoint exchange requires current space membership and device authority");
    }
    let proof = store
        .latest_identity_proof(peer_id)?
        .context("endpoint device has no admitted identity proof")?;
    let identity = derive_identity_state(&proof)?;
    let device = identity
        .devices
        .get(device_id)
        .context("endpoint device is revoked or unknown")?;
    if device.expires_ms < now_ms {
        bail!("endpoint device authorization expired");
    }
    Ok(device.device_pub.clone())
}

fn cache_key(context: &RoomContext) -> String {
    format!("network.endpoint_claims:{}", context.governance_room_id)
}

#[derive(Default, Serialize, Deserialize)]
struct EndpointHintCache {
    claims: Vec<EndpointClaimV1>,
    manual_since: std::collections::BTreeMap<String, i64>,
}

fn device_key(endpoint: &PeerEndpoint) -> String {
    format!("{}|{}", endpoint.peer_id, endpoint.device_id)
}

/// Explicit local routing choices win over claims issued before that choice.
pub fn note_manual_endpoint(
    store: &Store,
    context: &RoomContext,
    endpoint: &PeerEndpoint,
    now_ms: i64,
) -> Result<()> {
    store.update_local_state(
        &cache_key(context),
        |previous: Option<EndpointHintCache>| {
            let mut cache = previous.unwrap_or_default();
            cache.manual_since.retain(|_, imported| {
                *imported > now_ms.saturating_sub(ENDPOINT_CLAIM_LIFETIME_MS + MAX_CLOCK_SKEW_MS)
            });
            if cache.manual_since.len() >= MAX_CLAIMS
                && !cache.manual_since.contains_key(&device_key(endpoint))
            {
                bail!("too many explicit endpoint overrides");
            }
            cache.manual_since.insert(device_key(endpoint), now_ms);
            Ok(cache)
        },
    )
}

pub fn endpoint_claims(
    store: &Store,
    context: &RoomContext,
    now_ms: i64,
) -> Result<Vec<EndpointClaimV1>> {
    let cache: EndpointHintCache = store.local_state(&cache_key(context))?.unwrap_or_default();
    if cache.claims.is_empty() {
        return Ok(Vec::new());
    }
    let governance = derive_governance_state(
        &store.room_events(&context.governance_room_id)?,
        context,
        now_ms,
    );
    Ok(cache
        .claims
        .into_iter()
        .take(MAX_CLAIMS)
        .filter(|claim| {
            claim.issued_ms
                > *cache
                    .manual_since
                    .get(&device_key(&claim.endpoint))
                    .unwrap_or(&i64::MIN)
                && claim
                    .validate_with_governance(store, context, now_ms, &governance)
                    .is_ok()
        })
        .collect())
}

pub fn retain_endpoint_claims(
    store: &Store,
    context: &RoomContext,
    now_ms: i64,
    offered: Vec<EndpointClaimV1>,
) -> Result<()> {
    if offered.len() > MAX_CLAIMS {
        bail!("endpoint exchange exceeds claim count bound");
    }
    let governance = derive_governance_state(
        &store.room_events(&context.governance_room_id)?,
        context,
        now_ms,
    );
    let valid: Vec<_> = offered
        .into_iter()
        .filter(|claim| {
            claim
                .validate_with_governance(store, context, now_ms, &governance)
                .is_ok()
        })
        .collect();
    store.update_local_state(
        &cache_key(context),
        |previous: Option<EndpointHintCache>| {
            // Preserve each device's newest unexpired claim even when delivery is reordered.
            let mut cache = previous.unwrap_or_default();
            let claims = &mut cache.claims;
            claims.retain(|claim| {
                claim
                    .issued_ms
                    .saturating_add(ENDPOINT_CLAIM_LIFETIME_MS + MAX_CLOCK_SKEW_MS)
                    > now_ms
            });
            for next in valid {
                if let Some(old) = claims.iter_mut().find(|old| {
                    old.endpoint.peer_id == next.endpoint.peer_id
                        && old.endpoint.device_id == next.endpoint.device_id
                }) {
                    if (next.issued_ms, &next.sig) > (old.issued_ms, &old.sig) {
                        *old = next;
                    }
                } else {
                    claims.push(next);
                }
            }
            claims.sort_by(|a, b| b.issued_ms.cmp(&a.issued_ms).then(a.sig.cmp(&b.sig)));
            claims.truncate(MAX_CLAIMS);
            Ok(cache)
        },
    )
}

#[derive(Serialize, Deserialize)]
struct EndpointExchangeV1 {
    endpoint_exchange: u8,
    governance_room_id: String,
    claims: Vec<EndpointClaimV1>,
}

fn exchange_message(
    store: &Store,
    context: &RoomContext,
    now_ms: i64,
    loopback_transport: bool,
) -> Result<EndpointExchangeV1> {
    let mut message = EndpointExchangeV1 {
        endpoint_exchange: 1,
        governance_room_id: context.governance_room_id.clone(),
        claims: endpoint_claims(store, context, now_ms)?
            .into_iter()
            .filter(|claim| loopback_transport || !claim.endpoint.addr.ip().is_loopback())
            .collect(),
    };
    while serde_json::to_vec(&message)?.len() > MAX_EXCHANGE_BYTES {
        if message.claims.pop().is_none() {
            bail!("endpoint exchange metadata exceeds byte bound");
        }
    }
    Ok(message)
}

impl QuicNode {
    pub async fn exchange_endpoints(
        &self,
        remote: &PeerEndpoint,
        store: &mut Store,
        context: &RoomContext,
        now_ms: i64,
    ) -> Result<()> {
        authorized_device_key(store, context, &remote.peer_id, &remote.device_id, now_ms)?;
        let authenticated = tokio::time::timeout(
            SYNC_CONNECT_TIMEOUT,
            self.connect(
                remote.addr,
                remote.certificate_der()?,
                &remote.peer_id,
                &remote.device_id,
            ),
        )
        .await
        .context("endpoint exchange connect timed out")??;
        let (mut send, recv) =
            tokio::time::timeout(PEER_IO_TIMEOUT, authenticated.connection.open_bi()).await??;
        send_json(
            &mut send,
            &exchange_message(
                store,
                context,
                now_ms,
                authenticated.connection.remote_address().ip().is_loopback(),
            )?,
        )
        .await?;
        let response: EndpointExchangeV1 = recv_json(recv, MAX_EXCHANGE_BYTES).await?;
        if response.claims.len() > MAX_CLAIMS {
            bail!("endpoint exchange exceeds claim count bound");
        }
        if response.endpoint_exchange != 1
            || response.governance_room_id != context.governance_room_id
        {
            bail!("invalid endpoint exchange response");
        }
        let loopback_transport = authenticated.connection.remote_address().ip().is_loopback();
        retain_endpoint_claims(
            store,
            context,
            unix_ms(),
            response
                .claims
                .into_iter()
                .filter(|claim| loopback_transport || !claim.endpoint.addr.ip().is_loopback())
                .collect(),
        )?;
        authenticated
            .connection
            .close(0u32.into(), b"endpoint exchange done");
        Ok(())
    }

    pub(super) async fn serve_endpoint_exchange(
        &self,
        store: &Store,
        context: &RoomContext,
        now_ms: i64,
        authenticated: AuthenticatedConnection,
        mut send: quinn::SendStream,
        request: serde_json::Value,
    ) -> Result<ServedPeerRequest> {
        authorized_device_key(
            store,
            context,
            &authenticated.remote.peer_id,
            &authenticated.remote.device_id,
            now_ms,
        )?;
        if serde_json::to_vec(&request)?.len() > MAX_EXCHANGE_BYTES {
            bail!("endpoint exchange exceeds byte bound");
        }
        let request: EndpointExchangeV1 = serde_json::from_value(request)?;
        if request.endpoint_exchange != 1
            || request.governance_room_id != context.governance_room_id
        {
            bail!("invalid endpoint exchange request");
        }
        if request.claims.len() > MAX_CLAIMS {
            bail!("endpoint exchange exceeds claim count bound");
        }
        let loopback_transport = authenticated.connection.remote_address().ip().is_loopback();
        retain_endpoint_claims(
            store,
            context,
            now_ms,
            request
                .claims
                .into_iter()
                .filter(|claim| loopback_transport || !claim.endpoint.addr.ip().is_loopback())
                .collect(),
        )?;
        send_json(
            &mut send,
            &exchange_message(
                store,
                context,
                now_ms,
                authenticated.connection.remote_address().ip().is_loopback(),
            )?,
        )
        .await?;
        Ok(ServedPeerRequest::EndpointExchange)
    }
}
