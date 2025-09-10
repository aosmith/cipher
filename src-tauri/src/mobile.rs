use tauri::Manager;

#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Mobile-specific setup - completely static, no server
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
            println!("Running as pure static app - no server connections");
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri mobile application");
}