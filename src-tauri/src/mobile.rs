use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tauri::Manager;

#[cfg(target_os = "android")]
use std::os::unix::fs::PermissionsExt;

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        fs::create_dir_all(destination)?;
    }

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let src_path = entry.path();
        let dest_path = destination.join(entry.file_name());

        if metadata.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

fn prepare_embedded_runtime(app: &tauri::App<tauri::Wry>) -> io::Result<PathBuf> {
    let resource_dir = app
        .path()
        .resource_dir()
        .expect("failed to resolve resource directory");
    let runtime_source = resource_dir.join("runtime").join("dist");

    if !runtime_source.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("embedded runtime not found at {:?}", runtime_source),
        ));
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| app.path().app_data_dir().expect("data dir missing"));
    let runtime_target = cache_dir.join("ruby_runtime");

    if !runtime_target.exists() {
        copy_dir_recursive(&runtime_source, &runtime_target)?;
    }

    #[cfg(target_os = "android")]
    {
        let script = runtime_target.join("run_rails.sh");
        if script.exists() {
            let mut permissions = fs::metadata(&script)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions)?;
        }
    }

    Ok(runtime_target)
}

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
            let runtime =
                prepare_embedded_runtime(app).expect("failed to prepare embedded Ruby runtime");
            let script_path = runtime.join("run_rails.sh");

            std::thread::spawn(move || {
                println!("Launching embedded Rails from {:?}", script_path);
                let mut command = std::process::Command::new("sh");
                command.arg(script_path.to_string_lossy().to_string());
                command.current_dir(&runtime);
                command.env("RUBY_BUNDLED_RUNTIME", &runtime);

                match command.spawn() {
                    Ok(child) => {
                        println!("Embedded Rails server started");
                        let _ = child.wait_with_output();
                    }
                    Err(err) => {
                        println!("Failed to start embedded Rails: {}", err);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri mobile application");
}
