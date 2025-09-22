// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[cfg(not(mobile))]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init::<tauri::Wry>())
        .setup(|app| {
            // Start bundled Rails server for packaged builds only
            let resource_dir = app
                .path()
                .resource_dir()
                .expect("failed to resolve resource directory");

            let platform = if cfg!(target_os = "android") {
                "android"
            } else if cfg!(target_os = "ios") {
                "ios"
            } else {
                "desktop"
            };

            let mut candidate_roots: Vec<std::path::PathBuf> = vec![resource_dir.clone()];
            candidate_roots.push(resource_dir.join("_up_"));
            if let Ok(current_exe) = std::env::current_exe() {
                if let Some(exe_dir) = current_exe.parent() {
                    candidate_roots.push(exe_dir.join("../.."));
                }
            }
            candidate_roots.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));

            let rails_root = candidate_roots
                .into_iter()
                .find(|root| root.join("bin/rails").exists());

            println!(
                "{} app resource directory: {:?}; rails root: {:?}",
                platform, resource_dir, rails_root
            );

            if let Some(root) = rails_root {
                // Start Rails server in bundled directory (localhost-only for security)
                std::thread::spawn(move || {
                    // First, ensure storage directory exists
                    let storage_dir = root.join("storage");
                    if !storage_dir.exists() {
                        if let Err(e) = std::fs::create_dir_all(&storage_dir) {
                            println!("Failed to create storage directory: {}", e);
                        } else {
                            println!("Created storage directory at: {:?}", storage_dir);
                        }
                    }

                    // Prepare the database
                    println!("Preparing database for {} environment...", platform);
                    let db_prepare_command = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(&[
                                "/C",
                                "ruby",
                                "bin/rails",
                                "db:prepare",
                            ])
                            .env("RAILS_ENV", platform)
                            .current_dir(&root)
                            .output()
                    } else {
                        std::process::Command::new("ruby")
                            .args(&[
                                "bin/rails",
                                "db:prepare",
                            ])
                            .env("RAILS_ENV", platform)
                            .current_dir(&root)
                            .output()
                    };

                    match db_prepare_command {
                        Ok(output) => {
                            if output.status.success() {
                                println!("Database prepared successfully for {}", platform);
                                println!("Database preparation output: {}", String::from_utf8_lossy(&output.stdout));
                            } else {
                                println!("Database preparation failed for {}: {}", platform, String::from_utf8_lossy(&output.stderr));
                                println!("Exit status: {}", output.status);
                                // Also show stdout in case there are useful messages
                                println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
                            }
                        }
                        Err(e) => {
                            println!("Failed to run db:prepare for {}: {}", platform, e);
                        }
                    }

                    // Now start the Rails server
                    let rails_command = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(&[
                                "/C",
                                "ruby",
                                "bin/rails",
                                "server",
                                "-p",
                                "3000",
                                "-b",
                                "127.0.0.1",
                                "-e",
                                platform,
                            ])
                            .current_dir(&root)
                            .spawn()
                    } else {
                        std::process::Command::new("ruby")
                            .args(&[
                                "bin/rails",
                                "server",
                                "-p",
                                "3000",
                                "-b",
                                "127.0.0.1",
                                "-e",
                                platform,
                            ])
                            .current_dir(&root)
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
            } else {
                println!(
                    "Skipping bundled Rails launch for {} (bin/rails not found in bundled resources)",
                    platform
                );
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
