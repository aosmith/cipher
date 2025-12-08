# Global Mesh Encryption Implementation Plan

## Overview

Transform Cipher from friend-only connections to a global gossip mesh where:
- All nodes connect to each other
- All content is encrypted and gossiped to everyone
- Only friends can decrypt your content
- Keys rotate for forward secrecy
- Nodes purge non-relevant data based on configurable policies

## Architecture

### Message Flow

```
Alice posts "Hello" (has friends: Bob, Carol)
    │
    ▼
┌─────────────────────────────────────────────────┐
│ Create GossipEnvelope:                          │
│   message_id: random_32_bytes                   │
│   timestamp: now()                              │
│   sealed_boxes: [                               │
│     SealedBox(for_bob, encrypt(content, bob_key))   │
│     SealedBox(for_carol, encrypt(content, carol_key))│
│   ]                                             │
└─────────────────────────────────────────────────┘
    │
    ▼ Broadcast to cipher/content/v1
    │
    ├──────────────────┬──────────────────┐
    ▼                  ▼                  ▼
  [Bob]             [Carol]            [Dave]
  Decrypt ✓         Decrypt ✓          Can't decrypt
  Store locally     Store locally      Relay & purge later
```

### Encryption Scheme: X25519 + ChaCha20-Poly1305 with Ratcheting

```
Friend Establishment (QR code exchange):
┌─────────────────────────────────────────────────────────┐
│ 1. Alice generates invite:                              │
│    - Identity key (Ed25519, long-term)                  │
│    - Signed pre-key (X25519, rotates monthly)           │
│    - One-time pre-key (X25519, single use)              │
│                                                         │
│ 2. Bob scans QR, performs X3DH:                         │
│    - Bob's identity key × Alice's signed pre-key        │
│    - Bob's ephemeral × Alice's identity key             │
│    - Bob's ephemeral × Alice's signed pre-key           │
│    - Bob's ephemeral × Alice's one-time pre-key         │
│    = Root Key (32 bytes)                                │
│                                                         │
│ 3. Both derive initial chain keys from root key         │
└─────────────────────────────────────────────────────────┘

Per-Message Encryption (Double Ratchet):
┌─────────────────────────────────────────────────────────┐
│ Sending:                                                │
│ 1. If received new DH from peer, perform DH ratchet     │
│ 2. Derive message key from chain key                    │
│ 3. Encrypt content with message key (ChaCha20-Poly1305) │
│ 4. Advance chain key (one-way: SHA256)                  │
│ 5. Include our current DH public in header              │
│                                                         │
│ Receiving:                                              │
│ 1. If new DH public, perform DH ratchet                 │
│ 2. Derive message key from chain key                    │
│ 3. Decrypt content                                      │
│ 4. Advance chain key                                    │
│ 5. Delete old message key (forward secrecy)             │
└─────────────────────────────────────────────────────────┘
```

## Data Structures

### Rust Types

```rust
// src/app/crypto/sealed_box.rs

/// Envelope for gossiped content - can contain multiple sealed boxes
#[derive(Serialize, Deserialize, Clone)]
pub struct GossipEnvelope {
    /// Random ID for deduplication
    pub message_id: [u8; 32],
    /// Unix timestamp (for purging)
    pub timestamp: i64,
    /// Type hint (post, dm, friend_request, etc.)
    pub content_type: ContentType,
    /// One sealed box per intended recipient
    pub sealed_boxes: Vec<SealedBox>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ContentType {
    Post,
    DirectMessage,
    FriendRequest,
    FriendAccepted,
    KeyRotation,
}

/// A sealed box that only one recipient can open
#[derive(Serialize, Deserialize, Clone)]
pub struct SealedBox {
    /// Ephemeral X25519 public key (for ECDH)
    pub ephemeral_pubkey: [u8; 32],
    /// First 8 bytes of recipient's public key (for quick filtering)
    pub recipient_hint: [u8; 8],
    /// Nonce for ChaCha20-Poly1305
    pub nonce: [u8; 24],
    /// Encrypted payload: { sender_pubkey, content }
    pub ciphertext: Vec<u8>,
}

/// Decrypted content from a sealed box
#[derive(Serialize, Deserialize, Clone)]
pub struct SealedContent {
    /// Sender's public key (verified after decryption)
    pub sender_pubkey: [u8; 32],
    /// The actual content
    pub payload: ContentPayload,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ContentPayload {
    Post {
        content: String,
        attachments: Option<Vec<MediaAttachment>>,
    },
    DirectMessage {
        content: String,
        thread_id: Option<Uuid>,
    },
    FriendRequest {
        username: String,
        message: Option<String>,
    },
    FriendAccepted {
        username: String,
    },
    KeyRotation {
        new_signed_prekey: [u8; 32],
        signature: [u8; 64],
    },
}
```

### Ratchet State

```rust
// src/app/crypto/ratchet.rs

/// Double Ratchet state for a friend relationship
#[derive(Serialize, Deserialize)]
pub struct RatchetState {
    /// Friend's user ID
    pub friend_user_id: Uuid,
    /// Current root key (32 bytes)
    pub root_key: [u8; 32],
    /// Our sending chain key
    pub chain_key_send: Option<[u8; 32]>,
    /// Their sending chain key (our receiving)
    pub chain_key_recv: Option<[u8; 32]>,
    /// Our current DH key pair
    pub dh_keypair: Option<X25519KeyPair>,
    /// Their current DH public key
    pub dh_public_recv: Option<[u8; 32]>,
    /// Message counters
    pub msg_num_send: u32,
    pub msg_num_recv: u32,
    /// Skipped message keys (for out-of-order delivery)
    pub skipped_keys: HashMap<(u32, [u8; 32]), [u8; 32]>,
}

impl RatchetState {
    /// Create initial state after X3DH key exchange
    pub fn new(friend_user_id: Uuid, root_key: [u8; 32]) -> Self { ... }

    /// Encrypt a message, advancing the ratchet
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 32]), Error> { ... }

    /// Decrypt a message, advancing the ratchet
    pub fn decrypt(&mut self, ciphertext: &[u8], their_dh: &[u8; 32]) -> Result<Vec<u8>, Error> { ... }

    /// Perform DH ratchet step
    fn dh_ratchet(&mut self, their_dh: &[u8; 32]) { ... }

    /// Derive next chain key and message key
    fn kdf_chain(chain_key: &[u8; 32]) -> ([u8; 32], [u8; 32]) { ... }
}
```

## Database Schema

```sql
-- Migration: Add ratchet and relay tables

-- Ratchet state per friend (encrypted at rest with device key)
CREATE TABLE IF NOT EXISTS ratchet_state (
    id BLOB PRIMARY KEY,
    friend_user_id BLOB NOT NULL UNIQUE,
    state_encrypted BLOB NOT NULL,      -- Encrypted RatchetState JSON
    state_nonce BLOB NOT NULL,          -- Nonce for decryption
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (friend_user_id) REFERENCES users (id)
);

-- Relay cache for gossiped messages
CREATE TABLE IF NOT EXISTS relay_cache (
    message_id BLOB PRIMARY KEY,
    envelope BLOB NOT NULL,             -- Serialized GossipEnvelope
    received_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    relay_count INTEGER DEFAULT 0,
    size_bytes INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relay_cache_expires ON relay_cache(expires_at);
CREATE INDEX IF NOT EXISTS idx_relay_cache_received ON relay_cache(received_at);

-- Purge settings (user configurable)
CREATE TABLE IF NOT EXISTS purge_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Single row
    relay_max_age_hours INTEGER DEFAULT 1,
    relay_max_size_mb INTEGER DEFAULT 100,
    own_content_max_days INTEGER DEFAULT 30,
    friend_content_max_days INTEGER DEFAULT 7
);

-- Pre-keys for X3DH (one-time keys)
CREATE TABLE IF NOT EXISTS prekeys (
    id BLOB PRIMARY KEY,
    public_key BLOB NOT NULL,
    private_key_encrypted BLOB NOT NULL,
    used INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

-- Signed pre-key (rotates monthly)
CREATE TABLE IF NOT EXISTS signed_prekey (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Single row, current key
    public_key BLOB NOT NULL,
    private_key_encrypted BLOB NOT NULL,
    signature BLOB NOT NULL,
    created_at TEXT NOT NULL
);
```

## Implementation Phases

### Phase 1: Global Mesh Topology (Day 1-2)

**Goal**: All nodes connect via single content topic, remove per-user topics.

**Files to modify**:
- `src/app/iroh_network.rs` - Simplify to single `cipher/content/v1` topic
- `src/app/iroh_commands.rs` - Remove friend-specific topic subscriptions
- `src/js/p2p.js` - Update status to show mesh peers

**Changes**:
1. Remove `cipher/user/{pubkey}` topic subscriptions
2. Add `cipher/content/v1` as the single content topic
3. Keep `cipher/presence` for node discovery/health
4. Update `iroh_get_connection_status` to return mesh peer count

**Acceptance criteria**:
- [ ] All nodes join same content topic
- [ ] P2P status shows "online (N)" where N is total mesh peers
- [ ] Messages broadcast to all peers (still plaintext for now)

### Phase 2: Sealed Box Encryption (Day 3-5)

**Goal**: Implement sealed box encryption so only recipients can decrypt.

**New files**:
- `src/app/crypto/mod.rs` - Crypto module
- `src/app/crypto/sealed_box.rs` - SealedBox implementation
- `src/app/crypto/x25519.rs` - X25519 key operations

**Changes**:
1. Implement `GossipEnvelope` and `SealedBox` types
2. When creating a post:
   - Get list of friends
   - Create one `SealedBox` per friend using their public key
   - Broadcast `GossipEnvelope` to content topic
3. When receiving envelope:
   - Check `recipient_hint` against our pubkey prefix
   - Try to decrypt matching boxes
   - If successful, process content; if not, relay and cache

**Acceptance criteria**:
- [ ] Posts encrypted with one sealed box per friend
- [ ] Only friends can decrypt posts
- [ ] Non-friends relay but can't read content

### Phase 3: Double Ratchet (Day 6-8)

**Goal**: Implement key ratcheting for forward secrecy.

**New files**:
- `src/app/crypto/ratchet.rs` - Double Ratchet implementation
- `src/app/crypto/x3dh.rs` - X3DH key exchange

**Changes**:
1. Implement X3DH for initial key exchange (during QR scan)
2. Implement Double Ratchet for per-message keys
3. Store ratchet state in database (encrypted)
4. Update sealed box creation to use ratcheted keys
5. Handle out-of-order message delivery with skipped keys

**Acceptance criteria**:
- [ ] Initial key exchange via QR creates ratchet state
- [ ] Each message uses a unique key
- [ ] Old keys deleted after use
- [ ] Out-of-order messages still decrypt

### Phase 4: Relay Cache & Purging (Day 9-10)

**Goal**: Implement configurable purging of relay cache.

**Changes**:
1. Add `relay_cache` table
2. Store undecryptable envelopes with TTL
3. Background task purges expired entries
4. Add settings UI for purge configuration
5. Implement size-based purging (when cache exceeds max MB)

**Acceptance criteria**:
- [ ] Undecryptable messages cached temporarily
- [ ] Cache purged based on age (configurable hours)
- [ ] Cache purged based on size (configurable MB)
- [ ] Settings accessible in app

### Phase 5: Key Rotation & Pre-keys (Day 11-12)

**Goal**: Implement signed pre-keys and one-time pre-keys for X3DH.

**Changes**:
1. Generate batch of one-time pre-keys on first launch
2. Include pre-key in QR invite
3. Mark pre-key as used after friend establishment
4. Rotate signed pre-key monthly
5. Broadcast `KeyRotation` message to friends when rotating

**Acceptance criteria**:
- [ ] One-time pre-keys generated and stored
- [ ] QR code includes current pre-key
- [ ] Pre-keys marked used after exchange
- [ ] Signed pre-key rotates automatically

## File Structure

```
src/app/
├── crypto/
│   ├── mod.rs              # Module exports
│   ├── sealed_box.rs       # GossipEnvelope, SealedBox
│   ├── ratchet.rs          # Double Ratchet implementation
│   ├── x3dh.rs             # X3DH key exchange
│   └── x25519.rs           # X25519 operations (wraps existing)
├── database/
│   ├── schema.rs           # Add new tables
│   ├── ratchet.rs          # Ratchet state CRUD
│   ├── relay_cache.rs      # Relay cache CRUD
│   └── prekeys.rs          # Pre-key management
├── iroh_network.rs         # Simplified to global mesh
└── iroh_commands.rs        # Updated for new encryption
```

## Security Considerations

1. **Ratchet state encryption**: Stored encrypted with device-derived key
2. **Pre-key exhaustion**: Generate new batch when running low
3. **Replay protection**: Message IDs tracked to prevent replay
4. **Timing attacks**: Constant-time comparison for recipient hints
5. **Memory safety**: Zero sensitive keys after use

## Testing Strategy

1. **Unit tests**: Ratchet encrypt/decrypt, sealed box operations
2. **Integration tests**: Two-device message exchange
3. **Stress tests**: High message volume, out-of-order delivery
4. **Security tests**: Verify forward secrecy, key deletion

## Rollback Plan

If issues arise:
1. Feature flag: `CIPHER_USE_GLOBAL_MESH=false` reverts to old behavior
2. Database migrations are additive (old tables unchanged)
3. Old message format still parseable during transition
