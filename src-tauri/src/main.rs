// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string() 
    } else if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "ios") {
        "ios".to_string()
    } else {
        "linux".to_string()
    }
}

#[cfg(not(mobile))]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Start bundled Rails server for all platforms
            let resource_dir = app.path().resource_dir()
                .expect("failed to resolve resource directory");
            
            let platform = if cfg!(target_os = "android") {
                "android"
            } else if cfg!(target_os = "ios") {
                "ios"
            } else {
                "desktop"
            };
            
            println!("{} app resource directory: {:?}", platform, resource_dir);
            
            // Start Rails server in bundled directory
            std::thread::spawn(move || {
                let rails_command = if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(&["/C", "ruby", "bin/rails", "server", "-p", "3001", "-e", platform])
                        .current_dir(&resource_dir)
                        .spawn()
                } else {
                    std::process::Command::new("ruby")
                        .args(&["bin/rails", "server", "-p", "3001", "-e", platform])
                        .current_dir(&resource_dir)
                        .spawn()
                };
                
                match rails_command {
                    Ok(child) => {
                        println!("Rails server started successfully for {}", platform);
                        let _ = child.wait_with_output();
                    }
                    Err(e) => {
                        println!("Failed to start Rails server for {}: {}", platform, e);
                    }
                }
            });
            
            // Give Rails a moment to start
            std::thread::sleep(std::time::Duration::from_secs(3));
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}