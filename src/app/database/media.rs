use crate::app::types::{MediaAttachmentWithData, SqliteUuid};
use crate::app::Database;
use base64::{engine::general_purpose, Engine as _};
use rusqlite::{params, Result as SqliteResult};

impl Database {
    /// Upload media file to database
    /// user_id: The user uploading the media
    /// post_id: Post to attach media to (required - media must belong to a post)
    /// filename: Original filename (not stored for privacy)
    /// mime_type: MIME type (e.g., "image/png")
    /// data: Binary data of the file
    /// file_size: Size of the file in bytes
    #[allow(dead_code)]
    pub fn upload_media(
        &self,
        _user_id: SqliteUuid,
        post_id: SqliteUuid,
        _filename: &str,
        mime_type: &str,
        data: &[u8],
        file_size: i64,
    ) -> SqliteResult<MediaAttachmentWithData> {
        let conn = self.conn.lock().unwrap();
        let media_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO media_attachments (id, post_id, file_type, file_size, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![media_id, post_id, mime_type, file_size, data],
        )?;

        // Encode binary data as Base64 for return struct
        let base64_data = general_purpose::STANDARD.encode(data);

        Ok(MediaAttachmentWithData {
            id: media_id,
            post_id,
            file_type: mime_type.to_string(),
            file_size,
            data: base64_data,
        })
    }

    /// Get all media attachments for a post
    #[allow(dead_code)]
    pub fn get_post_media(
        &self,
        post_id: SqliteUuid,
    ) -> SqliteResult<Vec<MediaAttachmentWithData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, post_id, file_type, file_size, data
             FROM media_attachments WHERE post_id = ?1",
        )?;

        let media_iter = stmt.query_map([post_id], |row| {
            let binary_data: Vec<u8> = row.get("data")?;
            let base64_data = general_purpose::STANDARD.encode(&binary_data);

            Ok(MediaAttachmentWithData {
                id: row.get("id")?,
                post_id: row.get("post_id")?,
                file_type: row.get("file_type")?,
                file_size: row.get("file_size")?,
                data: base64_data,
            })
        })?;

        let mut media = Vec::new();
        for item in media_iter {
            media.push(item?);
        }
        Ok(media)
    }

    /// Get a specific media file by ID
    #[allow(dead_code)]
    pub fn get_media_file(&self, media_id: SqliteUuid) -> SqliteResult<MediaAttachmentWithData> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, post_id, file_type, file_size, data
             FROM media_attachments WHERE id = ?1",
            [media_id],
            |row| {
                let binary_data: Vec<u8> = row.get("data")?;
                let base64_data = general_purpose::STANDARD.encode(&binary_data);

                Ok(MediaAttachmentWithData {
                    id: row.get("id")?,
                    post_id: row.get("post_id")?,
                    file_type: row.get("file_type")?,
                    file_size: row.get("file_size")?,
                    data: base64_data,
                })
            },
        )
    }
}
