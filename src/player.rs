//! Player state and movement
//!
//! Contains Player struct, physics, input handling, and respawn logic.

use crate::audio;
use crate::combat::{spawn_bullet, BULLETS};
use crate::ffi::*;
use crate::game_state::{GamePhase, CONFIG, GAME_STATE};
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
pub const ANALOG_DEADZONE: f32 = 0.2;

// Player dimensions
pub const PLAYER_WIDTH: f32 = 0.8;
pub const PLAYER_HEIGHT: f32 = 1.2;

// Combat
pub const MAX_AMMO: u32 = 3;
pub const MELEE_DURATION: u32 = 12; // ticks active
pub const MELEE_RANGE: f32 = 1.8;
pub const RESPAWN_DELAY: u32 = 90; // 1.5 seconds
pub const SPAWN_INVULN_FRAMES: u32 = 60; // 1 second

// World bounds (respawn if player falls below this)
pub const DEATH_Y: f32 = -8.0;

// Motion trails
pub const TRAIL_COUNT: usize = 5;
pub const TRAIL_VELOCITY_THRESHOLD: f32 = 0.1;

// Squash/stretch
pub const SQUASH_DECAY: f32 = 0.85;

// Shoot flash duration
pub const SHOOT_FLASH_DURATION: u32 = 6;

// Melee windup (anticipation frames)
pub const MELEE_WINDUP_DURATION: u32 = 3;

// Feel polish
pub const JUMP_BUFFER_FRAMES: u32 = 6;
pub const COYOTE_FRAMES: u32 = 6;
pub const DROP_THROUGH_FRAMES: u32 = 10;
pub const FAST_FALL_THRESHOLD: f32 = -0.75;
pub const FAST_FALL_MULT: f32 = 1.75;

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
    pub ready: bool,
    pub is_bot: bool,

    // Combat
    pub ammo: u32,
    pub melee_timer: u32,  // > 0 means melee is active
    pub melee_windup: u32, // Anticipation frames before melee hitbox activates
    pub dead: bool,
    pub respawn_timer: u32,
    pub invuln_timer: u32,

    // Input feel
    pub jump_buffer: u32,
    pub coyote_timer: u32,
    pub drop_timer: u32,

    // Bot state (deterministic)
    pub ai_seed: u32,
    pub ai_shoot_cooldown: u32,
    pub ai_melee_cooldown: u32,
    pub ai_jump_hold: u32,

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
            ready: false,
            is_bot: false,
            ammo: MAX_AMMO,
            melee_timer: 0,
            melee_windup: 0,
            dead: false,
            respawn_timer: 0,
            invuln_timer: 0,
            jump_buffer: 0,
            coyote_timer: 0,
            drop_timer: 0,
            ai_seed: 0,
            ai_shoot_cooldown: 0,
            ai_melee_cooldown: 0,
            ai_jump_hold: 0,
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

fn apply_deadzone(v: f32) -> f32 {
    let av = abs(v);
    if av < ANALOG_DEADZONE {
        0.0
    } else {
        // Rescale so output ramps from 0 at deadzone edge to 1 at max.
        let scaled = (av - ANALOG_DEADZONE) / (1.0 - ANALOG_DEADZONE);
        scaled.min(1.0) * if v < 0.0 { -1.0 } else { 1.0 }
    }
}

#[derive(Clone, Copy)]
struct Controls {
    x: f32,
    y: f32,
    jump_pressed: bool,
    jump_held: bool,
    shoot_pressed: bool,
    melee_pressed: bool,
}

// =============================================================================
// PLAYER LOGIC
// =============================================================================

pub fn spawn_players() {
    unsafe {
        for i in 0..MAX_PLAYERS {
            if PLAYERS[i].active {
                // Get stage-specific spawn position for this player
                let (sx, sy) = crate::stage::get_spawn_position(i);
                let kills = PLAYERS[i].kills;
                let ready = PLAYERS[i].ready;
                let is_bot = PLAYERS[i].is_bot;
                let ai_seed = if PLAYERS[i].ai_seed != 0 {
                    PLAYERS[i].ai_seed
                } else {
                    // Deterministic but varied per slot.
                    (i as u32 + 1).wrapping_mul(1_103_515_245) ^ crate::game_state::TICK
                };
                PLAYERS[i] = Player {
                    x: sx,
                    y: sy,
                    vx: 0.0,
                    vy: 0.0,
                    on_ground: false,
                    facing_right: i % 2 == 0,
                    active: true,
                    ready,
                    is_bot,
                    ammo: MAX_AMMO,
                    melee_timer: 0,
                    melee_windup: 0,
                    dead: false,
                    respawn_timer: 0,
                    invuln_timer: SPAWN_INVULN_FRAMES,
                    jump_buffer: 0,
                    coyote_timer: 0,
                    drop_timer: 0,
                    ai_seed,
                    ai_shoot_cooldown: 0,
                    ai_melee_cooldown: 0,
                    ai_jump_hold: 0,
                    spawn_flash: 30, // Spawn flash effect (0.5 seconds at 60fps)
                    shoot_flash: 0,
                    squash_stretch: 0.0,
                    prev_positions: [(sx, sy); TRAIL_COUNT],
                    prev_idx: 0,
                    kills, // Preserve kills across rounds
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

fn read_human_controls(idx: usize) -> Controls {
    unsafe {
        let stick_x = apply_deadzone(left_stick_x(idx as u32));
        let stick_y = apply_deadzone(left_stick_y(idx as u32));

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

        Controls {
            x: input_x,
            y: input_y,
            jump_pressed: button_pressed(idx as u32, BUTTON_A) != 0,
            jump_held: button_held(idx as u32, BUTTON_A) != 0,
            shoot_pressed: button_pressed(idx as u32, BUTTON_B) != 0,
            melee_pressed: button_pressed(idx as u32, BUTTON_X) != 0,
        }
    }
}

fn ai_controls(idx: usize) -> Controls {
    unsafe {
        let p = &mut PLAYERS[idx];

        // Cooldowns / timers
        p.ai_shoot_cooldown = p.ai_shoot_cooldown.saturating_sub(1);
        p.ai_melee_cooldown = p.ai_melee_cooldown.saturating_sub(1);
        p.ai_jump_hold = p.ai_jump_hold.saturating_sub(1);

        let difficulty = CONFIG.bot_difficulty.min(2);
        let shoot_cd = match difficulty {
            0 => 40,
            1 => 25,
            _ => 15,
        };
        let melee_cd = match difficulty {
            0 => 30,
            1 => 20,
            _ => 12,
        };

        // Pick nearest target
        let px = p.x + PLAYER_WIDTH * 0.5;
        let py = p.y + PLAYER_HEIGHT * 0.5;
        let mut target_idx: Option<usize> = None;
        let mut best_dist_sq = 1.0e12_f32;
        for (i, other) in PLAYERS.iter().enumerate() {
            if i == idx || !other.active || other.dead {
                continue;
            }
            let ox = other.x + PLAYER_WIDTH * 0.5;
            let oy = other.y + PLAYER_HEIGHT * 0.5;
            let dx = ox - px;
            let dy = oy - py;
            let d = dx * dx + dy * dy;
            if d < best_dist_sq {
                best_dist_sq = d;
                target_idx = Some(i);
            }
        }

        // Default: idle
        let mut input_x = 0.0;
        let mut input_y = 0.0;
        let mut jump_pressed = false;
        let mut melee_pressed = false;
        let mut shoot_pressed = false;

        if let Some(ti) = target_idx {
            let t = &PLAYERS[ti];
            let tx = t.x + PLAYER_WIDTH * 0.5;
            let ty = t.y + PLAYER_HEIGHT * 0.5;
            let dx = tx - px;
            let dy = ty - py;

            // Aim toward target (8-way snap happens in spawn_bullet).
            input_x = if abs(dx) > 0.25 {
                if dx > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                0.0
            };
            input_y = if abs(dy) > 0.35 {
                if dy > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                0.0
            };

            // Movement: approach, but add a little strafe to feel less robotic.
            let mode = (crate::game_state::TICK / 45).wrapping_add(p.ai_seed) % 4;
            let prefer_distance = match difficulty {
                0 => 2.8,
                1 => 2.3,
                _ => 1.8,
            };
            let want_away = abs(dx) < prefer_distance && (mode == 1 || mode == 2);
            let move_dir = if want_away {
                if dx > 0.0 {
                    -1.0
                } else {
                    1.0
                }
            } else if abs(dx) > 0.35 {
                if dx > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                0.0
            };
            if input_x == 0.0 {
                input_x = move_dir;
            }

            // Jump to chase verticality.
            if p.on_ground && dy > 1.0 && abs(dx) < 5.0 {
                jump_pressed = true;
                p.ai_jump_hold = 8;
            }

            // Defensive parry: if an enemy bullet is close, swing.
            if p.ai_melee_cooldown == 0 {
                let mut bullet_threat = false;
                for b in &BULLETS {
                    if !b.active || b.owner == idx as u32 {
                        continue;
                    }
                    let ddx = b.x - px;
                    let ddy = b.y - py;
                    let dist_sq = ddx * ddx + ddy * ddy;
                    let r = MELEE_RANGE * 1.15;
                    if dist_sq < r * r {
                        bullet_threat = true;
                        break;
                    }
                }
                if bullet_threat {
                    melee_pressed = true;
                    p.ai_melee_cooldown = melee_cd;
                }
            }

            // Offensive melee when close.
            if !melee_pressed && p.ai_melee_cooldown == 0 && abs(dx) < 1.7 && abs(dy) < 1.2 {
                melee_pressed = true;
                p.ai_melee_cooldown = melee_cd;
            }

            // Shoot when not in melee and target is reasonably aligned.
            if p.ai_shoot_cooldown == 0
                && p.ammo > 0
                && p.melee_timer == 0
                && p.melee_windup == 0
                && abs(dx) < 10.0
                && abs(dy) < 6.0
                && !melee_pressed
            {
                // Easy bots whiff more by requiring clearer alignment.
                let aim_ok = match difficulty {
                    0 => abs(dx) > 1.5 || abs(dy) > 1.5,
                    1 => abs(dx) > 0.9 || abs(dy) > 0.9,
                    _ => true,
                };
                if aim_ok {
                    shoot_pressed = true;
                    p.ai_shoot_cooldown = shoot_cd;
                }
            }
        }

        Controls {
            x: input_x,
            y: input_y,
            jump_pressed,
            jump_held: p.ai_jump_hold > 0,
            shoot_pressed,
            melee_pressed,
        }
    }
}

fn read_controls(idx: usize) -> Controls {
    unsafe {
        if PLAYERS[idx].is_bot {
            ai_controls(idx)
        } else {
            read_human_controls(idx)
        }
    }
}

fn choose_safe_respawn_position(player_idx: usize) -> (f32, f32) {
    unsafe {
        let stage = (GAME_STATE.current_stage as usize).min(2);

        let preferred = ((crate::game_state::TICK / 30) as usize + player_idx) % 4;
        let mut best_score = -1.0_f32;
        let mut best = (
            crate::stage::SPAWN_POINTS[stage][0],
            crate::stage::SPAWN_POINTS[stage][1],
        );

        for k in 0..4 {
            let si = (preferred + k) % 4;
            let sx = crate::stage::SPAWN_POINTS[stage][si * 2];
            let sy = crate::stage::SPAWN_POINTS[stage][si * 2 + 1];

            let scx = sx + PLAYER_WIDTH * 0.5;
            let scy = sy + PLAYER_HEIGHT * 0.5;

            // Closest living opponent
            let mut min_player = 1.0e12_f32;
            for (i, other) in PLAYERS.iter().enumerate() {
                if i == player_idx || !other.active || other.dead {
                    continue;
                }
                let ox = other.x + PLAYER_WIDTH * 0.5;
                let oy = other.y + PLAYER_HEIGHT * 0.5;
                let dx = ox - scx;
                let dy = oy - scy;
                let d = dx * dx + dy * dy;
                if d < min_player {
                    min_player = d;
                }
            }

            // Closest active bullet
            let mut min_bullet = 1.0e12_f32;
            for b in &BULLETS {
                if !b.active {
                    continue;
                }
                let dx = b.x - scx;
                let dy = b.y - scy;
                let d = dx * dx + dy * dy;
                if d < min_bullet {
                    min_bullet = d;
                }
            }

            // Maximize the nearest threat distance (bullet or opponent).
            let score = if min_player < min_bullet {
                min_player
            } else {
                min_bullet
            };
            if score > best_score {
                best_score = score;
                best = (sx, sy);
            }
        }

        best
    }
}

fn overtime_killer_for(victim_idx: usize) -> u32 {
    unsafe {
        let vx = PLAYERS[victim_idx].x + PLAYER_WIDTH * 0.5;
        let vy = PLAYERS[victim_idx].y + PLAYER_HEIGHT * 0.5;
        let mut best_idx = victim_idx as u32;
        let mut best_dist_sq = 1.0e12_f32;
        for (i, other) in PLAYERS.iter().enumerate() {
            if i == victim_idx || !other.active || other.dead {
                continue;
            }
            let ox = other.x + PLAYER_WIDTH * 0.5;
            let oy = other.y + PLAYER_HEIGHT * 0.5;
            let dx = ox - vx;
            let dy = oy - vy;
            let d = dx * dx + dy * dy;
            if d < best_dist_sq {
                best_dist_sq = d;
                best_idx = i as u32;
            }
        }
        best_idx
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
                p.invuln_timer = SPAWN_INVULN_FRAMES;
                p.jump_buffer = 0;
                p.coyote_timer = 0;
                p.drop_timer = 0;
                p.ai_jump_hold = 0;

                // Safe respawn position (avoid bullets/players) for spawn-camp protection.
                let (spawn_x, spawn_y) = choose_safe_respawn_position(idx);
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

        // Decrement spawn invulnerability timer
        p.invuln_timer = p.invuln_timer.saturating_sub(1);

        // Read controls (human or bot)
        let c = read_controls(idx);
        let input_x = c.x;
        let input_y = c.y;

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

        // Coyote time bookkeeping
        if p.on_ground {
            p.coyote_timer = COYOTE_FRAMES;
        } else {
            p.coyote_timer = p.coyote_timer.saturating_sub(1);
        }

        // Jump buffering
        if c.jump_pressed {
            p.jump_buffer = JUMP_BUFFER_FRAMES;
        } else {
            p.jump_buffer = p.jump_buffer.saturating_sub(1);
        }

        // Drop-through (down + jump)
        if p.on_ground && c.jump_pressed && input_y < -0.6 {
            p.drop_timer = DROP_THROUGH_FRAMES;
            p.jump_buffer = 0;
            p.on_ground = false;
            p.vy = -0.05;
        }

        // Buffered jump (includes coyote)
        let can_jump = p.on_ground || p.coyote_timer > 0;
        if p.jump_buffer > 0 && can_jump && p.drop_timer == 0 {
            p.vy = JUMP_FORCE;
            p.on_ground = false;
            p.coyote_timer = 0;
            p.jump_buffer = 0;
            p.squash_stretch = 1.0; // Stretch on jump
            audio::play_jump(p.x / 10.0);
        }

        // Wall jump - check if touching wall and not on ground
        if c.jump_pressed && !p.on_ground && p.drop_timer == 0 {
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
        if !c.jump_held && p.vy > 0.0 {
            p.vy *= 0.5;
        }

        // Gravity
        p.vy -= GRAVITY;
        if !p.on_ground && input_y < FAST_FALL_THRESHOLD {
            p.vy -= GRAVITY * (FAST_FALL_MULT - 1.0);
        }

        // Shoot
        if c.shoot_pressed && p.ammo > 0 && p.melee_timer == 0 && p.melee_windup == 0 {
            spawn_bullet(idx, input_x, input_y);
            p.ammo -= 1;
            p.shoot_flash = SHOOT_FLASH_DURATION; // Trigger muzzle flash
                                                  // Play shoot sound with pan based on x position (-10 to 10 -> -1 to 1)
            audio::play_shoot(p.x / 10.0);
        }

        // Melee (with windup anticipation)
        if c.melee_pressed && p.melee_timer == 0 && p.melee_windup == 0 {
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
        let mut new_x = p.x + p.vx;
        let new_y = p.y + p.vy;

        // Track if player was grounded before collision check (for landing dust)
        let was_grounded = p.on_ground;

        // Platform collision
        p.on_ground = false;

        for platform in &PLATFORMS {
            if !platform.active {
                continue;
            }
            if p.drop_timer > 0 {
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
                        new_x += platform.move_speed;
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

        // Drop-through timer
        p.drop_timer = p.drop_timer.saturating_sub(1);

        // Level bounds (dynamic during overtime)
        let left = GAME_STATE.arena_left;
        let right = GAME_STATE.arena_right - PLAYER_WIDTH;
        let mut hit_wall = false;
        if p.x < left {
            p.x = left;
            hit_wall = true;
        } else if p.x > right {
            p.x = right;
            hit_wall = true;
        }

        // Overtime walls are lethal (awards point to closest opponent to keep matches moving).
        if GAME_STATE.overtime && hit_wall {
            let killer = overtime_killer_for(idx);
            kill_player(idx, killer);
            return;
        }

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
        victim.invuln_timer = 0;

        // Award kill (if not self-kill)
        if killer_owner != victim_idx as u32 {
            let killer = &mut PLAYERS[killer_owner as usize];
            killer.kills += 1;

            // Check for match win
            let kills_to_win = CONFIG.kills_to_win.max(1);
            if killer.kills >= kills_to_win {
                GAME_STATE.winner_idx = killer_owner.min(3);
                GAME_STATE.final_ko_timer = 75;
                GAME_STATE.round_end_timer = 0;
                GAME_STATE.phase = GamePhase::FinalKo;

                // Stop stage music; victory fanfare plays after the slow-mo beat.
                audio::stop_music();

                // Stronger final hit impact.
                crate::game_state::trigger_hit_freeze(12);
                crate::game_state::trigger_shake(1.0);
            }
        }

        // Brief pause on kill
        if GAME_STATE.phase == GamePhase::Playing {
            crate::game_state::start_transition_out();
            GAME_STATE.round_end_timer = 30; // Half second
        }
    }
}
