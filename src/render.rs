//! Rendering system
//!
//! Contains all rendering code: EPU setup, stage/player/bullet rendering, and UI.

use crate::combat::BULLETS;
use crate::ffi::*;
use crate::game_state::{
    GamePhase, PausePage, TransitionPhase, CONFIG, DEFLECT_PLAYER, DEFLECT_POPUP_TICKS,
    EFFECT_LIGHTS, GAME_STATE, IMPACT_FLASH, LOBBY_INDEX, MATCH_END_TICK, OPTIONS, PAUSE_INDEX,
    PAUSE_PAGE, ROUND_NUMBER, STAGE_SELECT_RANDOM, STAGE_SELECT_ROTATE, TICK, TRANSITION_PHASE,
    TRANSITION_PROGRESS,
};
use crate::particles::PARTICLES;
use crate::player::{
    abs, MELEE_DURATION, MELEE_WINDUP_DURATION, PLAYERS, PLAYER_COLORS, PLAYER_HEIGHT,
    PLAYER_WIDTH, SPAWN_INVULN_FRAMES, TRAIL_COUNT, TRAIL_VELOCITY_THRESHOLD,
};
use crate::stage::PLATFORMS;

// =============================================================================
// CONSTANTS
// =============================================================================

// Billboard modes
#[allow(dead_code)]
const BILLBOARD_CYLINDRICAL_Y: u32 = 2;

// =============================================================================
// MESH HANDLES
// =============================================================================

pub static mut CUBE_MESH: u32 = 0;
pub static mut CAPSULE_MESH: u32 = 0;
pub static mut SPHERE_MESH: u32 = 0;
pub static mut BULLET_MESH: u32 = 0;

// =============================================================================
// INITIALIZATION
// =============================================================================

pub fn init_meshes() {
    unsafe {
        CUBE_MESH = cube(1.0, 1.0, 1.0);
        CAPSULE_MESH = capsule(0.4, 0.8, 12, 6); // Body - pill shape
        SPHERE_MESH = sphere(0.3, 12, 8); // Head
        BULLET_MESH = sphere(0.1, 8, 6); // Small bullet sphere
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

pub fn draw_text_str(s: &str, x: f32, y: f32, size: f32) {
    unsafe {
        draw_text(s.as_ptr(), s.len() as u32, x, y, size);
    }
}

/// Helper to brighten a color for head/highlights
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

/// Helper to dim a color (reduce brightness)
fn dim_color(color: u32, factor: f32) -> u32 {
    let r = ((color >> 24) & 0xFF) as f32;
    let g = ((color >> 16) & 0xFF) as f32;
    let b = ((color >> 8) & 0xFF) as f32;
    let a = color & 0xFF;

    let r = (r * factor) as u32;
    let g = (g * factor) as u32;
    let b = (b * factor) as u32;

    (r << 24) | (g << 16) | (b << 8) | a
}

/// Helper to set alpha on a color
fn with_alpha(color: u32, alpha: u32) -> u32 {
    (color & 0xFFFFFF00) | (alpha & 0xFF)
}

/// Ease out bounce for animations
fn ease_out_bounce(t: f32) -> f32 {
    if t < 0.5 {
        // Overshoot then settle
        let tt = t * 2.0;
        1.0 + libm::sinf(tt * core::f32::consts::PI) * 0.3
    } else {
        1.0
    }
}

fn stage_name(stage: u32) -> &'static str {
    match stage {
        0 => "GRID ARENA",
        1 => "SCATTER FIELD",
        2 => "RING VOID",
        _ => "ARENA",
    }
}

fn stage_select_label(sel: u32) -> &'static str {
    if sel == STAGE_SELECT_RANDOM {
        "RANDOM"
    } else if sel == STAGE_SELECT_ROTATE {
        "ROTATE"
    } else {
        stage_name(sel)
    }
}

fn u32_to_str(mut v: u32, buf: &mut [u8; 10]) -> &str {
    if v == 0 {
        buf[0] = b'0';
        // SAFETY: single ASCII digit.
        return unsafe { core::str::from_utf8_unchecked(&buf[..1]) };
    }

    let mut tmp = [0u8; 10];
    let mut len = 0usize;
    while v > 0 && len < tmp.len() {
        tmp[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }

    for i in 0..len {
        buf[i] = tmp[len - 1 - i];
    }

    // SAFETY: ASCII digits only.
    unsafe { core::str::from_utf8_unchecked(&buf[..len]) }
}

fn mmss_to_str(total_secs: u32, buf: &mut [u8; 6]) -> &str {
    let minutes = (total_secs / 60).min(99);
    let seconds = total_secs % 60;
    buf[0] = b'0' + ((minutes / 10) as u8);
    buf[1] = b'0' + ((minutes % 10) as u8);
    buf[2] = b':';
    buf[3] = b'0' + ((seconds / 10) as u8);
    buf[4] = b'0' + ((seconds % 10) as u8);
    // SAFETY: fixed ASCII pattern "MM:SS".
    unsafe { core::str::from_utf8_unchecked(&buf[..5]) }
}

// =============================================================================
// EPU SETUP
// =============================================================================

fn setup_epu_grid_arena() {
    unsafe {
        env_blend(3); // Screen blend for the overlay glow

        // Layer 0: Gradient sky
        env_gradient(
            0,          // layer
            0x0a001aff, // zenith (dark purple-black)
            0x1a0a2eff, // sky_horizon (purple)
            0x1a0a2eff, // ground_horizon (purple)
            0x0a0a1aff, // nadir (near black)
            0.0,        // rotation
            0.0,        // shift
            0.0,        // sun_elevation (no sun)
            0,          // sun_disk
            0,          // sun_halo
            0,          // sun_intensity
            0,          // horizon_haze
            0,          // sun_warmth
            0,          // cloudiness
            0,          // cloud_phase
        );

        // Layer 1: Synthwave grid floor
        env_lines(
            1,            // layer
            0,            // variant (0=Floor)
            2,            // line_type (2=Grid)
            20,           // thickness (0-255)
            0.75,         // spacing
            45.0,         // fade_distance
            96,           // parallax (also selects depth slices)
            0x00FFFFFF,   // color_primary (cyan)
            0x40FFFFFF,   // color_accent (cyan glow)
            4,            // accent_every (every 4th line)
            TICK % 65536, // phase (scroll animation)
            0,            // profile (0=Grid)
            24,           // warp
            0,            // wobble
            128,          // glow
            0.0,          // axis_x
            0.0,          // axis_y
            1.0,          // axis_z
            0,            // seed (auto)
        );
    }
}

fn setup_epu_scatter_field() {
    unsafe {
        env_blend(0); // Alpha

        // Layer 0: Orange sunset gradient
        env_gradient(
            0,          // layer
            0x330000FF, // zenith (dark red)
            0xFF6600FF, // sky_horizon (orange)
            0x660000FF, // ground_horizon (dark orange)
            0x000000FF, // nadir (black)
            0.0,        // rotation
            0.0,        // shift
            0.3,        // sun_elevation (low sun for sunset)
            100,        // sun_disk
            150,        // sun_halo
            200,        // sun_intensity
            100,        // horizon_haze
            200,        // sun_warmth
            0,          // cloudiness
            0,          // cloud_phase
        );

        // Layer 1: Falling particles (Cells family 0, variant 1 = rain)
        env_cells(
            1,            // layer
            0,            // family (0=Particles)
            1,            // variant (1=Rain)
            200,          // density
            2,            // size_min
            10,           // size_max
            200,          // intensity
            220,          // shape
            96,           // motion
            140,          // parallax
            120,          // height_bias
            60,           // clustering
            0xFFFFFFFF,   // color_a
            0x808080FF,   // color_b
            0.0,          // axis_x
            0.0,          // axis_y
            1.0,          // axis_z
            TICK % 65536, // phase
            0,            // seed (auto)
        );
    }
}

fn setup_epu_ring_void() {
    unsafe {
        env_blend(1); // Additive for a punchy portal

        // Layer 0: Dark background
        env_gradient(
            0,          // layer
            0x000000FF, // zenith (black)
            0x0a001aFF, // sky_horizon (very dark purple)
            0x0a001aFF, // ground_horizon
            0x000000FF, // nadir (black)
            0.0,        // rotation
            0.0,        // shift
            0.0,        // sun_elevation (no sun)
            0,          // sun_disk
            0,          // sun_halo
            0,          // sun_intensity
            0,          // horizon_haze
            0,          // sun_warmth
            0,          // cloudiness
            0,          // cloud_phase
        );

        // Layer 1: Pulsing rings
        env_rings(
            1,                    // layer
            0,                    // family (0=Portal)
            8,                    // ring_count
            40,                   // thickness (0-255)
            0xFF00FF80,           // color_a (magenta)
            0x8000FF40,           // color_b (purple)
            0xFFFFFFFF,           // center_color (white)
            100,                  // center_falloff
            30.0,                 // spiral_twist (degrees)
            0.0,                  // axis_x
            0.0,                  // axis_y
            1.0,                  // axis_z (facing camera)
            TICK % 65536,         // phase (rotation)
            (TICK * 137) % 65536, // wobble
            32,                   // noise
            24,                   // dash
            160,                  // glow
            41,                   // seed
        );
    }
}

// =============================================================================
// STAGE RENDERING
// =============================================================================

pub fn render_stage() {
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
            let depth = 0.6; // Platform thickness in Z

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
                set_color(0xFF00FF60); // Magenta glow
                push_identity();
                push_translate(px, py, depth / 2.0 + 0.05);
                push_scale(platform.width * 1.02, platform.height * 1.02, 0.02);
                draw_mesh(CUBE_MESH);
            }
        }

        // Overtime: lethal neon walls close in.
        if GAME_STATE.overtime {
            let left = GAME_STATE.arena_left;
            let right = GAME_STATE.arena_right;
            let wall_w = 0.25;
            let wall_h = 18.0;
            let wall_y = 2.0;

            // Core wall
            set_color(0xFF0000CC);
            push_identity();
            push_translate(left + wall_w * 0.5, wall_y, 0.1);
            push_scale(wall_w, wall_h, 0.4);
            draw_mesh(CUBE_MESH);

            push_identity();
            push_translate(right - wall_w * 0.5, wall_y, 0.1);
            push_scale(wall_w, wall_h, 0.4);
            draw_mesh(CUBE_MESH);

            // Glow shell
            set_color(0xFF004060);
            push_identity();
            push_translate(left + wall_w * 0.5, wall_y, 0.35);
            push_scale(wall_w * 2.5, wall_h * 1.02, 0.05);
            draw_mesh(CUBE_MESH);

            push_identity();
            push_translate(right - wall_w * 0.5, wall_y, 0.35);
            push_scale(wall_w * 2.5, wall_h * 1.02, 0.05);
            draw_mesh(CUBE_MESH);
        }
    }
}

// =============================================================================
// PLAYER RENDERING
// =============================================================================

pub fn render_players() {
    unsafe {
        for (i, player) in PLAYERS.iter().enumerate() {
            if !player.active || player.dead {
                continue;
            }

            let center_x = player.x + PLAYER_WIDTH / 2.0;
            let center_y = player.y + PLAYER_HEIGHT / 2.0;

            // --- Motion trails (afterimages) ---
            let speed = libm::sqrtf(player.vx * player.vx + player.vy * player.vy);
            if speed > TRAIL_VELOCITY_THRESHOLD {
                // Render ghost images at previous positions
                for t in 0..TRAIL_COUNT {
                    // Get position from ring buffer (oldest to newest)
                    let idx = (player.prev_idx + TRAIL_COUNT - 1 - t) % TRAIL_COUNT;
                    let (px, py) = player.prev_positions[idx];

                    // Skip if too close to current position
                    let dx = px - player.x;
                    let dy = py - player.y;
                    if dx * dx + dy * dy < 0.01 {
                        continue;
                    }

                    // Fade based on trail index (older = more transparent)
                    let alpha = ((TRAIL_COUNT - t) as f32 / TRAIL_COUNT as f32 * 80.0) as u32;
                    let trail_color = with_alpha(dim_color(PLAYER_COLORS[i], 0.6), alpha);

                    set_color(trail_color);
                    push_identity();
                    push_translate(px + PLAYER_WIDTH / 2.0, py + PLAYER_HEIGHT / 2.0, -0.2);
                    push_scale(0.9, 0.9, 0.9); // Slightly smaller
                    draw_mesh(CAPSULE_MESH);
                }
            }

            // --- Squash/stretch transform ---
            let stretch_y = 1.0 + player.squash_stretch * 0.3; // -0.3 to +0.3
            let stretch_x = 1.0 - player.squash_stretch * 0.2; // Inverse for volume preservation

            // --- Respawn invincibility blink ---
            let visible = if player.invuln_timer > 0 {
                // Blink every 4 frames
                (player.invuln_timer / 4) % 2 == 0
            } else {
                true
            };

            // --- Melee windup pose ---
            let windup_tilt = if player.melee_windup > 0 {
                // Lean backward during windup
                let progress = player.melee_windup as f32 / MELEE_WINDUP_DURATION as f32;
                let direction = if player.facing_right { -1.0 } else { 1.0 };
                progress * 0.4 * direction
            } else {
                0.0
            };

            if visible {
                // Player body (capsule) - main color with scale pulse during spawn
                let scale_pulse = if player.spawn_flash > 0 {
                    1.0 + libm::sinf(player.spawn_flash as f32 * 0.5) * 0.05
                } else {
                    1.0
                };

                set_color(PLAYER_COLORS[i]);
                push_identity();
                push_translate(center_x, center_y, 0.0);

                // Apply squash/stretch
                push_scale(stretch_x * scale_pulse, stretch_y * scale_pulse, 1.0);

                // Slight tilt when moving + windup tilt
                let move_tilt = if abs(player.vx) > 0.02 {
                    player.vx * 5.0
                } else {
                    0.0
                };
                push_rotate_z(move_tilt + windup_tilt);

                draw_mesh(CAPSULE_MESH);

                // Player head (sphere) - slightly lighter color
                let head_color = brighten_color(PLAYER_COLORS[i]);
                set_color(head_color);
                push_identity();
                // Adjust head position for squash/stretch
                let head_y = player.y + PLAYER_HEIGHT * stretch_y - 0.1;
                push_translate(center_x, head_y, 0.1);
                push_scale(scale_pulse, scale_pulse, scale_pulse);
                draw_mesh(SPHERE_MESH);

                // Draw "eye" indicator for facing direction
                set_color(0xFFFFFFFF);
                push_identity();
                let eye_offset = if player.facing_right { 0.15 } else { -0.15 };
                push_translate(center_x + eye_offset, head_y + 0.1, 0.2);
                push_scale(0.08, 0.08, 0.08);
                draw_mesh(SPHERE_MESH);
            }

            // --- Muzzle flash effect ---
            if player.shoot_flash > 0 {
                let flash_progress = player.shoot_flash as f32 / 6.0; // 1.0 to 0.0
                let flash_alpha = (flash_progress * 255.0) as u32;

                // Direction of shot
                let dir_x = if player.facing_right { 1.0 } else { -1.0 };
                let flash_x = center_x + dir_x * 0.5;

                // Expanding ring
                let ring_scale = 0.3 + (1.0 - flash_progress) * 0.5;
                set_color(0xFFFF0000 | flash_alpha);
                push_identity();
                push_translate(flash_x, center_y, 0.3);
                push_scale(ring_scale, ring_scale, 0.1);
                draw_mesh(SPHERE_MESH);

                // Core bright flash
                set_color(0xFFFFFF00 | flash_alpha);
                push_identity();
                push_translate(flash_x, center_y, 0.35);
                push_scale(0.2 * flash_progress, 0.2 * flash_progress, 0.1);
                draw_mesh(SPHERE_MESH);
            }

            // --- Melee windup indicator ---
            if player.melee_windup > 0 {
                let progress = 1.0 - (player.melee_windup as f32 / MELEE_WINDUP_DURATION as f32);
                let alpha = ((1.0 - progress) * 150.0) as u32;
                let facing = if player.facing_right { 1.0 } else { -1.0 };

                // Draw charging arc
                set_color(0xFFFFFF00 | alpha);
                push_identity();
                push_translate(center_x - facing * 0.3, center_y, 0.15);
                push_rotate_z(-0.5 * facing);
                push_scale(0.8, 0.15, 0.1);
                draw_mesh(CUBE_MESH);
            }

            // Draw melee slash effect (animated arc sweep) - only when active (not windup)
            if player.melee_timer > 0 {
                let progress = 1.0 - (player.melee_timer as f32 / MELEE_DURATION as f32);
                let slash_alpha = ((1.0 - progress) * 255.0) as u32;

                // Sweep angle based on progress (arc from -45 to +45 degrees)
                let start_angle: f32 = -0.785; // -45 degrees
                let end_angle: f32 = 0.785; // +45 degrees
                let current_angle = start_angle + progress * (end_angle - start_angle);

                // Facing direction multiplier
                let facing = if player.facing_right { 1.0 } else { -1.0 };

                // Multiple slash lines for thickness
                for offset in [-0.1_f32, 0.0, 0.1].iter() {
                    push_identity();
                    push_translate(center_x + facing * 0.5, center_y, 0.2);
                    push_rotate_z(current_angle * facing + offset * facing);
                    push_scale(1.5, 0.1, 0.1); // Long thin slash
                    set_color(0xFFFFFF00 | slash_alpha);
                    draw_mesh(CUBE_MESH);
                }
            }

            // Spawn flash effect - glowing ring/aura
            if player.spawn_flash > 0 {
                let flash_progress = player.spawn_flash as f32 / 30.0; // 1.0 to 0.0
                let flash_alpha = (flash_progress * 200.0) as u32;
                let flash_color = (PLAYER_COLORS[i] & 0xFFFFFF00) | flash_alpha;

                // Draw expanding ring around player
                push_identity();
                push_translate(center_x, center_y, 0.1);
                let ring_scale = 1.5 - flash_progress * 0.5; // Expands from 1.0 to 1.5
                push_scale(ring_scale, ring_scale, 0.1);
                set_color(flash_color);
                draw_mesh(SPHERE_MESH);
            }

            // Invulnerability aura (subtle cyan shimmer)
            if player.invuln_timer > 0 {
                let t = player.invuln_timer as f32 / SPAWN_INVULN_FRAMES.max(1) as f32;
                let pulse = libm::sinf(TICK as f32 * 0.25) * 0.08 + 1.0;
                let alpha = (t * 100.0) as u32;
                set_color(with_alpha(0x00FFFFFF, alpha));
                push_identity();
                push_translate(center_x, center_y, 0.05);
                push_scale(1.25 * pulse, 1.35 * pulse, 0.12);
                draw_mesh(SPHERE_MESH);
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

// =============================================================================
// BULLET RENDERING
// =============================================================================

pub fn render_bullets() {
    unsafe {
        for bullet in &BULLETS {
            if !bullet.active {
                continue;
            }

            // Bright yellow bullet with glow effect
            set_color(0xFFFF00FF);
            push_identity();
            push_translate(bullet.x, bullet.y, 0.15);
            push_scale(1.5, 1.5, 1.5); // Scale up the small bullet mesh
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

// =============================================================================
// PARTICLE RENDERING
// =============================================================================

/// Render all active particles
pub fn render_particles() {
    unsafe {
        for p in &PARTICLES {
            if p.active {
                // Calculate alpha based on remaining lifetime
                let alpha = (p.lifetime as f32 / p.max_lifetime as f32 * 255.0) as u32;
                let color = (p.color & 0xFFFFFF00) | alpha;

                push_identity();
                push_translate(p.x, p.y, 0.0);
                let scale = p.size * (0.5 + p.lifetime as f32 / p.max_lifetime as f32 * 0.5);
                push_scale(scale, scale, scale);
                set_color(color);
                draw_mesh(SPHERE_MESH);
            }
        }
    }
}

// =============================================================================
// EFFECT LIGHTS RENDERING
// =============================================================================

/// Apply active effect lights to the scene
pub fn apply_effect_lights() {
    unsafe {
        for (idx, light) in EFFECT_LIGHTS.iter().enumerate() {
            if light.active && idx < 4 {
                // Set point light
                light_set_point(idx as u32, light.x, light.y, light.z);
                light_color(idx as u32, light.color);
                light_intensity(idx as u32, light.intensity);
                light_range(idx as u32, 5.0); // Effect range
            } else if idx < 4 {
                // Disable inactive light
                light_intensity(idx as u32, 0.0);
            }
        }
    }
}

// =============================================================================
// UI RENDERING
// =============================================================================

fn render_title() {
    unsafe {
        // EPU background for title screen (use grid arena style)
        setup_epu_grid_arena();
        draw_env();

        // Semi-transparent overlay for readability
        set_color(0x000000AA);
        draw_rect(200.0, 150.0, 560.0, 280.0);

        // Animated title - pulse/breathe effect
        let pulse = libm::sinf(TICK as f32 * 0.1) * 0.1 + 1.0; // 1.0 +/- 0.1
        let title_size = 64.0 * pulse;

        // Title glow effect - draw multiple times with decreasing alpha
        // Outer glow (cyan, low alpha)
        set_color(0x00FFFF30);
        draw_text_str("NEON DUEL", 316.0, 196.0, title_size + 6.0);

        // Middle glow
        set_color(0x00FFFF60);
        draw_text_str("NEON DUEL", 318.0, 198.0, title_size + 3.0);

        // Inner glow
        set_color(0x00FFFFAA);
        draw_text_str("NEON DUEL", 319.0, 199.0, title_size + 1.0);

        // Main title text (bright cyan)
        set_color(0x00FFFFFF);
        draw_text_str("NEON DUEL", 320.0, 200.0, title_size);

        // Subtitle with slight pulse
        let subtitle_pulse = libm::sinf(TICK as f32 * 0.08 + 1.0) * 0.05 + 1.0;
        let subtitle_size = 24.0 * subtitle_pulse;

        // Subtitle glow
        set_color(0xFF00FF60);
        draw_text_str("Platform Fighter", 359.0, 269.0, subtitle_size + 2.0);
        set_color(0xFF00FFFF); // Magenta
        draw_text_str("Platform Fighter", 360.0, 270.0, subtitle_size);

        // Player count
        set_color(0xFFFFFFFF);
        let players_str = match player_count() {
            1 => "1 Player",
            2 => "2 Players",
            3 => "3 Players",
            _ => "4 Players",
        };
        draw_text_str(players_str, 420.0, 320.0, 20.0);

        // Instructions with animated fade
        let blink_alpha = ((libm::sinf(TICK as f32 * 0.15) * 0.3 + 0.7) * 255.0) as u32;
        set_color(0x00FF0000 | blink_alpha);
        draw_text_str("Press A or START to begin", 340.0, 380.0, 18.0);

        // Controls hint
        set_color(0x808080FF);
        draw_text_str(
            "Move: D-Pad/Stick | Jump: A | Shoot: B | Melee: X",
            260.0,
            420.0,
            14.0,
        );

        // Animated character previews - bouncing player silhouettes
        let bounce1 = libm::sinf(TICK as f32 * 0.12) * 10.0;
        let bounce2 = libm::sinf(TICK as f32 * 0.12 + 2.0) * 10.0;
        let bounce3 = libm::sinf(TICK as f32 * 0.12 + 4.0) * 10.0;
        let bounce4 = libm::sinf(TICK as f32 * 0.12 + 6.0) * 10.0;

        // Draw small colored circles to represent players
        set_color(PLAYER_COLORS[0]);
        draw_rect(250.0, 340.0 + bounce1, 20.0, 30.0);
        set_color(PLAYER_COLORS[1]);
        draw_rect(280.0, 340.0 + bounce2, 20.0, 30.0);
        set_color(PLAYER_COLORS[2]);
        draw_rect(650.0, 340.0 + bounce3, 20.0, 30.0);
        set_color(PLAYER_COLORS[3]);
        draw_rect(680.0, 340.0 + bounce4, 20.0, 30.0);
    }
}

fn render_lobby() {
    unsafe {
        setup_epu_grid_arena();
        draw_env();

        // Panel
        set_color(0x000000B0);
        draw_rect(140.0, 70.0, 680.0, 400.0);

        // Header
        set_color(0x00FFFFFF);
        draw_text_str("LOBBY", 410.0, 95.0, 36.0);

        // Player slots
        let connected = player_count().min(4) as usize;
        let mut y = 155.0;
        for i in 0..4 {
            let label = match i {
                0 => "P1",
                1 => "P2",
                2 => "P3",
                _ => "P4",
            };

            // Color swatch
            set_color(PLAYER_COLORS[i]);
            draw_rect(175.0, y + 4.0, 18.0, 18.0);

            set_color(0xFFFFFFFF);
            draw_text_str(label, 205.0, y, 20.0);

            let (status, color) = if i < connected {
                if PLAYERS[i].ready {
                    ("READY", 0x00FF00FF)
                } else {
                    ("PRESS A", 0xAAAAAAFF)
                }
            } else if CONFIG.fill_bots {
                ("CPU", 0x00FFFFFF)
            } else {
                ("---", 0x666666FF)
            };

            set_color(color);
            draw_text_str(status, 280.0, y, 20.0);

            y += 34.0;
        }

        // Match settings (P1 controls)
        let settings_x = 460.0;
        let mut sy = 155.0;

        // Helper: highlight row
        let highlight = |idx: u32, y: f32| {
            if LOBBY_INDEX == idx {
                set_color(0x00FFFF30);
                draw_rect(settings_x - 14.0, y - 2.0, 330.0, 26.0);
            }
        };

        // Stage
        highlight(0, sy);
        set_color(0xFFFFFFFF);
        draw_text_str("STAGE", settings_x, sy, 18.0);
        set_color(0x00FFFFFF);
        draw_text_str(
            stage_select_label(CONFIG.stage_select),
            settings_x + 120.0,
            sy,
            18.0,
        );
        sy += 32.0;

        // Kills
        highlight(1, sy);
        set_color(0xFFFFFFFF);
        draw_text_str("KILLS", settings_x, sy, 18.0);
        let mut buf = [0u8; 10];
        set_color(0xFFFF00FF);
        draw_text_str(
            u32_to_str(CONFIG.kills_to_win, &mut buf),
            settings_x + 120.0,
            sy,
            18.0,
        );
        sy += 32.0;

        // Time
        highlight(2, sy);
        set_color(0xFFFFFFFF);
        draw_text_str("TIME", settings_x, sy, 18.0);
        if CONFIG.round_time_seconds == 0 {
            set_color(0xAAAAAAFF);
            draw_text_str("INFINITE", settings_x + 120.0, sy, 18.0);
        } else {
            let mut tbuf = [0u8; 10];
            set_color(0xAAAAAAFF);
            draw_text_str(
                u32_to_str(CONFIG.round_time_seconds, &mut tbuf),
                settings_x + 120.0,
                sy,
                18.0,
            );
            draw_text_str("s", settings_x + 150.0, sy, 18.0);
        }
        sy += 32.0;

        // CPUs
        highlight(3, sy);
        set_color(0xFFFFFFFF);
        draw_text_str("FILL CPU", settings_x, sy, 18.0);
        set_color(if CONFIG.fill_bots {
            0x00FF00FF
        } else {
            0xFF0000FF
        });
        draw_text_str(
            if CONFIG.fill_bots { "ON" } else { "OFF" },
            settings_x + 120.0,
            sy,
            18.0,
        );
        sy += 32.0;

        // CPU difficulty
        highlight(4, sy);
        set_color(0xFFFFFFFF);
        draw_text_str("CPU", settings_x, sy, 18.0);
        let diff = match CONFIG.bot_difficulty {
            0 => "EASY",
            1 => "NORMAL",
            _ => "HARD",
        };
        set_color(0xFF00FFFF);
        draw_text_str(diff, settings_x + 120.0, sy, 18.0);

        // Footer instructions
        set_color(0x808080FF);
        draw_text_str(
            "Players: A to ready | P1: D-Pad to change | START: begin | B: title",
            175.0,
            430.0,
            14.0,
        );

        // Eligibility hint
        let ready_humans = PLAYERS.iter().take(connected).filter(|p| p.ready).count() as u32;
        let cpu_fill = if CONFIG.fill_bots {
            (4 - connected) as u32
        } else {
            0
        };
        let total = ready_humans + cpu_fill;
        if total < 2 {
            set_color(0xFF4040FF);
            draw_text_str("Need 2 players to start", 360.0, 455.0, 16.0);
        }
    }
}

fn render_pause() {
    unsafe {
        // Dark overlay
        set_color(0x000000A8);
        draw_rect(0.0, 0.0, 960.0, 540.0);

        // Panel
        set_color(0x000000D0);
        draw_rect(300.0, 120.0, 360.0, 300.0);

        set_color(0x00FFFFFF);
        draw_text_str("PAUSED", 405.0, 145.0, 30.0);

        let base_x = 330.0;
        let mut y = 200.0;

        match PAUSE_PAGE {
            PausePage::Main => {
                let items = [
                    "RESUME",
                    "RESTART ROUND",
                    "RESTART MATCH",
                    "RETURN TO LOBBY",
                    "OPTIONS",
                ];
                for (i, item) in items.iter().enumerate() {
                    if PAUSE_INDEX == i as u32 {
                        set_color(0x00FFFF30);
                        draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                    }
                    set_color(0xFFFFFFFF);
                    draw_text_str(item, base_x, y, 18.0);
                    y += 32.0;
                }
            }
            PausePage::Options => {
                // Music volume
                if PAUSE_INDEX == 0 {
                    set_color(0x00FFFF30);
                    draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                }
                set_color(0xFFFFFFFF);
                draw_text_str("MUSIC", base_x, y, 18.0);
                set_color(0x808080FF);
                draw_rect(base_x + 120.0, y + 6.0, 180.0, 6.0);
                set_color(0x00FF00FF);
                draw_rect(base_x + 120.0, y + 6.0, 180.0 * OPTIONS.music_volume, 6.0);
                y += 32.0;

                // SFX volume
                if PAUSE_INDEX == 1 {
                    set_color(0x00FFFF30);
                    draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                }
                set_color(0xFFFFFFFF);
                draw_text_str("SFX", base_x, y, 18.0);
                set_color(0x808080FF);
                draw_rect(base_x + 120.0, y + 6.0, 180.0, 6.0);
                set_color(0xFFFF00FF);
                draw_rect(base_x + 120.0, y + 6.0, 180.0 * OPTIONS.sfx_volume, 6.0);
                y += 32.0;

                // Shake
                if PAUSE_INDEX == 2 {
                    set_color(0x00FFFF30);
                    draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                }
                set_color(0xFFFFFFFF);
                draw_text_str("SCREEN SHAKE", base_x, y, 18.0);
                set_color(if OPTIONS.screen_shake {
                    0x00FF00FF
                } else {
                    0xFF0000FF
                });
                draw_text_str(
                    if OPTIONS.screen_shake { "ON" } else { "OFF" },
                    base_x + 220.0,
                    y,
                    18.0,
                );
                y += 32.0;

                // Flash
                if PAUSE_INDEX == 3 {
                    set_color(0x00FFFF30);
                    draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                }
                set_color(0xFFFFFFFF);
                draw_text_str("SCREEN FLASH", base_x, y, 18.0);
                set_color(if OPTIONS.screen_flash {
                    0x00FF00FF
                } else {
                    0xFF0000FF
                });
                draw_text_str(
                    if OPTIONS.screen_flash { "ON" } else { "OFF" },
                    base_x + 220.0,
                    y,
                    18.0,
                );
                y += 32.0;

                // Back
                if PAUSE_INDEX == 4 {
                    set_color(0x00FFFF30);
                    draw_rect(base_x - 10.0, y - 3.0, 320.0, 26.0);
                }
                set_color(0xFFFFFFFF);
                draw_text_str("BACK", base_x, y, 18.0);
            }
        }

        set_color(0x808080FF);
        draw_text_str("A: select | B/START: back", 350.0, 395.0, 14.0);
    }
}

pub fn render_ui() {
    unsafe {
        // Impact flash overlay (drawn first, covers everything)
        if IMPACT_FLASH > 0 {
            let flash_alpha = (IMPACT_FLASH as f32 / 3.0 * 150.0) as u32;
            set_color(0xFFFFFF00 | flash_alpha);
            draw_rect(0.0, 0.0, 960.0, 540.0);
        }

        match GAME_STATE.phase {
            GamePhase::Title => {
                render_title();
                return;
            }
            GamePhase::Lobby => {
                render_lobby();
                return;
            }
            GamePhase::Paused => {
                render_pause();
                return;
            }
            _ => {}
        }

        // Demo watermark
        if GAME_STATE.demo_mode {
            set_color(0x00FFFFFF);
            draw_text_str("DEMO", 885.0, 12.0, 16.0);
        }

        // Score display
        let active_count = PLAYERS.iter().filter(|p| p.active).count() as u32;
        set_color(0x000000AA);
        draw_rect(10.0, 10.0, 260.0, 30.0 + (active_count as f32 * 28.0));

        let win_kills = CONFIG.kills_to_win.max(1);
        let mut y = 40.0;
        for (i, player) in PLAYERS.iter().enumerate() {
            if !player.active {
                continue;
            }

            // Color swatch + label
            set_color(PLAYER_COLORS[i]);
            draw_rect(18.0, y + 4.0, 12.0, 12.0);
            set_color(0xFFFFFFFF);
            let label = match i {
                0 => "P1",
                1 => "P2",
                2 => "P3",
                _ => "P4",
            };
            draw_text_str(label, 35.0, y, 16.0);

            if player.is_bot {
                set_color(0x00FFFFFF);
                draw_text_str("CPU", 68.0, y, 14.0);
            }

            // Kills / goal
            let mut kbuf = [0u8; 10];
            set_color(PLAYER_COLORS[i]);
            draw_text_str(u32_to_str(player.kills, &mut kbuf), 125.0, y, 16.0);
            set_color(0x808080FF);
            draw_text_str("/", 145.0, y, 16.0);
            let mut wbuf = [0u8; 10];
            draw_text_str(u32_to_str(win_kills, &mut wbuf), 155.0, y, 16.0);

            // Status
            if player.dead {
                set_color(0xFF4040FF);
                draw_text_str("DEAD", 190.0, y, 14.0);
                if player.respawn_timer > 0 {
                    let secs = (player.respawn_timer + 59) / 60;
                    let mut sbuf = [0u8; 10];
                    set_color(0xCCCCCCFF);
                    draw_text_str(u32_to_str(secs, &mut sbuf), 232.0, y, 14.0);
                }
            } else if player.invuln_timer > 0 {
                set_color(0x00FFFFAA);
                draw_text_str("SAFE", 190.0, y, 14.0);
            }

            y += 28.0;
        }

        // Round + stage (top center)
        set_color(0xAAAAAAFF);
        draw_text_str("ROUND", 420.0, 20.0, 18.0);
        let mut rbuf = [0u8; 10];
        draw_text_str(u32_to_str(ROUND_NUMBER, &mut rbuf), 485.0, 20.0, 18.0);
        set_color(0x808080FF);
        draw_text_str(stage_name(GAME_STATE.current_stage), 410.0, 42.0, 14.0);

        // Timer / overtime
        if CONFIG.round_time_seconds > 0 {
            if GAME_STATE.overtime {
                let alpha = if (TICK / 15) % 2 == 0 { 255 } else { 180 };
                set_color(with_alpha(0xFF4040FF, alpha));
                draw_text_str("OVERTIME", 820.0, 20.0, 18.0);
            } else {
                let total_secs = (GAME_STATE.round_time_left + 59) / 60;
                let mut tbuf = [0u8; 6];
                set_color(0xFFFFFFFF);
                draw_text_str(mmss_to_str(total_secs, &mut tbuf), 840.0, 20.0, 18.0);
            }
        }

        // Deflect popup
        if DEFLECT_POPUP_TICKS > 0 {
            let a = (DEFLECT_POPUP_TICKS.min(20) * 12).min(220);
            set_color(with_alpha(0x00FFFFFF, a));
            let who = match DEFLECT_PLAYER {
                0 => "P1",
                1 => "P2",
                2 => "P3",
                _ => "P4",
            };
            draw_text_str(who, 420.0, 105.0, 18.0);
            draw_text_str("DEFLECT!", 450.0, 105.0, 18.0);
        }

        // Final KO overlay
        if GAME_STATE.phase == GamePhase::FinalKo {
            let pulse = libm::sinf(TICK as f32 * 0.25) * 0.15 + 1.0;
            set_color(0xFF4040FF);
            draw_text_str("FINAL KO", 390.0, 80.0, 26.0 * pulse);
        }

        // Off-screen indicators (approximate mapping for vertical escapes)
        {
            let left = GAME_STATE.arena_left;
            let right = GAME_STATE.arena_right;
            let y_min = -8.0;
            let y_max = 8.0;
            let w = (right - left).max(0.001);

            for (i, p) in PLAYERS.iter().enumerate() {
                if !p.active || p.dead {
                    continue;
                }
                let cx = p.x + PLAYER_WIDTH * 0.5;
                let cy = p.y + PLAYER_HEIGHT * 0.5;
                let nx = ((cx - left) / w).clamp(0.0, 1.0);
                let sx = nx * 960.0;

                if cy > y_max {
                    set_color(PLAYER_COLORS[i]);
                    draw_rect(sx - 10.0, 8.0, 20.0, 8.0);
                } else if cy < y_min {
                    set_color(PLAYER_COLORS[i]);
                    draw_rect(sx - 10.0, 524.0, 20.0, 8.0);
                }
            }
        }

        // Countdown with animation
        if GAME_STATE.phase == GamePhase::Countdown {
            let seconds = (GAME_STATE.countdown / 60) + 1;
            let frame_in_second = GAME_STATE.countdown % 60;

            // Calculate animation progress (0.0 at start of second, 1.0 at end)
            let progress = 1.0 - (frame_in_second as f32 / 60.0);

            // Scale animation: starts big, bounces to normal
            let scale = if progress < 0.5 {
                // Overshoot then settle
                2.5 - progress * 3.0 * ease_out_bounce(progress * 2.0)
            } else {
                1.0
            };

            // Alpha fades in last 15 frames
            let alpha = if frame_in_second < 15 {
                (frame_in_second as f32 / 15.0 * 255.0) as u32
            } else {
                255
            };

            // Background box (also animated)
            let box_scale = 1.0 + (1.0 - progress.min(0.3) / 0.3) * 0.2;
            set_color(0x000000CC);
            draw_rect(
                480.0 - 80.0 * box_scale,
                290.0 - 40.0 * box_scale,
                160.0 * box_scale,
                80.0 * box_scale,
            );

            let countdown_str = match seconds {
                3 => "3",
                2 => "2",
                1 => "1",
                _ => "GO!",
            };

            // Color: numbers are white, GO! is green
            let text_color = if seconds == 0 {
                0x00FF0000 | alpha // Green for GO!
            } else {
                0xFFFFFF00 | alpha
            };

            let base_size = if seconds == 0 { 56.0 } else { 48.0 };
            let text_size = base_size * scale;

            // Glow effect for countdown
            if scale > 1.1 {
                set_color(with_alpha(text_color, alpha / 3));
                draw_text_str(
                    countdown_str,
                    480.0 - text_size * 0.3,
                    290.0,
                    text_size + 8.0,
                );
            }

            set_color(text_color);
            draw_text_str(countdown_str, 480.0 - text_size * 0.25, 290.0, text_size);
        }

        // Match end with polished animation
        if GAME_STATE.phase == GamePhase::MatchEnd {
            // Animation progress based on match end tick
            let anim_tick = MATCH_END_TICK;

            // Background fade in
            let bg_alpha = ((anim_tick as f32 / 30.0).min(1.0) * 221.0) as u32;
            set_color(0x00000000 | bg_alpha);
            draw_rect(0.0, 0.0, 960.0, 540.0);

            // Find winner (prefer stored winner_idx, but fall back to scanning)
            let win_kills = CONFIG.kills_to_win.max(1);
            let mut winner_idx = GAME_STATE.winner_idx as usize;
            for (i, player) in PLAYERS.iter().enumerate() {
                if player.active && player.kills >= win_kills {
                    winner_idx = i;
                    break;
                }
            }

            // Text slide in with overshoot
            let slide_progress = ((anim_tick as f32 - 15.0) / 30.0).clamp(0.0, 1.0);
            let slide_offset = if slide_progress < 1.0 {
                let t = slide_progress;
                // Overshoot easing
                let overshoot = 1.0 + libm::sinf(t * core::f32::consts::PI) * 0.2;
                (1.0 - t * overshoot) * 300.0
            } else {
                0.0
            };

            // Winner color with pulsing
            let pulse = libm::sinf(anim_tick as f32 * 0.15) * 0.2 + 0.8;
            let winner_color = PLAYER_COLORS[winner_idx];
            let r = (((winner_color >> 24) & 0xFF) as f32 * pulse) as u32;
            let g = (((winner_color >> 16) & 0xFF) as f32 * pulse) as u32;
            let b = (((winner_color >> 8) & 0xFF) as f32 * pulse) as u32;
            let pulsing_color = (r << 24) | (g << 16) | (b << 8) | 0xFF;

            // Winner text with glow
            let winner_text = match winner_idx {
                0 => "PLAYER 1 WINS!",
                1 => "PLAYER 2 WINS!",
                2 => "PLAYER 3 WINS!",
                _ => "PLAYER 4 WINS!",
            };

            // Glow layers
            if anim_tick > 20 {
                set_color(with_alpha(winner_color, 40));
                draw_text_str(winner_text, 316.0 - slide_offset, 246.0, 38.0);
                set_color(with_alpha(winner_color, 80));
                draw_text_str(winner_text, 318.0 - slide_offset, 248.0, 35.0);
            }

            // Main text
            set_color(pulsing_color);
            draw_text_str(winner_text, 320.0 - slide_offset, 250.0, 32.0);

            // Subtitle slides in from opposite direction
            let sub_progress = ((anim_tick as f32 - 45.0) / 30.0).clamp(0.0, 1.0);
            let sub_offset = (1.0 - sub_progress) * -200.0;

            if anim_tick > 45 {
                // Blinking prompt
                let blink_alpha = if (anim_tick / 30) % 2 == 0 { 255 } else { 180 };
                set_color(0xCCCCCC00 | blink_alpha);
                draw_text_str(
                    "START: rematch    B: lobby",
                    330.0 - sub_offset,
                    300.0,
                    16.0,
                );
            }

            // Draw winner character in spotlight (larger, centered)
            if anim_tick > 30 {
                let scale_in = ((anim_tick as f32 - 30.0) / 20.0).min(1.0);
                let char_scale = scale_in * 2.0;

                // Spotlight glow
                set_color(with_alpha(winner_color, (scale_in * 60.0) as u32));
                push_identity();
                push_translate(0.0, -1.0, 2.0);
                push_scale(3.0 * scale_in, 3.0 * scale_in, 0.1);
                draw_mesh(SPHERE_MESH);

                // Winner character
                set_color(winner_color);
                push_identity();
                push_translate(0.0, -1.0, 3.0);
                push_scale(char_scale, char_scale, char_scale);
                draw_mesh(CAPSULE_MESH);

                // Head
                set_color(brighten_color(winner_color));
                push_identity();
                push_translate(0.0, 0.5 * scale_in, 3.1);
                push_scale(char_scale * 0.8, char_scale * 0.8, char_scale * 0.8);
                draw_mesh(SPHERE_MESH);
            }
        }

        // Stage transition overlay
        if TRANSITION_PHASE != TransitionPhase::None {
            let alpha = (TRANSITION_PROGRESS * 255.0) as u32;
            set_color(0x00000000 | alpha);
            draw_rect(0.0, 0.0, 960.0, 540.0);
        }
    }
}
