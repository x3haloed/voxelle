use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub const EVENT_WEB_UPDATE_READY: &str = "voxelle:web-update-ready";

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
    Ok(read_text_file(&feed_file(app)?))
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
    let dir = active_bundle_path(app, version)?;
    let index = dir.join("index.html");
    if index.exists() {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    extract_zip_bytes(embedded_zip, &dir)?;
    Ok(dir)
}

fn extract_zip_bytes(zip_bytes: &[u8], out_dir: &Path) -> Result<(), String> {
    let mut z = zip::ZipArchive::new(Cursor::new(zip_bytes)).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if name.contains("..") {
            continue;
        }
        let out_path = out_dir.join(name);
        if f.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out_path, buf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn parse_version(s: &str) -> Option<semver::Version> {
    semver::Version::parse(s.trim()).ok()
}

async fn fetch_manifest(url: &str) -> Result<WebBundleManifestV1, String> {
    let resp = reqwest::Client::new()
        .get(url)
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
    Ok(m)
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

    let m = fetch_manifest(&feed).await?;

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

    let dir = active_bundle_path(app, &m.version)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    extract_zip_bytes(&bytes, &dir)?;

    persist_active_version(app, &m.version)?;
    if let Ok(mut g) = state.active_version.lock() {
        *g = m.version.clone();
    }
    state.server.set_root(dir);

    let _ = app.emit(EVENT_WEB_UPDATE_READY, m.version.clone());
    Ok(WebUpdateDownloadResult { activated_version: m.version })
}
