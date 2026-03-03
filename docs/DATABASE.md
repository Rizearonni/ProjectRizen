# Database Reference

Complete PostgreSQL database documentation for Project Rizen.

## Overview

The persistence layer uses PostgreSQL via [sqlx](https://github.com/launchbadge/sqlx) for:
- Player accounts and authentication
- Session management
- Character storage (max 5 per account)
- Character world state (position, zone)
- Inventory persistence
- Skill tree (memory) unlocks

## Quick Setup

### 1. Install PostgreSQL

**Windows:**
```powershell
# Via Chocolatey
choco install postgresql

# Or download installer from:
# https://www.postgresql.org/download/windows/
```

**Linux:**
```bash
# Debian/Ubuntu
sudo apt install postgresql postgresql-contrib

# Start service
sudo systemctl start postgresql
```

**macOS:**
```bash
brew install postgresql
brew services start postgresql
```

### 2. Create Database

```bash
# As postgres user
psql -U postgres -c "CREATE DATABASE rizen;"

# Optional: Create dedicated user
psql -U postgres -c "CREATE USER rizen_user WITH PASSWORD 'secure_password';"
psql -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE rizen TO rizen_user;"
```

### 3. Configure Connection

```powershell
# Windows PowerShell
$env:DATABASE_URL = "postgres://postgres:password@localhost:5432/rizen"

# Persist across sessions
[Environment]::SetEnvironmentVariable("DATABASE_URL", "postgres://postgres:password@localhost:5432/rizen", "User")
```

```bash
# Linux/macOS
export DATABASE_URL="postgres://postgres:password@localhost:5432/rizen"
```

### 4. Run Migrations

Migrations run automatically on server start, or manually:
```bash
psql -U postgres -d rizen -f crates/persistence/migrations/001_initial_schema.sql
```

---

## Schema

### accounts

Player login credentials.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `account_id` | UUID | PRIMARY KEY | Unique account identifier |
| `username` | VARCHAR(64) | NOT NULL, UNIQUE | Login username |
| `password_hash` | VARCHAR(256) | NOT NULL | bcrypt/argon2 hash |
| `created_at` | TIMESTAMPTZ | NOT NULL | Account creation time |

**Indexes:**
- `idx_accounts_username` on `LOWER(username)` — case-insensitive lookup

```sql
CREATE TABLE accounts (
    account_id UUID PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(256) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

### sessions

Active login sessions with expiration.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `session_token` | UUID | PRIMARY KEY | Unique session token |
| `account_id` | UUID | FK → accounts | Owner account |
| `expires_at` | TIMESTAMPTZ | NOT NULL | Expiration time (24hr default) |
| `created_at` | TIMESTAMPTZ | NOT NULL | Session creation time |

**Indexes:**
- `idx_sessions_account` on `account_id` — lookup by account  
- `idx_sessions_expires` on `expires_at` — cleanup queries

```sql
CREATE TABLE sessions (
    session_token UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Notes:**
- Sessions cascade delete when account is deleted
- Cleanup expired sessions periodically with `AccountRepo::cleanup_expired_sessions()`

---

### characters

Player characters (max 5 per account).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `character_id` | UUID | PRIMARY KEY | Unique character identifier |
| `account_id` | UUID | FK → accounts | Owner account |
| `name` | VARCHAR(32) | NOT NULL, UNIQUE | Character name |
| `appearance_seed` | BIGINT | NOT NULL | Procedural appearance seed |
| `level` | INTEGER | NOT NULL | Character level (1-100) |
| `memory_fragments` | INTEGER | NOT NULL | Currency (skill points) |
| `created_at` | TIMESTAMPTZ | NOT NULL | Character creation time |

**Indexes:**
- `idx_characters_account` on `account_id` — list characters per account
- `idx_characters_name` on `LOWER(name)` — case-insensitive name uniqueness

```sql
CREATE TABLE characters (
    character_id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    name VARCHAR(32) NOT NULL UNIQUE,
    appearance_seed BIGINT NOT NULL DEFAULT 0,
    level INTEGER NOT NULL DEFAULT 1,
    memory_fragments INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

### character_state

World position (one row per character).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `character_id` | UUID | PRIMARY KEY, FK | Character reference |
| `zone_id` | VARCHAR(64) | NOT NULL | Current zone (e.g., `zone.ossuary`) |
| `pos_x` | REAL | NOT NULL | X position |
| `pos_y` | REAL | NOT NULL | Y position (height) |
| `pos_z` | REAL | NOT NULL | Z position |
| `yaw` | REAL | NOT NULL | Facing angle (radians) |
| `updated_at` | TIMESTAMPTZ | NOT NULL | Last save time |

**Indexes:**
- `idx_character_state_zone` on `zone_id` — query characters in zone

```sql
CREATE TABLE character_state (
    character_id UUID PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    zone_id VARCHAR(64) NOT NULL,
    pos_x REAL NOT NULL DEFAULT 0.0,
    pos_y REAL NOT NULL DEFAULT 0.0,
    pos_z REAL NOT NULL DEFAULT 0.0,
    yaw REAL NOT NULL DEFAULT 0.0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Notes:**
- Uses UPSERT (INSERT ... ON CONFLICT UPDATE) for save operations
- Saved on logout, zone change, and periodic autosave

---

### inventory_slots

Character inventory (40 slots: 0-39).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `character_id` | UUID | FK → characters | Character reference |
| `slot_index` | INTEGER | — | Slot position (0-39) |
| `item_id` | VARCHAR(64) | NOT NULL | Item type ID (e.g., `item.bone_shard`) |
| `quantity` | INTEGER | NOT NULL | Stack count |

**Primary Key:** (`character_id`, `slot_index`)

```sql
CREATE TABLE inventory_slots (
    character_id UUID NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    slot_index INTEGER NOT NULL,
    item_id VARCHAR(64) NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (character_id, slot_index)
);
```

**Notes:**
- Empty slots have no row (not NULL values)
- Operations use UPSERT for add, DELETE for remove

---

### memory_unlocks

Skill tree unlocks.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `character_id` | UUID | FK → characters | Character reference |
| `node_id` | VARCHAR(64) | — | Skill node ID |
| `unlocked_at` | TIMESTAMPTZ | NOT NULL | Unlock time |

**Primary Key:** (`character_id`, `node_id`)

```sql
CREATE TABLE memory_unlocks (
    character_id UUID NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    node_id VARCHAR(64) NOT NULL,
    unlocked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (character_id, node_id)
);
```

---

## Connection Pool Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | Connection string (required for production) |
| `DATABASE_MAX_CONNECTIONS` | 10 | Maximum pool connections |
| `DATABASE_MIN_CONNECTIONS` | 1 | Minimum idle connections |
| `DATABASE_CONNECT_TIMEOUT` | 10 | Connection timeout (seconds) |
| `DATABASE_IDLE_TIMEOUT` | 300 | Idle connection lifetime (seconds) |

### Connection String Format

```
postgres://[user]:[password]@[host]:[port]/[database]?[options]
```

**Examples:**
```
postgres://postgres:postgres@localhost:5432/rizen
postgres://rizen_user:secret@192.168.1.100:5432/rizen
postgres://user:pass@db.example.com:5432/rizen?sslmode=require
```

---

## API Usage

### Account Operations

```rust
use persistence::{create_pool, DatabaseConfig, AccountRepo};

let config = DatabaseConfig::from_env()?;
let pool = create_pool(&config).await?;
let repo = AccountRepo::new(&pool);

// Create account
let account = repo.create(NewAccount {
    username: "player1".into(),
    password_hash: Some(hash),
}).await?;

// Dev mode: auto-create without password
let dev_account = repo.get_or_create_dev("test_user").await?;

// Session management
let session = repo.create_session(account.account_id).await?;
let valid = repo.get_session(session.session_token).await?;
repo.delete_session(session.session_token).await?;
repo.cleanup_expired_sessions().await?;
```

### Character Operations

```rust
use persistence::{CharacterRepo, NewCharacter, CharacterStateUpdate};

let repo = CharacterRepo::new(&pool);

// Create character
let character = repo.create(NewCharacter {
    account_id,
    name: "Hero".into(),
    appearance_seed: 12345,
}).await?;

// List account's characters
let chars = repo.get_by_account(account_id).await?;

// Save position
repo.save_state(character_id, CharacterStateUpdate {
    zone_id: "zone.ossuary".into(),
    pos_x: 10.0,
    pos_y: 5.0,
    pos_z: 20.0,
    yaw: 1.57,
}).await?;

// Inventory
repo.set_inventory_slot(character_id, 0, "item.bone_shard", 5).await?;
let inventory = repo.get_inventory(character_id).await?;

// Memory unlocks
repo.add_memory_unlock(character_id, "memory.dash").await?;
let has_dash = repo.has_memory_unlock(character_id, "memory.dash").await?;
```

---

## Maintenance

### Backup

```bash
pg_dump -U postgres rizen > backup.sql
pg_dump -U postgres -Fc rizen > backup.dump  # Custom format (compressed)
```

### Restore

```bash
psql -U postgres -d rizen < backup.sql
pg_restore -U postgres -d rizen backup.dump
```

### Cleanup Expired Sessions

```sql
DELETE FROM sessions WHERE expires_at < NOW();
```

Or via API:
```rust
account_repo.cleanup_expired_sessions().await?;
```

### Schema Reset (Development Only)

```bash
psql -U postgres -c "DROP DATABASE rizen; CREATE DATABASE rizen;"
# Migrations will recreate tables on next server start
```
