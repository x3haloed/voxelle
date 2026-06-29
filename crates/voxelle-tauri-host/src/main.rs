use std::path::PathBuf;

fn main() {
    tauri::Builder::default()
        .manage(voxelle_tauri::shell_state(default_home_root()))
        .invoke_handler(voxelle_tauri::invoke_handler())
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

fn default_home_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".voxelle")
}
