use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use voxelle_update::{ReleaseKeyRole, ReleaseManifestV1, TrustedReleaseKey};

pub const BETA_EVIDENCE_FORMAT_V1: &str = "voxelle-beta-evidence/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaEvidenceV1 {
    pub format: String,
    pub release_id: String,
    pub sequence: u64,
    pub source_commit: String,
    pub distribution: DistributionEvidenceV1,
    pub windows: WindowsEvidenceV1,
    pub field: FieldEvidenceV1,
    pub human: HumanEvidenceV1,
    pub custody: CustodyEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionEvidenceV1 {
    pub github_release_url: String,
    pub public_readback_verified: bool,
    pub macos_dmg_verified: bool,
    pub macos_universal_binary: bool,
    pub macos_packaged_launch: bool,
    pub live_activation: bool,
    pub rollback_to_previous: bool,
    pub reactivated_current: bool,
    pub executed_utc: String,
    pub operator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsEvidenceV1 {
    pub installer_name: String,
    pub installer_sha256: String,
    pub os_product_name: String,
    pub os_version: String,
    pub os_build: String,
    pub architecture: String,
    pub installed_executable_name: String,
    pub process_started: bool,
    pub main_window_visible: bool,
    pub first_launch_utc: String,
    pub operator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEvidenceV1 {
    pub executed_utc: String,
    pub operator: String,
    pub machines: Vec<FieldMachineV1>,
    pub a_to_b_diagnose: bool,
    pub b_to_a_diagnose: bool,
    pub a_to_b_sync: bool,
    pub b_to_a_sync: bool,
    pub offline_inviter: OfflineInviterEvidenceV1,
    pub message_receipts: Vec<MessageReceiptV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMachineV1 {
    pub role: String,
    pub machine_fingerprint: String,
    pub principal_id: String,
    pub device_id: String,
    pub listen_addr: String,
    pub advertise_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineInviterEvidenceV1 {
    pub inviter_role: String,
    pub forwarder_role: String,
    pub joiner_role: String,
    pub inviter_offline: bool,
    pub joined_through_forwarder: bool,
    pub retained_history_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceiptV1 {
    pub author_role: String,
    pub message_marker: String,
    pub visible_on_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanEvidenceV1 {
    pub executed_utc: String,
    pub operator: String,
    pub assistive_technology: AssistiveTechnologyEvidenceV1,
    pub media: MediaEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistiveTechnologyEvidenceV1 {
    pub platform: String,
    pub technology: String,
    pub keyboard_only: bool,
    pub fresh_setup: bool,
    pub invite_join: bool,
    pub conversation: bool,
    pub recovery: bool,
    pub customization: bool,
    pub degraded_connection: bool,
    pub compact_window_navigation: bool,
    pub media_controls: bool,
    pub microphone_toggle_controls: bool,
    pub camera_toggle_controls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaEvidenceV1 {
    pub participant_roles: Vec<String>,
    pub physical_microphone_capture: bool,
    pub physical_camera_capture: bool,
    pub permission_denial_recovery: bool,
    pub direct_audio_observed_by_all: bool,
    pub direct_video_observed_by_all: bool,
    pub direct_connection_state_visible: bool,
    pub leave_stopped_capture: bool,
    pub missing_peer_state_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodyEvidenceV1 {
    pub release_key_id: String,
    pub recovery_key_id: String,
    pub release_storage: String,
    pub recovery_storage: String,
    pub separately_protected: bool,
    pub offline: bool,
    pub development_copies_removed: bool,
    pub restore_tested: bool,
    pub attested_utc: String,
    pub operator: String,
}

pub fn template(
    manifest: &ReleaseManifestV1,
    roots: &[TrustedReleaseKey],
    source_commit: String,
) -> Result<BetaEvidenceV1> {
    if !is_lower_hex_commit(&source_commit) {
        return Err(anyhow!(
            "source commit must be a full lowercase 40-character Git commit"
        ));
    }
    let windows = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "native-installer" && artifact.target == "windows-x86_64")
        .ok_or_else(|| anyhow!("signed release has no Windows x86-64 native installer"))?;
    let release_key = key_for_id_and_role(roots, &manifest.signer_key_id, ReleaseKeyRole::Release)?;
    let recovery_key = one_key_for_role(roots, ReleaseKeyRole::Recovery)?;
    Ok(BetaEvidenceV1 {
        format: BETA_EVIDENCE_FORMAT_V1.to_string(),
        release_id: manifest.release_id.clone(),
        sequence: manifest.sequence,
        source_commit,
        distribution: DistributionEvidenceV1 {
            github_release_url: format!(
                "https://github.com/x3haloed/voxelle/releases/tag/{}",
                manifest.release_id
            ),
            public_readback_verified: false,
            macos_dmg_verified: false,
            macos_universal_binary: false,
            macos_packaged_launch: false,
            live_activation: false,
            rollback_to_previous: false,
            reactivated_current: false,
            executed_utc: String::new(),
            operator: String::new(),
        },
        windows: WindowsEvidenceV1 {
            installer_name: windows.name.clone(),
            installer_sha256: windows.sha256.clone(),
            os_product_name: String::new(),
            os_version: String::new(),
            os_build: String::new(),
            architecture: String::new(),
            installed_executable_name: String::new(),
            process_started: false,
            main_window_visible: false,
            first_launch_utc: String::new(),
            operator: String::new(),
        },
        field: FieldEvidenceV1 {
            executed_utc: String::new(),
            operator: String::new(),
            machines: vec!["A", "B", "C"]
                .into_iter()
                .map(|role| FieldMachineV1 {
                    role: role.to_string(),
                    machine_fingerprint: String::new(),
                    principal_id: String::new(),
                    device_id: String::new(),
                    listen_addr: String::new(),
                    advertise_addr: String::new(),
                })
                .collect(),
            a_to_b_diagnose: false,
            b_to_a_diagnose: false,
            a_to_b_sync: false,
            b_to_a_sync: false,
            offline_inviter: OfflineInviterEvidenceV1 {
                inviter_role: "A".to_string(),
                forwarder_role: "B".to_string(),
                joiner_role: "C".to_string(),
                inviter_offline: false,
                joined_through_forwarder: false,
                retained_history_visible: false,
            },
            message_receipts: vec!["A", "B", "C"]
                .into_iter()
                .map(|author_role| MessageReceiptV1 {
                    author_role: author_role.to_string(),
                    message_marker: String::new(),
                    visible_on_roles: Vec::new(),
                })
                .collect(),
        },
        human: HumanEvidenceV1 {
            executed_utc: String::new(),
            operator: String::new(),
            assistive_technology: AssistiveTechnologyEvidenceV1 {
                platform: String::new(),
                technology: String::new(),
                keyboard_only: false,
                fresh_setup: false,
                invite_join: false,
                conversation: false,
                recovery: false,
                customization: false,
                degraded_connection: false,
                compact_window_navigation: false,
                media_controls: false,
                microphone_toggle_controls: false,
                camera_toggle_controls: false,
            },
            media: MediaEvidenceV1 {
                participant_roles: Vec::new(),
                physical_microphone_capture: false,
                physical_camera_capture: false,
                permission_denial_recovery: false,
                direct_audio_observed_by_all: false,
                direct_video_observed_by_all: false,
                direct_connection_state_visible: false,
                leave_stopped_capture: false,
                missing_peer_state_visible: false,
            },
        },
        custody: CustodyEvidenceV1 {
            release_key_id: release_key.key_id.clone(),
            recovery_key_id: recovery_key.key_id.clone(),
            release_storage: String::new(),
            recovery_storage: String::new(),
            separately_protected: false,
            offline: false,
            development_copies_removed: false,
            restore_tested: false,
            attested_utc: String::new(),
            operator: String::new(),
        },
    })
}

pub fn validate(
    evidence: &BetaEvidenceV1,
    manifest: &ReleaseManifestV1,
    roots: &[TrustedReleaseKey],
    expected_commit: &str,
) -> Result<()> {
    validate_release_identity(evidence, manifest, expected_commit)?;
    validate_distribution(&evidence.distribution, manifest)?;
    validate_windows(&evidence.windows, manifest)?;
    validate_field(&evidence.field)?;
    validate_human(&evidence.human, &evidence.field)?;
    validate_custody(&evidence.custody, manifest, roots)?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct EvidenceStatus {
    pub section: &'static str,
    pub error: Option<String>,
}

pub fn status(
    evidence: &BetaEvidenceV1,
    manifest: &ReleaseManifestV1,
    roots: &[TrustedReleaseKey],
    expected_commit: &str,
) -> Vec<EvidenceStatus> {
    [
        (
            "release identity",
            validate_release_identity(evidence, manifest, expected_commit),
        ),
        (
            "distribution",
            validate_distribution(&evidence.distribution, manifest),
        ),
        ("Windows", validate_windows(&evidence.windows, manifest)),
        ("field", validate_field(&evidence.field)),
        ("human", validate_human(&evidence.human, &evidence.field)),
        (
            "custody",
            validate_custody(&evidence.custody, manifest, roots),
        ),
    ]
    .into_iter()
    .map(|(section, result)| EvidenceStatus {
        section,
        error: result.err().map(|error| error.to_string()),
    })
    .collect()
}

fn validate_release_identity(
    evidence: &BetaEvidenceV1,
    manifest: &ReleaseManifestV1,
    expected_commit: &str,
) -> Result<()> {
    if evidence.format != BETA_EVIDENCE_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported beta evidence format {}",
            evidence.format
        ));
    }
    if evidence.release_id != manifest.release_id || evidence.sequence != manifest.sequence {
        return Err(anyhow!(
            "beta evidence does not identify the signed release"
        ));
    }
    if evidence.source_commit != expected_commit || !is_lower_hex_commit(expected_commit) {
        return Err(anyhow!(
            "beta evidence source commit does not match the expected commit"
        ));
    }
    Ok(())
}

pub fn record_human(evidence: &mut BetaEvidenceV1, human: HumanEvidenceV1) -> Result<()> {
    if evidence.format != BETA_EVIDENCE_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported beta evidence format {}",
            evidence.format
        ));
    }
    validate_human(&human, &evidence.field)?;
    evidence.human = human;
    Ok(())
}

pub fn record_field(evidence: &mut BetaEvidenceV1, field: FieldEvidenceV1) -> Result<()> {
    if evidence.format != BETA_EVIDENCE_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported beta evidence format {}",
            evidence.format
        ));
    }
    validate_field(&field)?;
    evidence.field = field;
    Ok(())
}

pub fn record_distribution(
    evidence: &mut BetaEvidenceV1,
    distribution: DistributionEvidenceV1,
    manifest: &ReleaseManifestV1,
) -> Result<()> {
    if evidence.format != BETA_EVIDENCE_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported beta evidence format {}",
            evidence.format
        ));
    }
    if evidence.release_id != manifest.release_id || evidence.sequence != manifest.sequence {
        return Err(anyhow!(
            "beta evidence does not identify the signed release"
        ));
    }
    validate_distribution(&distribution, manifest)?;
    evidence.distribution = distribution;
    Ok(())
}

pub fn record_custody(
    evidence: &mut BetaEvidenceV1,
    mut custody: CustodyEvidenceV1,
    manifest: &ReleaseManifestV1,
    roots: &[TrustedReleaseKey],
) -> Result<()> {
    if evidence.format != BETA_EVIDENCE_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported beta evidence format {}",
            evidence.format
        ));
    }
    if evidence.release_id != manifest.release_id || evidence.sequence != manifest.sequence {
        return Err(anyhow!(
            "beta evidence does not identify the signed release"
        ));
    }
    custody.release_key_id =
        key_for_id_and_role(roots, &manifest.signer_key_id, ReleaseKeyRole::Release)?
            .key_id
            .clone();
    custody.recovery_key_id = one_key_for_role(roots, ReleaseKeyRole::Recovery)?
        .key_id
        .clone();
    validate_custody(&custody, manifest, roots)?;
    evidence.custody = custody;
    Ok(())
}

fn validate_distribution(
    distribution: &DistributionEvidenceV1,
    manifest: &ReleaseManifestV1,
) -> Result<()> {
    let expected_url = format!(
        "https://github.com/x3haloed/voxelle/releases/tag/{}",
        manifest.release_id
    );
    if distribution.github_release_url != expected_url {
        return Err(anyhow!(
            "distribution receipt does not identify the expected GitHub Release"
        ));
    }
    if !(distribution.public_readback_verified
        && distribution.macos_dmg_verified
        && distribution.macos_universal_binary
        && distribution.macos_packaged_launch
        && distribution.live_activation
        && distribution.rollback_to_previous
        && distribution.reactivated_current)
    {
        return Err(anyhow!(
            "public readback and complete packaged macOS release evidence are required"
        ));
    }
    require_text("distribution timestamp", &distribution.executed_utc)?;
    require_text("distribution operator", &distribution.operator)?;
    Ok(())
}

fn validate_windows(windows: &WindowsEvidenceV1, manifest: &ReleaseManifestV1) -> Result<()> {
    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "native-installer" && artifact.target == "windows-x86_64")
        .ok_or_else(|| anyhow!("signed release has no Windows x86-64 native installer"))?;
    if windows.installer_name != artifact.name || windows.installer_sha256 != artifact.sha256 {
        return Err(anyhow!(
            "Windows receipt does not match the signed installer"
        ));
    }
    require_text("Windows OS product name", &windows.os_product_name)?;
    require_text("Windows OS version", &windows.os_version)?;
    require_text("Windows OS build", &windows.os_build)?;
    require_text(
        "Windows installed executable name",
        &windows.installed_executable_name,
    )?;
    require_text("Windows first-launch timestamp", &windows.first_launch_utc)?;
    require_text("Windows operator", &windows.operator)?;
    if !windows
        .os_product_name
        .to_ascii_lowercase()
        .contains("windows")
    {
        return Err(anyhow!(
            "Windows smoke receipt must identify a Windows OS product"
        ));
    }
    if windows.architecture != "X64" {
        return Err(anyhow!(
            "Windows smoke test must execute on native X64 Windows"
        ));
    }
    if !windows
        .installed_executable_name
        .eq_ignore_ascii_case("voxelle-tauri-host.exe")
    {
        return Err(anyhow!(
            "Windows smoke receipt does not identify the Voxelle executable"
        ));
    }
    if !windows.process_started || !windows.main_window_visible {
        return Err(anyhow!(
            "Windows process start and visible main window are both required"
        ));
    }
    Ok(())
}

fn validate_field(field: &FieldEvidenceV1) -> Result<()> {
    require_text("field-test timestamp", &field.executed_utc)?;
    require_text("field-test operator", &field.operator)?;
    let expected_roles = roles();
    if field.machines.len() != expected_roles.len() {
        return Err(anyhow!("field test requires exactly three machines"));
    }
    let mut by_role = HashMap::new();
    let mut machines = HashSet::new();
    let mut principals = HashSet::new();
    let mut devices = HashSet::new();
    for machine in &field.machines {
        if !expected_roles.contains(machine.role.as_str())
            || by_role.insert(&machine.role, machine).is_some()
        {
            return Err(anyhow!(
                "field-test machine roles must be exactly A, B, and C"
            ));
        }
        require_text("machine fingerprint", &machine.machine_fingerprint)?;
        require_text("principal ID", &machine.principal_id)?;
        require_text("device ID", &machine.device_id)?;
        if !machines.insert(&machine.machine_fingerprint)
            || !principals.insert(&machine.principal_id)
            || !devices.insert(&machine.device_id)
        {
            return Err(anyhow!(
                "field-test machines, principals, and devices must be distinct"
            ));
        }
        validate_ipv6_socket("listen", &machine.listen_addr, true)?;
        validate_ipv6_socket("advertise", &machine.advertise_addr, false)?;
    }
    if !(field.a_to_b_diagnose && field.b_to_a_diagnose && field.a_to_b_sync && field.b_to_a_sync) {
        return Err(anyhow!(
            "bidirectional A/B diagnosis and sync must all succeed"
        ));
    }
    let offline = &field.offline_inviter;
    if offline.inviter_role != "A" || offline.forwarder_role != "B" || offline.joiner_role != "C" {
        return Err(anyhow!(
            "offline-inviter roles must be A inviter, B forwarder, C joiner"
        ));
    }
    if !(offline.inviter_offline
        && offline.joined_through_forwarder
        && offline.retained_history_visible)
    {
        return Err(anyhow!(
            "offline-inviter join and retained-history checks must succeed"
        ));
    }
    if field.message_receipts.len() != expected_roles.len() {
        return Err(anyhow!(
            "one converged message receipt is required from each role"
        ));
    }
    let mut authors = BTreeSet::new();
    let expected_visible: BTreeSet<String> =
        expected_roles.iter().map(|role| role.to_string()).collect();
    for receipt in &field.message_receipts {
        if !expected_roles.contains(receipt.author_role.as_str())
            || !authors.insert(receipt.author_role.as_str())
        {
            return Err(anyhow!("message authors must be exactly A, B, and C"));
        }
        require_text("message marker", &receipt.message_marker)?;
        let visible: BTreeSet<String> = receipt.visible_on_roles.iter().cloned().collect();
        if visible != expected_visible || visible.len() != receipt.visible_on_roles.len() {
            return Err(anyhow!(
                "every field-test message must be visible on A, B, and C"
            ));
        }
    }
    Ok(())
}

fn validate_human(human: &HumanEvidenceV1, field: &FieldEvidenceV1) -> Result<()> {
    require_text("human-test timestamp", &human.executed_utc)?;
    require_text("human-test operator", &human.operator)?;

    let assistive = &human.assistive_technology;
    if assistive.platform != "macOS" && assistive.platform != "Windows" {
        return Err(anyhow!(
            "assistive-technology evidence must identify macOS or Windows"
        ));
    }
    require_specific_text("assistive technology", &assistive.technology)?;
    if !(assistive.keyboard_only
        && assistive.fresh_setup
        && assistive.invite_join
        && assistive.conversation
        && assistive.recovery
        && assistive.customization
        && assistive.degraded_connection
        && assistive.compact_window_navigation
        && assistive.media_controls
        && assistive.microphone_toggle_controls
        && assistive.camera_toggle_controls)
    {
        return Err(anyhow!(
            "keyboard-only assistive-technology evidence must complete setup, join, conversation, recovery, customization, degraded-connection, compact-window navigation, media-control, microphone-toggle, and camera-toggle paths"
        ));
    }

    let media = &human.media;
    if !(2..=field.machines.len()).contains(&media.participant_roles.len()) {
        return Err(anyhow!(
            "physical media evidence requires at least two field-test machines"
        ));
    }
    let field_roles: BTreeSet<&str> = field
        .machines
        .iter()
        .map(|machine| machine.role.as_str())
        .collect();
    let media_roles: BTreeSet<&str> = media.participant_roles.iter().map(String::as_str).collect();
    if media_roles.len() != media.participant_roles.len()
        || !media_roles.iter().all(|role| field_roles.contains(role))
    {
        return Err(anyhow!(
            "media participant roles must be distinct machines from the field receipt"
        ));
    }
    if !(media.physical_microphone_capture
        && media.physical_camera_capture
        && media.permission_denial_recovery
        && media.direct_audio_observed_by_all
        && media.direct_video_observed_by_all
        && media.direct_connection_state_visible
        && media.leave_stopped_capture
        && media.missing_peer_state_visible)
    {
        return Err(anyhow!(
            "physical media evidence must cover capture, permission recovery, direct audio/video, connection state, leave cleanup, and missing-peer state"
        ));
    }
    Ok(())
}

fn validate_custody(
    custody: &CustodyEvidenceV1,
    manifest: &ReleaseManifestV1,
    roots: &[TrustedReleaseKey],
) -> Result<()> {
    let release = key_for_id_and_role(roots, &manifest.signer_key_id, ReleaseKeyRole::Release)?;
    let recovery = one_key_for_role(roots, ReleaseKeyRole::Recovery)?;
    if custody.release_key_id != release.key_id || custody.recovery_key_id != recovery.key_id {
        return Err(anyhow!(
            "custody receipt key IDs do not match the trusted capability roles"
        ));
    }
    require_text("release-key storage description", &custody.release_storage)?;
    require_text(
        "recovery-key storage description",
        &custody.recovery_storage,
    )?;
    require_text("custody timestamp", &custody.attested_utc)?;
    require_text("custody operator", &custody.operator)?;
    if custody.release_storage == custody.recovery_storage {
        return Err(anyhow!(
            "release and recovery keys must not claim the same storage"
        ));
    }
    if !(custody.separately_protected
        && custody.offline
        && custody.development_copies_removed
        && custody.restore_tested)
    {
        return Err(anyhow!("separate protection, offline custody, development-copy removal, and restore testing are required"));
    }
    Ok(())
}

fn validate_ipv6_socket(label: &str, value: &str, allow_unspecified: bool) -> Result<()> {
    let addr: SocketAddr = value
        .parse()
        .with_context(|| format!("parse {label} socket address {value}"))?;
    let IpAddr::V6(ip) = addr.ip() else {
        return Err(anyhow!("{label} address must be IPv6"));
    };
    let documentation_only = ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8;
    if ip.is_loopback()
        || (!allow_unspecified && ip.is_unspecified())
        || ip.is_multicast()
        || documentation_only
    {
        return Err(anyhow!("{label} address must be usable and non-loopback"));
    }
    Ok(())
}

fn one_key_for_role(
    roots: &[TrustedReleaseKey],
    role: ReleaseKeyRole,
) -> Result<&TrustedReleaseKey> {
    let keys: Vec<_> = roots.iter().filter(|key| key.role == role).collect();
    if keys.len() != 1 {
        return Err(anyhow!(
            "beta evidence requires exactly one {:?} trust root",
            role
        ));
    }
    Ok(keys[0])
}

fn key_for_id_and_role<'a>(
    roots: &'a [TrustedReleaseKey],
    key_id: &str,
    role: ReleaseKeyRole,
) -> Result<&'a TrustedReleaseKey> {
    roots
        .iter()
        .find(|key| key.key_id == key_id && key.role == role)
        .ok_or_else(|| {
            anyhow!(
                "trusted key {key_id} does not carry the expected {:?} role",
                role
            )
        })
}

fn require_text(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 512 {
        return Err(anyhow!("{label} must be present and bounded"));
    }
    Ok(())
}

fn require_specific_text(label: &str, value: &str) -> Result<()> {
    require_text(label, value)?;
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "none" | "n/a" | "na" | "unknown"
    ) {
        return Err(anyhow!("{label} must identify the tool actually used"));
    }
    Ok(())
}

fn roles() -> BTreeSet<&'static str> {
    ["A", "B", "C"].into_iter().collect()
}

fn is_lower_hex_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxelle_update::ReleaseArtifactV1;

    fn manifest() -> ReleaseManifestV1 {
        ReleaseManifestV1 {
            format: "voxelle-release-manifest/v1".to_string(),
            release_id: "v0.1.0-beta.3".to_string(),
            sequence: 3,
            channel: "beta".to_string(),
            artifacts: vec![ReleaseArtifactV1 {
                name: "Voxelle.exe".to_string(),
                sha256: "ab".repeat(32),
                bytes: 10,
                kind: "native-installer".to_string(),
                target: "windows-x86_64".to_string(),
            }],
            signer_key_id: "release".to_string(),
            signature_b64: "signature".to_string(),
        }
    }

    fn roots() -> Vec<TrustedReleaseKey> {
        vec![
            TrustedReleaseKey {
                key_id: "release".to_string(),
                spki_b64: "release-public".to_string(),
                role: ReleaseKeyRole::Release,
            },
            TrustedReleaseKey {
                key_id: "recovery".to_string(),
                spki_b64: "recovery-public".to_string(),
                role: ReleaseKeyRole::Recovery,
            },
        ]
    }

    fn valid() -> BetaEvidenceV1 {
        BetaEvidenceV1 {
            format: BETA_EVIDENCE_FORMAT_V1.to_string(),
            release_id: "v0.1.0-beta.3".to_string(),
            sequence: 3,
            source_commit: "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f".to_string(),
            distribution: DistributionEvidenceV1 {
                github_release_url:
                    "https://github.com/x3haloed/voxelle/releases/tag/v0.1.0-beta.3".to_string(),
                public_readback_verified: true,
                macos_dmg_verified: true,
                macos_universal_binary: true,
                macos_packaged_launch: true,
                live_activation: true,
                rollback_to_previous: true,
                reactivated_current: true,
                executed_utc: "2026-08-14T20:00:00Z".to_string(),
                operator: "operator".to_string(),
            },
            windows: WindowsEvidenceV1 {
                installer_name: "Voxelle.exe".to_string(),
                installer_sha256: "ab".repeat(32),
                os_product_name: "Windows 11 Pro".to_string(),
                os_version: "10.0.26100".to_string(),
                os_build: "26100".to_string(),
                architecture: "X64".to_string(),
                installed_executable_name: "voxelle-tauri-host.exe".to_string(),
                process_started: true,
                main_window_visible: true,
                first_launch_utc: "2026-08-14T20:00:00Z".to_string(),
                operator: "operator".to_string(),
            },
            field: FieldEvidenceV1 {
                executed_utc: "2026-08-14T21:00:00Z".to_string(),
                operator: "operator".to_string(),
                machines: ["A", "B", "C"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, role)| FieldMachineV1 {
                        role: role.to_string(),
                        machine_fingerprint: format!("machine-{role}"),
                        principal_id: format!("principal-{role}"),
                        device_id: format!("device-{role}"),
                        listen_addr: "[::]:47000".to_string(),
                        advertise_addr: format!("[fd42::{index}]:47000"),
                    })
                    .collect(),
                a_to_b_diagnose: true,
                b_to_a_diagnose: true,
                a_to_b_sync: true,
                b_to_a_sync: true,
                offline_inviter: OfflineInviterEvidenceV1 {
                    inviter_role: "A".to_string(),
                    forwarder_role: "B".to_string(),
                    joiner_role: "C".to_string(),
                    inviter_offline: true,
                    joined_through_forwarder: true,
                    retained_history_visible: true,
                },
                message_receipts: ["A", "B", "C"]
                    .into_iter()
                    .map(|role| MessageReceiptV1 {
                        author_role: role.to_string(),
                        message_marker: format!("message-{role}"),
                        visible_on_roles: vec!["A".to_string(), "B".to_string(), "C".to_string()],
                    })
                    .collect(),
            },
            human: HumanEvidenceV1 {
                executed_utc: "2026-08-14T21:30:00Z".to_string(),
                operator: "operator".to_string(),
                assistive_technology: AssistiveTechnologyEvidenceV1 {
                    platform: "macOS".to_string(),
                    technology: "VoiceOver".to_string(),
                    keyboard_only: true,
                    fresh_setup: true,
                    invite_join: true,
                    conversation: true,
                    recovery: true,
                    customization: true,
                    degraded_connection: true,
                    compact_window_navigation: true,
                    media_controls: true,
                    microphone_toggle_controls: true,
                    camera_toggle_controls: true,
                },
                media: MediaEvidenceV1 {
                    participant_roles: vec!["A".to_string(), "B".to_string()],
                    physical_microphone_capture: true,
                    physical_camera_capture: true,
                    permission_denial_recovery: true,
                    direct_audio_observed_by_all: true,
                    direct_video_observed_by_all: true,
                    direct_connection_state_visible: true,
                    leave_stopped_capture: true,
                    missing_peer_state_visible: true,
                },
            },
            custody: CustodyEvidenceV1 {
                release_key_id: "release".to_string(),
                recovery_key_id: "recovery".to_string(),
                release_storage: "encrypted removable medium one".to_string(),
                recovery_storage: "encrypted removable medium two".to_string(),
                separately_protected: true,
                offline: true,
                development_copies_removed: true,
                restore_tested: true,
                attested_utc: "2026-08-14T22:00:00Z".to_string(),
                operator: "operator".to_string(),
            },
        }
    }

    #[test]
    fn complete_beta_evidence_validates() {
        validate(
            &valid(),
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f",
        )
        .expect("valid evidence");
    }

    #[test]
    fn status_reports_every_invalid_section_in_one_pass() {
        let mut evidence = valid();
        evidence.source_commit = "wrong".to_string();
        evidence.distribution.live_activation = false;
        evidence.windows.main_window_visible = false;
        evidence.field.machines[0].advertise_addr = "[::1]:47000".to_string();
        evidence.human.assistive_technology.recovery = false;
        evidence.custody.restore_tested = false;

        let observed = status(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f",
        );

        assert_eq!(observed.len(), 6);
        assert!(observed.iter().all(|item| item.error.is_some()));
        assert_eq!(observed[0].section, "release identity");
        assert_eq!(observed[5].section, "custody");
    }

    #[test]
    fn status_marks_complete_evidence_as_ready() {
        let observed = status(
            &valid(),
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f",
        );

        assert!(observed.iter().all(|item| item.error.is_none()));
    }

    #[test]
    fn human_recorder_validates_before_replacing_the_template_section() {
        let complete = valid();
        let observed = complete.human.clone();
        let mut receipt = complete.clone();
        receipt.human.operator.clear();
        receipt.human.assistive_technology.recovery = false;

        record_human(&mut receipt, observed.clone()).expect("record valid human evidence");
        assert_eq!(receipt.human.operator, "operator");
        assert!(receipt.human.assistive_technology.recovery);
        assert_eq!(receipt.human.media.participant_roles, vec!["A", "B"]);

        let mut refused = observed;
        refused.media.physical_camera_capture = false;
        let retained_operator = receipt.human.operator.clone();
        assert!(record_human(&mut receipt, refused).is_err());
        assert_eq!(receipt.human.operator, retained_operator);
        assert!(receipt.human.media.physical_camera_capture);
    }

    #[test]
    fn field_recorder_validates_before_replacing_the_template_section() {
        let complete = valid();
        let observed = complete.field.clone();
        let mut receipt = complete.clone();
        receipt.field.operator.clear();
        receipt.field.a_to_b_sync = false;

        record_field(&mut receipt, observed.clone()).expect("record valid field evidence");
        assert_eq!(receipt.field.operator, "operator");
        assert!(receipt.field.a_to_b_sync);
        assert_eq!(receipt.field.machines[2].role, "C");

        let mut refused = observed;
        refused.machines[2].advertise_addr = "[::1]:47000".to_string();
        let retained_operator = receipt.field.operator.clone();
        assert!(record_field(&mut receipt, refused).is_err());
        assert_eq!(receipt.field.operator, retained_operator);
        assert_ne!(receipt.field.machines[2].advertise_addr, "[::1]:47000");
    }

    #[test]
    fn distribution_recorder_binds_the_manifest_before_replacing_the_section() {
        let complete = valid();
        let observed = complete.distribution.clone();
        let mut receipt = complete.clone();
        receipt.distribution.operator.clear();
        receipt.distribution.live_activation = false;

        record_distribution(&mut receipt, observed.clone(), &manifest())
            .expect("record valid distribution evidence");
        assert_eq!(receipt.distribution.operator, "operator");
        assert!(receipt.distribution.live_activation);

        let mut refused = observed;
        refused.github_release_url =
            "https://github.com/x3haloed/voxelle/releases/tag/wrong".to_string();
        let retained_operator = receipt.distribution.operator.clone();
        assert!(record_distribution(&mut receipt, refused, &manifest()).is_err());
        assert_eq!(receipt.distribution.operator, retained_operator);
        assert!(receipt.distribution.live_activation);

        let mut wrong_receipt = receipt.clone();
        wrong_receipt.sequence += 1;
        assert!(
            record_distribution(&mut wrong_receipt, complete.distribution, &manifest()).is_err()
        );
    }

    #[test]
    fn custody_recorder_derives_capability_ids_before_replacing_the_section() {
        let complete = valid();
        let mut observed = complete.custody.clone();
        observed.release_key_id.clear();
        observed.recovery_key_id.clear();
        let mut receipt = complete.clone();
        receipt.custody.operator.clear();
        receipt.custody.restore_tested = false;

        record_custody(&mut receipt, observed.clone(), &manifest(), &roots())
            .expect("record valid custody evidence");
        assert_eq!(receipt.custody.release_key_id, "release");
        assert_eq!(receipt.custody.recovery_key_id, "recovery");
        assert!(receipt.custody.restore_tested);

        let mut refused = observed;
        refused.recovery_storage = refused.release_storage.clone();
        let retained_operator = receipt.custody.operator.clone();
        assert!(record_custody(&mut receipt, refused, &manifest(), &roots()).is_err());
        assert_eq!(receipt.custody.operator, retained_operator);
        assert_ne!(
            receipt.custody.release_storage,
            receipt.custody.recovery_storage
        );
    }

    #[test]
    fn loopback_or_incomplete_external_evidence_is_rejected() {
        let mut evidence = valid();
        evidence.field.machines[0].advertise_addr = "[::1]:47000".to_string();
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.field.machines[0].advertise_addr = "[2001:db8::1]:47000".to_string();
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.custody.restore_tested = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.windows.main_window_visible = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.assistive_technology.recovery = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence
            .human
            .assistive_technology
            .compact_window_navigation = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence
            .human
            .assistive_technology
            .microphone_toggle_controls = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.assistive_technology.camera_toggle_controls = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.assistive_technology.technology = "none".to_string();
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.media.physical_camera_capture = false;
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.media.participant_roles = vec!["A".to_string(), "A".to_string()];
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());

        let mut evidence = valid();
        evidence.human.media.participant_roles = vec!["A".to_string(), "D".to_string()];
        assert!(validate(
            &evidence,
            &manifest(),
            &roots(),
            "3a3b6234cdf0b8a4ccf727f7eb8774696bbafa0f"
        )
        .is_err());
    }
}
