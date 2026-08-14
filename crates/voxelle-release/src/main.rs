use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use voxelle_app::builtin_product_generation;
use voxelle_core::Keypair;
use voxelle_update::{
    hex_sha256, package_signing_bytes, release_manifest_signing_bytes, ReleaseArtifactV1,
    ReleaseManifestV1, TrustedReleaseKey, TrustedReleaseKeysV1, UpdateManager, UpdatePackageV1,
    RELEASE_MANIFEST_FORMAT_V1, UPDATE_FORMAT_V1,
};

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
        } => keygen(&secret, &trust_roots),
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
    }
}

fn keygen(secret_path: &Path, trust_roots_path: &Path) -> Result<()> {
    if trust_roots_path.exists() {
        return Err(anyhow!(
            "refusing to overwrite trust roots {}",
            trust_roots_path.display()
        ));
    }
    let key = Keypair::generate().context("generate Ed25519 release key")?;
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
    if name.ends_with(".dmg") {
        "macos-universal"
    } else if name.ends_with(".exe") {
        "windows-x86_64"
    } else {
        "portable"
    }
}
