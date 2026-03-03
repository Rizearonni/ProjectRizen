# Running Project Rizen

Detailed instructions for building, running, and configuring the project.

## Prerequisites

- **Rust**: 1.75+ stable (`rustup update stable`)
- **PostgreSQL**: 14+ for persistence (optional for dev mode)
- **OS**: Windows 10+, Linux, or macOS

## Building

```bash
# Build all crates (debug)
cargo build

# Build release (optimized)
cargo build --release

# Build specific crate
cargo build -p server
cargo build -p client
```

## Running

### Development Mode (No Database)

```powershell
# Terminal 1: Start server
cargo run -p server

# Terminal 2: Start client
cargo run -p client
```

The server runs without a database in dev mode, using in-memory state.

### Production Mode (With Database)

```powershell
# Set database connection
$env:DATABASE_URL = "postgres://postgres:password@localhost:5432/rizen"

# Start server (auto-runs migrations)
cargo run -p server
```

## Server Configuration

The zone server binds to `127.0.0.1:3000` with these characteristics:

| Setting | Value | Description |
|---------|-------|-------------|
| Tick Rate | 20 Hz | Server simulation updates per second |
| Snapshot Rate | 10 Hz | World state broadcasts per second |
| WebSocket Path | `/ws/zone` | Client connection endpoint |
| Protocol | Binary | bincode-serialized messages |

### Server Logs

```
INFO  server > Starting zone server...
INFO  server > Tick rate: 20 Hz, Snapshot rate: 10 Hz
INFO  server > Zone server listening on 127.0.0.1:3000
```

## Client Configuration

The client auto-connects to `ws://127.0.0.1:3000/ws/zone`.

### Controls

| Key | Action |
|-----|--------|
| W/S | Move forward/backward |
| A/D | Strafe left/right |
| Q/E | Rotate left/right |
| Esc | Close window |

### Window Title HUD

Status displayed in window title:
```
Project Rizen — Connected — ents: 1 — tick: 890 — ping: 0ms — seq: 582
```

- **status**: Disconnected / Connecting... / Connected / Error
- **ents**: Entity count in current zone
- **tick**: Server tick number
- **ping**: Round-trip latency (placeholder)
- **seq**: Client input sequence

## Database Setup

### PostgreSQL Installation

**Windows:**
- Download from https://www.postgresql.org/download/windows/
- Or use Chocolatey: `choco install postgresql`

**Linux:**
```bash
sudo apt install postgresql postgresql-contrib  # Debian/Ubuntu
sudo dnf install postgresql-server              # Fedora
```

**macOS:**
```bash
brew install postgresql
brew services start postgresql
```

### Create Database

```bash
# Using psql
psql -U postgres -c "CREATE DATABASE rizen;"

# Using createdb
createdb -U postgres rizen
```

### Connection String Format

```
postgres://[user]:[password]@[host]:[port]/[database]
```

Examples:
```
postgres://postgres:postgres@localhost:5432/rizen       # Local default
postgres://rizen_user:secret@db.example.com:5432/rizen  # Remote
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | None | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | `10` | Pool size |
| `DATABASE_MIN_CONNECTIONS` | `1` | Minimum idle connections |
| `DATABASE_CONNECT_TIMEOUT` | `10` | Connection timeout (seconds) |
| `DATABASE_IDLE_TIMEOUT` | `300` | Idle connection timeout (seconds) |

### Schema Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `accounts` | Player accounts | `account_id`, `username`, `password_hash` |
| `sessions` | Login sessions | `session_token`, `account_id`, `expires_at` |
| `characters` | Player characters | `character_id`, `account_id`, `name`, `level` |
| `character_state` | World position | `character_id`, `zone_id`, `pos_x/y/z`, `yaw` |
| `inventory_slots` | Item storage | `character_id`, `slot_index`, `item_id`, `quantity` |
| `memory_unlocks` | Skill unlocks | `character_id`, `node_id`, `unlocked_at` |

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific crate
cargo test -p common
cargo test -p server
cargo test -p worldgen
cargo test -p data
cargo test -p persistence

# Run specific test
cargo test -p worldgen heightmap
```

### Test Summary

| Crate | Tests | Coverage |
|-------|-------|----------|
| `common` | 8 | IDs, protocol serialization, math |
| `server` | 3 | World tick, spawn/despawn |
| `worldgen` | 25 | Noise, chunks, features, integration |
| `data` | 17 | TOML parsing, validation, registry |
| `persistence` | 11 | Config, models, error types |
| **Total** | **65** | |

## Logging

Control log output with `RUST_LOG`:

```powershell
# Show all debug logs
$env:RUST_LOG = "debug"

# Server only
$env:RUST_LOG = "server=debug"

# Multiple filters
$env:RUST_LOG = "server=debug,persistence=info"

# Quiet mode
$env:RUST_LOG = "warn"
```

## Troubleshooting

### Port 3000 Already in Use

```powershell
# Find and kill process
Get-Process -Id (Get-NetTCPConnection -LocalPort 3000).OwningProcess | Stop-Process
```

### Database Connection Failed

1. Verify PostgreSQL is running:
   ```powershell
   Get-Service postgresql*
   ```

2. Check connection string:
   ```powershell
   psql $env:DATABASE_URL -c "SELECT 1;"
   ```

3. Verify database exists:
   ```powershell
   psql -U postgres -c "\l" | Select-String rizen
   ```

### Client Can't Connect

1. Ensure server is running first
2. Check firewall allows localhost:3000
3. Look for error in client window title

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build

# Update dependencies
cargo update
```
