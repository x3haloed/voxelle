// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod web_update;

#[tauri::command]
fn web_update_status(state: tauri::State<web_update::WebUpdateState>) -> web_update::WebUpdateStatus {
    web_update::status(&state)
}

#[tauri::command]
fn web_update_set_feed(state: tauri::State<web_update::WebUpdateState>, app: tauri::AppHandle, url: String) -> Result<(), String> {
    web_update::set_feed(&state, &app, &url)
}

#[tauri::command]
async fn web_update_check(state: tauri::State<'_, web_update::WebUpdateState>) -> Result<web_update::WebUpdateCheckResult, String> {
    web_update::check(&state).await
}

#[tauri::command]
async fn web_update_download(
    state: tauri::State<'_, web_update::WebUpdateState>,
    app: tauri::AppHandle,
) -> Result<web_update::WebUpdateDownloadResult, String> {
    web_update::download_and_activate(&state, &app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // In dev, keep using the configured devUrl.
            if cfg!(debug_assertions) {
                return Ok(());
            }

            let embedded_version = env!("CARGO_PKG_VERSION").to_string();
            let persisted_active = web_update::load_persisted_active_version(&app.handle()).unwrap_or_default();
            let active_version = if persisted_active.trim().is_empty() {
                embedded_version.clone()
            } else {
                persisted_active
            };

            let embedded_zip: &[u8] = {
                #[cfg(not(debug_assertions))]
                {
                    include_bytes!(concat!(env!("OUT_DIR"), "/voxelle_web_bundle.zip"))
                }
                #[cfg(debug_assertions)]
                {
                    &[]
                }
            };

            let root_dir = web_update::ensure_embedded_bundle(&app.handle(), embedded_zip, &active_version)?;
            // Ensure active version is persisted so status works and later updates compare correctly.
            let _ = web_update::persist_active_version(&app.handle(), &active_version);

            // Start localhost server that serves the currently active bundle from disk.
            let server = web_update::WebBundleServer::start(root_dir)?;

            // Restore persisted feed URL (optional; can be empty).
            let feed_url = web_update::load_persisted_feed_url(&app.handle()).unwrap_or_default();

            app.manage(web_update::WebUpdateState {
                server: server.clone(),
                active_version: std::sync::Arc::new(std::sync::Mutex::new(active_version.clone())),
                feed_url: std::sync::Arc::new(std::sync::Mutex::new(feed_url)),
            });

            // Navigate the main window to the localhost server.
            if let Some(w) = app.get_webview_window("main") {
                let url: tauri::Url = format!("http://127.0.0.1:{}/", server.port())
                    .parse()
                    .expect("localhost URL should be parseable");
                w.navigate(url).map_err(|e| e.to_string())?;
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            web_update_status,
            web_update_set_feed,
            web_update_check,
            web_update_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
