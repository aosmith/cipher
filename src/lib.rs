pub mod app;
pub use app::*;

// Mobile entry point for Android/iOS builds
#[cfg(mobile)]
use tauri::{Manager, Emitter};

#[cfg(mobile)]
use app::iroh_commands::{
    iroh_add_friend_by_public_key, iroh_announce_presence, iroh_enter_background,
    iroh_enter_foreground, iroh_generate_invite, iroh_get_connection_status, iroh_health_check,
    iroh_initialize, iroh_publish_post, iroh_publish_post_comment, iroh_publish_post_reaction,
    iroh_read_blob, iroh_recover, iroh_send_message, iroh_shutdown, iroh_subscribe_friend,
    parse_invite_code,
};

#[cfg(mobile)]
use app::community_commands::{
    announce_community_member, create_community, create_community_invite, create_community_post,
    get_community, get_community_feed, get_community_members, get_my_communities,
    join_community_by_invite, leave_community, publish_community_post,
};

#[cfg(mobile)]
#[tauri::mobile_entry_point]
fn main() {
    let app = tauri::Builder::default()
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
            create_post_with_id,
            edit_post,
            delete_post,
            // Messaging
            send_encrypted_message,
            get_messages_for_user,
            decrypt_message_for_user,
            verify_message_signature,
            edit_message,
            delete_message,
            cleanup_expired_messages,
            // Friends
            add_friend,
            get_friends,
            get_pending_friend_requests,
            get_outgoing_friend_requests,
            accept_friend_request,
            reject_friend_request,
            cancel_friend_request,
            search_friends,
            create_friend_invite,
            use_friend_invite,
            get_friends_of_friends,
            export_friends_list,
            import_friends_list,
            get_recent_contacts,
            update_recent_contact,
            // Post reactions
            add_post_reaction,
            remove_post_reaction,
            get_post_reactions,
            get_post_reaction_summary,
            // Post comments
            add_post_comment,
            get_post_comments,
            get_comment_replies,
            delete_post_comment,
            get_post_comment_count,
            // Post sharing
            share_post,
            get_shared_post,
            get_post_shares,
            // Notifications
            create_notification,
            get_notifications,
            get_unread_notifications,
            mark_notification_read,
            mark_all_notifications_read,
            delete_notification,
            get_unread_notification_count,
            cleanup_old_notifications,
            // Block/Mute (Safety)
            block_user,
            unblock_user,
            is_user_blocked,
            get_blocked_users,
            is_blocked_either_way,
            mute_user,
            unmute_user,
            is_user_muted,
            get_mute_settings,
            get_muted_users,
            cleanup_expired_mutes,
            update_mute_settings,
            // Media
            upload_media_file,
            get_media_attachments,
            get_media_file_data,
            save_media_to_downloads,
            // QR codes
            generate_qr_code,
            parse_qr_code_data,
            scan_qr_code_from_image,
            generate_friend_qr_code,
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
            // P2P Networking (Iroh-based)
            iroh_initialize,
            iroh_subscribe_friend,
            iroh_send_message,
            iroh_publish_post,
            iroh_publish_post_comment,
            iroh_publish_post_reaction,
            iroh_announce_presence,
            iroh_get_connection_status,
            iroh_health_check,
            iroh_recover,
            iroh_shutdown,
            iroh_enter_background,
            iroh_enter_foreground,
            iroh_generate_invite,
            iroh_add_friend_by_public_key,
            iroh_read_blob,
            parse_invite_code,
            // App Settings
            get_app_settings,
            set_storage_limit,
            can_store_data,
            add_storage_used,
            // Communities
            create_community,
            get_my_communities,
            get_community,
            get_community_members,
            leave_community,
            create_community_invite,
            join_community_by_invite,
            create_community_post,
            get_community_feed,
            publish_community_post,
            announce_community_member
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Run with lifecycle event handling
    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::Resumed => {
                // Mobile app resumed from background
                println!("[LIFECYCLE] App resumed - triggering foreground handler");

                // Emit event to frontend so it can call iroh_enter_foreground
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("app-resumed", ());
                }
            }
            tauri::RunEvent::Ready => {
                println!("[LIFECYCLE] App ready");
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                if let tauri::WindowEvent::Focused(focused) = event {
                    if focused {
                        println!("[LIFECYCLE] Window focused - app in foreground");
                        if let Some(window) = app_handle.get_webview_window(&label) {
                            let _ = window.emit("app-resumed", ());
                        }
                    } else {
                        println!("[LIFECYCLE] Window lost focus - app may be backgrounding");
                    }
                }
            }
            tauri::RunEvent::ExitRequested { api, .. } => {
                println!("[LIFECYCLE] App exit requested - triggering background handler");
                // Emit event to frontend so it can call iroh_enter_background
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("app-backgrounding", ());
                }
                // Don't prevent exit
                api.prevent_exit();
            }
            _ => {}
        }
    });
}
