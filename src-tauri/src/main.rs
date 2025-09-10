// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Start Rails server on app startup
            #[cfg(target_os = "windows")]
            {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("cmd")
                        .args(&["/C", "rails", "server", "-p", "3001", "-e", "desktop"])
                        .spawn();
                });
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("rails")
                        .args(&["server", "-p", "3001", "-e", "desktop"])
                        .spawn();
                });
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}