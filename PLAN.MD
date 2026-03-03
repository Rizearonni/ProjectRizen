Project Blueprint (Rust MMO + Procedural Zones)

This is the pre-scaffold blueprint you can drop into your repo as PLAN.md (or split into files). It locks in structure, boundaries, and “v1” specs so scaffolding is straightforward and professional.

1) Goals and non-goals
Goals (v1)

Server-authoritative MMO core loop

Zone-based architecture (even if you run one zone locally first)

Procedural terrain per-zone with deterministic seeds

Data-driven content for mobs/items/abilities/zones

Persistence (accounts/characters/progress/inventory)

Tooling early (admin commands, debug overlays)

Non-goals (v1)

Seamless “one-world” megaserver

Fully custom UI framework (we’ll use egui first)

Client-side prediction/reconciliation (optional later)

Voxel destructible terrain (heightmap first)

2) Updated milestone structure and progress gates
Milestone 1 — Connected World

Gate

20 clients connect locally, see each other, move with replication

Debug overlay shows: ping, server tick, entity count

Milestone 1.5 — Procedural Zone (Heightmap Chunks)

Gate

Client generates terrain from (zone_seed, params, chunk_coord)

Chunk streaming works; no seams at boundaries

Server and client agree on ground height for movement/collision

Milestone 2 — Combat + Mobs

Gate

Server-authoritative damage; client cannot spoof

Mob AI (idle→aggro→attack→leash) works with multiple players

Milestone 3 — Persistence + Real Login

Gate

Characters persist across server restarts (Postgres)

Character select/create works

Periodic saves + disconnect saves

Milestone 4 — Memory Loop

Gate

Memory fragments + memory tree unlocks are server-validated + persisted

Unlocking grants ability/passive and works in multiplayer

Milestone 5 — Zone Transfers + AOI v1

Gate

Portal transfer between 2 zones is safe (no dupes, no stuck)

AOI reduces bandwidth in crowded areas

Milestone 6 — Production Hardening

Gate

Rate limiting, sanity checks, logging/metrics are good enough to operate on VPS

3) Monorepo layout (recommended)
eldrynth/
  Cargo.toml                  # workspace
  crates/
    common/                   # shared types, protocol, IDs, math
    data/                     # data loading + validation + schemas
    worldgen/                 # deterministic procedural terrain + features
    server/                   # server runtime (auth + zone services)
    client/                   # winit/wgpu client + egui UI
    tools/                    # optional CLI tools (validate data, migrations, etc.)
  data/                       # TOML/JSON content (runtime loaded)
    zones/
    mobs/
    items/
    abilities/
    loot_tables/
  migrations/                 # sqlx migrations
  docs/
    PLAN.md                   # this doc
    PROTOCOL.md
    WORLDGEN_SPEC.md
    DB_SCHEMA.md
Crate responsibilities (hard boundaries)

common: IDs, time, math, protocol messages, versioning, utilities

data: parse + validate content files, provide strongly typed structs

worldgen: pure deterministic generation functions + chunk outputs

server: networking, auth, zone simulation, persistence, delta layer

client: rendering, input, UI, net client, terrain mesh cache

tools: validation, offline generation, smoke tests

4) Runtime architecture
Services (v1)

Auth Service (can be same binary, separate module)

login/register

session issuance

character list/create

Zone Service

authoritative simulation

entity replication

terrain delta distribution (optional early)

Early scaffolding: one executable server that runs both services; later you can split into separate processes.

Connections

Client connects to Auth → gets session_token

Client connects to Zone with session_token → receives zone info (seed/params) + enter world

5) Protocol v1 (messages and rules)
Transport choice (v1)

WebSocket for Milestone 1–3 (debuggable, NAT-friendly)

Upgrade later to QUIC for high-performance real-time

Envelope

All messages are wrapped:

struct Envelope {
  protocol_version: u16,   // start at 1
  msg_type: u16,           // enum discriminant
  request_id: Option<u32>, // for request/response style messages
  payload: Vec<u8>,        // bincode/msgpack
}
Message sets
Auth

AuthHello { client_version }

AuthLoginDev { username } (Milestone 1–2)

AuthLogin { username, password } (Milestone 3+)

AuthLoginOk { session_token, characters: Vec<CharacterSummary> }

AuthCreateCharacter { name, appearance_seed }

AuthCreateCharacterOk { character_id }

Zone connect / world

ZoneHello { session_token, character_id }

ZoneWelcome { zone_id, zone_seed, zone_params, server_tick_rate }

ZoneEnterWorldOk { entity_id, snapshot }

Input (client→server)

Send at 20–60Hz (start 20Hz):

InputMove { seq, move_x, move_y, yaw, buttons }

InputCast { seq, ability_id, target: TargetSpec }

InputInteract { seq, entity_id }

Snapshots (server→client)

Server tick 10–20Hz (start 10–20):

WorldSnapshot { server_tick, entities: Vec<EntityState> }

EntityDespawn { entity_id }

ServerEvent { text, severity } (system messages)

Chat

ChatSend { channel, text }

ChatBroadcast { from, channel, text }

Terrain (v1.5+)

ZoneParams is sent once (ZoneWelcome)

Deltas are chunk-scoped:

TerrainDeltaBatch { chunk_coord, ops: Vec<TerrainOp> }

TerrainOp examples:

RemoveFeature { feature_id }

NodeDepleted { node_id, until_tick }

PlaceObject { object_def, transform }

Critical rules

Server is source of truth for: damage, loot, inventory, memory unlocks

Client generates base terrain; server sends only deltas + authoritative entities

6) Simulation model
Time

Server tick rate: 20 (recommended start) or 10 if you want ultra-simple

Client frame rate: 60+, interpolates snapshot states

Server ECS components (minimum v1)

EntityId(u32) (with generation optional)

Transform { pos: Vec3, yaw: f32 }

Mover { speed, desired_dir } (or Velocity)

Stats { hp, max_hp, ... }

CombatState { cooldowns, gcd_until_tick }

Player { character_id, account_id }

Mob { mob_id, leash_origin, aggro_target }

Brain { state, timers }

Inventory { slots } (Milestone 3)

Client state (minimum v1)

NetClient (socket, queue, ping, seq)

EntityStore (renderable state cache)

TerrainCache (chunk meshes)

UIState (egui)

7) Data-driven content format (v1)
File format choice

Use TOML for human-editable configs (recommended)

JSON is okay, TOML is nicer for designers (you)

IDs

All content uses stable string IDs:

mobs: mob.skeleton_scout

abilities: ability.memory_dash

zones: zone.ossuary

Examples
data/zones/ossuary.toml
id = "zone.ossuary"
name = "The Ossuary"
seed = 1844674407370955161

[terrain]
chunk_size = 64                # meters
verts_per_side = 65            # for grid mesh
height_scale = 48.0

[terrain.noise]
base_freq = 0.002
octaves = 5
lacunarity = 2.0
gain = 0.5

[biomes]
# simple height/slope-based bands (v1)
ash_height_max = 0.35
rock_slope_min = 0.60

[features]
bones_density = 0.08
ruins_density = 0.02
lava_crack_density = 0.01

[spawns]
# procedural spawn regions later; start with explicit regions (v1)
[[spawns.region]]
name = "North Yard"
min = [ -256, -256 ]
max = [  256,  256 ]
mob_id = "mob.skeleton_scout"
cap = 12
respawn_seconds = 20
data/mobs/skeleton_scout.toml
id = "mob.skeleton_scout"
name = "Skeleton Scout"
level = 2
model = "primitive.capsule"  # v1 placeholder

[stats]
hp = 45
move_speed = 3.5
attack_range = 2.0
attack_damage = 6
attack_cooldown = 1.8
aggro_range = 12.0
leash_range = 18.0

[loot]
table = "loot.low_ossuary"
data/abilities/memory_dash.toml
id = "ability.memory_dash"
name = "Memory Dash"
cooldown_seconds = 8
gcd_seconds = 1

[effects]
type = "dash"
distance = 8.0
8) Procedural worldgen spec (v1.5 baseline)
Determinism contract

Terrain height is computed as:

height(x,z) = f(zone_seed, zone_params, world_x, world_z)

Use fixed noise algorithm and stable hashing

Avoid RNG calls that depend on iteration order (use coordinate hashing)

Chunking

Chunk coord: (cx, cz)

World-space bounds:

x ∈ [cx*chunk_size, (cx+1)*chunk_size]

Mesh grid:

verts_per_side (e.g. 65) so edges line up

Seamless edges ensured by sampling world coordinates, not local-only RNG

Feature placement

Use hashed coordinates for repeatable scatter:

feature_rng = hash(zone_seed, cx, cz, feature_kind)

Place simple instanced primitives first (bones/rocks/pillars)

Collision height

Server uses the same height(x,z) function for grounding and movement validation

Client uses it for visuals; movement remains authoritative on server

9) Persistence v1 (Postgres)
Tables (v1)

accounts

account_id UUID PK

username TEXT UNIQUE

password_hash TEXT NULL (NULL in dev auth)

created_at TIMESTAMP

sessions

session_token TEXT PK

account_id UUID

expires_at TIMESTAMP

characters

character_id UUID PK

account_id UUID FK

name TEXT

appearance_seed BIGINT

level INT

memory_fragments INT

created_at TIMESTAMP

character_state

character_id UUID PK

zone_id TEXT

pos_x REAL

pos_y REAL

pos_z REAL

yaw REAL

updated_at TIMESTAMP

inventory_slots

character_id UUID

slot_index INT

item_id TEXT

quantity INT

PK: (character_id, slot_index)

memory_unlocks

character_id UUID

node_id TEXT

unlocked_at TIMESTAMP

PK: (character_id, node_id)

Save rules

On disconnect: save character_state + inventory + progress

Periodic save: every 60s (v1)

Server restart must not lose data

10) UI screens (v1)
Must-have (Milestone 1–3)

Login (dev → real later)

Character select/create

In-game HUD:

hp/resource/exp

chat

minimal target info

Debug overlay:

ping, tick, entities, chunk count, fps

Memory UI (Milestone 4)

Memory tree

Node unlock confirmation

Tooltip with grants

All UI in egui initially.

11) Dev tooling and operational basics
Admin commands (server)

/teleport x z

/give item_id qty

/spawnmob mob_id count

/setlevel n

Logging/metrics

tracing everywhere

Structured logs for connects/disconnects, zone transfers, db errors

Simple metrics later (Milestone 6)

12) Progress-gate “Definition of Done” template

Every milestone PR/checkpoint must include:

✅ Runs from clean checkout

✅ Minimal smoke test path documented in docs/SMOKE_TEST.md

✅ Known issues list updated

✅ One short demo recording or screenshots (optional but highly recommended)