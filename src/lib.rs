pub mod app;
pub use app::*;

// Mobile entry point for Android/iOS builds
#[cfg(mobile)]
use tauri::Manager;

#[cfg(mobile)]
use app::iroh_commands::{
    iroh_add_friend_by_public_key, iroh_announce_presence, iroh_generate_invite,
    iroh_get_connection_status, iroh_initialize, iroh_publish_post, iroh_send_message,
    iroh_shutdown, iroh_subscribe_friend,
};

#[cfg(mobile)]
#[tauri::mobile_entry_point]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_barcode_scanner::init())
        .setup(|app| {
            println!("Starting Cipher mobile app");

            // Initialize database
            let app_data_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("Failed to get app data dir: {:?}", e);
                    return Err(e.into());
                }
            };

            if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                eprintln!("Failed to create app data directory: {:?}", e);
                return Err(e.into());
            }

            let db_path = app_data_dir.join("cipher.db");
            let db_path_str = db_path.to_string_lossy().to_string();

            println!("Initializing database at: {:?}", db_path_str);

            let database = match Database::new(&db_path_str) {
                Ok(db) => {
                    println!("Database initialized successfully");
                    db
                }
                Err(e) => {
                    eprintln!("Failed to initialize database: {:?}", e);
                    eprintln!("Database path: {:?}", db_path_str);
                    return Err(Box::new(e));
                }
            };

            app.manage(database);

            // Initialize notification server (but don't start it here - it's async)
            let notification_server = NotificationServer::new();
            app.manage(notification_server);

            println!("Cipher mobile app setup completed successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // User management
            create_new_user,
            restore_from_recovery_phrase,
            get_user_by_public_key,
            get_user_by_id,
            generate_recovery_phrase,
            validate_recovery_phrase,
            update_user_profile,
            upload_profile_picture,
            // Posts
            get_all_posts,
            create_post,
            edit_post,
            delete_post,
            // Messaging
            send_encrypted_message,
            get_messages_for_user,
            decrypt_message_for_user,
            verify_message_signature,
            search_messages,
            edit_message,
            delete_message,
            cleanup_expired_messages,
            // Friends
            add_friend,
            get_friends,
            create_friend_invite,
            use_friend_invite,
            export_friends_list,
            import_friends_list,
            get_recent_contacts,
            update_recent_contact,
            // Media
            upload_media_file,
            get_media_attachments,
            get_media_file_data,
            // QR codes
            generate_qr_code,
            parse_qr_code_data,
            scan_qr_code_from_image,
            // System
            get_platform,
            debug_log,
            get_websocket_port,
            start_notification_server,
            // Message features
            add_message_reaction,
            get_message_reactions,
            reply_to_message,
            get_message_thread,
            send_voice_message,
            get_voice_messages,
            delete_voice_message,
            // Device Management
            get_user_devices,
            get_device,
            update_device_name,
            remove_device,
            get_device_info_list,
            update_device_sync,
            verify_device_ownership,
            get_device_count,
            // Device-to-Device Sync
            get_sync_data,
            apply_sync_data,
            update_all_sync_timestamps,
            get_sync_status,
            // Typing Indicators
            set_typing_indicator,
            get_typing_indicator,
            clear_typing_indicator,
            cleanup_old_typing_indicators,
            // P2P Networking (Iroh-based)
            iroh_initialize,
            iroh_subscribe_friend,
            iroh_send_message,
            iroh_publish_post,
            iroh_announce_presence,
            iroh_get_connection_status,
            iroh_shutdown,
            iroh_generate_invite,
            iroh_add_friend_by_public_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
