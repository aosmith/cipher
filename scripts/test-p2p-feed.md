# Manual P2P Feed Testing Instructions

## Problem
Tauri applications share localStorage across all windows, making it impossible to run truly isolated instances simultaneously without building separate binaries.

## Solution: Manual Testing Workflow

### Step 1: Test Alice
1. Launch the app: `cargo run`
2. Create new account with display name "alice"
3. Save the generated 24-word recovery phrase
4. Create a post: "Hello from Alice!"
5. Note Alice's public key for later
6. Take screenshot of feed (should show only Alice's post)
7. Close the application

### Step 2: Test Bob
1. **Clear localStorage**: Open browser dev tools (if in browser) or manually reset
   - For Tauri: Delete `~/Library/Application Support/com.cipher.social/` (macOS)
   - Or: Add logout button that clears localStorage
2. Launch the app again: `cargo run`
3. Create new account with display name "bob"
4. Save the generated 24-word recovery phrase
5. Create a post: "Hello from Bob!"
6. Take screenshot of feed (should show only Bob's post)

### Step 3: Test Friend Connection
1. While logged in as Bob, add Alice as a friend (using her public key)
2. Bob's feed should now show:
   - Bob's own posts
   - Alice's posts (because they're friends)
3. Take screenshot

### Step 4: Verify Alice sees Bob
1. Logout from Bob
2. Login as Alice
3. Add Bob as a friend (using his public key)
4. Alice's feed should now show:
   - Alice's own posts
   - Bob's posts (because they're friends)
5. Take screenshot

## Expected Behavior

### Before Friendship
- Alice sees only her own posts
- Bob sees only his own posts

### After Friendship (Both Directions)
- Alice sees: Alice's posts + Bob's posts
- Bob sees: Bob's posts + Alice's posts

## Technical Details

The feed filtering is implemented in:
- `src/app/database/posts.rs:8-38` - SQL query with friend filtering
- `src/app.rs` - Tauri command accepting user_id
- `src/js/main.js` - Frontend passing current user ID

SQL query filters posts by:
1. Posts created by current user (`p.user_id = ?1`)
2. Posts from friends (bidirectional JOIN on `p2p_connections`)

## Alternative: Build Two Separate Binaries

For true simultaneous testing, you would need to:
1. Build with different app identifiers
2. Create separate `tauri.conf.json` files
3. Build two completely independent apps
