//! Stage management
//!
//! Contains Platform struct, stage layouts, and EPU configuration.

use crate::game_state::GAME_STATE;

// =============================================================================
// CONSTANTS
// =============================================================================

pub const MAX_PLATFORMS: usize = 16;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Clone, Copy)]
pub struct Platform {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub active: bool,
    pub moving: bool, // For Stage 3 moving platform
    pub move_speed: f32,
    pub move_min: f32,
    pub move_max: f32,
}

impl Platform {
    pub const fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            active: false,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        }
    }
}

// =============================================================================
// GLOBAL STATE
// =============================================================================

pub static mut PLATFORMS: [Platform; MAX_PLATFORMS] = [Platform::new(); MAX_PLATFORMS];
pub static mut HAS_PIT: bool = false;
pub static mut PIT_Y: f32 = -10.0;

// =============================================================================
// STAGE SETUP
// =============================================================================

fn setup_stage_grid_arena() {
    unsafe {
        HAS_PIT = false;

        // Clear platforms
        for p in &mut PLATFORMS {
            p.active = false;
        }

        // Ground
        PLATFORMS[0] = Platform {
            x: -10.0,
            y: -2.0,
            width: 20.0,
            height: 0.5,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };

        // Middle platforms (symmetrical)
        PLATFORMS[1] = Platform {
            x: -7.0,
            y: 1.0,
            width: 4.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[2] = Platform {
            x: 3.0,
            y: 1.0,
            width: 4.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };

        // Top platform
        PLATFORMS[3] = Platform {
            x: -3.0,
            y: 4.0,
            width: 6.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
    }
}

fn setup_stage_scatter_field() {
    unsafe {
        HAS_PIT = true;
        PIT_Y = -5.0;

        // Clear platforms
        for p in &mut PLATFORMS {
            p.active = false;
        }

        // Asymmetric platforms - no ground, pit below
        PLATFORMS[0] = Platform {
            x: -9.0,
            y: 0.0,
            width: 4.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[1] = Platform {
            x: -3.0,
            y: -1.0,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[2] = Platform {
            x: 2.0,
            y: 0.5,
            width: 3.5,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[3] = Platform {
            x: 6.0,
            y: -0.5,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };

        // Upper platforms
        PLATFORMS[4] = Platform {
            x: -6.0,
            y: 3.0,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[5] = Platform {
            x: 0.0,
            y: 4.0,
            width: 4.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[6] = Platform {
            x: 5.0,
            y: 2.5,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
    }
}

fn setup_stage_ring_void() {
    unsafe {
        HAS_PIT = true;
        PIT_Y = -6.0;

        // Clear platforms
        for p in &mut PLATFORMS {
            p.active = false;
        }

        // Floating platforms with gaps
        PLATFORMS[0] = Platform {
            x: -8.0,
            y: 0.0,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[1] = Platform {
            x: 5.0,
            y: 0.0,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };

        // Moving platform in center
        PLATFORMS[2] = Platform {
            x: -1.5,
            y: 1.0,
            width: 3.0,
            height: 0.4,
            active: true,
            moving: true,
            move_speed: 0.02,
            move_min: -4.0,
            move_max: 4.0,
        };

        // Upper corners
        PLATFORMS[3] = Platform {
            x: -7.0,
            y: 3.5,
            width: 2.5,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
        PLATFORMS[4] = Platform {
            x: 4.5,
            y: 3.5,
            width: 2.5,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };

        // Top center
        PLATFORMS[5] = Platform {
            x: -2.0,
            y: 5.0,
            width: 4.0,
            height: 0.4,
            active: true,
            moving: false,
            move_speed: 0.0,
            move_min: 0.0,
            move_max: 0.0,
        };
    }
}

pub fn setup_current_stage() {
    unsafe {
        match GAME_STATE.current_stage {
            0 => setup_stage_grid_arena(),
            1 => setup_stage_scatter_field(),
            2 => setup_stage_ring_void(),
            _ => setup_stage_grid_arena(),
        }
    }
}

pub fn update_platforms() {
    unsafe {
        for platform in &mut PLATFORMS {
            if !platform.active || !platform.moving {
                continue;
            }

            // Move platform
            platform.x += platform.move_speed;

            // Reverse at bounds
            if platform.x <= platform.move_min
                || platform.x + platform.width >= platform.move_max + platform.width
            {
                platform.move_speed = -platform.move_speed;
            }
        }
    }
}

// =============================================================================
// SPAWN POINTS
// =============================================================================

/// Spawn points per stage (up to 4 players)
/// Format: [(x, y), (x, y), (x, y), (x, y)] stored as flat array [x0, y0, x1, y1, ...]
/// Positions are chosen to be:
/// - On or above platforms (not in mid-air over pits)
/// - Symmetrically spread when possible
/// - Far enough apart to prevent spawn camping
pub const SPAWN_POINTS: [[f32; 8]; 3] = [
    // Stage 0: Grid Arena
    // - Players 0,1 on left/right middle platforms (y=1.0 + platform height ~1.4)
    // - Players 2,3 on top platform spread apart (y=4.0 + platform height ~4.4)
    [-5.0, 1.5, 5.0, 1.5, -2.0, 4.5, 2.0, 4.5],
    // Stage 1: Scatter Field
    // - Players 0,1 on lower left/right platforms
    // - Players 2,3 on upper platforms
    [-7.0, 0.5, 3.5, 1.0, -4.5, 3.5, 1.5, 4.5],
    // Stage 2: Ring Void
    // - Players 0,1 on left/right floating platforms (y=0.0 + height ~0.4)
    // - Players 2,3 on upper corner platforms (y=3.5 + height ~3.9)
    [-6.5, 0.5, 6.0, 0.5, -5.5, 4.0, 5.5, 4.0],
];

/// Get spawn position for player on current stage (used for initial spawn)
pub fn get_spawn_position(player_idx: usize) -> (f32, f32) {
    unsafe {
        let stage = GAME_STATE.current_stage as usize;
        let stage = stage.min(2); // Clamp to valid stage range
        let idx = player_idx.min(3) * 2; // 2 floats per position, max 4 players
        (SPAWN_POINTS[stage][idx], SPAWN_POINTS[stage][idx + 1])
    }
}
