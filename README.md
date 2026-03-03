# Project Rizen

A server-authoritative MMO built in Rust with procedural terrain, zone-based architecture, and PostgreSQL persistence.

## Quick Start

```bash
# Build everything
cargo build

# Run tests (65 total)
cargo test

# Start server (Terminal 1)
cargo run -p server

# Start client (Terminal 2)
cargo run -p client
```

## Architecture

```
┌─────────────┐     WebSocket      ┌─────────────┐
│   Client    │◄──────────────────►│   Server    │
│  (wgpu)     │   Binary Protocol  │  (axum)     │
└─────────────┘                    └──────┬──────┘
       │                                  │
       ▼                                  ▼
┌─────────────┐                    ┌─────────────┐
│  Worldgen   │                    │ Persistence │
│ (terrain)   │                    │ (PostgreSQL)│
└─────────────┘                    └─────────────┘
       │                                  │
       └──────────────┬───────────────────┘
                      ▼
               ┌─────────────┐
               │    Data     │
               │   (TOML)    │
               └─────────────┘
```

## Crates

| Crate | Purpose |
|-------|---------|
| `common` | Protocol messages, IDs, math types, bincode serialization |
| `server` | Zone server with 20Hz tick, WebSocket API on port 3000 |
| `client` | winit+wgpu window, auto-connect, WASD movement |
| `worldgen` | Deterministic heightmaps, chunk streaming, feature placement |
| `data` | TOML loader for zones, mobs, abilities with validation |
| `persistence` | PostgreSQL via sqlx for accounts, characters, inventory |

## Database Setup

### Prerequisites

- PostgreSQL 14+ installed and running
- `psql` or pgAdmin for database creation

### Create Database

**Windows (PowerShell):**
```powershell
# Using psql (assumes postgres user)
psql -U postgres -c "CREATE DATABASE rizen;"

# Or with pgAdmin: Right-click Databases → Create → Database → Name: "rizen"
```

**Linux/macOS:**
```bash
createdb rizen
# or
psql -U postgres -c "CREATE DATABASE rizen;"
```

### Configure Connection

Set the `DATABASE_URL` environment variable:

```powershell
# Windows PowerShell
$env:DATABASE_URL = "postgres://postgres:your_password@localhost:5432/rizen"

# Or set permanently
[Environment]::SetEnvironmentVariable("DATABASE_URL", "postgres://postgres:your_password@localhost:5432/rizen", "User")
```

```bash
# Linux/macOS
export DATABASE_URL="postgres://postgres:your_password@localhost:5432/rizen"
```

### Schema

The persistence crate auto-creates tables on first run. Schema:

```sql
-- accounts: Player login credentials
accounts (account_id UUID, username VARCHAR(64), password_hash, created_at)

-- sessions: Active login sessions (24hr expiry)
sessions (session_token UUID, account_id, expires_at, created_at)

-- characters: Player characters (max 5 per account)
characters (character_id UUID, account_id, name VARCHAR(32), appearance_seed, level, memory_fragments, created_at)

-- character_state: World position (saved on logout/zone change)
character_state (character_id, zone_id, pos_x, pos_y, pos_z, yaw, updated_at)

-- inventory_slots: Item storage (slot 0-39)
inventory_slots (character_id, slot_index, item_id, quantity)

-- memory_unlocks: Skill tree progression
memory_unlocks (character_id, node_id, unlocked_at)
```

### Manual Schema Creation (Optional)

If you prefer to create tables manually:
```bash
psql -U postgres -d rizen -f crates/persistence/migrations/001_initial_schema.sql
```

## Network Protocol

Binary WebSocket messages at `ws://127.0.0.1:3000/ws/zone`:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `ZoneHello` | C→S | Client identifies character |
| `ZoneWelcome` | S→C | Server sends zone info |
| `ZoneEnterWorldOk` | S→C | Spawn confirmed with entity ID |
| `InputMove` | C→S | WASD velocity + rotation |
| `WorldSnapshot` | S→C | All entity positions (10Hz) |

## Content Files

Game data lives in `data/` as TOML files:

```
data/
├── zones/
│   └── ossuary.toml      # Starter zone definition
├── mobs/
│   ├── skeleton_scout.toml
│   └── skeleton_warrior.toml
└── abilities/
    ├── memory_dash.toml
    ├── shadow_bolt.toml
    └── mend_wounds.toml
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `DATABASE_URL` | (dev mode) | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | `10` | Connection pool size |

## Testing

```bash
cargo test                    # All 65 tests
cargo test -p common          # Protocol tests (8)
cargo test -p server          # World simulation (3)
cargo test -p worldgen        # Terrain generation (25)
cargo test -p data            # TOML parsing (17)
cargo test -p persistence     # Database models (11)
```

## Documentation

- [docs/RUN.md](docs/RUN.md) - Detailed build and run instructions
- [docs/SMOKE_TEST.md](docs/SMOKE_TEST.md) - Server verification checklist
- [data/README.md](data/README.md) - Content file format reference

## Current Limitations

- **No egui UI**: Disabled due to egui-wgpu 0.30 lifetime issues; status shown in window title
- **No terrain rendering**: Worldgen outputs data but client doesn't render meshes yet
- **Dev auth only**: `get_or_create_dev()` bypasses password hashing for testing
