# Data Directory

Runtime content files in TOML format. All game content is data-driven and hot-reloadable.

## Directory Structure

```
data/
├── zones/              # Zone terrain and spawn definitions
│   └── ossuary.toml
├── mobs/               # Mob stats and AI definitions
│   ├── skeleton_scout.toml
│   └── skeleton_warrior.toml
└── abilities/          # Player ability definitions
    ├── memory_dash.toml
    ├── shadow_bolt.toml
    └── mend_wounds.toml
```

## Loading Data

```rust
use data::DataRegistry;

let registry = DataRegistry::load_from_dir("./data")?;

// Access by ID
let zone = registry.get_zone("zone.ossuary");
let mob = registry.get_mob("mob.skeleton_scout");
let ability = registry.get_ability("ability.memory_dash");

// Validate cross-references
let errors = registry.validate_references();
```

## ID Conventions

All content uses stable, prefixed string IDs:

| Type | Prefix | Example |
|------|--------|---------|
| Zone | `zone.` | `zone.ossuary` |
| Mob | `mob.` | `mob.skeleton_scout` |
| Ability | `ability.` | `ability.memory_dash` |
| Loot Table | `loot.` | `loot.low_ossuary` (planned) |
| Item | `item.` | `item.bone_shard` (planned) |

---

## Zone Schema

Zones define terrain generation, biomes, features, and mob spawns.

```toml
# Required
id = "zone.ossuary"              # Unique ID with zone. prefix
name = "The Ossuary"             # Display name
seed = 1844674407370955161       # u64 seed for deterministic generation

# Terrain parameters
[terrain]
chunk_size = 64                  # World units per chunk
verts_per_side = 65              # Vertices per chunk edge (chunk_size + 1)
height_scale = 48.0              # Maximum terrain height

[terrain.noise]
base_freq = 0.002                # Base noise frequency
octaves = 5                      # Noise octave count
lacunarity = 2.0                 # Frequency multiplier per octave
gain = 0.5                       # Amplitude multiplier per octave

# Biome thresholds
[biomes]
ash_height_max = 0.35            # Normalized height for ash biome
rock_slope_min = 0.60            # Slope threshold for rock biome

# Feature densities (0.0 - 1.0)
[features]
bones_density = 0.08             # Scattered bone features
ruins_density = 0.02             # Ruin structure features
lava_crack_density = 0.01        # Lava crack features

# Spawn regions (array)
[[spawns.region]]
name = "North Yard"              # Region display name
min = [-256, -256]               # Bottom-left corner [x, z]
max = [256, 256]                 # Top-right corner [x, z]
mob_id = "mob.skeleton_scout"    # Reference to mob definition
cap = 12                         # Maximum simultaneous spawns
respawn_seconds = 20             # Respawn delay after kill
```

---

## Mob Schema

Mobs define enemy stats, AI behavior, and loot references.

```toml
# Required
id = "mob.skeleton_scout"        # Unique ID with mob. prefix
name = "Skeleton Scout"          # Display name
level = 2                        # Mob level (affects combat math)
model = "primitive.capsule"      # Render model ID

# Combat stats
[stats]
hp = 45                          # Base hit points
move_speed = 4.0                 # Movement speed (units/sec)
attack_range = 2.0               # Melee attack range
attack_damage = 6                # Base attack damage
attack_cooldown = 1.8            # Seconds between attacks
aggro_range = 12.0               # Detection range for aggro
leash_range = 20.0               # Distance before mob resets (must be > aggro_range)

# Loot reference
[loot]
table = "loot.low_ossuary"       # Reference to loot table (planned)
```

### Validation Rules

- `leash_range` must be greater than `aggro_range`
- `id` must start with `mob.`
- Referenced loot tables will be validated when implemented

---

## Ability Schema

Abilities define cooldowns, effects, costs, and unlock requirements.

```toml
# Required
id = "ability.memory_dash"       # Unique ID with ability. prefix
name = "Memory Dash"             # Display name
cooldown_seconds = 8             # Cooldown duration
gcd_seconds = 0.5                # Global cooldown trigger (0 for off-GCD)

# Effect definition
[effects]
type = "dash"                    # Effect type: dash, damage, heal, buff
distance = 8.0                   # Dash distance (for type = "dash")

# OR for damage abilities:
[effects]
type = "damage"
amount = 25                      # Damage amount
is_projectile = true             # Optional: fires projectile

# OR for heal abilities:
[effects]
type = "heal"
amount = 20                      # Heal amount

# Cost (optional)
[cost]
memory_fragments = 10            # Currency cost

# Requirements (optional)
[requires]
level = 1                        # Minimum player level
```

### Effect Types

| Type | Required Fields | Description |
|------|-----------------|-------------|
| `dash` | `distance` | Move player forward |
| `damage` | `amount` | Deal damage, optionally `is_projectile` |
| `heal` | `amount` | Restore HP |
| `buff` | `buff_id`, `duration` | Apply status effect |

---

## Adding New Content

1. Create a TOML file in the appropriate directory
2. Use the correct ID prefix
3. Reference IDs from other content (mobs → zones, abilities → level)
4. Run validation: `cargo test -p data`

### Example: Adding a New Mob

```toml
# data/mobs/skeleton_archer.toml
id = "mob.skeleton_archer"
name = "Skeleton Archer"
level = 3
model = "primitive.capsule"

[stats]
hp = 35
move_speed = 3.0
attack_range = 15.0
attack_damage = 12
attack_cooldown = 2.5
aggro_range = 18.0
leash_range = 25.0

[loot]
table = "loot.low_ossuary"
```

Then add to a zone spawn:

```toml
# In data/zones/ossuary.toml
[[spawns.region]]
name = "Archer Tower"
min = [300, 200]
max = [400, 300]
mob_id = "mob.skeleton_archer"
cap = 4
respawn_seconds = 45
```
