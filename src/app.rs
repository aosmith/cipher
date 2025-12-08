use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use image::Luma;
use qrcode::QrCode;
use rqrr::PreparedImage;
use rusqlite::{params, OptionalExtension};
use tauri::{Manager, State};
// Removed WebSocket imports - now using Iroh for P2P networking
use std::fs;
use std::io::Write;
use std::path::Path;

// Type definitions - organized into modules
pub mod types;

// Re-export all types for backward compatibility
pub use types::*;

// Import Uuid from the uuid crate (distinct from our types::uuid module)
use ::uuid::Uuid;

// Iroh P2P networking
pub mod iroh_commands;
pub mod iroh_network;

// Crypto module for sealed box encryption
pub mod crypto;

// Database module
pub mod database;
pub use database::Database;

/// Create new user on first launch - generates 24-word recovery phrase
/// Returns user and recovery phrase (MUST be shown to user to save)
#[tauri::command]
pub async fn create_new_user(
    display_name: String,
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<UserWithRecoveryPhrase, String> {
    println!("Creating new user with display name: {}", display_name);

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app_data_dir: {}", e))?;
    let device_id = Database::get_or_create_device_id(&app_data_dir)?;
    println!("Using device ID: {}", device_id);

    match db.create_user_first_launch(display_name, device_id) {
        Ok((user, recovery_phrase)) => {
            println!(
                "User created successfully: {} with device_id: {:?}",
                user.username, user.device_id
            );
            println!("SECURITY: Recovery phrase generated - must be shown to user ONCE");
            Ok(UserWithRecoveryPhrase {
                user,
                recovery_phrase,
            })
        }
        Err(e) => {
            println!("User creation failed: {}", e);
            Err(format!("User creation failed: {}", e))
        }
    }
}

/// Restore user from 24-word recovery phrase (new device or data loss)
#[tauri::command]
pub async fn restore_from_recovery_phrase(
    display_name: String,
    recovery_phrase: String,
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<User, String> {
    println!("Restoring user with display name: {}", display_name);

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app_data_dir: {}", e))?;
    let device_id = Database::get_or_create_device_id(&app_data_dir)?;
    println!("Using device ID: {}", device_id);

    match db.restore_user_from_recovery_phrase(display_name, recovery_phrase, device_id) {
        Ok(user) => {
            println!(
                "User restored successfully: {} with device_id: {:?}",
                user.username, user.device_id
            );
            Ok(user)
        }
        Err(e) => {
            println!("User restoration failed: {}", e);
            Err(format!("User restoration failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_user_by_public_key(
    public_key: String,
    db: State<'_, Database>,
) -> Result<Option<User>, String> {
    db.find_user_by_public_key(&public_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_by_id(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Option<User>, String> {
    db.find_user_by_id(user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_recovery_phrase(word_count: Option<usize>) -> Result<String, String> {
    Database::generate_recovery_phrase(word_count)
}

#[tauri::command]
pub async fn validate_recovery_phrase(phrase: String) -> Result<bool, String> {
    Ok(Database::validate_recovery_phrase(&phrase))
}

#[tauri::command]
pub async fn create_post(
    user_id: SqliteUuid,
    content: String,
    attachments: Option<Vec<String>>,
    db: State<'_, Database>,
) -> Result<Post, String> {
    println!("[CREATE_POST] Command called with user_id: {}, content length: {}, has_attachments: {}",
        user_id, content.len(), attachments.is_some());

    println!("[CREATE_POST] About to call db.create_post()...");

    // Create the post locally in the database
    let post = db
        .create_post(user_id, &content, false)
        .map_err(|e| {
            println!("[CREATE_POST] Error creating post: {}", e);
            e.to_string()
        })?;

    println!("[CREATE_POST] Post created successfully with id: {}", post.id);

    // Automatically broadcast the post to the P2P network
    // Access the global Iroh network instance
    use crate::app::iroh_commands::IROH_NETWORK;

    // Clone the network Arc before any await points to avoid holding the lock
    let network_opt = IROH_NETWORK.lock().unwrap().as_ref().cloned();

    if let Some(network) = network_opt {
        println!(
            "[POST-BROADCAST] Auto-broadcasting post {} to P2P network",
            post.id
        );

        // Get post attachments from database (if any)
        let attachments = db.get_post_media(post.id).ok();

        let message = iroh_network::P2PMessage::Post {
            user_id: network.user_id,
            public_key: network.public_key.clone(),
            content: content.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            device_id: network.device_id.clone(),
            attachments,
        };

        // Broadcast to our own user topic
        let topic = format!("cipher/user/{}", network.public_key);
        match network.publish_message(&topic, message).await {
            Ok(_) => println!("[POST-BROADCAST] ✓ Post broadcast successfully"),
            Err(e) => println!("[POST-BROADCAST] Warning: Failed to broadcast post: {}", e),
        }
    } else {
        println!(
            "[POST-BROADCAST] Warning: Iroh network not initialized - post saved locally only"
        );
    }

    Ok(post)
}

#[tauri::command]
pub async fn get_all_posts(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Post>, String> {
    db.get_posts(user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_encrypted_message(
    sender_id: SqliteUuid,
    recipient_id: SqliteUuid,
    content: String,
    disappear_after_seconds: Option<i64>,
    db: State<'_, Database>,
) -> Result<Message, String> {
    db.send_encrypted_message(sender_id, recipient_id, &content, disappear_after_seconds)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_messages_for_user(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Message>, String> {
    db.get_messages_for_user(user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_friend(
    user_id: SqliteUuid,
    friend_user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<P2pConnection, String> {
    db.add_friend(user_id, friend_user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_friends(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<User>, String> {
    db.get_friends(user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_friends_of_friends(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<User>, String> {
    db.get_friends_of_friends(user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pending_friend_requests(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<User>, String> {
    db.get_pending_friend_requests(user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn accept_friend_request(
    user_id: SqliteUuid,
    friend_user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    println!("[ACCEPT-CMD] accept_friend_request called: user={} friend={}", user_id, friend_user_id);

    // Clone db for use in spawn_blocking (Database is Clone)
    let db_clone = db.inner().clone();
    let user_id_clone = user_id;
    let friend_user_id_clone = friend_user_id;

    // Run the blocking database operation on a separate thread pool
    tokio::task::spawn_blocking(move || {
        db_clone.accept_friend_request(user_id_clone, friend_user_id_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| {
        println!("[ACCEPT-CMD] DB error: {}", e);
        e.to_string()
    })?;

    println!("[ACCEPT-CMD] Database updated successfully");

    // Send FriendAccepted message to the requester over P2P (non-blocking)
    // Spawn this in a separate task so it doesn't block the response
    use crate::app::iroh_commands::IROH_NETWORK;

    let network_opt = IROH_NETWORK.lock().unwrap().as_ref().cloned();
    if let Some(network) = network_opt {
        // Get the friend's public key - also use spawn_blocking for this DB call
        let db_clone2 = db.inner().clone();
        let friend_opt = tokio::task::spawn_blocking(move || {
            db_clone2.find_user_by_id(friend_user_id)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();

        if let Some(friend) = friend_opt {
            if let Some(friend_public_key) = friend.public_key {
                // Spawn P2P notification in background - don't block the accept
                let friend_key = friend_public_key.clone();
                tokio::spawn(async move {
                    // Get our node address with a timeout
                    let endpoint_guard = network.endpoint.lock().await;
                    let (node_id_str, relay_url_str) = if let Some(endpoint) = endpoint_guard.as_ref() {
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            endpoint.node_addr()
                        ).await {
                            Ok(Ok(node_addr)) => {
                                let node_id = node_addr.node_id.to_string();
                                let relay_url = node_addr.relay_url()
                                    .map(|url| url.to_string())
                                    .unwrap_or_else(|| "https://euw1-1.relay.iroh.network.".to_string());
                                (node_id, relay_url)
                            }
                            _ => {
                                (endpoint.node_id().to_string(), "https://euw1-1.relay.iroh.network.".to_string())
                            }
                        }
                    } else {
                        (String::new(), String::new())
                    };
                    drop(endpoint_guard);

                    let message = iroh_network::P2PMessage::FriendAccepted {
                        from_user_id: network.user_id,
                        from_public_key: network.public_key.clone(),
                        from_display_name: network.display_name.clone(),
                        from_node_id: node_id_str,
                        from_relay_url: relay_url_str,
                        to_public_key: friend_key.clone(),
                    };

                    // GLOBAL MESH: Publish to global content topic
                    if let Err(e) = network.publish_message(iroh_network::CONTENT_TOPIC, message).await {
                        println!("[FRIEND-ACCEPT] Warning: Failed to send FriendAccepted message: {}", e);
                    } else {
                        println!("[FRIEND-ACCEPT] Sent FriendAccepted via global mesh to {}", friend_key);
                    }
                });
            }
        }
    }

    println!("[ACCEPT-CMD] Returning success");
    Ok(())
}

#[tauri::command]
pub async fn reject_friend_request(
    user_id: SqliteUuid,
    friend_user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        db_clone.reject_friend_request(user_id, friend_user_id)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| e.to_string())
}

/// Search for friends by username
/// This only searches within existing friendships for security
#[tauri::command]
pub async fn search_friends(
    user_id: SqliteUuid,
    username: String,
    db: State<'_, Database>,
) -> Result<Option<User>, String> {
    db.find_friend_by_username(user_id, &username)
        .map_err(|e| e.to_string())
}

/// Get outgoing friend requests (requests we sent that are still pending)
#[tauri::command]
pub async fn get_outgoing_friend_requests(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<User>, String> {
    db.get_outgoing_friend_requests(user_id)
        .map_err(|e| e.to_string())
}

/// Cancel an outgoing friend request
#[tauri::command]
pub async fn cancel_friend_request(
    user_id: SqliteUuid,
    friend_user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        db_clone.cancel_friend_request(user_id, friend_user_id)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn decrypt_message_for_user(
    encrypted_message: String,
    sender_public_key: String,
    recipient_private_key: String,
) -> Result<String, String> {
    Database::decrypt_message(
        &encrypted_message,
        &sender_public_key,
        &recipient_private_key,
    )
}

#[tauri::command]
pub async fn verify_message_signature(
    message: String,
    signature: String,
    public_key: String,
) -> Result<bool, String> {
    Ok(Database::verify_signature(
        &message,
        &signature,
        &public_key,
    ))
}

#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "ios") {
        "ios".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}

#[tauri::command]
pub fn generate_qr_code(data: String) -> Result<String, String> {
    // Generate QR code
    let code = QrCode::new(&data).map_err(|e| format!("Failed to generate QR code: {}", e))?;

    // Create a simple black and white image
    let size = code.width();
    let image_size = size * 8; // 8 pixels per module

    let mut imgbuf = image::ImageBuffer::new(image_size as u32, image_size as u32);

    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
        let qr_x = (x / 8) as usize;
        let qr_y = (y / 8) as usize;

        // Get the module value from QR code
        use qrcode::Color;
        let module = code[(qr_x, qr_y)];
        let value = match module {
            Color::Dark => 0u8,
            Color::Light => 255u8,
        };
        *pixel = Luma([value]);
    }

    // Convert to PNG bytes
    let mut png_data = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder
            .write_image(
                imgbuf.as_raw(),
                image_size as u32,
                image_size as u32,
                image::ExtendedColorType::L8,
            )
            .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    }

    // Convert to base64 data URL
    let base64_data = general_purpose::STANDARD.encode(&png_data);
    Ok(format!("data:image/png;base64,{}", base64_data))
}

#[tauri::command]
pub fn scan_qr_code_from_image(base64_image: String) -> Result<QrCodeData, String> {
    // Remove the data URL prefix if present
    let base64_data = if base64_image.starts_with("data:image/") {
        base64_image.split(',').nth(1).unwrap_or(&base64_image)
    } else {
        &base64_image
    };

    // Decode base64 image data
    let image_bytes = general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64 image: {}", e))?;

    // Load image using the image crate
    let img = image::load_from_memory(&image_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Convert to grayscale for QR code scanning
    let gray_img = img.to_luma8();
    let (_width, _height) = gray_img.dimensions();

    // Prepare image for QR code scanning
    let mut prepared_img = PreparedImage::prepare(gray_img);

    // Find and decode QR codes
    let grids = prepared_img.detect_grids();
    if grids.is_empty() {
        return Err("No QR code found in image".to_string());
    }

    for grid in grids {
        if let Ok((_metadata, content)) = grid.decode() {
            // Parse the decoded QR code content
            return parse_qr_code_data(content);
        }
    }

    Err("Failed to decode QR code from image".to_string())
}

#[tauri::command]
pub fn parse_qr_code_data(qr_data: String) -> Result<QrCodeData, String> {
    // Expected format: "cipher://add-friend?username=alice&public_key=abc123..."
    if !qr_data.starts_with("cipher://add-friend?") {
        return Err("Invalid QR code format".to_string());
    }

    let query_part = qr_data.strip_prefix("cipher://add-friend?").unwrap();
    let mut username = None;
    let mut public_key = None;

    for param in query_part.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "username" => {
                    username = Some(
                        urlencoding::decode(value)
                            .map_err(|_| "Invalid username encoding")?
                            .to_string(),
                    )
                }
                "public_key" => {
                    public_key = Some(
                        urlencoding::decode(value)
                            .map_err(|_| "Invalid public key encoding")?
                            .to_string(),
                    )
                }
                _ => {} // Ignore unknown parameters
            }
        }
    }

    let username = username.ok_or("Missing username in QR code")?;
    let public_key = public_key.ok_or("Missing public key in QR code")?;

    Ok(QrCodeData {
        username,
        public_key,
    })
}

/// Generate a friend QR code for a user
/// This creates the cipher://add-friend URL and generates a QR code image
#[tauri::command]
pub async fn generate_friend_qr_code(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    // Get user info
    let user = db
        .find_user_by_id(user_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "User not found".to_string())?;

    let username = user.username;
    let public_key = user
        .public_key
        .ok_or_else(|| "User has no public key".to_string())?;

    // Create the cipher://add-friend URL with URL-encoded parameters
    let encoded_username = urlencoding::encode(&username);
    let encoded_public_key = urlencoding::encode(&public_key);
    let friend_url = format!(
        "cipher://add-friend?username={}&public_key={}",
        encoded_username, encoded_public_key
    );

    // Generate QR code from the URL
    generate_qr_code(friend_url)
}


#[tauri::command]
pub async fn upload_media_file(
    file_data: String, // base64 encoded file data
    _filename: String, // Not stored for privacy
    file_type: String,
    _file_size: i64, // Not stored
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<MediaAttachment, String> {
    // Decode base64 data
    let file_bytes = general_purpose::STANDARD
        .decode(&file_data)
        .map_err(|e| format!("Failed to decode base64 data: {}", e))?;

    // Save attachment with BLOB data directly to database (privacy-focused: no filename, no filesize, no timestamp)
    let conn = db.conn.lock().unwrap();

    let attachment_id = SqliteUuid::new();
    let file_size = file_bytes.len() as i64;

    conn.execute(
        "INSERT INTO media_attachments (id, post_id, file_type, file_size, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![attachment_id, post_id, &file_type, file_size, &file_bytes],
    ).map_err(|e| format!("Failed to save attachment to database: {}", e))?;

    Ok(MediaAttachment {
        id: attachment_id,
        post_id,
        file_type,
        file_size,
    })
}

#[tauri::command]
pub async fn get_media_attachments(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<MediaAttachmentWithData>, String> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, post_id, file_type, file_size, data FROM media_attachments WHERE post_id = ?1"
    ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let attachment_iter = stmt
        .query_map([post_id], |row| {
            // Handle NULL BLOB data
            let file_bytes_option: Option<Vec<u8>> = row.get(4)?;
            let base64_data = file_bytes_option
                .map(|bytes| general_purpose::STANDARD.encode(&bytes))
                .unwrap_or_else(|| String::from(""));

            Ok(MediaAttachmentWithData {
                id: row.get(0)?,
                post_id: row.get(1)?,
                file_type: row.get(2)?,
                file_size: row.get(3)?,
                data: base64_data,
            })
        })
        .map_err(|e| format!("Failed to query attachments: {}", e))?;

    let mut attachments = Vec::new();
    for attachment in attachment_iter {
        attachments.push(attachment.map_err(|e| format!("Failed to parse attachment: {}", e))?);
    }

    Ok(attachments)
}

#[tauri::command]
pub async fn get_media_file_data(
    media_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<MediaAttachmentWithData, String> {
    let conn = db.conn.lock().unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT id, post_id, file_type, file_size, data
         FROM media_attachments WHERE id = ?1",
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let media = stmt
        .query_row([media_id], |row| {
            let file_bytes: Vec<u8> = row.get(4)?;
            let base64_data = general_purpose::STANDARD.encode(&file_bytes);

            Ok(MediaAttachmentWithData {
                id: row.get(0)?,
                post_id: row.get(1)?,
                file_type: row.get(2)?,
                file_size: row.get(3)?,
                data: base64_data,
            })
        })
        .map_err(|e| format!("Media not found: {}", e))?;

    Ok(media)
}

#[tauri::command]
pub async fn update_user_profile(
    user_id: SqliteUuid,
    bio: Option<String>,
    profile_picture: Option<String>,
    db: State<'_, Database>,
) -> Result<User, String> {
    db.update_user_profile(user_id, bio, profile_picture)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upload_profile_picture(
    user_id: SqliteUuid,
    file_data: String, // base64 encoded file data
    filename: String,
    _file_type: String,
    db: State<'_, Database>,
) -> Result<User, String> {
    // Create uploads directory if it doesn't exist
    let uploads_dir = Path::new("uploads/profiles");
    if !uploads_dir.exists() {
        fs::create_dir_all(uploads_dir)
            .map_err(|e| format!("Failed to create uploads directory: {}", e))?;
    }

    // Generate unique filename
    let extension = Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    let unique_filename = format!("profile_{}_{}.{}", user_id, Uuid::new_v4(), extension);
    let file_path = uploads_dir.join(&unique_filename);

    // Decode base64 and write file
    let file_bytes = general_purpose::STANDARD
        .decode(&file_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    let mut file =
        fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&file_bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    // Update user profile with new picture path
    let profile_picture_path = file_path.to_string_lossy().to_string();
    db.update_user_profile(user_id, None, Some(profile_picture_path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn debug_log(message: String) {
    println!("Frontend Debug: {}", message);
}

// Message Reactions and Threading Functions
#[tauri::command]
pub async fn add_message_reaction(
    message_id: SqliteUuid,
    user_id: SqliteUuid,
    emoji: String,
    db: State<'_, Database>,
) -> Result<MessageReaction, String> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Check if reaction already exists (toggle behavior)
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM message_reactions WHERE message_id = ?1 AND user_id = ?2 AND emoji = ?3",
            params![message_id, user_id, emoji],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(existing_id) = existing {
        // Remove existing reaction (toggle off)
        conn.execute(
            "DELETE FROM message_reactions WHERE id = ?1",
            params![existing_id],
        )
        .map_err(|e| e.to_string())?;

        return Err("Reaction removed".to_string());
    }

    // Add new reaction
    let reaction_id = SqliteUuid::new();

    conn.execute(
        "INSERT INTO message_reactions (id, message_id, user_id, emoji, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![reaction_id, message_id, user_id, emoji, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(MessageReaction {
        id: reaction_id,
        message_id,
        user_id,
        emoji,
        created_at: now,
    })
}

#[tauri::command]
pub async fn get_message_reactions(
    message_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<MessageReaction>, String> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, message_id, user_id, emoji, created_at FROM message_reactions WHERE message_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let reaction_iter = stmt
        .query_map(params![message_id], |row| {
            Ok(MessageReaction {
                id: row.get(0)?,
                message_id: row.get(1)?,
                user_id: row.get(2)?,
                emoji: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut reactions = Vec::new();
    for reaction in reaction_iter {
        reactions.push(reaction.map_err(|e| e.to_string())?);
    }

    Ok(reactions)
}

#[tauri::command]
pub async fn reply_to_message(
    sender_id: SqliteUuid,
    recipient_id: SqliteUuid,
    content: String,
    thread_id: SqliteUuid, // ID of the message being replied to
    db: State<'_, Database>,
) -> Result<Message, String> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let message_id = SqliteUuid::new();

    conn.execute(
        "INSERT INTO messages (id, sender_id, recipient_id, content, encrypted, thread_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![message_id, sender_id, recipient_id, content, true, thread_id, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(Message {
        id: message_id,
        sender_id,
        recipient_id,
        content,
        encrypted: true,
        signature: None,
        thread_id: Some(thread_id),
        disappear_after_seconds: None,
        disappears_at: None,
        created_at: now.clone(),
        updated_at: now,
        edited_at: None,
    })
}

#[tauri::command]
pub async fn get_message_thread(
    thread_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Message>, String> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at, edited_at
                  FROM messages WHERE (id = ?1 OR thread_id = ?1) AND (disappears_at IS NULL OR disappears_at > datetime('now')) ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let message_iter = stmt
        .query_map(params![thread_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                recipient_id: row.get(2)?,
                content: row.get(3)?,
                encrypted: row.get(4)?,
                signature: row.get(5)?,
                thread_id: row.get(6)?,
                disappear_after_seconds: row.get(7)?,
                disappears_at: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                edited_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut messages = Vec::new();
    for message in message_iter {
        messages.push(message.map_err(|e| e.to_string())?);
    }

    Ok(messages)
}

// Voice message commands
#[tauri::command]
pub async fn send_voice_message(
    sender_id: SqliteUuid,
    recipient_id: SqliteUuid,
    audio_data: String,
    duration_seconds: f64,
    waveform: Option<String>,
    thread_id: Option<SqliteUuid>,
    db: State<'_, Database>,
) -> Result<VoiceMessage, String> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    let voice_message_id = SqliteUuid::new();

    let result = conn.execute(
        "INSERT INTO voice_messages (id, sender_id, recipient_id, audio_data, duration_seconds, waveform, encrypted, thread_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![voice_message_id, sender_id, recipient_id, audio_data, duration_seconds, waveform, true, thread_id, now],
    );

    match result {
        Ok(_) => Ok(VoiceMessage {
            id: voice_message_id,
            sender_id,
            recipient_id,
            audio_data,
            duration_seconds,
            waveform,
            encrypted: true,
            thread_id,
            created_at: now,
        }),
        Err(e) => Err(format!("Failed to send voice message: {}", e)),
    }
}

#[tauri::command]
pub async fn get_voice_messages(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<VoiceMessage>, String> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, sender_id, recipient_id, audio_data, duration_seconds, waveform, encrypted, thread_id, created_at
                  FROM voice_messages WHERE sender_id = ?1 OR recipient_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;

    let voice_message_iter = stmt
        .query_map(params![user_id], |row| {
            Ok(VoiceMessage {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                recipient_id: row.get(2)?,
                audio_data: row.get(3)?,
                duration_seconds: row.get(4)?,
                waveform: row.get(5)?,
                encrypted: row.get(6)?,
                thread_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut voice_messages = Vec::new();
    for voice_message in voice_message_iter {
        voice_messages.push(voice_message.map_err(|e| e.to_string())?);
    }

    Ok(voice_messages)
}

#[tauri::command]
pub async fn delete_voice_message(
    voice_message_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    let conn = db.conn.lock().unwrap();

    // Check if user owns this voice message
    let mut stmt = conn
        .prepare("SELECT sender_id FROM voice_messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let sender_id: SqliteUuid = stmt
        .query_row(params![voice_message_id], |row| row.get(0))
        .map_err(|_| "Voice message not found".to_string())?;

    if sender_id != user_id {
        return Err("You can only delete your own voice messages".to_string());
    }

    conn.execute(
        "DELETE FROM voice_messages WHERE id = ?1",
        params![voice_message_id],
    )
    .map_err(|e| e.to_string())?;

    Ok("Voice message deleted successfully".to_string())
}

// Enhanced Friend Management Commands for P2P Networks

#[tauri::command]
pub async fn create_friend_invite(
    user_id: SqliteUuid,
    uses: i32,
    hours_valid: i32,
    db: State<'_, Database>,
) -> Result<FriendInvite, String> {
    // Use the database method instead of duplicating logic
    db.create_friend_invite(user_id, uses, hours_valid as i64)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn use_friend_invite(
    user_id: SqliteUuid,
    invite_code: String,
    db: State<'_, Database>,
) -> Result<User, String> {
    // Use the database method instead of duplicating logic
    db.use_friend_invite(user_id, invite_code)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_friends_list(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<FriendExport>, String> {
    let conn = db.conn.lock().unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT u.username, u.public_key, u.bio, p.created_at
                  FROM users u
                  JOIN p2p_connections p ON u.id = p.friend_user_id
                  WHERE p.user_id = ?1 AND p.status = 'accepted'
                  ORDER BY u.username",
        )
        .map_err(|e| e.to_string())?;

    let friends_iter = stmt
        .query_map(params![user_id], |row| {
            Ok(FriendExport {
                username: row.get(0)?,
                public_key: row.get(1)?,
                display_name: Some(row.get::<_, String>(0)?),
                bio: row.get(2)?,
                added_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut friends = Vec::new();
    for friend in friends_iter {
        friends.push(friend.map_err(|e| e.to_string())?);
    }

    Ok(friends)
}

#[tauri::command]
pub async fn import_friends_list(
    user_id: SqliteUuid,
    friends_json: String,
    db: State<'_, Database>,
) -> Result<FriendImportResult, String> {
    let friends: Vec<FriendExport> =
        serde_json::from_str(&friends_json).map_err(|_| "Invalid JSON format".to_string())?;

    let mut result = FriendImportResult {
        added: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
    };

    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    for friend in friends {
        // Check if user exists by public key
        let user_check = conn
            .prepare("SELECT id, username FROM users WHERE public_key = ?1")
            .unwrap()
            .query_row(params![friend.public_key], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            });

        match user_check {
            Ok((friend_id, existing_username)) => {
                // Check if already friends
                let friend_check = conn
                    .prepare("SELECT COUNT(*) FROM p2p_connections
                              WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)")
                    .unwrap()
                    .query_row(params![user_id, friend_id], |row| row.get::<_, i32>(0))
                    .unwrap_or(0);

                if friend_check > 0 {
                    result
                        .skipped
                        .push(format!("{} (already friends)", existing_username));
                } else {
                    // Add friendship
                    let insert_result = conn.execute(
                        "INSERT INTO p2p_connections (user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                         VALUES (?1, ?2, 'accepted', ?1, ?3, ?4)",
                        params![user_id, friend_id, now, now],
                    );

                    if insert_result.is_ok() {
                        // Add reverse connection
                        let _ = conn.execute(
                            "INSERT INTO p2p_connections (user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                             VALUES (?1, ?2, 'accepted', ?2, ?3, ?4)",
                            params![friend_id, user_id, now, now],
                        );
                        result.added.push(existing_username);
                    } else {
                        result
                            .errors
                            .push(format!("{} (database error)", existing_username));
                    }
                }
            }
            Err(_) => {
                result.skipped.push(format!(
                    "{} (user not found on this instance)",
                    friend.username
                ));
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_recent_contacts(
    user_id: SqliteUuid,
    limit: Option<i32>,
    db: State<'_, Database>,
) -> Result<Vec<RecentContact>, String> {
    let conn = db.conn.lock().unwrap();
    let limit = limit.unwrap_or(10);

    let mut stmt = conn
        .prepare("SELECT rc.contact_user_id, u.username, u.public_key, rc.last_interaction, rc.interaction_count
                  FROM recent_contacts rc
                  JOIN users u ON rc.contact_user_id = u.id
                  WHERE rc.user_id = ?1
                  ORDER BY rc.last_interaction DESC
                  LIMIT ?2")
        .map_err(|e| e.to_string())?;

    let contacts_iter = stmt
        .query_map(params![user_id, limit], |row| {
            Ok(RecentContact {
                user_id: row.get(0)?,
                username: row.get(1)?,
                public_key: row.get(2)?,
                last_interaction: row.get(3)?,
                interaction_count: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut contacts = Vec::new();
    for contact in contacts_iter {
        contacts.push(contact.map_err(|e| e.to_string())?);
    }

    Ok(contacts)
}

#[tauri::command]
pub async fn get_websocket_port() -> Result<u16, String> {
    Ok(8081) // Fixed port for now, could be configurable
}

#[tauri::command]
pub async fn start_notification_server(db: State<'_, Database>) -> Result<String, String> {
    let server = db.notification_server.clone();

    tokio::spawn(async move {
        if let Err(e) = server.start(8081).await {
            eprintln!("Failed to start notification server: {}", e);
        }
    });

    Ok("WebSocket notification server started on port 8081".to_string())
}

#[tauri::command]
pub async fn update_recent_contact(
    user_id: SqliteUuid,
    contact_user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    // Try to update existing contact
    let updated_rows = conn
        .execute(
            "UPDATE recent_contacts
         SET last_interaction = ?1, interaction_count = interaction_count + 1
         WHERE user_id = ?2 AND contact_user_id = ?3",
            params![now, user_id, contact_user_id],
        )
        .map_err(|e| e.to_string())?;

    // If no rows updated, insert new record
    if updated_rows == 0 {
        conn.execute(
            "INSERT INTO recent_contacts (user_id, contact_user_id, last_interaction, interaction_count)
             VALUES (?1, ?2, ?3, 1)",
            params![user_id, contact_user_id, now],
        ).map_err(|e| e.to_string())?;
    }

    Ok("Contact updated successfully".to_string())
}

// Message Search and Editing Functions
#[tauri::command]
pub async fn search_messages(
    user_id: SqliteUuid,
    query: String,
    db: State<'_, Database>,
) -> Result<Vec<Message>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let conn = db.conn.lock().unwrap();

    // Search in decrypted messages - first get all messages for the user
    let mut stmt = conn.prepare(
        "SELECT m.id, m.sender_id, m.recipient_id, m.content, m.encrypted, m.signature, m.thread_id, m.disappear_after_seconds, m.disappears_at, m.created_at, m.updated_at, m.edited_at
         FROM messages m
         WHERE (m.sender_id = ?1 OR m.recipient_id = ?1) AND (m.disappears_at IS NULL OR m.disappears_at > datetime('now'))
         ORDER BY m.created_at DESC"
    ).map_err(|e| e.to_string())?;

    let messages: Vec<Message> = stmt
        .query_map([user_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                recipient_id: row.get(2)?,
                content: row.get(3)?,
                encrypted: row.get::<_, i64>(4)? == 1,
                signature: row.get(5)?,
                thread_id: row.get(6)?,
                disappear_after_seconds: row.get(7)?,
                disappears_at: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                edited_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Filter messages by searching in decrypted content
    let mut matching_messages = Vec::new();
    let query_lower = query.to_lowercase();

    for message in messages {
        let search_content = if message.encrypted {
            // Decrypt the message to search its content
            // Get sender's public key and recipient's private key
            let sender_public_key: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT public_key FROM users WHERE id = ?1",
                [message.sender_id],
                |row| row.get(0),
            );

            let recipient_private_key: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT encryption_private_key FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            );

            if let (Ok(sender_pub), Ok(recipient_priv)) = (sender_public_key, recipient_private_key)
            {
                if let Ok(decrypted) =
                    Database::decrypt_message(&message.content, &sender_pub, &recipient_priv)
                {
                    decrypted.to_lowercase()
                } else {
                    continue; // Skip if can't decrypt
                }
            } else {
                continue; // Skip if can't get keys
            }
        } else {
            message.content.to_lowercase()
        };

        if search_content.contains(&query_lower) {
            matching_messages.push(message);
        }
    }

    Ok(matching_messages)
}

#[tauri::command]
pub async fn edit_message(
    message_id: SqliteUuid,
    user_id: SqliteUuid,
    new_content: String,
    db: State<'_, Database>,
) -> Result<Message, String> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    // Verify the user owns this message
    let message_owner: SqliteUuid = conn
        .query_row(
            "SELECT sender_id FROM messages WHERE id = ?1",
            [message_id],
            |row| row.get(0),
        )
        .map_err(|_| "Message not found".to_string())?;

    if message_owner != user_id {
        return Err("You can only edit your own messages".to_string());
    }

    // Get the message details to check if it's encrypted
    let mut stmt = conn
        .prepare("SELECT recipient_id, encrypted FROM messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let (recipient_id, is_encrypted): (SqliteUuid, i64) = stmt
        .query_row([message_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    let final_content = if is_encrypted == 1 {
        // Re-encrypt the new content
        let sender_keys: (String, String) = conn
            .query_row(
                "SELECT private_key, encryption_private_key FROM users WHERE id = ?1",
                [user_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;

        let recipient_public_key: String = conn
            .query_row(
                "SELECT encryption_public_key FROM users WHERE id = ?1",
                [recipient_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        Database::encrypt_message(&new_content, &recipient_public_key, &sender_keys.1)
            .map_err(|e| format!("Failed to encrypt message: {}", e))?
    } else {
        new_content.clone()
    };

    // Generate new signature for the original content
    let new_signature = if is_encrypted == 1 {
        let sender_private_key: String = conn
            .query_row(
                "SELECT private_key FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        Some(
            Database::sign_message(&new_content, &sender_private_key)
                .map_err(|e| format!("Failed to sign message: {}", e))?,
        )
    } else {
        None
    };

    // Update the message
    conn.execute(
        "UPDATE messages SET content = ?1, signature = ?2, updated_at = ?3 WHERE id = ?4",
        params![final_content, new_signature, now, message_id],
    )
    .map_err(|e| e.to_string())?;

    // Return updated message
    Ok(Message {
        id: message_id,
        sender_id: user_id,
        recipient_id,
        content: final_content,
        encrypted: is_encrypted == 1,
        signature: new_signature,
        thread_id: None, // We'll get this from DB if needed
        disappear_after_seconds: None,
        disappears_at: None,
        created_at: "".to_string(), // We'll get this from DB if needed
        updated_at: now.clone(),
        edited_at: Some(now),
    })
}

#[tauri::command]
pub async fn delete_message(
    message_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    let conn = db.conn.lock().unwrap();

    // Verify the user owns this message
    let message_owner: SqliteUuid = conn
        .query_row(
            "SELECT sender_id FROM messages WHERE id = ?1",
            [message_id],
            |row| row.get(0),
        )
        .map_err(|_| "Message not found".to_string())?;

    if message_owner != user_id {
        return Err("You can only delete your own messages".to_string());
    }

    // Delete the message
    conn.execute("DELETE FROM messages WHERE id = ?1", [message_id])
        .map_err(|e| e.to_string())?;

    // Also delete any reactions to this message
    conn.execute(
        "DELETE FROM message_reactions WHERE message_id = ?1",
        [message_id],
    )
    .map_err(|e| e.to_string())?;

    Ok("Message deleted successfully".to_string())
}


// Cleanup expired messages
#[tauri::command]
pub async fn cleanup_expired_messages(db: State<'_, Database>) -> Result<usize, String> {
    db.cleanup_expired_messages().map_err(|e| e.to_string())
}

// Post Reactions Commands
#[tauri::command]
pub async fn add_post_reaction(
    post_id: SqliteUuid,
    user_id: SqliteUuid,
    emoji: String,
    db: State<'_, Database>,
) -> Result<PostReaction, String> {
    db.add_post_reaction(post_id, user_id, &emoji)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_post_reaction(
    post_id: SqliteUuid,
    user_id: SqliteUuid,
    emoji: String,
    db: State<'_, Database>,
) -> Result<String, String> {
    db.remove_post_reaction(post_id, user_id, &emoji)
        .map_err(|e| e.to_string())?;
    Ok("Reaction removed successfully".to_string())
}

#[tauri::command]
pub async fn get_post_reactions(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<PostReaction>, String> {
    db.get_post_reactions(post_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_post_reaction_summary(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<(String, i32)>, String> {
    db.get_post_reaction_summary(post_id)
        .map_err(|e| e.to_string())
}

// Post Comments Commands
#[tauri::command]
pub async fn add_post_comment(
    post_id: SqliteUuid,
    user_id: SqliteUuid,
    content: String,
    parent_comment_id: Option<SqliteUuid>,
    db: State<'_, Database>,
) -> Result<PostComment, String> {
    db.add_post_comment(post_id, user_id, &content, parent_comment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_post_comments(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<PostComment>, String> {
    db.get_post_comments(post_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_comment_replies(
    parent_comment_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<PostComment>, String> {
    db.get_comment_replies(parent_comment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_post_comment(
    comment_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    db.delete_post_comment(comment_id, user_id)
        .map_err(|e| e.to_string())?;
    Ok("Comment deleted successfully".to_string())
}

#[tauri::command]
pub async fn get_post_comment_count(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<i32, String> {
    db.get_post_comment_count(post_id)
        .map_err(|e| e.to_string())
}

// Post Sharing Commands
#[tauri::command]
pub async fn share_post(
    user_id: SqliteUuid,
    original_post_id: SqliteUuid,
    share_comment: Option<String>,
    db: State<'_, Database>,
) -> Result<Post, String> {
    db.share_post(user_id, original_post_id, share_comment)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_shared_post(
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Option<Post>, String> {
    db.get_shared_post(post_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_post_shares(
    original_post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Post>, String> {
    db.get_post_shares(original_post_id)
        .map_err(|e| e.to_string())
}

// Post editing and deletion commands
#[tauri::command]
pub async fn edit_post(
    post_id: SqliteUuid,
    user_id: SqliteUuid,
    new_content: String,
    db: State<'_, Database>,
) -> Result<Post, String> {
    db.edit_post(post_id, user_id, &new_content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_post(
    post_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    db.delete_post(post_id, user_id)
        .map_err(|e| e.to_string())?;
    Ok("Post deleted successfully".to_string())
}

// Notification commands
#[tauri::command]
pub async fn create_notification(
    user_id: SqliteUuid,
    notification_type: String,
    title: String,
    message: String,
    data: Option<String>,
    db: State<'_, Database>,
) -> Result<Notification, String> {
    db.create_notification(user_id, &notification_type, &title, &message, data)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_notifications(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Notification>, String> {
    db.get_notifications(user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_unread_notifications(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Notification>, String> {
    db.get_unread_notifications(user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_notification_read(
    notification_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.mark_notification_read(notification_id, user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_all_notifications_read(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.mark_all_notifications_read(user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_notification(
    notification_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.delete_notification(notification_id, user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_unread_notification_count(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<i32, String> {
    db.get_unread_notification_count(user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_old_notifications(days: i32, db: State<'_, Database>) -> Result<(), String> {
    db.cleanup_old_notifications(days)
        .map_err(|e| e.to_string())
}

// Block/Mute commands
#[tauri::command]
pub async fn block_user(
    blocker_id: SqliteUuid,
    blocked_id: SqliteUuid,
    reason: Option<String>,
    db: State<'_, Database>,
) -> Result<BlockedUser, String> {
    db.block_user(blocker_id, blocked_id, reason)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unblock_user(
    blocker_id: SqliteUuid,
    blocked_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.unblock_user(blocker_id, blocked_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_user_blocked(
    blocker_id: SqliteUuid,
    blocked_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.is_user_blocked(blocker_id, blocked_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_blocked_users(
    blocker_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<BlockedUser>, String> {
    db.get_blocked_users(blocker_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_blocked_either_way(
    user1_id: SqliteUuid,
    user2_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.is_blocked_either_way(user1_id, user2_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mute_user(
    muter_id: SqliteUuid,
    muted_id: SqliteUuid,
    mute_notifications: bool,
    mute_messages: bool,
    mute_posts: bool,
    expires_at: Option<String>,
    db: State<'_, Database>,
) -> Result<MutedUser, String> {
    db.mute_user(
        muter_id,
        muted_id,
        mute_notifications,
        mute_messages,
        mute_posts,
        expires_at,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unmute_user(
    muter_id: SqliteUuid,
    muted_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.unmute_user(muter_id, muted_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_user_muted(
    muter_id: SqliteUuid,
    muted_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.is_user_muted(muter_id, muted_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_mute_settings(
    muter_id: SqliteUuid,
    muted_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Option<MutedUser>, String> {
    db.get_mute_settings(muter_id, muted_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_muted_users(
    muter_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<MutedUser>, String> {
    db.get_muted_users(muter_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_expired_mutes(db: State<'_, Database>) -> Result<(), String> {
    db.cleanup_expired_mutes().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_mute_settings(
    muter_id: SqliteUuid,
    muted_id: SqliteUuid,
    mute_notifications: bool,
    mute_messages: bool,
    mute_posts: bool,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.update_mute_settings(
        muter_id,
        muted_id,
        mute_notifications,
        mute_messages,
        mute_posts,
    )
    .map_err(|e| e.to_string())
}

// Device Management Commands

#[tauri::command]
pub async fn get_user_devices(
    user_public_key: String,
    db: State<'_, Database>,
) -> Result<Vec<Device>, String> {
    db.get_user_devices(&user_public_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_device(
    device_id: String,
    db: State<'_, Database>,
) -> Result<Option<Device>, String> {
    db.get_device(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_device_name(
    device_id: String,
    device_name: String,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.update_device_name(&device_id, &device_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_device(
    device_id: String,
    user_public_key: String,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.remove_device(&device_id, &user_public_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_device_info_list(
    user_public_key: String,
    current_device_id: String,
    db: State<'_, Database>,
) -> Result<Vec<DeviceInfo>, String> {
    db.get_device_info_list(&user_public_key, &current_device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_device_sync(device_id: String, db: State<'_, Database>) -> Result<(), String> {
    db.update_device_sync(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn verify_device_ownership(
    device_id: String,
    user_public_key: String,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.verify_device_ownership(&device_id, &user_public_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_device_count(
    user_public_key: String,
    db: State<'_, Database>,
) -> Result<usize, String> {
    db.get_device_count(&user_public_key)
        .map_err(|e| e.to_string())
}

// Device-to-Device Sync Commands

#[tauri::command]
pub async fn get_sync_data(
    device_id: String,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<crate::app::database::sync::SyncData, String> {
    db.get_sync_data(&device_id, user_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_sync_data(
    sync_data: String, // JSON string
    db: State<'_, Database>,
) -> Result<(), String> {
    let data: crate::app::database::sync::SyncData =
        serde_json::from_str(&sync_data).map_err(|e| format!("Invalid sync data: {}", e))?;
    db.apply_sync_data(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_all_sync_timestamps(
    device_id: String,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.update_all_sync_timestamps(&device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sync_status(
    device_id: String,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(usize, usize, usize), String> {
    db.get_sync_status(&device_id, user_id)
        .map_err(|e| e.to_string())
}

