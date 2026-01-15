//! Player state and movement
//!
//! Contains Player struct, physics, input handling, and respawn logic.

use crate::audio;
use crate::combat::spawn_bullet;
use crate::ffi::*;
use crate::game_state::{GamePhase, GAME_STATE};
use crate::stage::PLATFORMS;

// =============================================================================
// CONSTANTS
// =============================================================================

// Button indices (from ZX spec)
pub const BUTTON_UP: u32 = 0;
pub const BUTTON_DOWN: u32 = 1;
pub const BUTTON_LEFT: u32 = 2;
pub const BUTTON_RIGHT: u32 = 3;
pub const BUTTON_A: u32 = 4; // Jump
pub const BUTTON_B: u32 = 5; // Shoot
pub const BUTTON_X: u32 = 6; // Melee
pub const BUTTON_START: u32 = 12; // Pause/restart

// Physics (tuned for 60fps fixed timestep)
pub const GRAVITY: f32 = 0.025;
pub const JUMP_FORCE: f32 = 0.5;
pub const MOVE_SPEED: f32 = 0.15;
pub const FRICTION: f32 = 0.85;
pub const AIR_FRICTION: f32 = 0.95;

// Player dimensions
pub const PLAYER_WIDTH: f32 = 0.8;
pub const PLAYER_HEIGHT: f32 = 1.2;

// Combat
pub const MAX_AMMO: u32 = 3;
pub const MELEE_DURATION: u32 = 12; // ticks active
pub const MELEE_RANGE: f32 = 1.8;
pub const RESPAWN_DELAY: u32 = 90; // 1.5 seconds

// World bounds (respawn if player falls below this)
pub const DEATH_Y: f32 = -8.0;

// Match rules
pub const KILLS_TO_WIN: u32 = 5;

// Motion trails
pub const TRAIL_COUNT: usize = 5;
pub const TRAIL_VELOCITY_THRESHOLD: f32 = 0.1;

// Squash/stretch
pub const SQUASH_DECAY: f32 = 0.85;

// Shoot flash duration
pub const SHOOT_FLASH_DURATION: u32 = 6;

// Melee windup (anticipation frames)
pub const MELEE_WINDUP_DURATION: u32 = 3;

// Player colors (RGBA)
pub const PLAYER_COLORS: [u32; 4] = [
    0x00FFFFFF, // Cyan
    0xFF00FFFF, // Magenta
    0xFFFF00FF, // Yellow
    0x00FF00FF, // Green
];

pub const MAX_PLAYERS: usize = 4;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Clone, Copy)]
pub struct Player {
    // Position and velocity
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,

    // State
    pub on_ground: bool,
    pub facing_right: bool,
    pub active: bool,

    // Combat
    pub ammo: u32,
    pub melee_timer: u32,  // > 0 means melee is active
    pub melee_windup: u32, // Anticipation frames before melee hitbox activates
    pub dead: bool,
    pub respawn_timer: u32,

    // Effects
    pub spawn_flash: u32,    // Countdown timer for spawn flash effect
    pub shoot_flash: u32,    // Muzzle flash countdown
    pub squash_stretch: f32, // -1.0 = squash, 0 = normal, 1.0 = stretch
    pub prev_positions: [(f32, f32); TRAIL_COUNT], // Position history for motion trails
    pub prev_idx: usize,     // Ring buffer index for prev_positions

    // Score
    pub kills: u32,
}

impl Player {
    pub const fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            on_ground: false,
            facing_right: true,
            active: false,
            ammo: MAX_AMMO,
            melee_timer: 0,
            melee_windup: 0,
            dead: false,
            respawn_timer: 0,
            spawn_flash: 0,
            shoot_flash: 0,
            squash_stretch: 0.0,
            prev_positions: [(0.0, 0.0); TRAIL_COUNT],
            prev_idx: 0,
            kills: 0,
        }
    }
}

// =============================================================================
// GLOBAL STATE
// =============================================================================

pub static mut PLAYERS: [Player; MAX_PLAYERS] = [Player::new(); MAX_PLAYERS];

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

pub fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

pub fn abs(v: f32) -> f32 {
    if v < 0.0 {
        -v
    } else {
        v
    }
}

pub fn aabb_overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    let (x1, y1, w1, h1) = a;
    let (x2, y2, w2, h2) = b;
    x1 < x2 + w2 && x1 + w1 > x2 && y1 < y2 + h2 && y1 + h1 > y2
}

// =============================================================================
// PLAYER LOGIC
// =============================================================================

pub fn spawn_players() {
    unsafe {
        let count = player_count().min(MAX_PLAYERS as u32) as usize;

        for i in 0..MAX_PLAYERS {
            if i < count {
                // Get stage-specific spawn position for this player
                let (sx, sy) = crate::stage::get_spawn_position(i);
                PLAYERS[i] = Player {
                    x: sx,
                    y: sy,
                    vx: 0.0,
                    vy: 0.0,
                    on_ground: false,
                    facing_right: i % 2 == 0,
                    active: true,
                    ammo: MAX_AMMO,
                    melee_timer: 0,
                    melee_windup: 0,
                    dead: false,
                    respawn_timer: 0,
                    spawn_flash: 30, // Spawn flash effect (0.5 seconds at 60fps)
                    shoot_flash: 0,
                    squash_stretch: 0.0,
                    prev_positions: [(sx, sy); TRAIL_COUNT],
                    prev_idx: 0,
                    kills: PLAYERS[i].kills, // Preserve kills across rounds
                };
                // Play spawn sound with pan based on x position
                audio::play_spawn(sx / 10.0);
            } else {
                PLAYERS[i].active = false;
            }
        }
    }
}

fn check_wall_collision(x: f32, y_min: f32, y_max: f32) -> bool {
    unsafe {
        for platform in &PLATFORMS {
            if !platform.active {
                continue;
            }

            // Check if x is within platform horizontal bounds
            // and y range overlaps with platform
            if x >= platform.x
                && x <= platform.x + platform.width
                && y_max >= platform.y
                && y_min <= platform.y + platform.height
            {
                return true;
            }
        }
        false
    }
}

pub fn update_player(idx: usize) {
    unsafe {
        let p = &mut PLAYERS[idx];
        if !p.active {
            return;
        }

        // Handle respawn
        if p.dead {
            if p.respawn_timer > 0 {
                p.respawn_timer -= 1;
            } else {
                // Respawn
                p.dead = false;
                p.ammo = MAX_AMMO;
                p.melee_timer = 0;
                p.melee_windup = 0;
                p.spawn_flash = 30; // Spawn flash effect (0.5 seconds at 60fps)
                p.shoot_flash = 0;
                p.squash_stretch = 0.0;

                // Get stage-specific respawn position (random from predefined spawn points)
                let (spawn_x, spawn_y) = crate::stage::get_respawn_position();
                p.x = spawn_x;
                p.y = spawn_y;
                p.vx = 0.0;
                p.vy = 0.0;

                // Reset position history for trails
                for i in 0..TRAIL_COUNT {
                    p.prev_positions[i] = (spawn_x, spawn_y);
                }
                p.prev_idx = 0;

                // Play spawn sound with pan based on x position
                audio::play_spawn(spawn_x / 10.0);
            }
            return;
        }

        // Read input
        let stick_x = left_stick_x(idx as u32);
        let stick_y = left_stick_y(idx as u32);

        // Also check d-pad for digital input
        let dpad_h = if button_held(idx as u32, BUTTON_RIGHT) != 0 {
            1.0
        } else if button_held(idx as u32, BUTTON_LEFT) != 0 {
            -1.0
        } else {
            0.0
        };
        let dpad_v = if button_held(idx as u32, BUTTON_UP) != 0 {
            1.0
        } else if button_held(idx as u32, BUTTON_DOWN) != 0 {
            -1.0
        } else {
            0.0
        };

        // Combine analog and digital
        let input_x = if abs(stick_x) > abs(dpad_h) {
            stick_x
        } else {
            dpad_h
        };
        let input_y = if abs(stick_y) > abs(dpad_v) {
            stick_y
        } else {
            dpad_v
        };

        // Horizontal movement
        let accel = if p.on_ground {
            MOVE_SPEED * 0.15
        } else {
            MOVE_SPEED * 0.08
        };
        p.vx += input_x * accel;

        // Apply friction
        let friction = if p.on_ground { FRICTION } else { AIR_FRICTION };
        p.vx *= friction;

        // Clamp velocity
        p.vx = clamp(p.vx, -MOVE_SPEED, MOVE_SPEED);

        // Update facing direction
        if abs(input_x) > 0.3 {
            p.facing_right = input_x > 0.0;
        }

        // Jump
        let jump_pressed = button_pressed(idx as u32, BUTTON_A) != 0;
        let jump_held = button_held(idx as u32, BUTTON_A) != 0;

        if jump_pressed && p.on_ground {
            p.vy = JUMP_FORCE;
            p.on_ground = false;
            p.squash_stretch = 1.0; // Stretch on jump
                                    // Play jump sound with pan based on x position (-10 to 10 -> -1 to 1)
            audio::play_jump(p.x / 10.0);
        }

        // Wall jump - check if touching wall and not on ground
        if jump_pressed && !p.on_ground {
            let wall_left = check_wall_collision(p.x - 0.1, p.y, p.y + PLAYER_HEIGHT);
            let wall_right =
                check_wall_collision(p.x + PLAYER_WIDTH + 0.1, p.y, p.y + PLAYER_HEIGHT);

            if wall_left {
                p.vy = JUMP_FORCE * 0.9;
                p.vx = MOVE_SPEED * 0.8;
                p.facing_right = true;
                p.squash_stretch = 1.0; // Stretch on wall jump
                                        // Play jump sound with pan based on x position
                audio::play_jump(p.x / 10.0);
            } else if wall_right {
                p.vy = JUMP_FORCE * 0.9;
                p.vx = -MOVE_SPEED * 0.8;
                p.facing_right = false;
                p.squash_stretch = 1.0; // Stretch on wall jump
                                        // Play jump sound with pan based on x position
                audio::play_jump(p.x / 10.0);
            }
        }

        // Wall slide sparks - spawn particles when sliding down a wall
        if !p.on_ground && p.vy < -0.05 {
            let wall_left = check_wall_collision(p.x - 0.1, p.y, p.y + PLAYER_HEIGHT);
            let wall_right =
                check_wall_collision(p.x + PLAYER_WIDTH + 0.1, p.y, p.y + PLAYER_HEIGHT);

            if wall_left {
                crate::particles::spawn_wall_slide_sparks(p.x, p.y + PLAYER_HEIGHT * 0.5, false);
            } else if wall_right {
                crate::particles::spawn_wall_slide_sparks(
                    p.x + PLAYER_WIDTH,
                    p.y + PLAYER_HEIGHT * 0.5,
                    true,
                );
            }
        }

        // Variable jump height
        if !jump_held && p.vy > 0.0 {
            p.vy *= 0.5;
        }

        // Gravity
        p.vy -= GRAVITY;

        // Shoot
        let shoot_pressed = button_pressed(idx as u32, BUTTON_B) != 0;
        if shoot_pressed && p.ammo > 0 && p.melee_timer == 0 && p.melee_windup == 0 {
            spawn_bullet(idx, input_x, input_y);
            p.ammo -= 1;
            p.shoot_flash = SHOOT_FLASH_DURATION; // Trigger muzzle flash
                                                  // Play shoot sound with pan based on x position (-10 to 10 -> -1 to 1)
            audio::play_shoot(p.x / 10.0);
        }

        // Melee (with windup anticipation)
        let melee_pressed = button_pressed(idx as u32, BUTTON_X) != 0;
        if melee_pressed && p.melee_timer == 0 && p.melee_windup == 0 {
            p.melee_windup = MELEE_WINDUP_DURATION; // Start windup phase
        }

        // Handle melee windup -> active transition
        if p.melee_windup > 0 {
            p.melee_windup -= 1;
            if p.melee_windup == 0 {
                // Windup complete, start active melee
                p.melee_timer = MELEE_DURATION;
                // Melee gives a small dash in facing direction
                p.vx += if p.facing_right { 0.15 } else { -0.15 };
            }
        }

        // Update melee timer
        if p.melee_timer > 0 {
            p.melee_timer -= 1;
        }

        // Update spawn flash timer
        if p.spawn_flash > 0 {
            p.spawn_flash -= 1;
        }

        // Update shoot flash timer
        if p.shoot_flash > 0 {
            p.shoot_flash -= 1;
        }

        // Decay squash/stretch toward neutral
        p.squash_stretch *= SQUASH_DECAY;
        if abs(p.squash_stretch) < 0.01 {
            p.squash_stretch = 0.0;
        }

        // Store position in trail history (ring buffer)
        p.prev_positions[p.prev_idx] = (p.x, p.y);
        p.prev_idx = (p.prev_idx + 1) % TRAIL_COUNT;

        // Apply velocity (fixed timestep, no delta_time needed)
        let new_x = p.x + p.vx;
        let new_y = p.y + p.vy;

        // Track if player was grounded before collision check (for landing dust)
        let was_grounded = p.on_ground;

        // Platform collision
        p.on_ground = false;

        for platform in &PLATFORMS {
            if !platform.active {
                continue;
            }

            // Player AABB
            let px = new_x;
            let py = new_y;
            let pw = PLAYER_WIDTH;
            let ph = PLAYER_HEIGHT;

            // Platform AABB
            let plx = platform.x;
            let ply = platform.y;
            let plw = platform.width;
            let plh = platform.height;

            if aabb_overlap((px, py, pw, ph), (plx, ply, plw, plh)) {
                // Landing from above
                if p.vy <= 0.0 && p.y >= ply + plh - 0.2 {
                    p.y = ply + plh;
                    p.vy = 0.0;
                    p.on_ground = true;

                    // Move with platform if it's moving
                    if platform.moving {
                        p.x += platform.move_speed;
                    }
                }
            }
        }

        // Spawn landing dust and squash effect if player just landed
        if !was_grounded && p.on_ground {
            crate::particles::spawn_landing_dust(p.x + PLAYER_WIDTH / 2.0, p.y);
            p.squash_stretch = -1.0; // Squash on landing
        }

        // Update position
        if !p.on_ground || p.vy > 0.0 {
            p.y = new_y;
        }
        p.x = new_x;

        // Level bounds
        p.x = clamp(p.x, -10.0, 10.0 - PLAYER_WIDTH);

        // Fall death (universal - all stages)
        if p.y < DEATH_Y {
            kill_player(idx, idx as u32); // Self-kill (no points)
        }
    }
}

pub fn kill_player(victim_idx: usize, killer_owner: u32) {
    unsafe {
        let victim = &mut PLAYERS[victim_idx];
        if victim.dead {
            return;
        }

        // Play death sound
        audio::play_death();

        // Trigger screen shake on death
        crate::game_state::trigger_shake(0.8);

        // Spawn death particles at victim's center position with their color
        let center_x = victim.x + PLAYER_WIDTH / 2.0;
        let center_y = victim.y + PLAYER_HEIGHT / 2.0;
        crate::particles::spawn_death_particles(center_x, center_y, PLAYER_COLORS[victim_idx]);

        victim.dead = true;
        victim.respawn_timer = RESPAWN_DELAY;

        // Award kill (if not self-kill)
        if killer_owner != victim_idx as u32 {
            let killer = &mut PLAYERS[killer_owner as usize];
            killer.kills += 1;

            // Check for match win
            if killer.kills >= KILLS_TO_WIN {
                GAME_STATE.phase = GamePhase::MatchEnd;
                // Reset match end animation tick
                crate::game_state::reset_match_end_tick();
                // Stop stage music and play victory fanfare
                audio::stop_music();
                audio::play_victory();
                // Spawn victory confetti with winner's color
                crate::particles::spawn_victory_confetti(PLAYER_COLORS[killer_owner as usize]);
            }
        }

        // Brief pause on kill
        if GAME_STATE.phase == GamePhase::Playing {
            GAME_STATE.round_end_timer = 30; // Half second
        }
    }
}
