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

// =============================================================================
// MODULES
// =============================================================================

mod audio;
mod combat;
mod ffi;
mod game_state;
mod particles;
mod player;
mod render;
mod stage;

use combat::{update_bullets, update_melee_hits, BULLETS};
use ffi::*;
use game_state::{
    advance_stage, is_frozen, update_camera_fov, update_effect_lights, update_hit_freeze,
    update_impact_flash, update_match_end_tick, update_shake, GamePhase, CAMERA_FOV, GAME_STATE,
    SCREEN_SHAKE_X, SCREEN_SHAKE_Y, TICK,
};
use player::{spawn_players, update_player, BUTTON_A, BUTTON_START, MAX_PLAYERS, PLAYERS};
use render::{
    apply_effect_lights, init_meshes, render_bullets, render_particles, render_players,
    render_stage, render_ui,
};
use stage::{setup_current_stage, update_platforms};

// =============================================================================
// GAME FLOW
// =============================================================================

fn reset_round() {
    unsafe {
        // Clear bullets
        for b in &mut BULLETS {
            b.active = false;
        }

        // Clear particles
        particles::clear_particles();

        // Setup stage and spawn players
        setup_current_stage();
        spawn_players();

        // Start music for the current stage
        audio::play_music_for_stage(GAME_STATE.current_stage);

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
        game_state::ROUND_NUMBER = 1;
        reset_round();
    }
}

// =============================================================================
// ENTRY POINTS
// =============================================================================

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        // Dark background
        set_clear_color(0x0a0a1aff);

        // Create mesh handles for 3D rendering
        init_meshes();

        // Initialize audio system
        audio::init_audio();

        // Start on title screen with menu music
        GAME_STATE.phase = GamePhase::Title;
        audio::play_menu_music();
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
                        // Stop menu music before starting match
                        audio::stop_music();
                        reset_match();
                        GAME_STATE.phase = GamePhase::Countdown;
                        return;
                    }
                }
            }

            GamePhase::Countdown => {
                if GAME_STATE.countdown > 0 {
                    // Play countdown beep at each second (180=3, 120=2, 60=1)
                    if GAME_STATE.countdown == 180
                        || GAME_STATE.countdown == 120
                        || GAME_STATE.countdown == 60
                    {
                        audio::play_countdown();
                    }
                    GAME_STATE.countdown -= 1;
                } else {
                    // Play GO sound when countdown ends
                    audio::play_go();
                    GAME_STATE.phase = GamePhase::Playing;
                }
            }

            GamePhase::Playing => {
                // Update hit freeze first
                update_hit_freeze();

                // Update impact flash (visual effect)
                update_impact_flash();

                // Update camera FOV (zoom effect)
                update_camera_fov();

                // Update effect lights
                update_effect_lights();

                // Only update game logic if not frozen
                if !is_frozen() {
                    update_platforms();

                    for i in 0..MAX_PLAYERS {
                        update_player(i);
                    }

                    update_bullets();
                    update_melee_hits();

                    // Update particles
                    particles::update_particles();

                    // Handle round end timer (brief pause after kill)
                    if GAME_STATE.round_end_timer > 0 {
                        GAME_STATE.round_end_timer -= 1;

                        // When timer hits 0 after a kill, advance stage and reset round
                        if GAME_STATE.round_end_timer == 0 {
                            advance_stage();
                            game_state::ROUND_NUMBER += 1;
                            reset_round();
                        }
                    }
                }

                // Always update screen shake (visual effect during freeze)
                update_shake();
            }

            GamePhase::RoundEnd => {
                // Currently unused - kills just cause brief pause
            }

            GamePhase::MatchEnd => {
                // Update match end animation tick
                update_match_end_tick();

                // Update particles (for victory confetti)
                particles::update_particles();

                // Check for restart - go back to title
                for (i, player) in PLAYERS.iter().enumerate() {
                    if player.active && button_pressed(i as u32, BUTTON_START) != 0 {
                        // Stop any music and return to title with menu music
                        audio::stop_music();
                        GAME_STATE.phase = GamePhase::Title;
                        audio::play_menu_music();
                        return;
                    }
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn render() {
    unsafe {
        // Get shake offsets
        let shake_x = SCREEN_SHAKE_X;
        let shake_y = SCREEN_SHAKE_Y;

        // Set camera for side-view with shake offset applied
        camera_set(
            0.0 + shake_x,
            2.0 + shake_y,
            12.0,
            0.0 + shake_x,
            2.0 + shake_y,
            0.0,
        );

        // Use dynamic camera FOV (zooms in on kills)
        camera_fov(CAMERA_FOV);

        // Apply effect lights for visual feedback
        apply_effect_lights();

        // Render in order
        render_stage();
        render_players();
        render_bullets();
        render_particles();
        render_ui();
    }
}
