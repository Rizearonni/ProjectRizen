-- Initial database schema for ProjectRizen
-- Run with: sqlx migrate run

-- Accounts table
CREATE TABLE IF NOT EXISTS accounts (
    account_id UUID PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(256) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_accounts_username ON accounts(LOWER(username));

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    session_token UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sessions_account ON sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

-- Characters table
CREATE TABLE IF NOT EXISTS characters (
    character_id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    name VARCHAR(32) NOT NULL UNIQUE,
    appearance_seed BIGINT NOT NULL DEFAULT 0,
    level INTEGER NOT NULL DEFAULT 1,
    memory_fragments INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_characters_account ON characters(account_id);
CREATE INDEX IF NOT EXISTS idx_characters_name ON characters(LOWER(name));

-- Character state (world position)
CREATE TABLE IF NOT EXISTS character_state (
    character_id UUID PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    zone_id VARCHAR(64) NOT NULL,
    pos_x REAL NOT NULL DEFAULT 0.0,
    pos_y REAL NOT NULL DEFAULT 0.0,
    pos_z REAL NOT NULL DEFAULT 0.0,
    yaw REAL NOT NULL DEFAULT 0.0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_character_state_zone ON character_state(zone_id);

-- Inventory slots
CREATE TABLE IF NOT EXISTS inventory_slots (
    character_id UUID NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    slot_index INTEGER NOT NULL,
    item_id VARCHAR(64) NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (character_id, slot_index)
);

-- Memory tree unlocks
CREATE TABLE IF NOT EXISTS memory_unlocks (
    character_id UUID NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    node_id VARCHAR(64) NOT NULL,
    unlocked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (character_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_memory_unlocks_character ON memory_unlocks(character_id);
