use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// Efficient UUID handling for SQLite - stores as 16-byte BLOB instead of 36-char string
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SqliteUuid(Uuid);

impl SqliteUuid {
    pub fn new() -> Self {
        SqliteUuid(Uuid::new_v4())
    }

    pub fn new_v4() -> Self {
        SqliteUuid(Uuid::new_v4())
    }

    /// Create deterministic UUID from public key for multi-device sync
    /// Same public key always produces same UUID across all devices
    /// Uses UUID v5 (name-based SHA-1) with Cipher namespace
    pub fn from_public_key(public_key: &str) -> Self {
        // Cipher namespace (random UUID for our app)
        const CIPHER_NAMESPACE: Uuid = Uuid::from_bytes([
            0x6c, 0x69, 0x70, 0x68, 0x65, 0x72, 0x2d, 0x61, 0x70, 0x70, 0x2d, 0x75, 0x75, 0x69,
            0x64, 0x00,
        ]);

        SqliteUuid(Uuid::new_v5(&CIPHER_NAMESPACE, public_key.as_bytes()))
    }

    pub fn parse_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(SqliteUuid(Uuid::parse_str(s)?))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        SqliteUuid(Uuid::from_bytes(bytes))
    }
}

impl Default for SqliteUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SqliteUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SqliteUuid {
    fn from(uuid: Uuid) -> Self {
        SqliteUuid(uuid)
    }
}

impl From<SqliteUuid> for String {
    fn from(uuid: SqliteUuid) -> Self {
        uuid.to_string()
    }
}

// Store as BLOB (16 bytes) instead of TEXT (36 bytes) - 55% space saving
impl ToSql for SqliteUuid {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Blob(self.as_bytes())))
    }
}

impl FromSql for SqliteUuid {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Blob(bytes) => {
                if bytes.len() != 16 {
                    return Err(FromSqlError::Other(
                        format!("Invalid UUID blob length: expected 16, got {}", bytes.len())
                            .into(),
                    ));
                }
                let mut array = [0u8; 16];
                array.copy_from_slice(bytes);
                Ok(SqliteUuid::from_bytes(array))
            }
            ValueRef::Text(s) => {
                // Fallback for text-stored UUIDs (for migration compatibility)
                let s = std::str::from_utf8(s).map_err(|e| FromSqlError::Other(Box::new(e)))?;
                SqliteUuid::parse_str(s).map_err(|e| FromSqlError::Other(Box::new(e)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
