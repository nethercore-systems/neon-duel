//! NEON DUEL - Platform Fighter for ZX Console
//!
//! A 2-4 player one-hit-kill arena game inspired by Towerfall and Samurai Gunn.
//! Showcases ZX rollback netcode, EPU procedural backgrounds, and matcap rendering.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

// Import ZX FFI bindings
#[path = "../../nethercore/include/zx.rs"]
mod ffi;
use ffi::*;

// =============================================================================
// CONSTANTS
// =============================================================================

// Button indices (from ZX spec)
// 0=UP, 1=DOWN, 2=LEFT, 3=RIGHT, 4=A, 5=B, 6=X, 7=Y, 8=L1, 9=R1, 10=L3, 11=R3, 12=START, 13=SELECT
const BUTTON_UP: u32 = 0;
const BUTTON_DOWN: u32 = 1;
const BUTTON_LEFT: u32 = 2;
const BUTTON_RIGHT: u32 = 3;
const BUTTON_A: u32 = 4;      // Jump
const BUTTON_B: u32 = 5;      // Shoot
const BUTTON_X: u32 = 6;      // Melee
const BUTTON_START: u32 = 12; // Pause/restart

// Billboard modes
const BILLBOARD_CYLINDRICAL_Y: u32 = 2;

// Game constants
const MAX_PLAYERS: usize = 4;
const MAX_BULLETS: usize = 32;

// Physics (tuned for 60fps fixed timestep)
const GRAVITY: f32 = 0.025;
const JUMP_FORCE: f32 = 0.5;
const MOVE_SPEED: f32 = 0.15;
const FRICTION: f32 = 0.85;
const AIR_FRICTION: f32 = 0.95;

// Player dimensions
const PLAYER_WIDTH: f32 = 0.8;
const PLAYER_HEIGHT: f32 = 1.2;

// Combat
const BULLET_SPEED: f32 = 0.4;
const BULLET_LIFETIME: u32 = 120; // 2 seconds at 60fps
const MAX_AMMO: u32 = 3;
const MELEE_DURATION: u32 = 12;   // ticks active
const MELEE_RANGE: f32 = 1.8;
const RESPAWN_DELAY: u32 = 90;    // 1.5 seconds

// World bounds (respawn if player falls below this)
const DEATH_Y: f32 = -8.0;

// Match rules
const KILLS_TO_WIN: u32 = 5;

// Player colors (RGBA)
const PLAYER_COLORS: [u32; 4] = [
    0x00FFFFFF, // Cyan
    0xFF00FFFF, // Magenta
    0xFFFF00FF, // Yellow
    0x00FF00FF, // Green
];

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Clone, Copy)]
struct Player {
    // Position and velocity
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,

    // State
    on_ground: bool,
    facing_right: bool,
    active: bool,

    // Combat
    ammo: u32,
    melee_timer: u32,  // > 0 means melee is active
    dead: bool,
    respawn_timer: u32,

    // Score
    kills: u32,
}

impl Player {
    const fn new() -> Self {
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
            dead: false,
            respawn_timer: 0,
            kills: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct Bullet {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    owner: u32,     // Player index who fired
    lifetime: u32,
    active: bool,
}

impl Bullet {
    const fn new() -> Self {
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

#[derive(Clone, Copy)]
struct Platform {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    active: bool,
    moving: bool,       // For Stage 3 moving platform
    move_speed: f32,
    move_min: f32,
    move_max: f32,
}

impl Platform {
    const fn new() -> Self {
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

#[derive(Clone, Copy, PartialEq)]
enum GamePhase {
    Title,      // Main menu / title screen
    Countdown,  // 3-2-1 before round starts
    Playing,    // Active gameplay
    RoundEnd,   // Someone got a kill, brief pause
    MatchEnd,   // Someone won the match
}

#[derive(Clone, Copy)]
struct GameState {
    phase: GamePhase,
    countdown: u32,
    round_end_timer: u32,
    current_stage: u32,
}

impl GameState {
    const fn new() -> Self {
        Self {
            phase: GamePhase::Title,
            countdown: 180, // 3 seconds
            round_end_timer: 0,
            current_stage: 0,
        }
    }
}

// =============================================================================
// GAME STATE (static for rollback safety)
// =============================================================================

static mut PLAYERS: [Player; MAX_PLAYERS] = [Player::new(); MAX_PLAYERS];
static mut BULLETS: [Bullet; MAX_BULLETS] = [Bullet::new(); MAX_BULLETS];
static mut PLATFORMS: [Platform; 16] = [Platform::new(); 16];
static mut GAME_STATE: GameState = GameState::new();
static mut TICK: u32 = 0;

// Stage data
static mut HAS_PIT: bool = false;
static mut PIT_Y: f32 = -10.0;

// Mesh handles (created in init)
static mut CUBE_MESH: u32 = 0;
static mut CAPSULE_MESH: u32 = 0;
static mut SPHERE_MESH: u32 = 0;
static mut BULLET_MESH: u32 = 0;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min { min } else if v > max { max } else { v }
}

fn abs(v: f32) -> f32 {
    if v < 0.0 { -v } else { v }
}

fn sign(v: f32) -> f32 {
    if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }
}

fn draw_text_str(s: &str, x: f32, y: f32, size: f32) {
    unsafe {
        draw_text(s.as_ptr(), s.len() as u32, x, y, size);
    }
}

// AABB collision
fn aabb_overlap(
    x1: f32, y1: f32, w1: f32, h1: f32,
    x2: f32, y2: f32, w2: f32, h2: f32,
) -> bool {
    x1 < x2 + w2 && x1 + w1 > x2 && y1 < y2 + h2 && y1 + h1 > y2
}

// Point in AABB
fn point_in_aabb(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

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
            x: -10.0, y: -2.0, width: 20.0, height: 0.5, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };

        // Middle platforms (symmetrical)
        PLATFORMS[1] = Platform {
            x: -7.0, y: 1.0, width: 4.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[2] = Platform {
            x: 3.0, y: 1.0, width: 4.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };

        // Top platform
        PLATFORMS[3] = Platform {
            x: -3.0, y: 4.0, width: 6.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
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
            x: -9.0, y: 0.0, width: 4.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[1] = Platform {
            x: -3.0, y: -1.0, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[2] = Platform {
            x: 2.0, y: 0.5, width: 3.5, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[3] = Platform {
            x: 6.0, y: -0.5, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };

        // Upper platforms
        PLATFORMS[4] = Platform {
            x: -6.0, y: 3.0, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[5] = Platform {
            x: 0.0, y: 4.0, width: 4.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[6] = Platform {
            x: 5.0, y: 2.5, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
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
            x: -8.0, y: 0.0, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[1] = Platform {
            x: 5.0, y: 0.0, width: 3.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };

        // Moving platform in center
        PLATFORMS[2] = Platform {
            x: -1.5, y: 1.0, width: 3.0, height: 0.4, active: true,
            moving: true, move_speed: 0.02, move_min: -4.0, move_max: 4.0,
        };

        // Upper corners
        PLATFORMS[3] = Platform {
            x: -7.0, y: 3.5, width: 2.5, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
        PLATFORMS[4] = Platform {
            x: 4.5, y: 3.5, width: 2.5, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };

        // Top center
        PLATFORMS[5] = Platform {
            x: -2.0, y: 5.0, width: 4.0, height: 0.4, active: true,
            moving: false, move_speed: 0.0, move_min: 0.0, move_max: 0.0,
        };
    }
}

fn setup_current_stage() {
    unsafe {
        match GAME_STATE.current_stage {
            0 => setup_stage_grid_arena(),
            1 => setup_stage_scatter_field(),
            2 => setup_stage_ring_void(),
            _ => setup_stage_grid_arena(),
        }
    }
}

// =============================================================================
// GAME INITIALIZATION
// =============================================================================

fn spawn_players() {
    unsafe {
        let count = player_count().min(MAX_PLAYERS as u32) as usize;

        // Spawn positions based on stage
        let spawn_positions: [(f32, f32); 4] = [
            (-6.0, 2.0),
            (6.0, 2.0),
            (-3.0, 5.0),
            (3.0, 5.0),
        ];

        for i in 0..MAX_PLAYERS {
            if i < count {
                let (sx, sy) = spawn_positions[i];
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
                    dead: false,
                    respawn_timer: 0,
                    kills: PLAYERS[i].kills, // Preserve kills across rounds
                };
            } else {
                PLAYERS[i].active = false;
            }
        }
    }
}

fn reset_round() {
    unsafe {
        // Clear bullets
        for b in &mut BULLETS {
            b.active = false;
        }

        // Setup stage and spawn players
        setup_current_stage();
        spawn_players();

        // Start countdown
        GAME_STATE.phase = GamePhase::Countdown;
        GAME_STATE.countdown = 180; // 3 seconds
    }
}

fn reset_match() {
    unsafe {
        // Reset all kills
        for p in &mut PLAYERS {
            p.kills = 0;
        }

        GAME_STATE.current_stage = 0;
        reset_round();
    }
}

// =============================================================================
// INITIALIZATION
// =============================================================================

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        // Dark background
        set_clear_color(0x0a0a1aFF);

        // Create mesh handles for 3D rendering
        CUBE_MESH = cube(1.0, 1.0, 1.0);
        CAPSULE_MESH = capsule(0.4, 0.8, 12, 6);  // Body - pill shape
        SPHERE_MESH = sphere(0.3, 12, 8);         // Head
        BULLET_MESH = sphere(0.1, 8, 6);          // Small bullet sphere

        // Initialize game
        reset_match();
    }
}

// =============================================================================
// UPDATE LOGIC
// =============================================================================

fn update_platforms() {
    unsafe {
        for platform in &mut PLATFORMS {
            if !platform.active || !platform.moving {
                continue;
            }

            // Move platform
            platform.x += platform.move_speed;

            // Reverse at bounds
            if platform.x <= platform.move_min || platform.x + platform.width >= platform.move_max + platform.width {
                platform.move_speed = -platform.move_speed;
            }
        }
    }
}

fn update_player(idx: usize) {
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

                // Random spawn position
                let spawn_x = random_f32_range(-6.0, 6.0);
                p.x = spawn_x;
                p.y = 6.0;
                p.vx = 0.0;
                p.vy = 0.0;
            }
            return;
        }

        // Read input
        let stick_x = left_stick_x(idx as u32);
        let stick_y = left_stick_y(idx as u32);

        // Also check d-pad for digital input
        let dpad_h = if button_held(idx as u32, BUTTON_RIGHT) != 0 { 1.0 }
                     else if button_held(idx as u32, BUTTON_LEFT) != 0 { -1.0 }
                     else { 0.0 };
        let dpad_v = if button_held(idx as u32, BUTTON_UP) != 0 { 1.0 }
                     else if button_held(idx as u32, BUTTON_DOWN) != 0 { -1.0 }
                     else { 0.0 };

        // Combine analog and digital
        let input_x = if abs(stick_x) > abs(dpad_h) { stick_x } else { dpad_h };
        let input_y = if abs(stick_y) > abs(dpad_v) { stick_y } else { dpad_v };

        // Horizontal movement
        let accel = if p.on_ground { MOVE_SPEED * 0.15 } else { MOVE_SPEED * 0.08 };
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
        }

        // Wall jump - check if touching wall and not on ground
        if jump_pressed && !p.on_ground {
            let wall_left = check_wall_collision(p.x - 0.1, p.y, p.y + PLAYER_HEIGHT);
            let wall_right = check_wall_collision(p.x + PLAYER_WIDTH + 0.1, p.y, p.y + PLAYER_HEIGHT);

            if wall_left {
                p.vy = JUMP_FORCE * 0.9;
                p.vx = MOVE_SPEED * 0.8;
                p.facing_right = true;
            } else if wall_right {
                p.vy = JUMP_FORCE * 0.9;
                p.vx = -MOVE_SPEED * 0.8;
                p.facing_right = false;
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
        if shoot_pressed && p.ammo > 0 && p.melee_timer == 0 {
            spawn_bullet(idx, input_x, input_y);
            p.ammo -= 1;
        }

        // Melee
        let melee_pressed = button_pressed(idx as u32, BUTTON_X) != 0;
        if melee_pressed && p.melee_timer == 0 {
            p.melee_timer = MELEE_DURATION;

            // Melee gives a small dash in facing direction
            p.vx += if p.facing_right { 0.15 } else { -0.15 };
        }

        // Update melee timer
        if p.melee_timer > 0 {
            p.melee_timer -= 1;
        }

        // Apply velocity (fixed timestep, no delta_time needed)
        let new_x = p.x + p.vx;
        let new_y = p.y + p.vy;

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

            if aabb_overlap(px, py, pw, ph, plx, ply, plw, plh) {
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

fn check_wall_collision(x: f32, y_min: f32, y_max: f32) -> bool {
    unsafe {
        for platform in &PLATFORMS {
            if !platform.active {
                continue;
            }

            // Check if x is within platform horizontal bounds
            // and y range overlaps with platform
            if x >= platform.x && x <= platform.x + platform.width {
                if y_max >= platform.y && y_min <= platform.y + platform.height {
                    return true;
                }
            }
        }
        false
    }
}

fn spawn_bullet(player_idx: usize, aim_x: f32, aim_y: f32) {
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
            bullet.x = p.x + PLAYER_WIDTH / 2.0;
            bullet.y = p.y + PLAYER_HEIGHT / 2.0;
            bullet.vx = dx * BULLET_SPEED;
            bullet.vy = dy * BULLET_SPEED;
            bullet.owner = player_idx as u32;
            bullet.lifetime = BULLET_LIFETIME;
            bullet.active = true;

            return;
        }
    }
}

fn normalize_aim(x: f32, y: f32, facing_right: bool) -> (f32, f32) {
    // Snap to 8 directions
    let threshold = 0.3;

    let ax = if x > threshold { 1.0 } else if x < -threshold { -1.0 } else { 0.0 };
    let ay = if y > threshold { 1.0 } else if y < -threshold { -1.0 } else { 0.0 };

    // If no direction, use facing direction
    if ax == 0.0 && ay == 0.0 {
        return (if facing_right { 1.0 } else { -1.0 }, 0.0);
    }

    // Normalize diagonal
    if ax != 0.0 && ay != 0.0 {
        let inv_sqrt2 = 0.7071;
        return (ax * inv_sqrt2, ay * inv_sqrt2);
    }

    (ax, ay)
}

fn update_bullets() {
    unsafe {
        for bullet in &mut BULLETS {
            if !bullet.active {
                continue;
            }

            // Move bullet (fixed timestep)
            bullet.x += bullet.vx;
            bullet.y += bullet.vy;

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

                if point_in_aabb(bullet.x, bullet.y, platform.x, platform.y, platform.width, platform.height) {
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
                    let melee_x = player.x + PLAYER_WIDTH / 2.0 + (if player.facing_right { MELEE_RANGE / 2.0 } else { -MELEE_RANGE / 2.0 });
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
                        continue;
                    }
                }

                // Hit detection
                let px = player.x;
                let py = player.y;
                let pw = PLAYER_WIDTH;
                let ph = PLAYER_HEIGHT;

                if point_in_aabb(bullet.x, bullet.y, px, py, pw, ph) {
                    // Kill player
                    kill_player(i, bullet.owner);
                    bullet.active = false;
                    break;
                }
            }
        }
    }
}

fn update_melee_hits() {
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
                    melee_x, melee_y, melee_w, melee_h,
                    target.x, target.y, PLAYER_WIDTH, PLAYER_HEIGHT,
                ) {
                    kill_player(target_idx, attacker_idx as u32);
                }
            }
        }
    }
}

fn kill_player(victim_idx: usize, killer_owner: u32) {
    unsafe {
        let victim = &mut PLAYERS[victim_idx];
        if victim.dead {
            return;
        }

        victim.dead = true;
        victim.respawn_timer = RESPAWN_DELAY;

        // Award kill (if not self-kill)
        if killer_owner != victim_idx as u32 {
            let killer = &mut PLAYERS[killer_owner as usize];
            killer.kills += 1;

            // Check for match win
            if killer.kills >= KILLS_TO_WIN {
                GAME_STATE.phase = GamePhase::MatchEnd;
            }
        }

        // Brief pause on kill
        if GAME_STATE.phase == GamePhase::Playing {
            GAME_STATE.round_end_timer = 30; // Half second
        }
    }
}

#[no_mangle]
pub extern "C" fn update() {
    unsafe {
        TICK += 1;

        match GAME_STATE.phase {
            GamePhase::Title => {
                // Any player pressing A or START begins the game
                for i in 0..player_count() {
                    if button_pressed(i, BUTTON_A) != 0 || button_pressed(i, BUTTON_START) != 0 {
                        reset_match();
                        GAME_STATE.phase = GamePhase::Countdown;
                        return;
                    }
                }
            }

            GamePhase::Countdown => {
                if GAME_STATE.countdown > 0 {
                    GAME_STATE.countdown -= 1;
                } else {
                    GAME_STATE.phase = GamePhase::Playing;
                }
            }

            GamePhase::Playing => {
                update_platforms();

                for i in 0..MAX_PLAYERS {
                    update_player(i);
                }

                update_bullets();
                update_melee_hits();

                // Handle round end timer (brief pause after kill)
                if GAME_STATE.round_end_timer > 0 {
                    GAME_STATE.round_end_timer -= 1;
                }
            }

            GamePhase::RoundEnd => {
                // Currently unused - kills just cause brief pause
            }

            GamePhase::MatchEnd => {
                // Check for restart - go back to title
                for i in 0..MAX_PLAYERS {
                    if PLAYERS[i].active && button_pressed(i as u32, BUTTON_START) != 0 {
                        GAME_STATE.phase = GamePhase::Title;
                        return;
                    }
                }
            }
        }
    }
}

// =============================================================================
// RENDERING
// =============================================================================

fn setup_epu_grid_arena() {
    unsafe {
        // Layer 0: Synthwave grid floor
        // env_lines(layer, variant, line_type, thickness, spacing, fade_distance,
        //           color_primary, color_accent, accent_every, phase)
        env_lines(
            0,                // layer
            0,                // variant (0=Floor)
            2,                // line_type (2=Grid)
            20,               // thickness (0-255)
            0.5,              // spacing
            30.0,             // fade_distance
            0x00FFFFFF,       // color_primary (cyan)
            0x00808080,       // color_accent (dim cyan)
            4,                // accent_every (every 4th line)
            (TICK % 65536) as u32, // phase (scroll animation)
        );

        // Layer 1: Gradient sky
        // env_gradient(layer, zenith, sky_horizon, ground_horizon, nadir, rotation, shift)
        env_gradient(
            1,                // layer
            0x0a001aFF,       // zenith (dark purple-black)
            0x1a0a2eFF,       // sky_horizon (purple)
            0x1a0a2eFF,       // ground_horizon (purple)
            0x0a0a1aFF,       // nadir (near black)
            0.0,              // rotation
            0.0,              // shift
        );
    }
}

fn setup_epu_scatter_field() {
    unsafe {
        // Layer 0: Falling particles
        // env_scatter(layer, variant, density, size, glow, streak_length,
        //             color_primary, color_secondary, parallax_rate, parallax_size, phase)
        env_scatter(
            0,                // layer
            1,                // variant (1=Vertical/rain)
            200,              // density (0-255)
            30,               // size (0-255)
            20,               // glow (0-255)
            10,               // streak_length (0-63)
            0xFFFFFF00,       // color_primary (white)
            0x808080FF,       // color_secondary (gray)
            50,               // parallax_rate
            20,               // parallax_size
            (TICK % 65536) as u32, // phase
        );

        // Layer 1: Orange sunset gradient
        // env_gradient(layer, zenith, sky_horizon, ground_horizon, nadir, rotation, shift)
        env_gradient(
            1,
            0x330000FF,       // zenith (dark red)
            0xFF6600FF,       // sky_horizon (orange)
            0x660000FF,       // ground_horizon (dark orange)
            0x000000FF,       // nadir (black)
            0.0,              // rotation
            0.0,              // shift
        );
    }
}

fn setup_epu_ring_void() {
    unsafe {
        // Layer 0: Pulsing rings
        // env_rings(layer, ring_count, thickness, color_a, color_b,
        //           center_color, center_falloff, spiral_twist,
        //           axis_x, axis_y, axis_z, phase)
        env_rings(
            0,                // layer
            8,                // ring_count
            40,               // thickness (0-255)
            0xFF00FF80,       // color_a (magenta)
            0x8000FF40,       // color_b (purple)
            0xFFFFFFFF,       // center_color (white)
            100,              // center_falloff
            30.0,             // spiral_twist (degrees)
            0.0,              // axis_x
            0.0,              // axis_y
            1.0,              // axis_z (facing camera)
            (TICK % 65536) as u32, // phase (rotation)
        );

        // Layer 1: Dark background
        // env_gradient(layer, zenith, sky_horizon, ground_horizon, nadir, rotation, shift)
        env_gradient(
            1,
            0x000000FF,       // zenith (black)
            0x0a001aFF,       // sky_horizon (very dark purple)
            0x0a001aFF,       // ground_horizon
            0x000000FF,       // nadir (black)
            0.0,              // rotation
            0.0,              // shift
        );
    }
}

fn render_stage() {
    unsafe {
        // Configure EPU based on stage
        match GAME_STATE.current_stage {
            0 => setup_epu_grid_arena(),
            1 => setup_epu_scatter_field(),
            2 => setup_epu_ring_void(),
            _ => setup_epu_grid_arena(),
        }

        // Draw EPU layers
        draw_env();

        // Draw platforms with 3D depth
        for platform in &PLATFORMS {
            if !platform.active {
                continue;
            }

            let px = platform.x + platform.width / 2.0;
            let py = platform.y + platform.height / 2.0;
            let depth = 0.6;  // Platform thickness in Z

            // Main platform body - darker base color
            set_color(0x303050FF);
            push_identity();
            push_translate(px, py, -depth / 2.0);
            push_scale(platform.width, platform.height, depth);
            draw_mesh(CUBE_MESH);

            // Top surface highlight
            set_color(0x505080FF);
            push_identity();
            push_translate(px, py + platform.height * 0.4, 0.0);
            push_scale(platform.width * 0.95, platform.height * 0.2, depth * 1.01);
            draw_mesh(CUBE_MESH);

            // Edge glow for moving platforms
            if platform.moving {
                set_color(0xFF00FF60);  // Magenta glow
                push_identity();
                push_translate(px, py, depth / 2.0 + 0.05);
                push_scale(platform.width * 1.02, platform.height * 1.02, 0.02);
                draw_mesh(CUBE_MESH);
            }
        }
    }
}

fn render_players() {
    unsafe {
        for (i, player) in PLAYERS.iter().enumerate() {
            if !player.active || player.dead {
                continue;
            }

            let center_x = player.x + PLAYER_WIDTH / 2.0;
            let center_y = player.y + PLAYER_HEIGHT / 2.0;

            // Player body (capsule) - main color
            set_color(PLAYER_COLORS[i]);
            push_identity();
            push_translate(center_x, center_y, 0.0);

            // Slight tilt when moving for dynamic feel
            if abs(player.vx) > 0.02 {
                let tilt = player.vx * 5.0;
                push_rotate_z(tilt);
            }

            draw_mesh(CAPSULE_MESH);

            // Player head (sphere) - slightly lighter color
            let head_color = brighten_color(PLAYER_COLORS[i]);
            set_color(head_color);
            push_identity();
            push_translate(center_x, player.y + PLAYER_HEIGHT - 0.1, 0.1);
            draw_mesh(SPHERE_MESH);

            // Draw "eye" indicator for facing direction
            set_color(0xFFFFFFFF);
            push_identity();
            let eye_offset = if player.facing_right { 0.15 } else { -0.15 };
            push_translate(center_x + eye_offset, player.y + PLAYER_HEIGHT, 0.2);
            push_scale(0.08, 0.08, 0.08);
            draw_mesh(SPHERE_MESH);

            // Draw melee slash effect (arc-shaped)
            if player.melee_timer > 0 {
                // Slash brightness based on timer
                let alpha = ((player.melee_timer as f32 / MELEE_DURATION as f32) * 200.0) as u32;
                set_color(0xFFFFFF00 | alpha);

                push_identity();

                let slash_x = if player.facing_right {
                    center_x + 0.6
                } else {
                    center_x - 0.6
                };

                push_translate(slash_x, center_y, 0.15);

                // Rotate slash based on facing
                if !player.facing_right {
                    push_rotate_z(3.14159);
                }

                push_scale(MELEE_RANGE * 0.6, PLAYER_HEIGHT * 0.4, 0.05);
                draw_mesh(CUBE_MESH);
            }

            // Ammo indicator (small spheres above player)
            for a in 0..player.ammo {
                // Alternate colors slightly for visual interest
                let ammo_color = if a % 2 == 0 { 0xFFFF00FF } else { 0xFFDD00FF };
                set_color(ammo_color);
                push_identity();
                push_translate(
                    center_x - 0.25 + (a as f32 * 0.25),
                    player.y + PLAYER_HEIGHT + 0.35,
                    0.1,
                );
                push_scale(0.6, 0.6, 0.6);
                draw_mesh(BULLET_MESH);
            }
        }
    }
}

// Helper to brighten a color for head/highlights
fn brighten_color(color: u32) -> u32 {
    let r = ((color >> 24) & 0xFF).min(255);
    let g = ((color >> 16) & 0xFF).min(255);
    let b = ((color >> 8) & 0xFF).min(255);
    let a = color & 0xFF;

    let r = (r + 40).min(255);
    let g = (g + 40).min(255);
    let b = (b + 40).min(255);

    (r << 24) | (g << 16) | (b << 8) | a
}

fn render_bullets() {
    unsafe {
        for bullet in &BULLETS {
            if !bullet.active {
                continue;
            }

            // Bright yellow bullet with glow effect
            set_color(0xFFFF00FF);
            push_identity();
            push_translate(bullet.x, bullet.y, 0.15);
            push_scale(1.5, 1.5, 1.5);  // Scale up the small bullet mesh
            draw_mesh(BULLET_MESH);

            // Subtle glow/trail behind bullet
            set_color(0xFFFF0060);
            push_identity();
            push_translate(bullet.x - bullet.vx * 0.5, bullet.y - bullet.vy * 0.5, 0.1);
            push_scale(1.0, 1.0, 1.0);
            draw_mesh(BULLET_MESH);
        }
    }
}

fn render_title() {
    unsafe {
        // Title background
        set_color(0x000000DD);
        draw_rect(200.0, 150.0, 560.0, 280.0);

        // Game title
        set_color(0x00FFFFFF); // Cyan
        draw_text_str("NEON DUEL", 320.0, 200.0, 64.0);

        // Subtitle
        set_color(0xFF00FFFF); // Magenta
        draw_text_str("Platform Fighter", 360.0, 270.0, 24.0);

        // Player count
        set_color(0xFFFFFFFF);
        let players_str = match player_count() {
            1 => "1 Player",
            2 => "2 Players",
            3 => "3 Players",
            _ => "4 Players",
        };
        draw_text_str(players_str, 420.0, 320.0, 20.0);

        // Instructions
        set_color(0x00FF00FF); // Green
        draw_text_str("Press A or START to begin", 340.0, 380.0, 18.0);

        // Controls hint
        set_color(0x808080FF);
        draw_text_str("Move: D-Pad/Stick | Jump: A | Shoot: B | Melee: X", 260.0, 420.0, 14.0);
    }
}

fn render_ui() {
    unsafe {
        // Title screen
        if GAME_STATE.phase == GamePhase::Title {
            render_title();
            return;
        }

        // Score display
        set_color(0x000000AA);
        draw_rect(10.0, 10.0, 200.0, 30.0 + (player_count() as f32 * 25.0));

        let mut y = 40.0;
        for (i, player) in PLAYERS.iter().enumerate() {
            if !player.active {
                continue;
            }

            set_color(PLAYER_COLORS[i]);

            // "P1: X" format
            let label = match i {
                0 => "P1:",
                1 => "P2:",
                2 => "P3:",
                _ => "P4:",
            };
            draw_text_str(label, 20.0, y, 18.0);

            // Kill count
            let kills_str = match player.kills {
                0 => "0",
                1 => "1",
                2 => "2",
                3 => "3",
                4 => "4",
                _ => "5",
            };
            draw_text_str(kills_str, 70.0, y, 18.0);

            // Goal indicator
            set_color(0x808080FF);
            draw_text_str("/5", 90.0, y, 18.0);

            y += 25.0;
        }

        // Countdown
        if GAME_STATE.phase == GamePhase::Countdown {
            set_color(0x000000CC);
            draw_rect(400.0, 250.0, 160.0, 80.0);

            set_color(0xFFFFFFFF);
            let seconds = (GAME_STATE.countdown / 60) + 1;
            let countdown_str = match seconds {
                3 => "3",
                2 => "2",
                1 => "1",
                _ => "GO!",
            };
            draw_text_str(countdown_str, 460.0, 290.0, 48.0);
        }

        // Match end
        if GAME_STATE.phase == GamePhase::MatchEnd {
            set_color(0x000000DD);
            draw_rect(250.0, 200.0, 460.0, 140.0);

            // Find winner
            let mut winner_idx = 0;
            for (i, player) in PLAYERS.iter().enumerate() {
                if player.kills >= KILLS_TO_WIN {
                    winner_idx = i;
                    break;
                }
            }

            set_color(PLAYER_COLORS[winner_idx]);
            let winner_text = match winner_idx {
                0 => "PLAYER 1 WINS!",
                1 => "PLAYER 2 WINS!",
                2 => "PLAYER 3 WINS!",
                _ => "PLAYER 4 WINS!",
            };
            draw_text_str(winner_text, 320.0, 250.0, 32.0);

            set_color(0xCCCCCCFF);
            draw_text_str("Press START to play again", 310.0, 300.0, 16.0);
        }
    }
}

#[no_mangle]
pub extern "C" fn render() {
    unsafe {
        // Set camera for side-view
        camera_set(0.0, 2.0, 12.0, 0.0, 2.0, 0.0);
        camera_fov(50.0);

        // Render in order
        render_stage();
        render_players();
        render_bullets();
        render_ui();
    }
}
