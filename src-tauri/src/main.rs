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
                // Get app data directory for user-writable storage
                let app_data_dir = app
                    .path()
                    .app_data_dir()
                    .expect("failed to resolve app data directory");

                // Start Rails server in bundled directory (localhost-only for security)
                std::thread::spawn(move || {
                    // First, ensure app data and storage directories exist
                    let storage_dir = app_data_dir.join("storage");
                    if !storage_dir.exists() {
                        if let Err(e) = std::fs::create_dir_all(&storage_dir) {
                            println!("Failed to create storage directory: {}", e);
                        } else {
                            println!("Created storage directory at: {:?}", storage_dir);
                        }
                    }

                    // Set database path to app data directory
                    let db_path = storage_dir.join(format!("{}.sqlite3", platform));
                    std::env::set_var("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()));
                    println!("Database path set to: {:?}", db_path);

                    // Initialize the database with explicit steps
                    println!("Initializing database for {} environment...", platform);

                    // Step 1: Create database
                    let db_create_command = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(&[
                                "/C",
                                "ruby",
                                "bin/rails",
                                "db:create",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    } else {
                        std::process::Command::new("ruby")
                            .args(&[
                                "bin/rails",
                                "db:create",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    };

                    match db_create_command {
                        Ok(output) => {
                            println!("Database create result: {}", output.status);
                            if !output.stdout.is_empty() {
                                println!("Create stdout: {}", String::from_utf8_lossy(&output.stdout));
                            }
                            if !output.stderr.is_empty() {
                                println!("Create stderr: {}", String::from_utf8_lossy(&output.stderr));
                            }
                        }
                        Err(e) => {
                            println!("Failed to run db:create for {}: {}", platform, e);
                        }
                    }

                    // Step 2: Load schema
                    let db_schema_command = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(&[
                                "/C",
                                "ruby",
                                "bin/rails",
                                "db:schema:load",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    } else {
                        std::process::Command::new("ruby")
                            .args(&[
                                "bin/rails",
                                "db:schema:load",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    };

                    match db_schema_command {
                        Ok(output) => {
                            if output.status.success() {
                                println!("Database schema loaded successfully for {}", platform);
                            } else {
                                println!("Schema load failed for {}: {}", platform, String::from_utf8_lossy(&output.stderr));
                                // Fallback to migrations if schema load fails
                                println!("Attempting fallback to db:migrate...");
                                let db_migrate_command = if cfg!(target_os = "windows") {
                                    std::process::Command::new("cmd")
                                        .args(&[
                                            "/C",
                                            "ruby",
                                            "bin/rails",
                                            "db:migrate",
                                        ])
                                        .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                                        .current_dir(&root)
                                        .output()
                                } else {
                                    std::process::Command::new("ruby")
                                        .args(&[
                                            "bin/rails",
                                            "db:migrate",
                                        ])
                                        .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                                        .current_dir(&root)
                                        .output()
                                };

                                match db_migrate_command {
                                    Ok(migrate_output) => {
                                        if migrate_output.status.success() {
                                            println!("Database migrations completed successfully for {}", platform);
                                        } else {
                                            println!("Migration failed for {}: {}", platform, String::from_utf8_lossy(&migrate_output.stderr));
                                        }
                                    }
                                    Err(e) => {
                                        println!("Failed to run db:migrate for {}: {}", platform, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("Failed to run db:schema:load for {}: {}", platform, e);
                        }
                    }

                    // Verify database was properly initialized by checking for users table
                    println!("Verifying database initialization for {}...", platform);
                    let verify_command = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(&[
                                "/C",
                                "ruby",
                                "-e",
                                "require_relative 'config/environment'; puts User.table_exists? ? 'Database verified' : 'Database missing tables'",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    } else {
                        std::process::Command::new("ruby")
                            .args(&[
                                "-e",
                                "require_relative 'config/environment'; puts User.table_exists? ? 'Database verified' : 'Database missing tables'",
                            ])
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
                            .current_dir(&root)
                            .output()
                    };

                    match verify_command {
                        Ok(verify_output) => {
                            let output_str = String::from_utf8_lossy(&verify_output.stdout);
                            println!("Database verification result: {}", output_str.trim());
                        }
                        Err(e) => {
                            println!("Failed to verify database for {}: {}", platform, e);
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
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
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
                            .env("RAILS_ENV", platform)
                            .env("DATABASE_URL", format!("sqlite3://{}", db_path.to_string_lossy()))
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
