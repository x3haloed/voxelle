use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use voxelle_core::{jcs_bytes, verify_signature_from_spki_b64};

pub const UPDATE_FORMAT_V1: &str = "voxelle-product-update/v1";
pub const UPDATE_SIGNATURE_DOMAIN_V1: &[u8] = b"voxelle/product-update/v1\0";
pub const RELEASE_MANIFEST_FORMAT_V1: &str = "voxelle-release-manifest/v1";
pub const RELEASE_MANIFEST_SIGNATURE_DOMAIN_V1: &[u8] = b"voxelle/release-manifest/v1\0";
pub const TRUST_TRANSITION_FORMAT_V1: &str = "voxelle-release-trust-transition/v1";
pub const TRUST_TRANSITION_SIGNATURE_DOMAIN_V1: &[u8] = b"voxelle/release-trust-transition/v1\0";
pub const MAX_UPDATE_PACKAGE_BYTES: usize = 1024 * 1024;
pub const MAX_RELEASE_ID_BYTES: usize = 128;
pub const MAX_CHANNEL_BYTES: usize = 32;
pub const GITHUB_RELEASE_BASE: &str =
    "https://github.com/x3haloed/voxelle/releases/latest/download";
pub const RELEASE_MANIFEST_NAME: &str = "VOXELLE-RELEASE.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdatePackageV1 {
    pub format: String,
    pub release_id: String,
    pub sequence: u64,
    pub channel: String,
    pub min_kernel_version: String,
    pub payload: Value,
    pub signer_key_id: String,
    pub signature_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedReleaseKey {
    pub key_id: String,
    pub spki_b64: String,
    #[serde(default)]
    pub role: ReleaseKeyRole,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseKeyRole {
    #[default]
    Release,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedReleaseKeysV1 {
    pub v: u8,
    pub keys: Vec<TrustedReleaseKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustTransitionV1 {
    pub format: String,
    pub sequence: u64,
    pub add: Vec<TrustedReleaseKey>,
    pub remove_key_ids: Vec<String>,
    pub signer_key_id: String,
    pub signature_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseArtifactV1 {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseManifestV1 {
    pub format: String,
    pub release_id: String,
    pub sequence: u64,
    pub channel: String,
    pub artifacts: Vec<ReleaseArtifactV1>,
    pub signer_key_id: String,
    pub signature_b64: String,
}

#[derive(Debug, Clone)]
pub struct AvailableProductUpdate {
    pub manifest: ReleaseManifestV1,
    pub artifact: ReleaseArtifactV1,
}

#[derive(Debug, Clone)]
pub struct DownloadedProductUpdate {
    pub available: AvailableProductUpdate,
    pub package_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationPointerV1 {
    pub v: u8,
    pub package_sha256: String,
    pub release_id: String,
    pub sequence: u64,
}

#[derive(Debug, Clone)]
pub struct VerifiedPackage {
    package: UpdatePackageV1,
    raw: Vec<u8>,
    sha256: String,
}

impl VerifiedPackage {
    pub fn package(&self) -> &UpdatePackageV1 {
        &self.package
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub fn pointer(&self) -> GenerationPointerV1 {
        GenerationPointerV1 {
            v: 1,
            package_sha256: self.sha256.clone(),
            release_id: self.package.release_id.clone(),
            sequence: self.package.sequence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSource {
    Current,
    PreviousRecovery,
}

#[derive(Debug, Clone)]
pub struct LoadedPackage {
    pub package: VerifiedPackage,
    pub source: ActiveSource,
}

#[derive(Debug, Clone)]
pub struct UpdateManager {
    root: PathBuf,
    kernel_version: Version,
    trusted_keys: BTreeMap<String, TrustedReleaseKey>,
    trust_sequence: u64,
}

impl UpdateManager {
    pub fn new(
        root: impl Into<PathBuf>,
        kernel_version: &str,
        trusted_keys: impl IntoIterator<Item = TrustedReleaseKey>,
    ) -> Result<Self> {
        let kernel_version = Version::parse(kernel_version).context("parse kernel version")?;
        let mut keys = BTreeMap::new();
        for key in trusted_keys {
            validate_bounded("release key id", &key.key_id, MAX_RELEASE_ID_BYTES)?;
            if keys.insert(key.key_id.clone(), key.clone()).is_some() {
                return Err(anyhow!("duplicate trusted release key {}", key.key_id));
            }
        }
        let mut manager = Self {
            root: root.into(),
            kernel_version,
            trusted_keys: keys,
            trust_sequence: 0,
        };
        manager.replay_trust_transitions()?;
        Ok(manager)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn trust_sequence(&self) -> u64 {
        self.trust_sequence
    }

    pub fn trusted_key_count(&self) -> usize {
        self.trusted_keys.len()
    }

    pub fn apply_trust_transition_bytes(&mut self, raw: &[u8]) -> Result<TrustTransitionV1> {
        let transition = self.verify_trust_transition_bytes(raw)?;
        let directory = self.root.join("trust-transitions");
        fs::create_dir_all(&directory).context("create trust transition directory")?;
        let path = directory.join(format!("{:020}.json", transition.sequence));
        if path.exists() {
            return Err(anyhow!("trust transition sequence already exists"));
        }
        atomic_write(&path, raw).context("persist trust transition")?;
        self.apply_verified_trust_transition(&transition)?;
        Ok(transition)
    }

    pub fn verify_trust_transition_bytes(&self, raw: &[u8]) -> Result<TrustTransitionV1> {
        if raw.is_empty() || raw.len() > 64 * 1024 {
            return Err(anyhow!("trust transition is empty or too large"));
        }
        let transition: TrustTransitionV1 =
            serde_json::from_slice(raw).context("parse trust transition")?;
        validate_trust_transition_shape(&transition)?;
        if transition.sequence != self.trust_sequence + 1 {
            return Err(anyhow!(
                "trust transition sequence {} does not follow {}",
                transition.sequence,
                self.trust_sequence
            ));
        }
        let signer = self
            .trusted_keys
            .get(&transition.signer_key_id)
            .ok_or_else(|| anyhow!("trust transition signer is not currently trusted"))?;
        verify_signature_from_spki_b64(
            &signer.spki_b64,
            &trust_transition_signing_bytes(&transition)?,
            &transition.signature_b64,
        )
        .context("verify trust transition signature")?;
        let mut projected = self.trusted_keys.clone();
        apply_trust_operations(&mut projected, &transition)?;
        Ok(transition)
    }

    pub fn verify_bytes(&self, raw: &[u8]) -> Result<VerifiedPackage> {
        if raw.is_empty() || raw.len() > MAX_UPDATE_PACKAGE_BYTES {
            return Err(anyhow!(
                "update package must be between 1 and {MAX_UPDATE_PACKAGE_BYTES} bytes"
            ));
        }
        let package: UpdatePackageV1 =
            serde_json::from_slice(raw).context("parse product update package")?;
        validate_package_shape(&package)?;
        let min_kernel =
            Version::parse(&package.min_kernel_version).context("parse minimum kernel version")?;
        if min_kernel > self.kernel_version {
            return Err(anyhow!(
                "update requires kernel {min_kernel}, installed kernel is {}",
                self.kernel_version
            ));
        }
        let signer = self
            .trusted_keys
            .get(&package.signer_key_id)
            .ok_or_else(|| anyhow!("update signer {} is not trusted", package.signer_key_id))?;
        if signer.role != ReleaseKeyRole::Release {
            return Err(anyhow!("recovery keys cannot sign product updates"));
        }
        let message = package_signing_bytes(&package)?;
        verify_signature_from_spki_b64(&signer.spki_b64, &message, &package.signature_b64)
            .context("verify product update signature")?;
        let sha256 = hex_sha256(raw);
        Ok(VerifiedPackage {
            package,
            raw: raw.to_vec(),
            sha256,
        })
    }

    pub fn verify_release_manifest_bytes(&self, raw: &[u8]) -> Result<ReleaseManifestV1> {
        if raw.is_empty() || raw.len() > MAX_UPDATE_PACKAGE_BYTES {
            return Err(anyhow!("release manifest is empty or too large"));
        }
        let manifest: ReleaseManifestV1 =
            serde_json::from_slice(raw).context("parse release manifest")?;
        validate_release_manifest_shape(&manifest)?;
        let signer = self
            .trusted_keys
            .get(&manifest.signer_key_id)
            .ok_or_else(|| anyhow!("release signer {} is not trusted", manifest.signer_key_id))?;
        if signer.role != ReleaseKeyRole::Release {
            return Err(anyhow!("recovery keys cannot sign release manifests"));
        }
        verify_signature_from_spki_b64(
            &signer.spki_b64,
            &release_manifest_signing_bytes(&manifest)?,
            &manifest.signature_b64,
        )
        .context("verify release manifest signature")?;
        Ok(manifest)
    }

    pub async fn discover_github_release(&self) -> Result<AvailableProductUpdate> {
        let raw = fetch_bounded(
            &format!("{GITHUB_RELEASE_BASE}/{RELEASE_MANIFEST_NAME}"),
            MAX_UPDATE_PACKAGE_BYTES,
        )
        .await
        .context("download latest GitHub release manifest")?;
        let manifest = self.verify_release_manifest_bytes(&raw)?;
        let artifact = manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == "product-update" && artifact.target == "any")
            .cloned()
            .ok_or_else(|| {
                anyhow!("signed release has no product-update artifact for any target")
            })?;
        if artifact.bytes as usize > MAX_UPDATE_PACKAGE_BYTES {
            return Err(anyhow!(
                "signed product update exceeds the kernel size limit"
            ));
        }
        Ok(AvailableProductUpdate { manifest, artifact })
    }

    pub async fn download_github_update(
        &self,
        available: AvailableProductUpdate,
    ) -> Result<DownloadedProductUpdate> {
        let package_bytes = fetch_bounded(
            &format!("{GITHUB_RELEASE_BASE}/{}", available.artifact.name),
            MAX_UPDATE_PACKAGE_BYTES,
        )
        .await
        .context("download signed product update")?;
        self.verify_downloaded_update(available, package_bytes)
    }

    pub fn verify_downloaded_update(
        &self,
        available: AvailableProductUpdate,
        package_bytes: Vec<u8>,
    ) -> Result<DownloadedProductUpdate> {
        if package_bytes.len() as u64 != available.artifact.bytes
            || hex_sha256(&package_bytes) != available.artifact.sha256
        {
            return Err(anyhow!(
                "downloaded product update does not match the signed release manifest"
            ));
        }
        let verified = self.verify_bytes(&package_bytes)?;
        let package = verified.package();
        if package.release_id != available.manifest.release_id
            || package.sequence != available.manifest.sequence
            || package.channel != available.manifest.channel
        {
            return Err(anyhow!(
                "product update identity does not match the signed release manifest"
            ));
        }
        Ok(DownloadedProductUpdate {
            available,
            package_bytes,
        })
    }

    pub fn stage(&self, package: &VerifiedPackage) -> Result<PathBuf> {
        let package_dir = self.root.join("packages");
        fs::create_dir_all(&package_dir).context("create update package directory")?;
        let path = package_dir.join(format!("{}.voxupdate", package.sha256));
        if path.exists() {
            let existing = fs::read(&path).context("read staged update package")?;
            if existing != package.raw {
                return Err(anyhow!("staged package hash collision"));
            }
            return Ok(path);
        }
        atomic_write(&path, &package.raw).context("stage update package")?;
        Ok(path)
    }

    pub fn stage_candidate(&self, package: &VerifiedPackage) -> Result<GenerationPointerV1> {
        self.stage(package)?;
        let pointer = package.pointer();
        write_json_atomic(&self.root.join("staged.json"), &pointer)
            .context("persist staged generation pointer")?;
        self.prune_packages()?;
        Ok(pointer)
    }

    pub fn load_staged(&self) -> Result<Option<VerifiedPackage>> {
        let Some(pointer) = self.read_pointer("staged.json")? else {
            return Ok(None);
        };
        Ok(Some(self.load_pointer(&pointer)?))
    }

    pub fn activate_staged(&self) -> Result<GenerationPointerV1> {
        let package = self
            .load_staged()?
            .ok_or_else(|| anyhow!("no verified product generation is staged"))?;
        let pointer = self.activate(&package)?;
        remove_file_if_exists(&self.root.join("staged.json"))?;
        self.prune_packages()?;
        Ok(pointer)
    }

    pub fn discard_staged(&self) -> Result<Option<GenerationPointerV1>> {
        let pointer = self.read_pointer("staged.json").ok().flatten();
        remove_file_if_exists(&self.root.join("staged.json"))?;
        self.prune_packages()?;
        Ok(pointer)
    }

    pub fn activate(&self, package: &VerifiedPackage) -> Result<GenerationPointerV1> {
        self.stage(package)?;
        if let Some(active) = self.read_pointer("active.json")? {
            if package.package.sequence <= active.sequence {
                return Err(anyhow!(
                    "update sequence {} is not newer than active sequence {}",
                    package.package.sequence,
                    active.sequence
                ));
            }
            write_json_atomic(&self.root.join("previous.json"), &active)
                .context("retain previous update pointer")?;
        }
        let pointer = package.pointer();
        write_json_atomic(&self.root.join("active.json"), &pointer)
            .context("activate update pointer")?;
        self.prune_packages()?;
        Ok(pointer)
    }

    pub fn rollback(&self) -> Result<GenerationPointerV1> {
        let previous = self
            .read_pointer("previous.json")?
            .ok_or_else(|| anyhow!("no previous product generation is available"))?;
        let verified = self.load_pointer(&previous)?;
        let current = self.read_pointer("active.json")?;
        write_json_atomic(&self.root.join("active.json"), &verified.pointer())
            .context("activate previous update pointer")?;
        if let Some(current) = current {
            write_json_atomic(&self.root.join("previous.json"), &current)
                .context("retain rolled-back update pointer")?;
        }
        Ok(verified.pointer())
    }

    pub fn deactivate_to_builtin(&self) -> Result<Option<GenerationPointerV1>> {
        let Some(active) = self.read_pointer("active.json")? else {
            return Ok(None);
        };
        self.load_pointer(&active)?;
        write_json_atomic(&self.root.join("previous.json"), &active)
            .context("retain signed generation before built-in rollback")?;
        fs::remove_file(self.root.join("active.json"))
            .context("deactivate signed generation pointer")?;
        Ok(Some(active))
    }

    pub fn load_active(&self) -> Result<Option<LoadedPackage>> {
        let active = match self.read_pointer("active.json") {
            Ok(active) => active,
            Err(active_error) => return self.recover_previous(active_error),
        };
        if let Some(active) = active {
            match self.load_pointer(&active) {
                Ok(package) => {
                    return Ok(Some(LoadedPackage {
                        package,
                        source: ActiveSource::Current,
                    }))
                }
                Err(active_error) => return self.recover_previous(active_error),
            }
        }
        Ok(None)
    }

    fn recover_previous(&self, active_error: anyhow::Error) -> Result<Option<LoadedPackage>> {
        if let Some(previous) = self.read_pointer("previous.json")? {
            let package = self.load_pointer(&previous).with_context(|| {
                format!(
                    "active generation failed verification ({active_error:#}); previous generation also failed"
                )
            })?;
            write_json_atomic(&self.root.join("active.json"), &package.pointer())
                .context("repair active pointer from previous generation")?;
            return Ok(Some(LoadedPackage {
                package,
                source: ActiveSource::PreviousRecovery,
            }));
        }
        Err(active_error).context("load active product generation")
    }

    pub fn active_pointer(&self) -> Result<Option<GenerationPointerV1>> {
        self.read_pointer("active.json")
    }

    pub fn previous_pointer(&self) -> Result<Option<GenerationPointerV1>> {
        self.read_pointer("previous.json")
    }

    pub fn staged_pointer(&self) -> Result<Option<GenerationPointerV1>> {
        self.read_pointer("staged.json")
    }

    pub fn prune_packages(&self) -> Result<usize> {
        let package_dir = self.root.join("packages");
        if !package_dir.exists() {
            return Ok(0);
        }
        let mut retained = std::collections::BTreeSet::new();
        for name in ["active.json", "previous.json", "staged.json"] {
            if let Some(pointer) = self.read_pointer(name)? {
                retained.insert(format!("{}.voxupdate", pointer.package_sha256));
            }
        }
        let mut removed = 0;
        for entry in fs::read_dir(&package_dir).context("read update package directory")? {
            let entry = entry.context("read update package entry")?;
            if !entry
                .file_type()
                .context("inspect update package entry")?
                .is_file()
            {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.ends_with(".voxupdate") && !retained.contains(name) {
                fs::remove_file(entry.path()).context("remove unreferenced update package")?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn read_pointer(&self, name: &str) -> Result<Option<GenerationPointerV1>> {
        let path = self.root.join(name);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        let pointer: GenerationPointerV1 =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        if pointer.v != 1 || pointer.package_sha256.len() != 64 {
            return Err(anyhow!("invalid generation pointer {}", path.display()));
        }
        Ok(Some(pointer))
    }

    fn load_pointer(&self, pointer: &GenerationPointerV1) -> Result<VerifiedPackage> {
        let path = self
            .root
            .join("packages")
            .join(format!("{}.voxupdate", pointer.package_sha256));
        let raw = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        if hex_sha256(&raw) != pointer.package_sha256 {
            return Err(anyhow!("staged update package hash does not match pointer"));
        }
        let package = self.verify_bytes(&raw)?;
        if package.package.release_id != pointer.release_id
            || package.package.sequence != pointer.sequence
        {
            return Err(anyhow!(
                "staged update package does not match pointer metadata"
            ));
        }
        Ok(package)
    }

    fn replay_trust_transitions(&mut self) -> Result<()> {
        let directory = self.root.join("trust-transitions");
        if !directory.exists() {
            return Ok(());
        }
        let mut paths = fs::read_dir(&directory)
            .context("read trust transition directory")?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("json"));
        paths.sort();
        if paths.len() > 32 {
            return Err(anyhow!("too many persisted trust transitions"));
        }
        for path in paths {
            let raw = fs::read(&path)
                .with_context(|| format!("read trust transition {}", path.display()))?;
            let transition = self
                .verify_trust_transition_bytes(&raw)
                .with_context(|| format!("verify trust transition {}", path.display()))?;
            self.apply_verified_trust_transition(&transition)?;
        }
        Ok(())
    }

    fn apply_verified_trust_transition(&mut self, transition: &TrustTransitionV1) -> Result<()> {
        apply_trust_operations(&mut self.trusted_keys, transition)?;
        self.trust_sequence = transition.sequence;
        Ok(())
    }
}

async fn fetch_bounded(url: &str, max_bytes: usize) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Voxelle-Update/0.1")
        .build()
        .context("build release transport client")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("request release artifact")?
        .error_for_status()
        .context("release artifact HTTP status")?;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(anyhow!("release artifact exceeds the download size limit"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("read release artifact body")?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(anyhow!("release artifact exceeds the download size limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(anyhow!("release artifact is empty"));
    }
    Ok(bytes)
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
    }
}

pub fn package_signing_bytes(package: &UpdatePackageV1) -> Result<Vec<u8>> {
    let mut unsigned = package.clone();
    unsigned.signature_b64.clear();
    let canonical = jcs_bytes(&unsigned).context("canonicalize product update package")?;
    let mut message = Vec::with_capacity(UPDATE_SIGNATURE_DOMAIN_V1.len() + canonical.len());
    message.extend_from_slice(UPDATE_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&canonical);
    Ok(message)
}

pub fn release_manifest_signing_bytes(manifest: &ReleaseManifestV1) -> Result<Vec<u8>> {
    let mut unsigned = manifest.clone();
    unsigned.signature_b64.clear();
    let canonical = jcs_bytes(&unsigned).context("canonicalize release manifest")?;
    let mut message =
        Vec::with_capacity(RELEASE_MANIFEST_SIGNATURE_DOMAIN_V1.len() + canonical.len());
    message.extend_from_slice(RELEASE_MANIFEST_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&canonical);
    Ok(message)
}

pub fn trust_transition_signing_bytes(transition: &TrustTransitionV1) -> Result<Vec<u8>> {
    let mut unsigned = transition.clone();
    unsigned.signature_b64.clear();
    let canonical = jcs_bytes(&unsigned).context("canonicalize trust transition")?;
    let mut message =
        Vec::with_capacity(TRUST_TRANSITION_SIGNATURE_DOMAIN_V1.len() + canonical.len());
    message.extend_from_slice(TRUST_TRANSITION_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&canonical);
    Ok(message)
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn validate_package_shape(package: &UpdatePackageV1) -> Result<()> {
    if package.format != UPDATE_FORMAT_V1 {
        return Err(anyhow!("unsupported update format {}", package.format));
    }
    if package.sequence == 0 {
        return Err(anyhow!("update sequence must be positive"));
    }
    validate_bounded("release id", &package.release_id, MAX_RELEASE_ID_BYTES)?;
    validate_bounded("channel", &package.channel, MAX_CHANNEL_BYTES)?;
    validate_bounded(
        "signer key id",
        &package.signer_key_id,
        MAX_RELEASE_ID_BYTES,
    )?;
    if package.signature_b64.len() > 256 {
        return Err(anyhow!("update signature is too large"));
    }
    Ok(())
}

fn validate_release_manifest_shape(manifest: &ReleaseManifestV1) -> Result<()> {
    if manifest.format != RELEASE_MANIFEST_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported release manifest format {}",
            manifest.format
        ));
    }
    if manifest.sequence == 0 || manifest.artifacts.is_empty() || manifest.artifacts.len() > 64 {
        return Err(anyhow!(
            "release manifest sequence or artifact count is invalid"
        ));
    }
    validate_bounded("release id", &manifest.release_id, MAX_RELEASE_ID_BYTES)?;
    validate_bounded("channel", &manifest.channel, MAX_CHANNEL_BYTES)?;
    validate_bounded(
        "signer key id",
        &manifest.signer_key_id,
        MAX_RELEASE_ID_BYTES,
    )?;
    let mut names = std::collections::BTreeSet::new();
    for artifact in &manifest.artifacts {
        validate_bounded("artifact name", &artifact.name, 255)?;
        validate_bounded("artifact kind", &artifact.kind, 64)?;
        validate_bounded("artifact target", &artifact.target, 128)?;
        if artifact.name.contains('/')
            || artifact.name.contains('\\')
            || artifact.bytes == 0
            || artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(anyhow!("release artifact metadata is invalid"));
        }
        if !names.insert(&artifact.name) {
            return Err(anyhow!("duplicate release artifact {}", artifact.name));
        }
    }
    Ok(())
}

fn validate_trust_transition_shape(transition: &TrustTransitionV1) -> Result<()> {
    if transition.format != TRUST_TRANSITION_FORMAT_V1 {
        return Err(anyhow!(
            "unsupported trust transition format {}",
            transition.format
        ));
    }
    if transition.sequence == 0
        || transition.add.len() > 8
        || transition.remove_key_ids.len() > 8
        || (transition.add.is_empty() && transition.remove_key_ids.is_empty())
    {
        return Err(anyhow!("trust transition operation count is invalid"));
    }
    validate_bounded(
        "trust transition signer",
        &transition.signer_key_id,
        MAX_RELEASE_ID_BYTES,
    )?;
    if transition.signature_b64.len() > 256 {
        return Err(anyhow!("trust transition signature is too large"));
    }
    let mut add_ids = std::collections::BTreeSet::new();
    for key in &transition.add {
        validate_bounded("release key id", &key.key_id, MAX_RELEASE_ID_BYTES)?;
        validate_bounded("release public key", &key.spki_b64, 256)?;
        if !add_ids.insert(&key.key_id) {
            return Err(anyhow!("duplicate added release key {}", key.key_id));
        }
    }
    let mut remove_ids = std::collections::BTreeSet::new();
    for key_id in &transition.remove_key_ids {
        validate_bounded("removed release key id", key_id, MAX_RELEASE_ID_BYTES)?;
        if !remove_ids.insert(key_id) || add_ids.contains(key_id) {
            return Err(anyhow!("ambiguous trust transition key {key_id}"));
        }
    }
    Ok(())
}

fn apply_trust_operations(
    keys: &mut BTreeMap<String, TrustedReleaseKey>,
    transition: &TrustTransitionV1,
) -> Result<()> {
    for key in &transition.add {
        if keys.contains_key(&key.key_id) {
            return Err(anyhow!("release key {} is already trusted", key.key_id));
        }
        if key.role != ReleaseKeyRole::Release {
            return Err(anyhow!(
                "embedded recovery keys can only change in a native kernel release"
            ));
        }
        keys.insert(key.key_id.clone(), key.clone());
    }
    for key_id in &transition.remove_key_ids {
        let Some(key) = keys.get(key_id) else {
            return Err(anyhow!("release key {key_id} is not currently trusted"));
        };
        if key.role == ReleaseKeyRole::Recovery {
            return Err(anyhow!(
                "embedded recovery keys can only change in a native kernel release"
            ));
        }
        keys.remove(key_id);
    }
    if keys.is_empty() {
        return Err(anyhow!("trust transition cannot remove every release key"));
    }
    if !keys
        .values()
        .any(|key| key.role == ReleaseKeyRole::Recovery)
    {
        return Err(anyhow!(
            "trust transition must preserve an embedded recovery key"
        ));
    }
    Ok(())
}

fn validate_bounded(label: &str, value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(anyhow!(
            "{label} is empty, too large, or contains control characters"
        ));
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("serialize atomic JSON")?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("atomic write target has no parent"))?;
    fs::create_dir_all(parent).context("create atomic write parent")?;
    let mut temp = NamedTempFile::new_in(parent).context("create atomic temporary file")?;
    temp.write_all(bytes)
        .context("write atomic temporary file")?;
    temp.as_file()
        .sync_all()
        .context("sync atomic temporary file")?;
    temp.persist(path)
        .map_err(|error| error.error)
        .context("persist atomic file")?;
    if let Ok(directory) = fs::File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxelle_core::Keypair;

    fn signed_package(key: &Keypair, sequence: u64, payload: Value) -> Vec<u8> {
        let mut package = UpdatePackageV1 {
            format: UPDATE_FORMAT_V1.to_string(),
            release_id: format!("beta-{sequence}"),
            sequence,
            channel: "beta".to_string(),
            min_kernel_version: "0.1.0".to_string(),
            payload,
            signer_key_id: key.id.clone(),
            signature_b64: String::new(),
        };
        package.signature_b64 = key.sign(&package_signing_bytes(&package).expect("signing bytes"));
        serde_json::to_vec_pretty(&package).expect("package JSON")
    }

    fn signed_manifest(key: &Keypair) -> Vec<u8> {
        let artifact = b"native package";
        let mut manifest = ReleaseManifestV1 {
            format: RELEASE_MANIFEST_FORMAT_V1.to_string(),
            release_id: "beta-1".to_string(),
            sequence: 1,
            channel: "beta".to_string(),
            artifacts: vec![ReleaseArtifactV1 {
                name: "Voxelle.dmg".to_string(),
                sha256: hex_sha256(artifact),
                bytes: artifact.len() as u64,
                kind: "desktop-installer".to_string(),
                target: "macos-aarch64".to_string(),
            }],
            signer_key_id: key.id.clone(),
            signature_b64: String::new(),
        };
        manifest.signature_b64 =
            key.sign(&release_manifest_signing_bytes(&manifest).expect("signing bytes"));
        serde_json::to_vec_pretty(&manifest).expect("manifest JSON")
    }

    fn signed_trust_transition(
        signer: &Keypair,
        sequence: u64,
        add: Vec<TrustedReleaseKey>,
        remove_key_ids: Vec<String>,
    ) -> Vec<u8> {
        let mut transition = TrustTransitionV1 {
            format: TRUST_TRANSITION_FORMAT_V1.to_string(),
            sequence,
            add,
            remove_key_ids,
            signer_key_id: signer.id.clone(),
            signature_b64: String::new(),
        };
        transition.signature_b64 = signer.sign(
            &trust_transition_signing_bytes(&transition).expect("trust transition signing bytes"),
        );
        serde_json::to_vec_pretty(&transition).expect("trust transition JSON")
    }

    fn manager(root: &Path, key: &Keypair) -> UpdateManager {
        let recovery = Keypair::generate().expect("recovery key");
        UpdateManager::new(
            root,
            "0.1.0",
            [
                TrustedReleaseKey {
                    key_id: key.id.clone(),
                    spki_b64: key.spki_b64.clone(),
                    role: ReleaseKeyRole::Release,
                },
                TrustedReleaseKey {
                    key_id: recovery.id,
                    spki_b64: recovery.spki_b64,
                    role: ReleaseKeyRole::Recovery,
                },
            ],
        )
        .expect("manager")
    }

    #[test]
    fn signed_package_activates_persists_and_rolls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let one = manager
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        manager.activate(&one).expect("activate one");
        assert_eq!(
            manager
                .load_active()
                .expect("load")
                .expect("active")
                .package
                .package()
                .payload["name"],
            "one"
        );

        let two = manager
            .verify_bytes(&signed_package(&key, 2, serde_json::json!({"name": "two"})))
            .expect("verify two");
        manager.activate(&two).expect("activate two");
        assert_eq!(manager.rollback().expect("rollback").sequence, 1);
        assert_eq!(
            manager
                .load_active()
                .expect("load")
                .expect("active")
                .package
                .package()
                .payload["name"],
            "one"
        );
    }

    #[test]
    fn first_signed_generation_can_return_to_builtin_recovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let one = manager
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        manager.activate(&one).expect("activate one");
        let deactivated = manager
            .deactivate_to_builtin()
            .expect("deactivate")
            .expect("active pointer");
        assert_eq!(deactivated.sequence, 1);
        assert!(manager.load_active().expect("load").is_none());
        assert_eq!(
            manager
                .previous_pointer()
                .expect("previous")
                .unwrap()
                .sequence,
            1
        );
    }

    #[test]
    fn release_manifest_authenticates_exact_artifact_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let raw = signed_manifest(&key);
        let verified = manager
            .verify_release_manifest_bytes(&raw)
            .expect("verify manifest");
        assert_eq!(verified.artifacts[0].name, "Voxelle.dmg");

        let mut tampered: Value = serde_json::from_slice(&raw).expect("JSON");
        tampered["artifacts"][0]["sha256"] = Value::String("0".repeat(64));
        assert!(manager
            .verify_release_manifest_bytes(&serde_json::to_vec(&tampered).expect("JSON"))
            .expect_err("tampered artifact metadata")
            .to_string()
            .contains("signature"));
    }

    #[test]
    fn staged_generation_survives_restart_and_storage_is_bounded_to_live_pointers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let initial = manager(dir.path(), &key);
        let one = initial
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        initial.activate(&one).expect("activate one");
        let two = initial
            .verify_bytes(&signed_package(&key, 2, serde_json::json!({"name": "two"})))
            .expect("verify two");
        initial.stage_candidate(&two).expect("stage two");

        let reopened = manager(dir.path(), &key);
        assert_eq!(
            reopened
                .load_staged()
                .expect("load staged")
                .expect("staged")
                .package()
                .sequence,
            2
        );
        reopened.activate_staged().expect("activate staged");

        let three = reopened
            .verify_bytes(&signed_package(
                &key,
                3,
                serde_json::json!({"name": "three"}),
            ))
            .expect("verify three");
        reopened.stage(&three).expect("orphan three");
        assert_eq!(reopened.prune_packages().expect("prune"), 1);
        let package_count = fs::read_dir(reopened.root().join("packages"))
            .expect("packages")
            .count();
        assert_eq!(package_count, 2, "active and rollback packages remain");
    }

    #[test]
    fn downloaded_update_must_match_manifest_and_package_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let raw = signed_package(&key, 7, serde_json::json!({"name": "seven"}));
        let artifact = ReleaseArtifactV1 {
            name: "beta-7.voxupdate".to_string(),
            sha256: hex_sha256(&raw),
            bytes: raw.len() as u64,
            kind: "product-update".to_string(),
            target: "any".to_string(),
        };
        let available = AvailableProductUpdate {
            manifest: ReleaseManifestV1 {
                format: RELEASE_MANIFEST_FORMAT_V1.to_string(),
                release_id: "beta-7".to_string(),
                sequence: 7,
                channel: "beta".to_string(),
                artifacts: vec![artifact.clone()],
                signer_key_id: key.id.clone(),
                signature_b64: "already verified upstream".to_string(),
            },
            artifact,
        };
        manager
            .verify_downloaded_update(available.clone(), raw.clone())
            .expect("matching update");

        let mut wrong_hash = available.clone();
        wrong_hash.artifact.sha256 = "0".repeat(64);
        assert!(manager
            .verify_downloaded_update(wrong_hash, raw.clone())
            .expect_err("hash mismatch")
            .to_string()
            .contains("manifest"));

        let mut wrong_identity = available;
        wrong_identity.manifest.sequence = 8;
        assert!(manager
            .verify_downloaded_update(wrong_identity, raw)
            .expect_err("identity mismatch")
            .to_string()
            .contains("identity"));
    }

    #[test]
    fn signed_trust_transition_rotates_release_authority_and_replays_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = Keypair::generate().expect("old key");
        let new = Keypair::generate().expect("new key");
        let mut rotating = manager(dir.path(), &old);
        let transition = signed_trust_transition(
            &old,
            1,
            vec![TrustedReleaseKey {
                key_id: new.id.clone(),
                spki_b64: new.spki_b64.clone(),
                role: ReleaseKeyRole::Release,
            }],
            vec![old.id.clone()],
        );
        rotating
            .apply_trust_transition_bytes(&transition)
            .expect("apply transition");
        assert_eq!(rotating.trust_sequence(), 1);
        assert_eq!(rotating.trusted_key_count(), 2);
        assert!(rotating
            .verify_bytes(&signed_package(&old, 1, Value::Null))
            .expect_err("old key removed")
            .to_string()
            .contains("not trusted"));
        rotating
            .verify_bytes(&signed_package(&new, 1, Value::Null))
            .expect("new key trusted");

        let reopened = manager(dir.path(), &old);
        assert_eq!(reopened.trust_sequence(), 1);
        reopened
            .verify_bytes(&signed_package(&new, 2, Value::Null))
            .expect("rotation replayed");
        assert!(reopened
            .verify_bytes(&signed_package(&old, 2, Value::Null))
            .is_err());
    }

    #[test]
    fn trust_transition_rejects_tamper_replay_unknown_signer_and_preserves_recovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = Keypair::generate().expect("old key");
        let new = Keypair::generate().expect("new key");
        let attacker = Keypair::generate().expect("attacker key");
        let mut manager = manager(dir.path(), &old);
        let raw = signed_trust_transition(
            &old,
            1,
            vec![TrustedReleaseKey {
                key_id: new.id.clone(),
                spki_b64: new.spki_b64.clone(),
                role: ReleaseKeyRole::Release,
            }],
            vec![],
        );
        let mut tampered: Value = serde_json::from_slice(&raw).expect("JSON");
        tampered["remove_key_ids"] = serde_json::json!([old.id.clone()]);
        assert!(manager
            .verify_trust_transition_bytes(&serde_json::to_vec(&tampered).expect("JSON"))
            .expect_err("tamper")
            .to_string()
            .contains("signature"));
        let unknown = signed_trust_transition(
            &attacker,
            1,
            vec![TrustedReleaseKey {
                key_id: new.id.clone(),
                spki_b64: new.spki_b64.clone(),
                role: ReleaseKeyRole::Release,
            }],
            vec![],
        );
        assert!(manager
            .verify_trust_transition_bytes(&unknown)
            .expect_err("unknown signer")
            .to_string()
            .contains("not currently trusted"));
        manager
            .apply_trust_transition_bytes(&raw)
            .expect("apply one");
        assert!(manager
            .apply_trust_transition_bytes(&raw)
            .expect_err("replay")
            .to_string()
            .contains("does not follow"));

        let retire_all_release_keys =
            signed_trust_transition(&old, 2, vec![], vec![old.id.clone(), new.id.clone()]);
        manager
            .apply_trust_transition_bytes(&retire_all_release_keys)
            .expect("recovery authority remains");
        assert_eq!(manager.trusted_key_count(), 1);
        assert!(manager
            .verify_bytes(&signed_package(&new, 1, Value::Null))
            .expect_err("all release keys retired")
            .to_string()
            .contains("not trusted"));
    }

    #[test]
    fn tamper_unknown_signer_downgrade_and_new_kernel_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let raw = signed_package(&key, 2, serde_json::json!({"name": "two"}));
        let mut tampered: Value = serde_json::from_slice(&raw).expect("JSON");
        tampered["payload"]["name"] = Value::String("malicious".to_string());
        assert!(manager
            .verify_bytes(&serde_json::to_vec(&tampered).expect("tampered"))
            .expect_err("tamper rejected")
            .to_string()
            .contains("signature"));

        let attacker = Keypair::generate().expect("attacker");
        assert!(manager
            .verify_bytes(&signed_package(
                &attacker,
                2,
                serde_json::json!({"name": "attacker"})
            ))
            .expect_err("unknown signer")
            .to_string()
            .contains("not trusted"));

        let two = manager.verify_bytes(&raw).expect("verify two");
        manager.activate(&two).expect("activate two");
        let one = manager
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        assert!(manager
            .activate(&one)
            .expect_err("downgrade")
            .to_string()
            .contains("not newer"));

        let mut future: UpdatePackageV1 =
            serde_json::from_slice(&signed_package(&key, 3, Value::Null)).expect("future");
        future.min_kernel_version = "99.0.0".to_string();
        future.signature_b64 = key.sign(&package_signing_bytes(&future).expect("sign future"));
        assert!(manager
            .verify_bytes(&serde_json::to_vec(&future).expect("future JSON"))
            .expect_err("future kernel")
            .to_string()
            .contains("requires kernel"));
    }

    #[test]
    fn corrupt_active_package_recovers_previous_verified_generation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let one = manager
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        manager.activate(&one).expect("activate one");
        let two = manager
            .verify_bytes(&signed_package(&key, 2, serde_json::json!({"name": "two"})))
            .expect("verify two");
        manager.activate(&two).expect("activate two");
        let active_path = manager
            .root()
            .join("packages")
            .join(format!("{}.voxupdate", two.sha256()));
        fs::write(active_path, b"corrupt").expect("corrupt active");

        let recovered = manager.load_active().expect("recover").expect("active");
        assert_eq!(recovered.source, ActiveSource::PreviousRecovery);
        assert_eq!(recovered.package.package().sequence, 1);
        assert_eq!(
            manager.active_pointer().expect("pointer").unwrap().sequence,
            1
        );
    }

    #[test]
    fn interrupted_active_pointer_write_recovers_previous_generation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = Keypair::generate().expect("key");
        let manager = manager(dir.path(), &key);
        let one = manager
            .verify_bytes(&signed_package(&key, 1, serde_json::json!({"name": "one"})))
            .expect("verify one");
        manager.activate(&one).expect("activate one");
        let two = manager
            .verify_bytes(&signed_package(&key, 2, serde_json::json!({"name": "two"})))
            .expect("verify two");
        manager.activate(&two).expect("activate two");
        fs::write(manager.root().join("active.json"), b"{\"v\":1")
            .expect("simulate interrupted non-atomic writer");

        let recovered = manager.load_active().expect("recover").expect("active");
        assert_eq!(recovered.source, ActiveSource::PreviousRecovery);
        assert_eq!(recovered.package.package().sequence, 1);
        assert_eq!(
            manager.active_pointer().expect("pointer").unwrap().sequence,
            1
        );
    }
}
