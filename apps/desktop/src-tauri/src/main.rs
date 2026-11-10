// Desktop shell (ADR-001): the UI and the WASM engine run inside the system
// webview; this process only provides the window and native file dialogs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running Circuitaria");
}
