use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use voxelle_app::builtin_product_generation;
use voxelle_core::Keypair;
use voxelle_update::{
    hex_sha256, package_signing_bytes, release_manifest_signing_bytes,
    trust_transition_signing_bytes, ReleaseArtifactV1, ReleaseKeyRole, ReleaseManifestV1,
    TrustTransitionV1, TrustedReleaseKey, TrustedReleaseKeysV1, UpdateManager, UpdatePackageV1,
    RELEASE_MANIFEST_FORMAT_V1, TRUST_TRANSITION_FORMAT_V1, UPDATE_FORMAT_V1,
};

mod evidence;

#[derive(Debug, Parser)]
#[command(
    name = "voxelle-release",
    about = "Manual Voxelle release signing and verification"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Keygen {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long, default_value = "release")]
        role: String,
    },
    GenerationTemplate {
        #[arg(long)]
        output: PathBuf,
    },
    PackageGeneration {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        generation: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        release_id: String,
        #[arg(long)]
        sequence: u64,
        #[arg(long, default_value = "beta")]
        channel: String,
        #[arg(long, default_value = "0.1.0")]
        min_kernel_version: String,
    },
    SignManifest {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        release_id: String,
        #[arg(long)]
        sequence: u64,
        #[arg(long, default_value = "beta")]
        channel: String,
        #[arg(long = "artifact", required = true)]
        artifacts: Vec<PathBuf>,
    },
    SignTrustTransition {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        sequence: u64,
        #[arg(long)]
        add_trust_roots: Option<PathBuf>,
        #[arg(long = "remove-key-id")]
        remove_key_ids: Vec<String>,
    },
    VerifyPackage {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        package: PathBuf,
        #[arg(long, default_value = "0.1.0")]
        kernel_version: String,
    },
    VerifyRelease {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        artifact_dir: PathBuf,
    },
    ListReleaseArtifacts {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    VerifyTrustTransition {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        transition: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
    BetaEvidenceTemplate {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        source_commit: String,
    },
    RecordHumanBetaEvidence(Box<RecordHumanBetaEvidenceArgs>),
    RecordFieldBetaEvidence(Box<RecordFieldBetaEvidenceArgs>),
    RecordDistributionBetaEvidence(Box<RecordDistributionBetaEvidenceArgs>),
    RecordCustodyBetaEvidence(Box<RecordCustodyBetaEvidenceArgs>),
    BetaEvidenceStatus {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long)]
        expected_commit: String,
    },
    VerifyBetaEvidence {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long)]
        expected_commit: String,
    },
    VerifySigningSecret {
        #[arg(long)]
        trust_roots: PathBuf,
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        role: String,
    },
}

#[derive(Debug, Args)]
struct RecordHumanBetaEvidenceArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    executed_utc: String,
    #[arg(long)]
    operator: String,
    #[arg(long, value_parser = ["macOS", "Windows"])]
    platform: String,
    #[arg(long)]
    technology: String,
    #[arg(long = "media-role", required = true, num_args = 2..=3, value_parser = ["A", "B", "C"])]
    media_roles: Vec<String>,
    #[arg(long, required = true)]
    attest_keyboard_only: bool,
    #[arg(long, required = true)]
    attest_fresh_setup: bool,
    #[arg(long, required = true)]
    attest_invite_join: bool,
    #[arg(long, required = true)]
    attest_conversation: bool,
    #[arg(long, required = true)]
    attest_recovery: bool,
    #[arg(long, required = true)]
    attest_customization: bool,
    #[arg(long, required = true)]
    attest_degraded_connection: bool,
    #[arg(long, required = true)]
    attest_compact_window_navigation: bool,
    #[arg(long, required = true)]
    attest_media_controls: bool,
    #[arg(long, required = true)]
    attest_microphone_toggle_controls: bool,
    #[arg(long, required = true)]
    attest_camera_toggle_controls: bool,
    #[arg(long, required = true)]
    attest_physical_microphone_capture: bool,
    #[arg(long, required = true)]
    attest_physical_camera_capture: bool,
    #[arg(long, required = true)]
    attest_permission_denial_recovery: bool,
    #[arg(long, required = true)]
    attest_direct_audio_observed_by_all: bool,
    #[arg(long, required = true)]
    attest_direct_video_observed_by_all: bool,
    #[arg(long, required = true)]
    attest_direct_connection_state_visible: bool,
    #[arg(long, required = true)]
    attest_leave_stopped_capture: bool,
    #[arg(long, required = true)]
    attest_missing_peer_state_visible: bool,
}

#[derive(Debug, Args)]
struct RecordFieldBetaEvidenceArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    executed_utc: String,
    #[arg(long)]
    operator: String,
    #[arg(long)]
    machine_a_fingerprint: String,
    #[arg(long)]
    machine_a_principal: String,
    #[arg(long)]
    machine_a_device: String,
    #[arg(long)]
    machine_a_listen: String,
    #[arg(long)]
    machine_a_advertise: String,
    #[arg(long)]
    machine_b_fingerprint: String,
    #[arg(long)]
    machine_b_principal: String,
    #[arg(long)]
    machine_b_device: String,
    #[arg(long)]
    machine_b_listen: String,
    #[arg(long)]
    machine_b_advertise: String,
    #[arg(long)]
    machine_c_fingerprint: String,
    #[arg(long)]
    machine_c_principal: String,
    #[arg(long)]
    machine_c_device: String,
    #[arg(long)]
    machine_c_listen: String,
    #[arg(long)]
    machine_c_advertise: String,
    #[arg(long)]
    message_a_marker: String,
    #[arg(long)]
    message_b_marker: String,
    #[arg(long)]
    message_c_marker: String,
    #[arg(long, required = true)]
    attest_a_to_b_diagnose: bool,
    #[arg(long, required = true)]
    attest_b_to_a_diagnose: bool,
    #[arg(long, required = true)]
    attest_a_to_b_sync: bool,
    #[arg(long, required = true)]
    attest_b_to_a_sync: bool,
    #[arg(long, required = true)]
    attest_inviter_a_offline: bool,
    #[arg(long, required = true)]
    attest_c_joined_through_b: bool,
    #[arg(long, required = true)]
    attest_c_retained_history_visible: bool,
    #[arg(long, required = true)]
    attest_a_message_visible_on_all: bool,
    #[arg(long, required = true)]
    attest_b_message_visible_on_all: bool,
    #[arg(long, required = true)]
    attest_c_message_visible_on_all: bool,
}

#[derive(Debug, Args)]
struct RecordDistributionBetaEvidenceArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    trust_roots: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    executed_utc: String,
    #[arg(long)]
    operator: String,
    #[arg(long, required = true)]
    attest_public_readback_verified: bool,
    #[arg(long, required = true)]
    attest_macos_dmg_verified: bool,
    #[arg(long, required = true)]
    attest_macos_universal_binary: bool,
    #[arg(long, required = true)]
    attest_macos_packaged_launch: bool,
    #[arg(long, required = true)]
    attest_live_activation: bool,
    #[arg(long, required = true)]
    attest_rollback_to_previous: bool,
    #[arg(long, required = true)]
    attest_reactivated_current: bool,
}

#[derive(Debug, Args)]
struct RecordCustodyBetaEvidenceArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    trust_roots: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    release_storage: String,
    #[arg(long)]
    recovery_storage: String,
    #[arg(long)]
    attested_utc: String,
    #[arg(long)]
    operator: String,
    #[arg(long, required = true)]
    attest_separately_protected: bool,
    #[arg(long, required = true)]
    attest_offline: bool,
    #[arg(long, required = true)]
    attest_development_copies_removed: bool,
    #[arg(long, required = true)]
    attest_restore_tested: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReleaseSigningSecretV1 {
    v: u8,
    key_id: String,
    secret_key_b64: String,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Keygen {
            secret,
            trust_roots,
            role,
        } => keygen(&secret, &trust_roots, &role),
        Command::GenerationTemplate { output } => {
            write_new_json(&output, &builtin_product_generation())?;
            println!("wrote {}", output.display());
            Ok(())
        }
        Command::PackageGeneration {
            secret,
            generation,
            output,
            release_id,
            sequence,
            channel,
            min_kernel_version,
        } => package_generation(
            &secret,
            &generation,
            &output,
            release_id,
            sequence,
            channel,
            min_kernel_version,
        ),
        Command::SignManifest {
            secret,
            output,
            release_id,
            sequence,
            channel,
            artifacts,
        } => sign_manifest(&secret, &output, release_id, sequence, channel, &artifacts),
        Command::SignTrustTransition {
            secret,
            output,
            sequence,
            add_trust_roots,
            remove_key_ids,
        } => sign_trust_transition(
            &secret,
            &output,
            sequence,
            add_trust_roots.as_deref(),
            remove_key_ids,
        ),
        Command::VerifyPackage {
            trust_roots,
            package,
            kernel_version,
        } => {
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", &kernel_version, roots)?;
            let bytes = fs::read(&package).context("read product update package")?;
            let verified = manager.verify_bytes(&bytes)?;
            println!(
                "verified {} sequence {} sha256 {}",
                verified.package().release_id,
                verified.package().sequence,
                verified.sha256()
            );
            Ok(())
        }
        Command::VerifyRelease {
            trust_roots,
            manifest,
            artifact_dir,
        } => verify_release(&trust_roots, &manifest, &artifact_dir),
        Command::ListReleaseArtifacts {
            trust_roots,
            manifest,
        } => list_release_artifacts(&trust_roots, &manifest),
        Command::VerifyTrustTransition {
            trust_roots,
            transition,
            state_dir,
        } => {
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(state_dir, "0.1.0", roots)?;
            let raw = fs::read(&transition).context("read trust transition")?;
            let verified = manager.verify_trust_transition_bytes(&raw)?;
            println!(
                "verified trust transition {} signed by {}",
                verified.sequence, verified.signer_key_id
            );
            Ok(())
        }
        Command::BetaEvidenceTemplate {
            trust_roots,
            manifest,
            output,
            source_commit,
        } => {
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", "0.1.0", roots.clone())?;
            let manifest = manager.verify_release_manifest_bytes(&fs::read(manifest)?)?;
            let template = evidence::template(&manifest, &roots, source_commit)?;
            write_new_json(&output, &template)?;
            println!("wrote beta evidence template {}", output.display());
            Ok(())
        }
        Command::RecordHumanBetaEvidence(args) => {
            let RecordHumanBetaEvidenceArgs {
                input,
                output,
                executed_utc,
                operator,
                platform,
                technology,
                media_roles,
                attest_keyboard_only,
                attest_fresh_setup,
                attest_invite_join,
                attest_conversation,
                attest_recovery,
                attest_customization,
                attest_degraded_connection,
                attest_compact_window_navigation,
                attest_media_controls,
                attest_microphone_toggle_controls,
                attest_camera_toggle_controls,
                attest_physical_microphone_capture,
                attest_physical_camera_capture,
                attest_permission_denial_recovery,
                attest_direct_audio_observed_by_all,
                attest_direct_video_observed_by_all,
                attest_direct_connection_state_visible,
                attest_leave_stopped_capture,
                attest_missing_peer_state_visible,
            } = *args;
            let mut receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&input).context("read beta evidence receipt")?)
                    .context("parse beta evidence receipt")?;
            let human = evidence::HumanEvidenceV1 {
                executed_utc,
                operator,
                assistive_technology: evidence::AssistiveTechnologyEvidenceV1 {
                    platform,
                    technology,
                    keyboard_only: attest_keyboard_only,
                    fresh_setup: attest_fresh_setup,
                    invite_join: attest_invite_join,
                    conversation: attest_conversation,
                    recovery: attest_recovery,
                    customization: attest_customization,
                    degraded_connection: attest_degraded_connection,
                    compact_window_navigation: attest_compact_window_navigation,
                    media_controls: attest_media_controls,
                    microphone_toggle_controls: attest_microphone_toggle_controls,
                    camera_toggle_controls: attest_camera_toggle_controls,
                },
                media: evidence::MediaEvidenceV1 {
                    participant_roles: media_roles,
                    physical_microphone_capture: attest_physical_microphone_capture,
                    physical_camera_capture: attest_physical_camera_capture,
                    permission_denial_recovery: attest_permission_denial_recovery,
                    direct_audio_observed_by_all: attest_direct_audio_observed_by_all,
                    direct_video_observed_by_all: attest_direct_video_observed_by_all,
                    direct_connection_state_visible: attest_direct_connection_state_visible,
                    leave_stopped_capture: attest_leave_stopped_capture,
                    missing_peer_state_visible: attest_missing_peer_state_visible,
                },
            };
            evidence::record_human(&mut receipt, human)?;
            write_new_json(&output, &receipt)?;
            println!("recorded human beta evidence in {}", output.display());
            Ok(())
        }
        Command::RecordFieldBetaEvidence(args) => {
            let RecordFieldBetaEvidenceArgs {
                input,
                output,
                executed_utc,
                operator,
                machine_a_fingerprint,
                machine_a_principal,
                machine_a_device,
                machine_a_listen,
                machine_a_advertise,
                machine_b_fingerprint,
                machine_b_principal,
                machine_b_device,
                machine_b_listen,
                machine_b_advertise,
                machine_c_fingerprint,
                machine_c_principal,
                machine_c_device,
                machine_c_listen,
                machine_c_advertise,
                message_a_marker,
                message_b_marker,
                message_c_marker,
                attest_a_to_b_diagnose,
                attest_b_to_a_diagnose,
                attest_a_to_b_sync,
                attest_b_to_a_sync,
                attest_inviter_a_offline,
                attest_c_joined_through_b,
                attest_c_retained_history_visible,
                attest_a_message_visible_on_all,
                attest_b_message_visible_on_all,
                attest_c_message_visible_on_all,
            } = *args;
            let mut receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&input).context("read beta evidence receipt")?)
                    .context("parse beta evidence receipt")?;
            let machine =
                |role: &str,
                 machine_fingerprint: String,
                 principal_id: String,
                 device_id: String,
                 listen_addr: String,
                 advertise_addr: String| evidence::FieldMachineV1 {
                    role: role.to_string(),
                    machine_fingerprint,
                    principal_id,
                    device_id,
                    listen_addr,
                    advertise_addr,
                };
            let visible_on_all = vec!["A".to_string(), "B".to_string(), "C".to_string()];
            let message = |author_role: &str, message_marker: String| evidence::MessageReceiptV1 {
                author_role: author_role.to_string(),
                message_marker,
                visible_on_roles: visible_on_all.clone(),
            };
            let field = evidence::FieldEvidenceV1 {
                executed_utc,
                operator,
                machines: vec![
                    machine(
                        "A",
                        machine_a_fingerprint,
                        machine_a_principal,
                        machine_a_device,
                        machine_a_listen,
                        machine_a_advertise,
                    ),
                    machine(
                        "B",
                        machine_b_fingerprint,
                        machine_b_principal,
                        machine_b_device,
                        machine_b_listen,
                        machine_b_advertise,
                    ),
                    machine(
                        "C",
                        machine_c_fingerprint,
                        machine_c_principal,
                        machine_c_device,
                        machine_c_listen,
                        machine_c_advertise,
                    ),
                ],
                a_to_b_diagnose: attest_a_to_b_diagnose,
                b_to_a_diagnose: attest_b_to_a_diagnose,
                a_to_b_sync: attest_a_to_b_sync,
                b_to_a_sync: attest_b_to_a_sync,
                offline_inviter: evidence::OfflineInviterEvidenceV1 {
                    inviter_role: "A".to_string(),
                    forwarder_role: "B".to_string(),
                    joiner_role: "C".to_string(),
                    inviter_offline: attest_inviter_a_offline,
                    joined_through_forwarder: attest_c_joined_through_b,
                    retained_history_visible: attest_c_retained_history_visible,
                },
                message_receipts: vec![
                    message("A", message_a_marker),
                    message("B", message_b_marker),
                    message("C", message_c_marker),
                ],
            };
            if !(attest_a_message_visible_on_all
                && attest_b_message_visible_on_all
                && attest_c_message_visible_on_all)
            {
                return Err(anyhow!(
                    "every field message visibility attestation is required"
                ));
            }
            evidence::record_field(&mut receipt, field)?;
            write_new_json(&output, &receipt)?;
            println!("recorded field beta evidence in {}", output.display());
            Ok(())
        }
        Command::RecordDistributionBetaEvidence(args) => {
            let RecordDistributionBetaEvidenceArgs {
                input,
                output,
                trust_roots,
                manifest,
                executed_utc,
                operator,
                attest_public_readback_verified,
                attest_macos_dmg_verified,
                attest_macos_universal_binary,
                attest_macos_packaged_launch,
                attest_live_activation,
                attest_rollback_to_previous,
                attest_reactivated_current,
            } = *args;
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", "0.1.0", roots)?;
            let manifest = manager.verify_release_manifest_bytes(&fs::read(manifest)?)?;
            let mut receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&input).context("read beta evidence receipt")?)
                    .context("parse beta evidence receipt")?;
            let distribution = evidence::DistributionEvidenceV1 {
                github_release_url: format!(
                    "https://github.com/x3haloed/voxelle/releases/tag/{}",
                    manifest.release_id
                ),
                public_readback_verified: attest_public_readback_verified,
                macos_dmg_verified: attest_macos_dmg_verified,
                macos_universal_binary: attest_macos_universal_binary,
                macos_packaged_launch: attest_macos_packaged_launch,
                live_activation: attest_live_activation,
                rollback_to_previous: attest_rollback_to_previous,
                reactivated_current: attest_reactivated_current,
                executed_utc,
                operator,
            };
            evidence::record_distribution(&mut receipt, distribution, &manifest)?;
            write_new_json(&output, &receipt)?;
            println!(
                "recorded distribution beta evidence in {}",
                output.display()
            );
            Ok(())
        }
        Command::RecordCustodyBetaEvidence(args) => {
            let RecordCustodyBetaEvidenceArgs {
                input,
                output,
                trust_roots,
                manifest,
                release_storage,
                recovery_storage,
                attested_utc,
                operator,
                attest_separately_protected,
                attest_offline,
                attest_development_copies_removed,
                attest_restore_tested,
            } = *args;
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", "0.1.0", roots.clone())?;
            let manifest = manager.verify_release_manifest_bytes(&fs::read(manifest)?)?;
            let mut receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&input).context("read beta evidence receipt")?)
                    .context("parse beta evidence receipt")?;
            let custody = evidence::CustodyEvidenceV1 {
                release_key_id: String::new(),
                recovery_key_id: String::new(),
                release_storage,
                recovery_storage,
                separately_protected: attest_separately_protected,
                offline: attest_offline,
                development_copies_removed: attest_development_copies_removed,
                restore_tested: attest_restore_tested,
                attested_utc,
                operator,
            };
            evidence::record_custody(&mut receipt, custody, &manifest, &roots)?;
            write_new_json(&output, &receipt)?;
            println!("recorded custody beta evidence in {}", output.display());
            Ok(())
        }
        Command::BetaEvidenceStatus {
            trust_roots,
            manifest,
            evidence: evidence_path,
            expected_commit,
        } => {
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", "0.1.0", roots.clone())?;
            let manifest = manager.verify_release_manifest_bytes(&fs::read(manifest)?)?;
            let receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&evidence_path).context("read beta evidence")?)
                    .context("parse beta evidence")?;
            let status = evidence::status(&receipt, &manifest, &roots, &expected_commit);
            let mut failures = 0;
            for item in status {
                if let Some(error) = item.error {
                    failures += 1;
                    println!("FAIL {}: {}", item.section, error);
                } else {
                    println!("PASS {}", item.section);
                }
            }
            if failures == 0 {
                println!("all beta evidence sections are complete and internally consistent");
                Ok(())
            } else {
                Err(anyhow!(
                    "beta evidence has {failures} incomplete or invalid sections"
                ))
            }
        }
        Command::VerifyBetaEvidence {
            trust_roots,
            manifest,
            evidence: evidence_path,
            expected_commit,
        } => {
            let roots = read_trust_roots(&trust_roots)?;
            let manager = UpdateManager::new(".", "0.1.0", roots.clone())?;
            let manifest = manager.verify_release_manifest_bytes(&fs::read(manifest)?)?;
            let receipt: evidence::BetaEvidenceV1 =
                serde_json::from_slice(&fs::read(&evidence_path).context("read beta evidence")?)
                    .context("parse beta evidence")?;
            evidence::validate(&receipt, &manifest, &roots, &expected_commit)?;
            println!(
                "verified complete beta evidence for {} at {}",
                receipt.release_id, receipt.source_commit
            );
            Ok(())
        }
        Command::VerifySigningSecret {
            trust_roots,
            secret,
            role,
        } => verify_signing_secret(&trust_roots, &secret, &role),
    }
}

fn verify_signing_secret(trust_roots: &Path, secret_path: &Path, role: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(secret_path)
        .with_context(|| format!("inspect signing secret {}", secret_path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(anyhow!("signing secret must be a regular non-symlink file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(anyhow!(
                "signing secret must not be accessible to group or others"
            ));
        }
    }
    let expected_role = match role {
        "release" => ReleaseKeyRole::Release,
        "recovery" => ReleaseKeyRole::Recovery,
        value => return Err(anyhow!("unsupported release key role {value}")),
    };
    let key = read_signing_key(secret_path)?;
    let roots = read_trust_roots(trust_roots)?;
    let trusted = roots
        .iter()
        .find(|trusted| trusted.key_id == key.id)
        .ok_or_else(|| anyhow!("signing secret is not present in the trusted release roots"))?;
    if trusted.role != expected_role || trusted.spki_b64 != key.spki_b64 {
        return Err(anyhow!(
            "signing secret does not match the expected trusted capability"
        ));
    }
    println!("verified {role} signing secret {}", key.id);
    Ok(())
}

fn sign_trust_transition(
    secret_path: &Path,
    output: &Path,
    sequence: u64,
    add_trust_roots: Option<&Path>,
    remove_key_ids: Vec<String>,
) -> Result<()> {
    let key = read_signing_key(secret_path)?;
    let add = match add_trust_roots {
        Some(path) => read_trust_roots(path)?,
        None => Vec::new(),
    };
    let mut transition = TrustTransitionV1 {
        format: TRUST_TRANSITION_FORMAT_V1.to_string(),
        sequence,
        add,
        remove_key_ids,
        signer_key_id: key.id.clone(),
        signature_b64: String::new(),
    };
    transition.signature_b64 = key.sign(&trust_transition_signing_bytes(&transition)?);
    write_new_json(output, &transition)?;
    println!("wrote signed trust transition {}", output.display());
    Ok(())
}

fn keygen(secret_path: &Path, trust_roots_path: &Path, role: &str) -> Result<()> {
    if trust_roots_path.exists() {
        return Err(anyhow!(
            "refusing to overwrite trust roots {}",
            trust_roots_path.display()
        ));
    }
    let key = Keypair::generate().context("generate Ed25519 release key")?;
    let role = match role {
        "release" => ReleaseKeyRole::Release,
        "recovery" => ReleaseKeyRole::Recovery,
        value => return Err(anyhow!("unsupported release key role {value}")),
    };
    let secret = ReleaseSigningSecretV1 {
        v: 1,
        key_id: key.id.clone(),
        secret_key_b64: key.secret_key_b64(),
    };
    write_secret_new(secret_path, &secret)?;
    let roots = TrustedReleaseKeysV1 {
        v: 1,
        keys: vec![TrustedReleaseKey {
            key_id: key.id.clone(),
            spki_b64: key.spki_b64,
            role,
        }],
    };
    write_new_json(trust_roots_path, &roots)?;
    println!("generated release root {}", key.id);
    println!("private signing key: {}", secret_path.display());
    println!("public trust roots: {}", trust_roots_path.display());
    Ok(())
}

fn package_generation(
    secret_path: &Path,
    generation_path: &Path,
    output: &Path,
    release_id: String,
    sequence: u64,
    channel: String,
    min_kernel_version: String,
) -> Result<()> {
    let key = read_signing_key(secret_path)?;
    let payload = serde_json::from_slice(&fs::read(generation_path).context("read generation")?)
        .context("parse generation JSON")?;
    let mut package = UpdatePackageV1 {
        format: UPDATE_FORMAT_V1.to_string(),
        release_id,
        sequence,
        channel,
        min_kernel_version,
        payload,
        signer_key_id: key.id.clone(),
        signature_b64: String::new(),
    };
    package.signature_b64 = key.sign(&package_signing_bytes(&package)?);
    write_new_json(output, &package)?;
    println!("wrote signed product package {}", output.display());
    Ok(())
}

fn sign_manifest(
    secret_path: &Path,
    output: &Path,
    release_id: String,
    sequence: u64,
    channel: String,
    artifact_paths: &[PathBuf],
) -> Result<()> {
    let key = read_signing_key(secret_path)?;
    let mut artifacts = Vec::new();
    for path in artifact_paths {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("artifact has no UTF-8 file name: {}", path.display()))?
            .to_string();
        artifacts.push(ReleaseArtifactV1 {
            kind: artifact_kind(&name).to_string(),
            target: artifact_target(&name).to_string(),
            name,
            sha256: hex_sha256(&bytes),
            bytes: bytes.len() as u64,
        });
    }
    artifacts.sort_by(|left, right| left.name.cmp(&right.name));
    let mut manifest = ReleaseManifestV1 {
        format: RELEASE_MANIFEST_FORMAT_V1.to_string(),
        release_id,
        sequence,
        channel,
        artifacts,
        signer_key_id: key.id.clone(),
        signature_b64: String::new(),
    };
    manifest.signature_b64 = key.sign(&release_manifest_signing_bytes(&manifest)?);
    write_new_json(output, &manifest)?;
    println!("wrote signed release manifest {}", output.display());
    Ok(())
}

fn verify_release(trust_roots: &Path, manifest_path: &Path, artifact_dir: &Path) -> Result<()> {
    let roots = read_trust_roots(trust_roots)?;
    let manager = UpdateManager::new(".", "0.1.0", roots)?;
    let manifest_bytes = fs::read(manifest_path).context("read release manifest")?;
    let manifest = manager.verify_release_manifest_bytes(&manifest_bytes)?;
    for artifact in &manifest.artifacts {
        let path = artifact_dir.join(&artifact.name);
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        if bytes.len() as u64 != artifact.bytes || hex_sha256(&bytes) != artifact.sha256 {
            return Err(anyhow!(
                "artifact {} does not match signed manifest",
                artifact.name
            ));
        }
    }
    println!(
        "verified release {} sequence {} with {} artifact(s)",
        manifest.release_id,
        manifest.sequence,
        manifest.artifacts.len()
    );
    Ok(())
}

fn list_release_artifacts(trust_roots: &Path, manifest_path: &Path) -> Result<()> {
    let roots = read_trust_roots(trust_roots)?;
    let manager = UpdateManager::new(".", "0.1.0", roots)?;
    let manifest_bytes = fs::read(manifest_path).context("read release manifest")?;
    let manifest = manager.verify_release_manifest_bytes(&manifest_bytes)?;
    for artifact in manifest.artifacts {
        println!("{}", artifact.name);
    }
    Ok(())
}

fn read_signing_key(path: &Path) -> Result<Keypair> {
    let secret: ReleaseSigningSecretV1 =
        serde_json::from_slice(&fs::read(path).context("read release signing key")?)
            .context("parse release signing key")?;
    if secret.v != 1 {
        return Err(anyhow!(
            "unsupported release signing key version {}",
            secret.v
        ));
    }
    let key = Keypair::from_secret_key_b64(&secret.secret_key_b64)?;
    if key.id != secret.key_id {
        return Err(anyhow!("release signing key id does not match secret"));
    }
    Ok(key)
}

fn read_trust_roots(path: &Path) -> Result<Vec<TrustedReleaseKey>> {
    let roots: TrustedReleaseKeysV1 =
        serde_json::from_slice(&fs::read(path).context("read release trust roots")?)
            .context("parse release trust roots")?;
    if roots.v != 1 {
        return Err(anyhow!(
            "unsupported release trust roots version {}",
            roots.v
        ));
    }
    Ok(roots.keys)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create output directory")?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create {} without overwrite", path.display()))?;
    serde_json::to_writer_pretty(&mut file, value).context("write JSON")?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn write_secret_new(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create secret directory")?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("create private signing key {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, value).context("write private signing key")?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn artifact_kind(name: &str) -> &'static str {
    if name.ends_with(".voxupdate") {
        "product-update"
    } else if name.ends_with(".dmg") || name.ends_with(".exe") {
        "native-installer"
    } else {
        "release-asset"
    }
}

fn artifact_target(name: &str) -> &'static str {
    if name.ends_with(".voxupdate") {
        "any"
    } else if name.ends_with(".dmg") {
        "macos-universal"
    } else if name.ends_with(".exe") {
        "windows-x86_64"
    } else {
        "portable"
    }
}
