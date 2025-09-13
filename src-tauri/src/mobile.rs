use tauri::Manager;

#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "ios") {
        "ios".to_string()
    } else {
        "mobile".to_string()
    }
}

#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app: &mut tauri::App<tauri::Wry>| {
            // Start bundled Rails server for mobile platforms
            let resource_dir = app.path().resource_dir()
                .expect("failed to resolve resource directory");
            
            let platform = if cfg!(target_os = "android") {
                "android"
            } else if cfg!(target_os = "ios") {
                "ios"
            } else {
                "mobile"
            };
            
            println!("Cipher mobile app started for platform: {}", platform);
            println!("Mobile app resource directory: {:?}", resource_dir);
            
            // Start Rails server in bundled directory (localhost-only for security)
            std::thread::spawn(move || {
                let rails_command = if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(&["/C", "ruby", "bin/rails", "server", "-p", "3001", "-b", "127.0.0.1", "-e", platform])
                        .current_dir(&resource_dir)
                        .spawn()
                } else {
                    std::process::Command::new("ruby")
                        .args(&["bin/rails", "server", "-p", "3001", "-b", "127.0.0.1", "-e", platform])
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
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri mobile application");
}