fn main() {
    tauri::Builder::default()
        .manage(voxelle_tauri::shell_state(voxelle_app::resolve_home_root(
            None,
        )))
        .invoke_handler(voxelle_tauri::invoke_handler())
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}
