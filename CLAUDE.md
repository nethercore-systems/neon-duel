# NEON DUEL - Claude Code Instructions

## Overview

NEON DUEL is a 2-4 player one-hit-kill platform fighter inspired by Towerfall and Samurai Gunn.
Built for ZX console to showcase rollback netcode, EPU procedural backgrounds, and matcap rendering.

## Quick Commands

```bash
# Build WASM
cargo build --target wasm32-unknown-unknown --release

# Format
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings

# Run with nethercore player (from workspace root)
cd ../nethercore && cargo run -- ../neon-duel/target/wasm32-unknown-unknown/release/neon_duel.wasm
```

## Project Structure

```
neon-duel/
├── src/
│   ├── lib.rs          # Entry point (init/update/render), game loop
│   ├── player.rs       # Player state, movement, combat
│   ├── projectile.rs   # Bullet logic, collision
│   ├── stage.rs        # Stage layouts, EPU config, hazards
│   ├── game_state.rs   # Round/match state machine
│   └── ui.rs           # HUD, winner screen
├── assets/
│   ├── specs/          # Speccade JSON specs for SFX/music
│   └── generated/      # Output from Speccade
├── Cargo.toml
├── nether.toml         # Game metadata for ZX
└── CLAUDE.md           # This file
```

## Design Constraints

### Core Mechanics
- **One-hit kills** - Any projectile or melee hit = death
- **3 bullets per life** - Reload on respawn only
- **8-directional aiming** - Not 360°, keeps it tight
- **Wall-jump** - Contact with wall + jump input
- **Bullet deflection** - Melee timed with incoming bullet

### Controls
| Action | Input |
|--------|-------|
| Move | D-pad/stick (8-dir) |
| Jump | A button (variable height) |
| Shoot | B button + aim direction |
| Melee | X button |

### Constants (tune these)
```rust
const GRAVITY: f32 = 0.5;
const JUMP_FORCE: f32 = 12.0;
const MOVE_SPEED: f32 = 5.0;
const BULLET_SPEED: f32 = 15.0;
const MELEE_DURATION: u32 = 10; // ticks
const MELEE_RANGE: f32 = 1.5;
const RESPAWN_DELAY: u32 = 60; // ticks (1 second at 60fps)
```

## FFI Reference

Import FFI from nethercore:
```rust
#[path = "../nethercore/include/zx.rs"]
mod ffi;
use ffi::*;
```

### Key Functions
- `player_count()` - Number of players (1-4)
- `button_pressed(player, button)` - Button just pressed this frame
- `button_held(player, button)` - Button currently held
- `left_stick_x/y(player)` - Analog stick (-1.0 to 1.0)
- `dpad_left/right/up/down(player)` - D-pad state

### Rendering
- `push_identity()`, `push_translate()`, `push_rotate_z()` - Transform stack
- `draw_billboard()` - Camera-facing quad
- `draw_rect()`, `draw_text()` - 2D UI
- `set_color()` - Vertex color (0xRRGGBBAA)

### EPU (Procedural Backgrounds)
- `env_gradient()` - Vertical color blend (skies)
- `env_lines()` - Infinite grid (synthwave floor)
- `env_scatter()` - Particles (stars, debris)
- `env_rings()` - Concentric circles (portals)
- `draw_env()` - Render configured EPU layers

## Stages

1. **GRID ARENA** - Symmetrical, no hazards, EPU Lines + Gradient
2. **SCATTER FIELD** - Asymmetric, bottom pit (fall = death), EPU Scatter + Gradient
3. **RING VOID** - Floating platforms, moving platform, EPU Rings

## Rollback Safety

All game state must be in static variables (WASM memory is snapshotted):
```rust
static mut PLAYERS: [Player; 4] = [Player::new(); 4];
static mut BULLETS: [Bullet; 32] = [Bullet::new(); 32];
static mut GAME_STATE: GameState = GameState::new();
```

Use `random()`, `random_range()`, `random_f32()` from FFI for deterministic RNG.

## Asset Pipeline

SFX and music generated via Speccade specs in `assets/specs/`.
Run `speccade generate --spec-dir assets/specs/ --out-root assets/generated/` to regenerate.

## Iteration Tips

1. Get movement feeling good first (Phase 1)
2. Add shooting, test hit detection (Phase 2)
3. Add round/match flow (Phase 3)
4. Build stages one at a time (Phase 4)
5. Polish last (screen shake, particles)
