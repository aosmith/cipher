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

// Community commands
pub mod community_commands;

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
    _app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<UserWithRecoveryPhrase, String> {
    println!("Creating new user with display name: {}", display_name);

    let device_id = db.get_device_id()?;
    println!("Using device ID: {}", device_id);

    match db.create_user_first_launch(display_name, device_id) {
        Ok((user, recovery_phrase)) => {
            println!(
                "User created successfully: {} with device_id: {:?}",
                user.display_name, user.device_id
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
    _app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<User, String> {
    println!("Restoring user with display name: {}", display_name);

    let device_id = db.get_device_id()?;
    println!("Using device ID: {}", device_id);

    match db.restore_user_from_recovery_phrase(display_name, recovery_phrase, device_id) {
        Ok(user) => {
            println!(
                "User restored successfully: {} with device_id: {:?}",
                user.display_name, user.device_id
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
    println!(
        "[CREATE_POST] Command called with user_id: {}, content length: {}, has_attachments: {}",
        user_id,
        content.len(),
        attachments.is_some()
    );

    println!("[CREATE_POST] About to call db.create_post()...");

    // Create the post locally in the database
    let post = db.create_post(user_id, &content, false).map_err(|e| {
        println!("[CREATE_POST] Error creating post: {}", e);
        e.to_string()
    })?;

    println!(
        "[CREATE_POST] Post created successfully with id: {}",
        post.id
    );

    // Automatically broadcast the post to the P2P network
    // Access the global Iroh network instance
    use crate::app::iroh_commands::IROH_NETWORK;

    // Clone the network Arc before any await points to avoid holding the lock
    let network_opt = IROH_NETWORK.lock().unwrap().as_ref().cloned();

    // NOTE: Posts are NOT auto-broadcast here. The frontend should call iroh_publish_post
    // separately to broadcast via encrypted sealed envelopes. We NEVER send unencrypted
    // posts over the network.
    if network_opt.is_some() {
        println!(
            "[POST-BROADCAST] Post {} created - frontend should call iroh_publish_post to broadcast encrypted",
            post.id
        );
    } else {
        println!(
            "[POST-BROADCAST] Post {} created - Iroh network not initialized, saved locally only",
            post.id
        );
    }

    Ok(post)
}

/// Create a post with a specific ID (for syncing posts from other devices)
#[tauri::command]
pub async fn create_post_with_id(
    post_id: String,
    user_id: SqliteUuid,
    content: String,
    _attachments: Option<Vec<String>>,
    db: State<'_, Database>,
) -> Result<Post, String> {
    println!(
        "[CREATE_POST_WITH_ID] Command called with post_id: {}, user_id: {}",
        post_id, user_id
    );

    // Parse the post_id
    let parsed_post_id =
        SqliteUuid::parse_str(&post_id).map_err(|e| format!("Invalid post_id: {}", e))?;

    // Create the post with the specific ID
    let post = db
        .create_post_with_id(parsed_post_id, user_id, &content, false)
        .map_err(|e| {
            println!("[CREATE_POST_WITH_ID] Error creating post: {}", e);
            e.to_string()
        })?;

    println!(
        "[CREATE_POST_WITH_ID] Post created/updated with id: {}",
        post.id
    );
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
    println!(
        "[ACCEPT-CMD] accept_friend_request called: user={} friend={}",
        user_id, friend_user_id
    );

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
        // Get our encryption key and friend's public key - use spawn_blocking for DB calls
        let db_clone2 = db.inner().clone();
        let our_user_id = user_id;
        let (friend_opt, friend_enc_key) = tokio::task::spawn_blocking(move || {
            let friend = db_clone2.find_user_by_id(friend_user_id).ok().flatten();
            // The requester's encryption key was stored when their sealed
            // FriendRequest arrived - the acceptance is sealed back to it
            let enc_key = db_clone2
                .get_user_encryption_public_key(friend_user_id)
                .ok()
                .flatten();
            (friend, enc_key)
        })
        .await
        .unwrap_or((None, None));
        let _ = our_user_id; // friendship row lookups above already used it

        if let Some(friend) = friend_opt {
            if let Some(friend_public_key) = friend.public_key {
                match friend_enc_key {
                    Some(friend_enc_key) => {
                        // Spawn P2P notification in background - don't block the accept
                        tokio::spawn(async move {
                            if let Err(e) =
                                network.send_friend_accepted_sealed(&friend_enc_key).await
                            {
                                println!(
                                    "[FRIEND-ACCEPT] Warning: Failed to send FriendAccepted: {}",
                                    e
                                );
                            } else {
                                println!(
                                    "[FRIEND-ACCEPT] Sent sealed FriendAccepted to {}",
                                    friend_public_key
                                );
                            }
                        });
                    }
                    None => println!(
                        "[FRIEND-ACCEPT] No encryption key stored for {} - cannot notify; \
                         the presence-triggered resend will deliver once their key is known",
                        friend_public_key
                    ),
                }
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
    tokio::task::spawn_blocking(move || db_clone.reject_friend_request(user_id, friend_user_id))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Search for friends by display name
/// This only searches within existing friendships for security
#[tauri::command]
pub async fn search_friends(
    user_id: SqliteUuid,
    display_name: String,
    db: State<'_, Database>,
) -> Result<Option<User>, String> {
    db.find_friend_by_display_name(user_id, &display_name)
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
    tokio::task::spawn_blocking(move || db_clone.cancel_friend_request(user_id, friend_user_id))
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
    // Expected format: "cipher://add-friend?display_name=alice&public_key=abc123..."
    // Also supports legacy format with "username=" for backwards compatibility
    if !qr_data.starts_with("cipher://add-friend?") {
        return Err("Invalid QR code format".to_string());
    }

    let query_part = qr_data.strip_prefix("cipher://add-friend?").unwrap();
    let mut display_name = None;
    let mut public_key = None;

    for param in query_part.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "display_name" | "username" => {
                    // Support both new "display_name" and legacy "username" parameter
                    display_name = Some(
                        urlencoding::decode(value)
                            .map_err(|_| "Invalid display_name encoding")?
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

    let display_name = display_name.ok_or("Missing display_name in QR code")?;
    let public_key = public_key.ok_or("Missing public key in QR code")?;

    Ok(QrCodeData {
        display_name,
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

    let display_name = user.display_name;
    let public_key = user
        .public_key
        .ok_or_else(|| "User has no public key".to_string())?;

    // Create the cipher://add-friend URL with URL-encoded parameters
    let encoded_display_name = urlencoding::encode(&display_name);
    let encoded_public_key = urlencoding::encode(&public_key);
    let friend_url = format!(
        "cipher://add-friend?display_name={}&public_key={}",
        encoded_display_name, encoded_public_key
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
    println!(
        "[UPLOAD-MEDIA] Uploading media for post_id: {}, type: {}, data_len: {}",
        post_id,
        file_type,
        file_data.len()
    );

    // Decode base64 data
    let file_bytes = general_purpose::STANDARD
        .decode(&file_data)
        .map_err(|e| format!("Failed to decode base64 data: {}", e))?;

    println!("[UPLOAD-MEDIA] Decoded {} bytes", file_bytes.len());

    // Save attachment with BLOB data directly to database (privacy-focused: no filename, no filesize, no timestamp)
    let conn = db.conn.lock().unwrap();

    let attachment_id = SqliteUuid::new();
    let file_size = file_bytes.len() as i64;

    conn.execute(
        "INSERT INTO media_attachments (id, post_id, file_type, file_size, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![attachment_id, post_id, &file_type, file_size, &file_bytes],
    ).map_err(|e| format!("Failed to save attachment to database: {}", e))?;

    println!(
        "[UPLOAD-MEDIA] ✓ Saved attachment {} for post {}",
        attachment_id, post_id
    );

    Ok(MediaAttachment {
        id: attachment_id,
        post_id,
        file_type,
        file_size,
    })
}

/// Save media file to device's downloads/pictures folder
#[tauri::command]
pub async fn save_media_to_downloads(
    base64_data: String,
    filename: String,
    _mime_type: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Decode base64 data
    let file_bytes = general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|e| format!("Failed to decode base64 data: {}", e))?;

    // Determine the save directory based on platform
    #[cfg(target_os = "android")]
    let save_dir = {
        // On Android, save to the app's external files directory which is accessible
        // Use Pictures subdirectory for images
        let base_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;

        // Try to use a more accessible location
        let pictures_dir = base_dir.join("Pictures");
        fs::create_dir_all(&pictures_dir)
            .map_err(|e| format!("Failed to create Pictures directory: {}", e))?;
        pictures_dir
    };

    #[cfg(not(target_os = "android"))]
    let save_dir = {
        // On desktop, use the downloads directory
        let download_dir = app_handle
            .path()
            .download_dir()
            .map_err(|e| format!("Failed to get downloads dir: {}", e))?;

        // Create a Cipher subdirectory
        let cipher_dir = download_dir.join("Cipher");
        fs::create_dir_all(&cipher_dir)
            .map_err(|e| format!("Failed to create Cipher directory: {}", e))?;
        cipher_dir
    };

    // Sanitize filename
    let safe_filename = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    let file_path = save_dir.join(&safe_filename);

    // Write the file
    let mut file =
        fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&file_bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    println!(
        "[SAVE_MEDIA] Saved {} ({} bytes) to {:?}",
        safe_filename,
        file_bytes.len(),
        file_path
    );

    Ok(file_path.to_string_lossy().to_string())
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
    display_name: Option<String>,
    bio: Option<String>,
    profile_picture: Option<String>,
    db: State<'_, Database>,
) -> Result<User, String> {
    db.update_user_profile(user_id, display_name, bio, profile_picture)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upload_profile_picture(
    user_id: SqliteUuid,
    file_data: String, // base64 encoded file data
    filename: String,
    _file_type: String,
    db: State<'_, Database>,
    app_handle: tauri::AppHandle,
) -> Result<User, String> {
    // Must be an absolute path in the app data dir: the process cwd is "/" for
    // a macOS .app bundle, so the old cwd-relative "uploads/profiles" could
    // never be created.
    let uploads_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("uploads")
        .join("profiles");
    if !uploads_dir.exists() {
        fs::create_dir_all(&uploads_dir)
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
    db.update_user_profile(user_id, None, None, Some(profile_picture_path))
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
    // Toggle behaviour: an identical reaction that already exists is removed.
    // The id column is a 16-byte BLOB - reading it as i64 made `.optional()`
    // see a type error instead of QueryReturnedNoRows, so the toggle-off path
    // always failed.
    let existing: Option<SqliteUuid> = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM message_reactions WHERE message_id = ?1 AND user_id = ?2 AND emoji = ?3",
            params![message_id, user_id, &emoji],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    }; // drop the lock before calling into Database (its methods lock too)

    if existing.is_some() {
        db.remove_message_reaction(message_id, user_id, &emoji)
            .map_err(|e| e.to_string())?;
        return Err("Reaction removed".to_string());
    }

    db.add_message_reaction(message_id, user_id, &emoji)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_message_reactions(
    message_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<MessageReaction>, String> {
    db.get_message_reactions(message_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reply_to_message(
    sender_id: SqliteUuid,
    recipient_id: SqliteUuid,
    content: String,
    thread_id: SqliteUuid, // ID of the message being replied to
    db: State<'_, Database>,
) -> Result<Message, String> {
    // Delegate to the encrypting/signing implementation. The previous inline
    // INSERT stored the raw plaintext while flagging it encrypted = true.
    db.reply_to_message(sender_id, recipient_id, &content, thread_id, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_message_thread(
    thread_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Message>, String> {
    // The inline version compared disappears_at (RFC3339) against
    // datetime('now') (space-separated), which never matches lexicographically.
    db.get_message_thread(thread_id).map_err(|e| e.to_string())
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
            "SELECT u.display_name, u.public_key, u.bio, p.created_at
                  FROM users u
                  JOIN p2p_connections p ON u.id = p.friend_user_id
                  WHERE p.user_id = ?1 AND p.status = 'accepted'
                  ORDER BY u.display_name",
        )
        .map_err(|e| e.to_string())?;

    let friends_iter = stmt
        .query_map(params![user_id], |row| {
            Ok(FriendExport {
                display_name: row.get(0)?,
                public_key: row.get(1)?,
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
            .prepare("SELECT id, display_name FROM users WHERE public_key = ?1")
            .unwrap()
            // users.id is a 16-byte BLOB; reading it as i64 made every lookup
            // fail with a FromSql type error, so import reported "user not
            // found" for every friend.
            .query_row(params![friend.public_key], |row| {
                Ok((row.get::<_, SqliteUuid>(0)?, row.get::<_, String>(1)?))
            });

        match user_check {
            Ok((friend_id, existing_display_name)) => {
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
                        .push(format!("{} (already friends)", existing_display_name));
                } else {
                    // Add friendship. id is the BLOB primary key - omitting it
                    // inserted NULL primary keys.
                    let insert_result = conn.execute(
                        "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'accepted', ?2, ?4, ?5)",
                        params![SqliteUuid::new(), user_id, friend_id, now, now],
                    );

                    if insert_result.is_ok() {
                        // Add reverse connection
                        let _ = conn.execute(
                            "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                             VALUES (?1, ?2, ?3, 'accepted', ?3, ?4, ?5)",
                            params![SqliteUuid::new(), friend_id, user_id, now, now],
                        );
                        result.added.push(existing_display_name);
                    } else {
                        result
                            .errors
                            .push(format!("{} (database error)", existing_display_name));
                    }
                }
            }
            Err(_) => {
                result.skipped.push(format!(
                    "{} (user not found on this instance)",
                    friend.display_name
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
        .prepare("SELECT rc.contact_user_id, u.display_name, u.public_key, rc.last_interaction, rc.interaction_count
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
                display_name: row.get(1)?,
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

#[tauri::command]
pub async fn edit_message(
    message_id: SqliteUuid,
    user_id: SqliteUuid,
    new_content: String,
    db: State<'_, Database>,
) -> Result<Message, String> {
    // Delegate to the single re-encrypting/re-signing implementation. The
    // inline version fabricated created_at: "" and dropped thread_id; the
    // shared one returns the row as stored.
    db.edit_message(message_id, user_id, &new_content)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Message not found".to_string(),
            rusqlite::Error::InvalidQuery => "You can only edit your own messages".to_string(),
            other => other.to_string(),
        })
}

#[tauri::command]
pub async fn delete_message(
    message_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    // Sender OR recipient may delete: this only removes the row from THIS
    // device's database (there is no P2P retraction), and a recipient must be
    // able to remove a message they received. Reactions go with it via the
    // ON DELETE CASCADE that is now actually enforced.
    db.delete_message(message_id, user_id)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Message not found".to_string(),
            rusqlite::Error::InvalidQuery => {
                "You can only delete messages you sent or received".to_string()
            }
            other => other.to_string(),
        })?;

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
    // Device sync is stateless now: responders are gated on the requester's
    // watermark, not per-device sync_state cursors. This command returns the
    // full dataset; device_id is kept for JS-API compatibility.
    let _ = device_id;
    db.get_sync_data(user_id, "1970-01-01T00:00:00+00:00")
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
) -> Result<(usize, usize, usize, usize, usize), String> {
    // Stateless sync: counts everything syncable; device_id kept for JS-API
    // compatibility.
    let _ = device_id;
    db.get_sync_status(user_id, "1970-01-01T00:00:00+00:00")
        .map_err(|e| e.to_string())
}

// ============================================================================
// App Settings Commands
// ============================================================================

#[tauri::command]
pub async fn get_app_settings(
    db: State<'_, Database>,
) -> Result<crate::app::database::settings::AppSettings, String> {
    db.get_app_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_storage_limit(limit_bytes: i64, db: State<'_, Database>) -> Result<(), String> {
    db.set_storage_limit(limit_bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn can_store_data(bytes: i64, db: State<'_, Database>) -> Result<bool, String> {
    db.can_store(bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_storage_used(bytes: i64, db: State<'_, Database>) -> Result<i64, String> {
    db.add_storage_used(bytes).map_err(|e| e.to_string())
}
