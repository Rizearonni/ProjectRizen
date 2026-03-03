# Smoke Test - Zone Server

This document describes how to verify the zone server is working correctly.

## Prerequisites

- Rust toolchain installed
- A WebSocket test tool (e.g., `websocat`, browser dev tools, or Postman)

## Starting the Server

```bash
cd ProjectRizen
cargo run -p server
```

Expected output:
```
INFO  server > Starting zone server...
INFO  server > Tick rate: 20 Hz, Snapshot rate: 10 Hz
INFO  server > Zone server listening on 127.0.0.1:3000
INFO  server::zone > Tick loop started
```

## Testing with websocat

Install websocat:
```bash
# Windows (via cargo)
cargo install websocat

# Or download from: https://github.com/vi/websocat/releases
```

Connect to the zone server:
```bash
websocat ws://127.0.0.1:3000/ws/zone --binary
```

**Note:** The server expects binary protocol messages, not text. Manual testing with websocat is limited without writing a test client.

## Testing with the Client (Step 3)

```bash
# Terminal 1: Start server
cargo run -p server

# Terminal 2: Start client
cargo run -p client
```

The client will:
1. Open a window with HUD in title bar
2. Auto-connect to `ws://127.0.0.1:3000/ws/zone`
3. Send `ZoneHello` message
4. Receive `ZoneWelcome` and `ZoneEnterWorldOk`
5. Begin receiving `WorldSnapshot` at 10Hz
6. Send `InputMove` when WASD keys pressed

### Window Title HUD

The window title displays real-time status:
```
Project Rizen — Connected — ents: 1 — tick: 890 — ping: 0ms — seq: 582
```

- **status**: Disconnected / Connecting... / Connected / Error
- **ents**: Number of entities in the world
- **tick**: Server tick from latest snapshot
- **ping**: Placeholder (not yet implemented)
- **seq**: Client input sequence number

**Current Limitation:** The egui UI overlay is disabled due to lifetime issues with egui-wgpu 0.30. The window shows a dark background, but all network functionality works. Monitor the server logs to verify client connection and input.

## Expected Server Logs on Connection

```
INFO  server::zone > New WebSocket connection
INFO  server::zone > Received ZoneHello character_id=...
INFO  server::zone > Player spawned entity_id=Entity(0)
DEBUG server::zone > Snapshot broadcast tick=2
DEBUG server::zone > Snapshot broadcast tick=4
...
```

## Expected Server Logs on Disconnect

```
INFO  server::zone > Client disconnected entity_id=Entity(0)
INFO  server::zone > Player despawned entity_id=Entity(0)
```

## Verification Checklist

- [ ] Server starts without errors
- [ ] Server listens on port 3000
- [ ] Tick loop logs appear at steady rate
- [ ] WebSocket connections are accepted at `/ws/zone`
- [ ] Protocol version mismatches are rejected with warning
- [ ] Client window title shows "Connected" after connecting
- [ ] Client window title shows entity count updating
- [ ] Client window title shows tick incrementing

## Troubleshooting

### Port already in use

Change the bind address in code or kill the existing process:
```powershell
Get-Process -Id (Get-NetTCPConnection -LocalPort 3000).OwningProcess | Stop-Process
```

### No snapshot broadcasts

Snapshots only broadcast when at least one client is connected and subscribed.
