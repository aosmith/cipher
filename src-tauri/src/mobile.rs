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
        .setup(|app| {
            // Mobile-specific setup with embedded Rails server
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
            println!("Resource directory: {:?}", resource_dir);
            println!("Starting embedded Rails server...");
            
            // Start embedded Rails server for mobile (localhost-only for security)
            std::thread::spawn(move || {
                let rails_command = std::process::Command::new("ruby")
                    .args(&["bin/rails", "server", "-p", "3001", "-b", "127.0.0.1", "-e", platform])
                    .current_dir(&resource_dir)
                    .spawn();
                
                match rails_command {
                    Ok(child) => {
                        println!("Embedded Rails server started successfully for {}", platform);
                        let _ = child.wait_with_output();
                    }
                    Err(e) => {
                        println!("Failed to start embedded Rails server for {}: {}", platform, e);
                        println!("Falling back to static assets only");
                    }
                }
            });
            
            // Give Rails a moment to start
            std::thread::sleep(std::time::Duration::from_secs(3));
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri mobile application");
}