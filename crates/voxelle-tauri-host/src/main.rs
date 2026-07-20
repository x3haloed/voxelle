use std::path::PathBuf;

fn main() {
    tauri::Builder::default()
        .manage(voxelle_tauri::shell_state(home_root()))
        .invoke_handler(voxelle_tauri::invoke_handler())
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

fn home_root() -> PathBuf {
    std::env::var_os("VOXELLE_HOME_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(default_home_root)
}

fn default_home_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".voxelle")
}
