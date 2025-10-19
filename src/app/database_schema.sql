-- Database schema for Cipher token economy
-- Add these tables to support the token system

-- User wallet addresses (actual tokens are on blockchain)
CREATE TABLE IF NOT EXISTS wallet_addresses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    solana_pubkey TEXT NOT NULL, -- Public key on Solana
    usdc_token_account TEXT, -- USDC associated token account
    cipher_token_account TEXT, -- If we create a Cipher SPL token
    last_known_balance_usdc REAL, -- Cached balance (for display only)
    last_balance_check TEXT, -- When we last queried blockchain
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY, -- UUID
    from_user_id INTEGER NOT NULL,
    to_user_id INTEGER,
    tx_type TEXT NOT NULL,
    amount INTEGER NOT NULL,
    gas_fee INTEGER NOT NULL,
    status TEXT DEFAULT 'pending',
    signature TEXT,
    proof_of_work TEXT,
    blockchain_tx_id TEXT,
    nonce INTEGER NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (from_user_id) REFERENCES users(id),
    FOREIGN KEY (to_user_id) REFERENCES users(id)
);

-- Payment channels for micropayments
CREATE TABLE IF NOT EXISTS payment_channels (
    id TEXT PRIMARY KEY,
    party_a_id INTEGER NOT NULL,
    party_b_id INTEGER NOT NULL,
    deposit_a INTEGER NOT NULL,
    deposit_b INTEGER NOT NULL,
    balance_a INTEGER NOT NULL,
    balance_b INTEGER NOT NULL,
    nonce INTEGER DEFAULT 0,
    is_open BOOLEAN DEFAULT 1,
    settlement_tx_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at TEXT,
    FOREIGN KEY (party_a_id) REFERENCES users(id),
    FOREIGN KEY (party_b_id) REFERENCES users(id)
);

-- Airdrop allocations
CREATE TABLE IF NOT EXISTS airdrop_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    wallet_address TEXT NOT NULL,
    total_amount INTEGER NOT NULL,
    claimed_amount INTEGER DEFAULT 0,
    vesting_amount INTEGER DEFAULT 0,
    phase TEXT NOT NULL,
    eligibility_score REAL NOT NULL,
    claimed BOOLEAN DEFAULT 0,
    claim_tx_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    claimed_at TEXT,
    vesting_ends_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Anti-abuse tracking
CREATE TABLE IF NOT EXISTS spam_penalties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    penalty_multiplier REAL NOT NULL,
    violation_count INTEGER DEFAULT 1,
    violation_type TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Economic reputation
CREATE TABLE IF NOT EXISTS economic_reputation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    reputation_score REAL DEFAULT 100.0,
    total_staked INTEGER DEFAULT 0,
    positive_actions INTEGER DEFAULT 0,
    negative_actions INTEGER DEFAULT 0,
    slash_events INTEGER DEFAULT 0,
    last_activity TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Content monetization
CREATE TABLE IF NOT EXISTS content_monetization (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    creator_id INTEGER NOT NULL,
    content_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    price_amount INTEGER NOT NULL,
    total_earnings INTEGER DEFAULT 0,
    purchase_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (creator_id) REFERENCES users(id)
);

-- Content purchases
CREATE TABLE IF NOT EXISTS content_purchases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_id TEXT NOT NULL,
    purchaser_id INTEGER NOT NULL,
    amount_paid INTEGER NOT NULL,
    tx_id TEXT NOT NULL,
    purchased_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (purchaser_id) REFERENCES users(id)
);

-- Atomic swaps
CREATE TABLE IF NOT EXISTS atomic_swaps (
    id TEXT PRIMARY KEY,
    party_a_id INTEGER NOT NULL,
    party_b_id INTEGER NOT NULL,
    amount_a INTEGER NOT NULL,
    amount_b INTEGER NOT NULL,
    hash_lock TEXT NOT NULL,
    preimage TEXT,
    status TEXT DEFAULT 'pending',
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    FOREIGN KEY (party_a_id) REFERENCES users(id),
    FOREIGN KEY (party_b_id) REFERENCES users(id)
);

-- Solana wallet integration
CREATE TABLE IF NOT EXISTS solana_wallets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    pubkey TEXT NOT NULL,
    encrypted_private_key TEXT,
    balance_sol REAL DEFAULT 0,
    balance_usdc REAL DEFAULT 0,
    balance_cipher REAL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Rate limiting tracking
CREATE TABLE IF NOT EXISTS rate_limit_violations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    ip_address TEXT,
    endpoint TEXT NOT NULL,
    violation_count INTEGER DEFAULT 1,
    last_violation TEXT NOT NULL DEFAULT (datetime('now')),
    blocked_until TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_transactions_from_user ON transactions(from_user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_to_user ON transactions(to_user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp);
CREATE INDEX IF NOT EXISTS idx_wallets_user_id ON wallets(user_id);
CREATE INDEX IF NOT EXISTS idx_payment_channels_parties ON payment_channels(party_a_id, party_b_id);
CREATE INDEX IF NOT EXISTS idx_airdrop_claimed ON airdrop_allocations(claimed);
CREATE INDEX IF NOT EXISTS idx_spam_penalties_user ON spam_penalties(user_id);
CREATE INDEX IF NOT EXISTS idx_spam_penalties_expires ON spam_penalties(expires_at);
CREATE INDEX IF NOT EXISTS idx_content_purchases_content ON content_purchases(content_id);
CREATE INDEX IF NOT EXISTS idx_rate_limit_user ON rate_limit_violations(user_id);
CREATE INDEX IF NOT EXISTS idx_rate_limit_ip ON rate_limit_violations(ip_address);

-- Triggers for updated_at timestamps
CREATE TRIGGER IF NOT EXISTS update_wallets_timestamp
    AFTER UPDATE ON wallets
    BEGIN
        UPDATE wallets SET updated_at = datetime('now')
        WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_reputation_timestamp
    AFTER UPDATE ON economic_reputation
    BEGIN
        UPDATE economic_reputation SET updated_at = datetime('now')
        WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_solana_wallets_timestamp
    AFTER UPDATE ON solana_wallets
    BEGIN
        UPDATE solana_wallets SET updated_at = datetime('now')
        WHERE id = NEW.id;
    END;