// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn microsoft_login() -> String {
    // Placeholder for Microsoft login logic
    "Microsoft login initiated".into()
}

#[tauri::command]
fn offline_login() -> String {
    // Placeholder for offline login logic
    "Offline login initiated".into()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![microsoft_login, offline_login])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}