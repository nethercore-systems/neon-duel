//! Combat system
//!
//! Contains Bullet struct, shooting, melee, and collision detection.

use crate::audio;
use crate::game_state;
use crate::particles;
use crate::player::{
    aabb_overlap, kill_player, MELEE_RANGE, PLAYERS, PLAYER_COLORS, PLAYER_HEIGHT, PLAYER_WIDTH,
};
use crate::stage::PLATFORMS;

// =============================================================================
// CONSTANTS
// =============================================================================

pub const MAX_BULLETS: usize = 32;
pub const BULLET_SPEED: f32 = 0.4;
pub const BULLET_LIFETIME: u32 = 120; // 2 seconds at 60fps

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Clone, Copy)]
pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub owner: u32, // Player index who fired
    pub lifetime: u32,
    pub active: bool,
}

impl Bullet {
    pub const fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            owner: 0,
            lifetime: 0,
            active: false,
        }
    }
}

// =============================================================================
// GLOBAL STATE
// =============================================================================

pub static mut BULLETS: [Bullet; MAX_BULLETS] = [Bullet::new(); MAX_BULLETS];

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Point in AABB collision
pub fn point_in_aabb(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

/// Normalize aim to 8 directions
fn normalize_aim(x: f32, y: f32, facing_right: bool) -> (f32, f32) {
    // Snap to 8 directions
    let threshold = 0.3;

    let ax = if x > threshold {
        1.0
    } else if x < -threshold {
        -1.0
    } else {
        0.0
    };
    let ay = if y > threshold {
        1.0
    } else if y < -threshold {
        -1.0
    } else {
        0.0
    };

    // If no direction, use facing direction
    if ax == 0.0 && ay == 0.0 {
        return (if facing_right { 1.0 } else { -1.0 }, 0.0);
    }

    // Normalize diagonal
    if ax != 0.0 && ay != 0.0 {
        let inv_sqrt2 = core::f32::consts::FRAC_1_SQRT_2;
        return (ax * inv_sqrt2, ay * inv_sqrt2);
    }

    (ax, ay)
}

// =============================================================================
// BULLET LOGIC
// =============================================================================

pub fn spawn_bullet(player_idx: usize, aim_x: f32, aim_y: f32) {
    unsafe {
        let p = &PLAYERS[player_idx];

        // Find inactive bullet slot
        for bullet in &mut BULLETS {
            if bullet.active {
                continue;
            }

            // Determine aim direction (8-directional)
            let (dx, dy) = normalize_aim(aim_x, aim_y, p.facing_right);

            // Spawn bullet
            let spawn_x = p.x + PLAYER_WIDTH / 2.0;
            let spawn_y = p.y + PLAYER_HEIGHT / 2.0;
            bullet.x = spawn_x;
            bullet.y = spawn_y;
            bullet.vx = dx * BULLET_SPEED;
            bullet.vy = dy * BULLET_SPEED;
            bullet.owner = player_idx as u32;
            bullet.lifetime = BULLET_LIFETIME;
            bullet.active = true;

            // Spawn muzzle flash effect light (yellow, fast decay)
            game_state::spawn_effect_light(spawn_x, spawn_y, 0xFFFF00FF, 1.5, 0.7);

            return;
        }
    }
}

pub fn update_bullets() {
    unsafe {
        for bullet in &mut BULLETS {
            if !bullet.active {
                continue;
            }

            // Move bullet (fixed timestep)
            bullet.x += bullet.vx;
            bullet.y += bullet.vy;

            // Spawn bullet trail particles every 3 frames
            if bullet.lifetime % 3 == 0 {
                particles::spawn_bullet_trail(bullet.x, bullet.y);
            }

            // Lifetime
            bullet.lifetime -= 1;
            if bullet.lifetime == 0 {
                bullet.active = false;
                continue;
            }

            // Screen bounds
            if bullet.x < -12.0 || bullet.x > 12.0 || bullet.y < -10.0 || bullet.y > 10.0 {
                bullet.active = false;
                continue;
            }

            // Platform collision (bullets stop on platforms)
            for platform in &PLATFORMS {
                if !platform.active {
                    continue;
                }

                if point_in_aabb(
                    bullet.x,
                    bullet.y,
                    platform.x,
                    platform.y,
                    platform.width,
                    platform.height,
                ) {
                    bullet.active = false;
                    break;
                }
            }

            if !bullet.active {
                continue;
            }

            // Player collision
            for (i, player) in PLAYERS.iter().enumerate() {
                if !player.active || player.dead {
                    continue;
                }

                // Can't hit self
                if i as u32 == bullet.owner {
                    continue;
                }

                // Check melee deflection
                if player.melee_timer > 0 {
                    let melee_x = player.x
                        + PLAYER_WIDTH / 2.0
                        + (if player.facing_right {
                            MELEE_RANGE / 2.0
                        } else {
                            -MELEE_RANGE / 2.0
                        });
                    let melee_y = player.y + PLAYER_HEIGHT / 2.0;

                    let dx = bullet.x - melee_x;
                    let dy = bullet.y - melee_y;
                    let dist = libm::sqrtf(dx * dx + dy * dy);

                    if dist < MELEE_RANGE {
                        // Deflect bullet - reverse direction and change owner
                        bullet.vx = -bullet.vx;
                        bullet.vy = -bullet.vy;
                        bullet.owner = i as u32;
                        bullet.lifetime = BULLET_LIFETIME; // Reset lifetime
                                                           // Play deflect sound
                        audio::play_deflect();
                        // Screen shake on deflect
                        game_state::trigger_shake(0.3);
                        // Hit freeze for deflect (3 frames ~50ms, brief acknowledgment)
                        game_state::trigger_hit_freeze(3);
                        // Deflect effect light (cyan flash)
                        game_state::spawn_effect_light(bullet.x, bullet.y, 0x00FFFFFF, 2.0, 0.75);
                        continue;
                    }
                }

                // Hit detection
                let px = player.x;
                let py = player.y;
                let pw = PLAYER_WIDTH;
                let ph = PLAYER_HEIGHT;

                if point_in_aabb(bullet.x, bullet.y, px, py, pw, ph) {
                    // Play hit sound before killing player
                    audio::play_hit();
                    // Screen shake on bullet hit
                    game_state::trigger_shake(0.6);
                    // Hit freeze for impact (5 frames ~83ms)
                    game_state::trigger_hit_freeze(5);
                    // Impact flash on hit
                    game_state::trigger_impact_flash();
                    // Camera zoom on kill
                    game_state::trigger_camera_zoom();
                    // Death effect light (victim's color, bright)
                    game_state::spawn_effect_light(
                        px + pw / 2.0,
                        py + ph / 2.0,
                        PLAYER_COLORS[i],
                        3.0,
                        0.8,
                    );
                    // Kill player
                    kill_player(i, bullet.owner);
                    bullet.active = false;
                    break;
                }
            }
        }
    }
}

pub fn update_melee_hits() {
    unsafe {
        for (attacker_idx, attacker) in PLAYERS.iter().enumerate() {
            if !attacker.active || attacker.dead || attacker.melee_timer == 0 {
                continue;
            }

            // Melee hitbox
            let melee_x = if attacker.facing_right {
                attacker.x + PLAYER_WIDTH
            } else {
                attacker.x - MELEE_RANGE
            };
            let melee_y = attacker.y;
            let melee_w = MELEE_RANGE;
            let melee_h = PLAYER_HEIGHT;

            for (target_idx, target) in PLAYERS.iter().enumerate() {
                if target_idx == attacker_idx {
                    continue;
                }
                if !target.active || target.dead {
                    continue;
                }

                // Check if target is hit by melee
                if aabb_overlap(
                    (melee_x, melee_y, melee_w, melee_h),
                    (target.x, target.y, PLAYER_WIDTH, PLAYER_HEIGHT),
                ) {
                    // Play hit sound for melee hit
                    audio::play_hit();
                    // Screen shake on melee hit
                    game_state::trigger_shake(0.5);
                    // Hit freeze for melee (6 frames ~100ms, slightly longer for up-close hit)
                    game_state::trigger_hit_freeze(6);
                    // Impact flash on hit
                    game_state::trigger_impact_flash();
                    // Camera zoom on kill
                    game_state::trigger_camera_zoom();
                    // Death effect light (victim's color, bright)
                    game_state::spawn_effect_light(
                        target.x + PLAYER_WIDTH / 2.0,
                        target.y + PLAYER_HEIGHT / 2.0,
                        PLAYER_COLORS[target_idx],
                        3.0,
                        0.8,
                    );
                    kill_player(target_idx, attacker_idx as u32);
                }
            }
        }
    }
}
