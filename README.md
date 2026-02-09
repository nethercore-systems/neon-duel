# NEON DUEL

NEON DUEL is a 2-4 player one-hit-kill platform fighter for Nethercore ZX.
The project is a Rust `no_std` WebAssembly game focused on rollback-safe gameplay, fast rounds, and strong visual readability.

## Current gameplay pillars

- One-hit kills (projectile or melee)
- Limited ammo with respawn reload
- 8-direction aim and movement-driven dueling
- Bullet deflection timing windows
- Stage variety with procedural EPU backgrounds

## Quick start

```bash
# One-time target setup
rustup target add wasm32-unknown-unknown

# Build release wasm
cargo build --target wasm32-unknown-unknown --release

# Run from Nethercore player (workspace root)
cd ../nethercore
cargo run -- ../neon-duel/target/wasm32-unknown-unknown/release/neon_duel.wasm
```

## Dev commands

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo build --target wasm32-unknown-unknown --release
```

## Repo map

```text
neon-duel/
  src/
    lib.rs          # Entry point and game loop
    game_state.rs   # Match/round phase state machine and config
    player.rs       # Player input, movement, and state
    combat.rs       # Bullets, melee, hit logic
    stage.rs        # Stage definitions and platform behavior
    render.rs       # Scene/UI rendering
    particles.rs    # Particle system updates and draw helpers
    audio.rs        # Music and SFX routing
    ffi.rs          # ZX FFI bindings and wrappers
  assets/
    specs/          # SpecCade source specs
    generated/      # Generated audio outputs and summaries
  Cargo.toml
  nether.toml       # Game metadata
  CLAUDE.md         # Repo constraints and implementation guidance
```

## Asset pipeline

Audio assets are generated from specs in `assets/specs/`.
Use Speccade from the workspace to regenerate as needed:

```bash
speccade generate-all --spec-dir assets/specs/ --out-root assets/generated/
```

## Documentation

- `CLAUDE.md`: detailed development constraints, controls, and systems notes
- `nether.toml`: game metadata consumed by Nethercore ZX tooling
