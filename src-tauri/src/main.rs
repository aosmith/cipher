// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

// Struct to hold the Rails server process
struct RailsServer(Mutex<Option<Child>>);

#[tauri::command]
fn start_rails_server(state: tauri::State<RailsServer>) -> Result<String, String> {
    let mut server = state.0.lock().unwrap();
    
    if server.is_some() {
        return Ok("Rails server is already running".to_string());
    }
    
    // Platform-specific Rails server command
    let (command, args) = if cfg!(target_os = "windows") {
        ("ruby", vec!["bin/rails", "server", "-p", "3001", "-e", "desktop"])
    } else {
        ("bin/rails", vec!["server", "-p", "3001", "-e", "desktop"])
    };
    
    let child = Command::new(command)
        .args(&args)
        .current_dir(".")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start Rails server: {}", e))?;
    
    *server = Some(child);
    Ok("Rails server started successfully".to_string())
}

#[tauri::command]
fn stop_rails_server(state: tauri::State<RailsServer>) -> Result<String, String> {
    let mut server = state.0.lock().unwrap();
    
    if let Some(mut child) = server.take() {
        child.kill().map_err(|e| format!("Failed to stop Rails server: {}", e))?;
        Ok("Rails server stopped".to_string())
    } else {
        Ok("Rails server was not running".to_string())
    }
}

#[tauri::command]
fn open_external_url(url: String) {
    let _ = open::that(url);
}

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
    // Create system tray
    let quit = CustomMenuItem::new("quit".to_string(), "Quit Cipher");
    let show = CustomMenuItem::new("show".to_string(), "Show Cipher");
    let hide = CustomMenuItem::new("hide".to_string(), "Hide Cipher");
    
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);
    
    let tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .manage(RailsServer(Mutex::new(None)))
        .system_tray(tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick {
                position: _,
                size: _,
                ..
            } => {
                let window = app.get_window("main").unwrap();
                if window.is_visible().unwrap() {
                    window.hide().unwrap();
                } else {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "hide" => {
                    let window = app.get_window("main").unwrap();
                    window.hide().unwrap();
                }
                _ => {}
            },
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Hide instead of closing when user clicks the close button
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .setup(|app| {
            // Auto-start Rails server when app launches
            let app_handle = app.handle();
            let state = app_handle.state::<RailsServer>();
            
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let _ = start_rails_server(state);
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_rails_server,
            stop_rails_server,
            open_external_url,
            get_platform
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}