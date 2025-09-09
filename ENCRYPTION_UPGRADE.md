# One-to-Many Encryption System Upgrade

The Cipher application has been upgraded from a one-to-one encryption scheme to a sophisticated one-to-many encryption system that allows users to share files securely with selected friends.

## Architecture Overview

### Previous System (One-to-One)
- Files encrypted with a single key
- Key encrypted with owner's public key only
- Only the file owner could decrypt and view files

### New System (One-to-Many)
- Files encrypted with a symmetric key (AES-GCM)
- Symmetric key encrypted separately for each authorized user using their public key
- Multiple users can decrypt and access the same file
- Fine-grained access control through friendship system

## Key Components

### 1. Friendship Management (`Friendship` model)
- **Purpose**: Manages user relationships for file sharing
- **Features**: Friend requests, acceptance/decline, blocking
- **Status Types**: `pending`, `accepted`, `declined`, `blocked`
- **API Endpoints**: Send requests, respond to requests, manage friendships

### 2. Attachment Sharing (`AttachmentShare` model)
- **Purpose**: Stores encrypted keys for each user who has access to a file
- **Data**: Links attachments to users with their encrypted decryption keys
- **Access Control**: Server-side verification of user access rights

### 3. Group Encryption (`GroupEncryption` class)
- **Purpose**: Client-side encryption/decryption for multiple recipients
- **Key Features**:
  - Generate symmetric keys for file encryption
  - Encrypt symmetric keys for multiple recipients
  - Decrypt files using recipient's private key
  - Share existing files with additional users

### 4. Friend Selector UI (`FriendSelector` class)
- **Purpose**: Interactive UI for selecting friends during file uploads
- **Features**:
  - Search and filter friends
  - Multiple selection with limits
  - Visual feedback and validation
  - Responsive design

### 5. Friend Management Interface
- **Purpose**: Complete friendship management system
- **Features**:
  - Send/accept/decline friend requests
  - View friends list
  - Remove friendships
  - Real-time status updates

## Security Features

### Encryption Flow
1. **File Upload with Sharing**:
   - User selects file and friends to share with
   - System generates random AES-256 key
   - File encrypted with AES-GCM (authenticated encryption)
   - AES key encrypted with each recipient's public key using NaCl Box
   - Encrypted file and per-user keys stored separately

2. **File Decryption**:
   - User requests file access
   - System verifies user has permission (AttachmentShare exists)
   - Returns user's encrypted key and encrypted file data
   - Client decrypts key using private key, then decrypts file

### Security Properties
- **Forward Secrecy**: Each file has unique encryption key
- **Access Control**: Server enforces permissions before serving encrypted keys
- **Key Separation**: File encryption keys never stored in plaintext
- **Authenticated Encryption**: AES-GCM prevents tampering
- **Zero-Knowledge**: Server never sees private keys or plaintext data

## API Endpoints

### Friend Management
- `GET /api/v1/friends` - Get user's friends list
- `POST /api/v1/friends/send_request` - Send friend request
- `POST /api/v1/friends/respond_to_request` - Accept/decline requests
- `DELETE /api/v1/friends/:id` - Remove friendship
- `GET /api/v1/friends/requests` - Get pending requests

### User Lookup
- `POST /api/v1/users/by_public_key` - Find user by public key

### File Sharing (Enhanced)
- Existing attachment endpoints enhanced with sharing support
- Client-side encryption handles multiple recipients
- Server validates access permissions

## Database Schema Changes

### New Tables
```sql
-- Friendships between users
CREATE TABLE friendships (
  id bigint PRIMARY KEY,
  requester_id bigint NOT NULL, -- Foreign key to users
  addressee_id bigint NOT NULL, -- Foreign key to users  
  status string DEFAULT 'pending',
  created_at timestamp,
  updated_at timestamp
);

-- Per-user encrypted keys for shared files
CREATE TABLE attachment_shares (
  id bigint PRIMARY KEY,
  attachment_id bigint NOT NULL, -- Foreign key to attachments
  user_id bigint NOT NULL,       -- Foreign key to users
  encrypted_key text NOT NULL,   -- User's encrypted decryption key
  created_at timestamp,
  updated_at timestamp
);
```

### Enhanced Models
- **User**: Added friendship associations and helper methods
- **Attachment**: Modified encryption to support multiple recipients
- **Friendship**: Manages user relationships with state machine
- **AttachmentShare**: Links users to accessible files

## UI Improvements

### Navigation
- New "Friends" link in main navigation
- Access to friend management and sharing features

### Friend Management Page (`/users/friends`)
- Add friends by username
- View pending requests (sent and received)
- Accept/decline incoming requests
- Manage existing friendships

### File Upload (Enhanced)
- Friend selector modal during uploads
- Search and select multiple friends
- Visual confirmation of sharing choices
- Cost calculation including sharing overhead

### File Viewing (Enhanced)
- Shows which friends have access to files
- Option to share existing files with additional friends
- Access control indicators

## Usage Flow

### Setting Up Friendships
1. User visits Friends page
2. Searches for friends by username
3. Sends friend requests
4. Recipients accept/decline requests
5. Both users can now share files

### Sharing a New File
1. User uploads file
2. Friend selector appears
3. User selects friends to share with
4. File encrypted for all selected recipients
5. Blockchain payment (if enabled) covers sharing costs

### Accessing Shared Files
1. User views shared file
2. System checks permissions
3. If authorized, provides encrypted key
4. Client decrypts and displays file

### Sharing Existing Files
1. User views owned file
2. Selects "Share with more friends"
3. Friend selector appears
4. System encrypts existing key for new recipients
5. New users gain access

## Performance Considerations

### Client-Side
- Encryption/decryption happens in Web Workers (future enhancement)
- Symmetric encryption for large files (AES-GCM)
- Asymmetric encryption only for key exchange (NaCl Box)

### Server-Side
- Efficient queries with proper indexing
- Permission checks before serving encrypted data
- Friendship caching for frequent lookups

### Storage
- Each shared file requires one AttachmentShare per recipient
- Encrypted keys are ~200 bytes each
- Scales linearly with sharing breadth

## Security Audit Points

### Threat Model
- ✅ Malicious server cannot read file contents
- ✅ Unauthorized users cannot access shared files  
- ✅ Revoked friends lose access to new shares
- ⚠️ Existing shared files need manual revocation (future: key rotation)

### Cryptographic Choices
- ✅ AES-256-GCM for file encryption (fast, authenticated)
- ✅ NaCl Box for key exchange (battle-tested, secure)
- ✅ Secure random key generation (crypto.getRandomValues)
- ✅ Proper IV handling (unique per encryption)

### Implementation Security
- ✅ Private keys never leave client
- ✅ Server validates permissions before serving keys
- ✅ SQL injection prevention (parameterized queries)
- ✅ CSRF protection on state-changing operations

## Future Enhancements

### Key Rotation
- Implement periodic key rotation for long-lived shares
- Allow revocation of access to previously shared files

### Group Management
- Create named groups for easier bulk sharing
- Group-based permissions and inheritance

### Performance Optimization  
- Web Worker encryption for large files
- Streaming encryption/decryption
- Progressive loading of shared files

### Enhanced Access Control
- Time-based access expiration
- View-only vs download permissions
- Access audit logs

## Migration Guide

### For Existing Files
- Files uploaded before upgrade remain owner-only accessible
- Re-upload or use "Share with friends" to enable group access
- No automatic migration to preserve security guarantees

### For Developers
- Import new JavaScript modules: `group_encryption.js`, `friend_selector.js`
- Update file upload flows to include friend selection
- Implement permission checks in file serving logic
- Test friendship workflows and encryption/decryption

## Testing

### Unit Tests
- Friendship model state transitions
- Attachment sharing permissions  
- Encryption/decryption roundtrips
- API endpoint security

### Integration Tests
- End-to-end file sharing workflow
- Friend request/acceptance flow
- Permission enforcement
- UI component interactions

### Security Tests
- Unauthorized access attempts
- Malformed encryption data handling
- Permission boundary testing
- Client-side key protection

---

The one-to-many encryption system transforms Cipher from a personal encrypted storage solution into a collaborative secure file sharing platform while maintaining zero-knowledge security guarantees.