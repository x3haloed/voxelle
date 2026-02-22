fn main() {
    // Best-effort: build an embedded zip of `apps/web/dist` for release builds.
    // This supports "refresh-to-update" where the host can extract the initial bundle
    // to an app cache directory and later swap it out via downloads.
    //
    // In dev, the app loads `build.devUrl` directly, so this is optional.
    if std::env::var("PROFILE").ok().as_deref() == Some("release") {
        if let Err(e) = build_web_bundle_zip() {
            println!("cargo:warning=failed to build embedded web bundle zip: {e}");
        }
    }
    tauri_build::build()
}

fn build_web_bundle_zip() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use walkdir::WalkDir;

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let dist_dir = manifest_dir.join("../../web/dist");
    if !dist_dir.exists() {
        return Err(format!("missing dist dir: {}", dist_dir.display()).into());
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let out_zip = out_dir.join("voxelle_web_bundle.zip");

    let f = fs::File::create(&out_zip)?;
    let mut zip = zip::ZipWriter::new(f);
    let options = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for entry in WalkDir::new(&dist_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let rel = path.strip_prefix(&dist_dir).unwrap_or(path);
        let rel_str = rel
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        if rel_str.is_empty() {
            continue;
        }

        if entry.file_type().is_dir() {
            zip.add_directory(rel_str, options)?;
            continue;
        }

        zip.start_file(rel_str, options)?;
        let mut rf = fs::File::open(path)?;
        io::copy(&mut rf, &mut zip)?;
    }

    zip.finish()?.flush()?;
    println!("cargo:rerun-if-changed={}", dist_dir.display());
    Ok(())
}
