// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::{
    accept_friend_request, add_friend, add_message_reaction, add_post_comment, add_post_reaction,
    block_user, cancel_friend_request, cleanup_expired_messages, cleanup_expired_mutes,
    cleanup_old_notifications, create_friend_invite, create_new_user, create_notification,
    create_post, create_post_with_id, debug_log, decrypt_message_for_user, delete_message,
    delete_notification, delete_post, delete_post_comment, edit_message, edit_post,
    export_friends_list, generate_friend_qr_code, generate_qr_code, generate_recovery_phrase,
    get_all_posts, get_blocked_users, get_comment_replies, get_friends, get_friends_of_friends,
    get_media_attachments, get_media_file_data, get_message_reactions, get_message_thread,
    get_messages_for_user, get_mute_settings, get_muted_users, get_notifications,
    get_outgoing_friend_requests, get_pending_friend_requests, get_platform,
    get_post_comment_count, get_post_comments, get_post_reaction_summary, get_post_reactions,
    get_post_shares, get_recent_contacts, get_shared_post, get_unread_notification_count,
    get_unread_notifications, get_user_by_id, get_user_by_public_key, get_websocket_port,
    import_friends_list, is_blocked_either_way, is_user_blocked, is_user_muted,
    mark_all_notifications_read, mark_notification_read, mute_user, parse_qr_code_data,
    reject_friend_request, remove_post_reaction, reply_to_message, restore_from_recovery_phrase,
    save_media_to_downloads, scan_qr_code_from_image, search_friends, send_encrypted_message,
    share_post, start_notification_server, unblock_user, unmute_user, update_mute_settings,
    update_recent_contact, update_user_profile, upload_media_file, upload_profile_picture,
    use_friend_invite, validate_recovery_phrase, verify_message_signature, Database,
};
use tauri::Emitter;
use tauri::Manager;

// P2P networking commands (Iroh-based)
use app::iroh_commands::{
    iroh_add_friend_by_public_key, iroh_announce_presence, iroh_enter_background,
    iroh_enter_foreground, iroh_generate_invite, iroh_get_connection_status, iroh_health_check,
    iroh_initialize, iroh_publish_post, iroh_publish_post_comment, iroh_publish_post_reaction,
    iroh_read_blob, iroh_recover, iroh_send_message, iroh_shutdown, iroh_subscribe_friend,
    parse_invite_code,
};

// Device management commands
use app::{
    get_device, get_device_count, get_device_info_list, get_user_devices, remove_device,
    update_device_name, update_device_sync, verify_device_ownership,
};

// Device-to-device sync commands
use app::{apply_sync_data, get_sync_data, get_sync_status, update_all_sync_timestamps};

// App settings commands
use app::{add_storage_used, can_store_data, get_app_settings, set_storage_limit};

// Community commands
use app::community_commands::{
    announce_community_member, create_community, create_community_invite, create_community_post,
    get_community, get_community_feed, get_community_members, get_my_communities,
    join_community_by_invite, leave_community, publish_community_post,
};

/// Initialize the database for the Tauri application
fn init_database(app: &tauri::App) -> Database {
    // Check for test data directory override (only for testing, never in production builds)
    let db_path = if let Ok(test_data_dir) = std::env::var("CIPHER_TEST_DATA_DIR") {
        // Only allow /tmp/ paths for safety
        if test_data_dir.starts_with("/tmp/") {
            println!("Using test data directory: {}", test_data_dir);
            std::path::PathBuf::from(&test_data_dir).join("cipher.db")
        } else {
            panic!("CIPHER_TEST_DATA_DIR must be in /tmp/ for safety");
        }
    } else {
        // Normal production path
        let app_data_dir = app
            .path()
            .app_data_dir()
            .expect("failed to resolve app data directory");
        app_data_dir.join("cipher.db")
    };

    // Create app data directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create app data directory");
    }

    let database =
        Database::new(&db_path.to_string_lossy()).expect("Failed to initialize database");

    println!("Database initialized at: {:?}", db_path);

    database
}

/// Setup function for the Tauri application
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Cipher desktop app");

    // Initialize database
    let database = init_database(app);

    // Store database in app state
    app.manage(database);

    Ok(())
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init::<tauri::Wry>())
        .setup(setup_app)
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
            // Post editing and deletion
            edit_post,
            delete_post,
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
            // Message features
            add_message_reaction,
            get_message_reactions,
            reply_to_message,
            get_message_thread,
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
                // Desktop app resumed (rare on desktop, but can happen)
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
            tauri::RunEvent::ExitRequested { .. } => {
                // Let the exit proceed. prevent_exit() here left a windowless zombie
                // process holding the device keypair - peers kept connecting to the
                // dead instance's NodeId and the next launch fought it for the DB.
                // SQLite WAL is crash-safe, so no flush dance is needed.
                println!("[LIFECYCLE] App exit requested - exiting");
            }
            _ => {}
        }
    });
}
