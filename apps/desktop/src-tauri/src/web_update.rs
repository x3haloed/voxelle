use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use zip::read::ZipFile;

pub const EVENT_WEB_UPDATE_READY: &str = "voxelle:web-update-ready";
pub const DEFAULT_FEED: &str = "gh:x3haloed/voxelle";

// Update feed manifest (JSON), fetched from `feed_url`:
// {
//   "v": 1,
//   "version": "0.1.1",
//   "zip_url": "https://example.com/voxelle-web-0.1.1.zip",
//   "sha256": "<hex-lowercase-sha256-of-zip>"
// }

#[derive(Clone)]
pub struct WebBundleServer {
    port: u16,
    root: Arc<Mutex<PathBuf>>,
    _thread: Arc<std::thread::JoinHandle<()>>,
}

impl WebBundleServer {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_root(&self, p: PathBuf) {
        if let Ok(mut g) = self.root.lock() {
            *g = p;
        }
    }

    pub fn start(root_dir: PathBuf) -> Result<Self, String> {
        let root = Arc::new(Mutex::new(root_dir));
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| "unsupported server addr".to_string())?
            .port();

        let root2 = root.clone();
        let t = std::thread::spawn(move || loop {
            let Some(req) = server.recv_timeout(std::time::Duration::from_millis(200)).ok().flatten() else {
                continue;
            };
            if let Err(e) = handle_req(req, &root2) {
                eprintln!("web-bundle server error: {e}");
            }
        });

        Ok(Self { port, root, _thread: Arc::new(t) })
    }
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn handle_req(req: tiny_http::Request, root: &Arc<Mutex<PathBuf>>) -> Result<(), String> {
    let url = req.url().split('?').next().unwrap_or("/");
    let mut path = url.to_string();
    if path.is_empty() || path == "/" {
        path = "/index.html".into();
    }

    // Normalize and prevent traversal.
    let path = path.trim_start_matches('/');
    let root_dir = root.lock().map_err(|_| "root lock poisoned")?.clone();
    let candidate = root_dir.join(path);
    let candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => root_dir.join("index.html"),
    };
    if !candidate.starts_with(&root_dir) {
        let resp = tiny_http::Response::from_string("bad path").with_status_code(400);
        req.respond(resp).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let (data, ctype) = if candidate.is_file() {
        (std::fs::read(&candidate).map_err(|e| e.to_string())?, content_type_for_path(&candidate))
    } else {
        let index = root_dir.join("index.html");
        (std::fs::read(&index).map_err(|e| e.to_string())?, content_type_for_path(&index))
    };

    let resp = tiny_http::Response::from_data(data).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], ctype.as_bytes()).map_err(|_| "bad header".to_string())?,
    );
    req.respond(resp).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Clone)]
pub struct WebUpdateState {
    pub server: WebBundleServer,
    pub active_version: Arc<Mutex<String>>,
    pub feed_url: Arc<Mutex<String>>,
}

#[derive(Serialize)]
pub struct WebUpdateStatus {
    pub active_version: String,
    pub feed_url: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct WebUpdateCheckResult {
    pub available: bool,
    pub version: Option<String>,
    pub zip_url: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
struct WebBundleManifestV1 {
    v: u8,
    version: String,
    zip_url: String,
    sha256: String,
}

#[derive(Serialize)]
pub struct WebUpdateDownloadResult {
    pub activated_version: String,
}

fn cache_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("voxelle");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base)
}

fn bundles_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let p = cache_root(app)?.join("web_bundles");
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}

fn feed_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(cache_root(app)?.join("web_feed_url.txt"))
}

fn active_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(cache_root(app)?.join("web_active_version.txt"))
}

fn active_bundle_path(app: &tauri::AppHandle, version: &str) -> Result<PathBuf, String> {
    Ok(bundles_dir(app)?.join(version))
}

fn read_text_file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default().trim().to_string()
}

pub fn load_persisted_feed_url(app: &tauri::AppHandle) -> Result<String, String> {
    let p = feed_file(app)?;
    if !p.exists() {
        // Default to this repo's GitHub Releases if the user has never configured a feed.
        return Ok(DEFAULT_FEED.to_string());
    }
    Ok(read_text_file(&p))
}

pub fn load_persisted_active_version(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(read_text_file(&active_file(app)?))
}

pub fn persist_feed_url(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    std::fs::write(feed_file(app)?, url).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn persist_active_version(app: &tauri::AppHandle, version: &str) -> Result<(), String> {
    std::fs::write(active_file(app)?, version).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn ensure_embedded_bundle(app: &tauri::AppHandle, embedded_zip: &[u8], version: &str) -> Result<PathBuf, String> {
    let version = validate_bundle_version(version)?;
    let dir = active_bundle_path(app, &version)?;
    let index = dir.join("index.html");
    if index.exists() {
        return Ok(dir);
    }
    install_bundle_from_zip_bytes(app, embedded_zip, &version)?;
    Ok(dir)
}

fn validate_bundle_version(version: &str) -> Result<String, String> {
    let v = version.trim();
    if v.is_empty() {
        return Err("bundle version missing".into());
    }
    let parsed = semver::Version::parse(v).map_err(|_| "bundle version must be semver".to_string())?;
    Ok(parsed.to_string())
}

fn create_unique_tmp_dir(final_dir: &Path) -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock before unix epoch".to_string())?;
    let nonce = now.as_nanos();
    let pid = std::process::id();

    let parent = final_dir.parent().ok_or_else(|| "bundle dir has no parent".to_string())?;
    let base = final_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "bundle".to_string());

    for attempt in 0..32u32 {
        let candidate = parent.join(format!(".tmp-{}-{}-{}-{}", base, pid, nonce, attempt));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("failed to create unique temp dir".into())
}

fn install_bundle_from_zip_bytes(app: &tauri::AppHandle, zip_bytes: &[u8], version: &str) -> Result<PathBuf, String> {
    let final_dir = active_bundle_path(app, version)?;
    if final_dir.join("index.html").exists() {
        return Ok(final_dir);
    }

    let base = bundles_dir(app)?;
    let base_can = base.canonicalize().map_err(|e| e.to_string())?;
    if !final_dir.starts_with(&base_can) && !final_dir.starts_with(&base) {
        return Err("bundle path invalid".into());
    }

    let tmp_dir = create_unique_tmp_dir(&final_dir)?;
    // If we fail, try to clean up, but always return the original error.
    let extracted = extract_zip_bytes(zip_bytes, &tmp_dir);
    if let Err(e) = extracted {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }
    if !tmp_dir.join("index.html").exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err("bundle missing index.html".into());
    }

    if final_dir.exists() {
        let _ = std::fs::remove_dir_all(&final_dir);
    }
    std::fs::rename(&tmp_dir, &final_dir).map_err(|e| e.to_string())?;
    Ok(final_dir)
}

fn extract_zip_bytes(zip_bytes: &[u8], out_dir: &Path) -> Result<(), String> {
    let mut z = zip::ZipArchive::new(Cursor::new(zip_bytes)).map_err(|e| e.to_string())?;
    // Defensive limits: prevent zip bombs and pathological archives.
    // A production web bundle should be far smaller than these.
    let max_files: usize = 2048;
    let max_total_uncompressed: u64 = 50 * 1024 * 1024;
    let max_file_uncompressed: u64 = 10 * 1024 * 1024;
    if z.len() > max_files {
        return Err("zip contains too many entries".into());
    }

    let mut total_uncompressed: u64 = 0;
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| e.to_string())?;
        let out_rel = safe_zip_entry_path(&f).ok_or_else(|| "zip entry path invalid".to_string())?;
        if f.is_dir() {
            let out_path = out_dir.join(out_rel);
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }

        let out_path = out_dir.join(out_rel);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        let mut limited = f.take(max_file_uncompressed.saturating_add(1));
        let written = std::io::copy(&mut limited, &mut out).map_err(|e| e.to_string())?;
        if written > max_file_uncompressed {
            return Err("zip entry too large".into());
        }
        total_uncompressed = total_uncompressed.saturating_add(written);
        if total_uncompressed > max_total_uncompressed {
            return Err("zip expands too large".into());
        }
    }
    Ok(())
}

fn safe_zip_entry_path(f: &ZipFile<'_>) -> Option<PathBuf> {
    // `enclosed_name` rejects absolute paths and `..` traversal.
    let p = f.enclosed_name()?.to_path_buf();
    // Extra hardening: ignore Windows-y or weird paths that can sneak through.
    let s = p.to_string_lossy();
    if s.contains('\\') || s.contains(':') {
        return None;
    }
    // Limit pathological path lengths and components.
    if s.len() > 1024 {
        return None;
    }
    for c in p.components() {
        let std::path::Component::Normal(os) = c else { continue };
        if os.to_string_lossy().len() > 255 {
            return None;
        }
    }
    Some(p)
}

fn parse_version(s: &str) -> Option<semver::Version> {
    semver::Version::parse(s.trim()).ok()
}

async fn fetch_manifest(url: &str) -> Result<WebBundleManifestV1, String> {
    let feed = normalize_feed_url(url);
    let resp = reqwest::Client::new()
        .get(feed)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("manifest http {}", resp.status()));
    }
    let m = resp.json::<WebBundleManifestV1>().await.map_err(|e| e.to_string())?;
    if m.v != 1 {
        return Err("manifest.v must be 1".into());
    }
    if m.version.trim().is_empty() || m.zip_url.trim().is_empty() || m.sha256.trim().is_empty() {
        return Err("manifest missing fields".into());
    }
    // Also acts as path-hardening (bundle dir names should be safe).
    let _ = validate_bundle_version(&m.version)?;
    Ok(m)
}

fn normalize_feed_url(feed: &str) -> String {
    let s = feed.trim();
    if s.is_empty() {
        return "".to_string();
    }

    // Shorthand: `gh:owner/repo` (or `github:owner/repo`) resolves to the latest GitHub Release asset:
    // https://github.com/<owner>/<repo>/releases/latest/download/voxelle-web-manifest.json
    if let Some(rest) = s.strip_prefix("gh:").or_else(|| s.strip_prefix("github:")) {
        let slug = rest.trim().trim_matches('/');
        if !slug.is_empty() {
            return format!(
                "https://github.com/{}/releases/latest/download/voxelle-web-manifest.json",
                slug
            );
        }
    }

    s.to_string()
}

pub fn status(state: &WebUpdateState) -> WebUpdateStatus {
    WebUpdateStatus {
        active_version: state.active_version.lock().map(|g| g.clone()).unwrap_or_default(),
        feed_url: state.feed_url.lock().map(|g| g.clone()).unwrap_or_default(),
        port: state.server.port(),
    }
}

pub fn set_feed(state: &WebUpdateState, app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    let u = url.trim().to_string();
    persist_feed_url(app, &u)?;
    if let Ok(mut g) = state.feed_url.lock() {
        *g = u;
    }
    Ok(())
}

pub async fn check(state: &WebUpdateState) -> Result<WebUpdateCheckResult, String> {
    let feed = state.feed_url.lock().map_err(|_| "feed lock poisoned")?.clone();
    if feed.trim().is_empty() {
        return Ok(WebUpdateCheckResult { available: false, version: None, zip_url: None, sha256: None });
    }
    let m = fetch_manifest(&feed).await?;

    let active = state
        .active_version
        .lock()
        .map_err(|_| "active lock poisoned")?
        .clone();
    let active_v = parse_version(&active);
    let new_v = parse_version(&m.version);
    let available = match (active_v, new_v) {
        (Some(a), Some(n)) => n > a,
        _ => m.version.trim() != active.trim(),
    };

    Ok(WebUpdateCheckResult {
        available,
        version: Some(m.version),
        zip_url: Some(m.zip_url),
        sha256: Some(m.sha256),
    })
}

pub async fn download_and_activate(
    state: &WebUpdateState,
    app: &tauri::AppHandle,
) -> Result<WebUpdateDownloadResult, String> {
    let feed = state.feed_url.lock().map_err(|_| "feed lock poisoned")?.clone();
    if feed.trim().is_empty() {
        return Err("feed url not set".into());
    }

    let mut m = fetch_manifest(&feed).await?;
    m.version = validate_bundle_version(&m.version)?;

    let resp = reqwest::Client::new()
        .get(&m.zip_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("zip http {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
    let digest = Sha256::digest(&bytes);
    let got_hex = hex::encode(digest);
    let want_hex = m.sha256.trim().to_lowercase();
    if got_hex != want_hex {
        return Err("sha256 mismatch".into());
    }

    // Extract into a temp dir first, then atomically rename into place to avoid partial bundles.
    let final_dir = active_bundle_path(app, &m.version)?;
    if final_dir.join("index.html").exists() {
        // Already installed.
        persist_active_version(app, &m.version)?;
        if let Ok(mut g) = state.active_version.lock() {
            *g = m.version.clone();
        }
        state.server.set_root(final_dir);
        let _ = app.emit(EVENT_WEB_UPDATE_READY, m.version.clone());
        return Ok(WebUpdateDownloadResult { activated_version: m.version });
    }

    let final_dir = install_bundle_from_zip_bytes(app, &bytes, &m.version)?;

    persist_active_version(app, &m.version)?;
    if let Ok(mut g) = state.active_version.lock() {
        *g = m.version.clone();
    }
    state.server.set_root(final_dir);

    let _ = app.emit(EVENT_WEB_UPDATE_READY, m.version.clone());
    Ok(WebUpdateDownloadResult { activated_version: m.version })
}
