# NEON DUEL - Claude Code Instructions

## Overview

NEON DUEL is a 2-4 player one-hit-kill platform fighter for Nethercore ZX.
Built to showcase rollback-safe game logic, EPU procedural backgrounds, and stylized arena combat.

For onboarding and command snippets, start with `README.md`.

## Quick commands

```bash
# Build release wasm
cargo build --target wasm32-unknown-unknown --release

# Format
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings

# Run with Nethercore player (from workspace root)
cd ../nethercore && cargo run -- ../neon-duel/target/wasm32-unknown-unknown/release/neon_duel.wasm
```

## Project structure

```text
neon-duel/
  src/
    lib.rs          # Entry point (init/update/render), top-level flow
    game_state.rs   # Round/match state machine and menu state
    player.rs       # Player state, movement, aiming, and bots
    combat.rs       # Bullets, melee, hit and deflect resolution
    stage.rs        # Stage layouts and platform updates
    render.rs       # 3D world + HUD rendering
    particles.rs    # Particle effects
    audio.rs        # Music and SFX control
    ffi.rs          # ZX FFI declarations
  assets/
    specs/          # SpecCade source specs
    generated/      # Generated audio artifacts
  Cargo.toml
  nether.toml
  README.md
  CLAUDE.md
```

## Design constraints

### Core mechanics

- One-hit kills: any projectile or melee hit causes a KO.
- Ammo pressure: players have limited shots per life.
- 8-direction aiming keeps combat readable and intentional.
- Melee can deflect bullets with correct timing.

### Controls

| Action | Input |
|--------|-------|
| Move | D-pad or stick |
| Jump | A button |
| Shoot | B button + aim direction |
| Melee | X button |
| Pause | Start |

## Rollback safety rules

- Keep gameplay state deterministic and rollback-safe.
- Avoid non-deterministic platform APIs in gameplay logic.
- Keep authoritative match/player/combat state in static game memory.
- Use deterministic random sources exposed by ZX FFI where needed.

## ZX FFI reference

Import FFI through `src/ffi.rs`.

Key categories used by this game:
- Input: `player_count`, `button_pressed`, `button_held`, `left_stick_x`, `left_stick_y`
- Rendering transforms and draw: `push_identity`, `push_translate`, `push_rotate_z`, `draw_*`
- Environment (EPU): `env_gradient`, `env_lines`, `env_scatter`, `env_rings`, `draw_env`
- Random: `random`, `random_range`, `random_f32`

## Asset pipeline

Audio and music specs live in `assets/specs/` and generate output into `assets/generated/`.

```bash
speccade generate-all --spec-dir assets/specs/ --out-root assets/generated/
```
