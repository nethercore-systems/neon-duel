//! NEON DUEL - Platform Fighter for ZX Console
//!
//! A 2-4 player one-hit-kill arena game inspired by Towerfall and Samurai Gunn.
//! Showcases ZX rollback netcode, EPU procedural backgrounds, and matcap rendering.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

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
    is_frozen, round_time_limit_ticks, update_camera_fov, update_deflect_popup,
    update_effect_lights, update_hit_freeze, update_impact_flash, update_match_end_tick,
    update_shake, update_transition, GamePhase, PausePage, CAMERA_FOV, CONFIG, GAME_STATE,
    LOBBY_INDEX, OPTIONS, PAUSE_INDEX, PAUSE_PAGE, ROUND_NUMBER, SCREEN_SHAKE_X, SCREEN_SHAKE_Y,
    STAGE_SELECT_RANDOM, STAGE_SELECT_ROTATE, TICK, TITLE_IDLE_TICKS,
};
use player::{
    spawn_players, update_player, BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT,
    BUTTON_START, BUTTON_UP, MAX_PLAYERS, PLAYERS,
};
use render::{
    apply_effect_lights, init_meshes, render_bullets, render_particles, render_players,
    render_stage, render_ui,
};
use stage::{setup_current_stage, update_platforms};

// =============================================================================
// GAME FLOW
// =============================================================================

const TITLE_DEMO_DELAY_TICKS: u32 = 60 * 10;
const ARENA_LEFT_DEFAULT: f32 = -10.0;
const ARENA_RIGHT_DEFAULT: f32 = 10.0;
const OVERTIME_SHRINK_SPEED: f32 = 0.03; // world units/frame per side
const OVERTIME_MIN_WIDTH: f32 = 2.5; // when reached, someone is getting crushed

fn any_input_pressed() -> bool {
    unsafe {
        for i in 0..player_count() {
            if button_pressed(i, BUTTON_A) != 0
                || button_pressed(i, BUTTON_B) != 0
                || button_pressed(i, player::BUTTON_X) != 0
                || button_pressed(i, BUTTON_START) != 0
                || button_pressed(i, BUTTON_UP) != 0
                || button_pressed(i, BUTTON_DOWN) != 0
                || button_pressed(i, BUTTON_LEFT) != 0
                || button_pressed(i, BUTTON_RIGHT) != 0
            {
                return true;
            }
        }
        false
    }
}

fn enter_title() {
    unsafe {
        GAME_STATE.phase = GamePhase::Title;
        GAME_STATE.demo_mode = false;
        TITLE_IDLE_TICKS = 0;
        game_state::TRANSITION_PHASE = game_state::TransitionPhase::None;
        game_state::TRANSITION_PROGRESS = 0.0;

        // Reset lobby state
        for p in &mut PLAYERS {
            *p = player::Player::new();
        }

        audio::play_menu_music();
    }
}

fn enter_lobby() {
    unsafe {
        GAME_STATE.phase = GamePhase::Lobby;
        GAME_STATE.demo_mode = false;
        TITLE_IDLE_TICKS = 0;
        LOBBY_INDEX = 0;
        game_state::TRANSITION_PHASE = game_state::TransitionPhase::None;
        game_state::TRANSITION_PROGRESS = 0.0;

        // Clear join/ready; keep config/options.
        for p in &mut PLAYERS {
            *p = player::Player::new();
        }
    }
}

fn apply_round_defaults() {
    unsafe {
        GAME_STATE.overtime = false;
        GAME_STATE.arena_left = ARENA_LEFT_DEFAULT;
        GAME_STATE.arena_right = ARENA_RIGHT_DEFAULT;
        GAME_STATE.round_time_left = round_time_limit_ticks();
    }
}

fn pick_stage_for_new_round() {
    unsafe {
        let sel = CONFIG.stage_select;
        if sel == STAGE_SELECT_ROTATE {
            GAME_STATE.current_stage = (GAME_STATE.current_stage + 1) % game_state::NUM_STAGES;
        } else if sel == STAGE_SELECT_RANDOM {
            GAME_STATE.current_stage = random_range(0, game_state::NUM_STAGES as i32).max(0) as u32;
        }
    }
}

fn set_start_stage_for_match() {
    unsafe {
        let sel = CONFIG.stage_select;
        GAME_STATE.current_stage = if sel < game_state::NUM_STAGES {
            sel
        } else if sel == STAGE_SELECT_RANDOM {
            random_range(0, game_state::NUM_STAGES as i32).max(0) as u32
        } else {
            0 // rotate
        };
    }
}

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

        apply_round_defaults();

        // Fade in for round start
        game_state::start_transition_in();

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

        set_start_stage_for_match();
        ROUND_NUMBER = 1;
        reset_round();
    }
}

fn start_match(demo_mode: bool) {
    unsafe {
        audio::stop_music();
        GAME_STATE.demo_mode = demo_mode;

        // Activate players (humans that are ready), and fill with bots if enabled.
        let connected = player_count().min(MAX_PLAYERS as u32) as usize;

        // Ensure connected slots are humans (even if inactive).
        for i in 0..MAX_PLAYERS {
            PLAYERS[i].is_bot = false;
            if i >= connected {
                PLAYERS[i].ready = false;
            }
        }

        // If demo: force 4 bots.
        if demo_mode {
            for i in 0..MAX_PLAYERS {
                PLAYERS[i].active = true;
                PLAYERS[i].ready = true;
                PLAYERS[i].is_bot = true;
            }
        } else {
            // If nobody is ready but someone hit START, auto-ready P1.
            let mut any_ready = false;
            for i in 0..connected {
                if PLAYERS[i].ready {
                    any_ready = true;
                    break;
                }
            }
            if !any_ready && connected > 0 {
                PLAYERS[0].ready = true;
                PLAYERS[0].active = true;
            }

            // Disable non-ready humans.
            for i in 0..MAX_PLAYERS {
                if i < connected {
                    PLAYERS[i].active = PLAYERS[i].ready;
                } else {
                    PLAYERS[i].active = false;
                }
            }

            // Fill remaining empty seats with bots (only for non-connected slots).
            if CONFIG.fill_bots {
                for i in connected..MAX_PLAYERS {
                    if !PLAYERS[i].active {
                        PLAYERS[i].active = true;
                        PLAYERS[i].ready = true;
                        PLAYERS[i].is_bot = true;
                    }
                }
            }
        }

        // Need at least 2 participants.
        let mut participants = 0;
        for p in &PLAYERS {
            if p.active {
                participants += 1;
            }
        }
        if participants < 2 {
            return;
        }

        reset_match();
    }
}

fn update_overtime() {
    unsafe {
        if !GAME_STATE.overtime {
            return;
        }

        let width = GAME_STATE.arena_right - GAME_STATE.arena_left;
        if width > OVERTIME_MIN_WIDTH {
            GAME_STATE.arena_left += OVERTIME_SHRINK_SPEED;
            GAME_STATE.arena_right -= OVERTIME_SHRINK_SPEED;
        }
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
        audio::set_music_volume(OPTIONS.music_volume);
        audio::set_sfx_volume(OPTIONS.sfx_volume);

        // Start on title screen with menu music
        enter_title();
    }
}

#[no_mangle]
pub extern "C" fn update() {
    unsafe {
        TICK += 1;

        if game_state::is_transitioning() {
            let _ = update_transition();
        }

        match GAME_STATE.phase {
            GamePhase::Title => {
                if any_input_pressed() {
                    TITLE_IDLE_TICKS = 0;
                } else {
                    TITLE_IDLE_TICKS += 1;
                }

                // A/START -> lobby
                for i in 0..player_count() {
                    if button_pressed(i, BUTTON_A) != 0 || button_pressed(i, BUTTON_START) != 0 {
                        enter_lobby();
                        return;
                    }
                }

                // Attract mode demo
                if TITLE_IDLE_TICKS > TITLE_DEMO_DELAY_TICKS {
                    start_match(true);
                    return;
                }
            }

            GamePhase::Lobby => {
                // Back to title
                if player_count() > 0 && button_pressed(0, BUTTON_B) != 0 {
                    enter_title();
                    return;
                }

                // Join/ready toggles for connected players
                let connected = player_count().min(MAX_PLAYERS as u32) as usize;
                for i in 0..connected {
                    if button_pressed(i as u32, BUTTON_A) != 0 {
                        let p = &mut PLAYERS[i];
                        p.ready = !p.ready;
                        p.active = p.ready;
                        p.is_bot = false;
                    }
                }
                // Clear non-connected slots
                for i in connected..MAX_PLAYERS {
                    PLAYERS[i].ready = false;
                    PLAYERS[i].active = false;
                    PLAYERS[i].is_bot = false;
                }

                // Settings navigation (P1)
                if connected > 0 {
                    if button_pressed(0, BUTTON_UP) != 0 {
                        LOBBY_INDEX = (LOBBY_INDEX + 5 - 1) % 5;
                    } else if button_pressed(0, BUTTON_DOWN) != 0 {
                        LOBBY_INDEX = (LOBBY_INDEX + 1) % 5;
                    }

                    if button_pressed(0, BUTTON_LEFT) != 0 {
                        match LOBBY_INDEX {
                            0 => {
                                // Stage select
                                if CONFIG.stage_select == 0 {
                                    CONFIG.stage_select = STAGE_SELECT_ROTATE;
                                } else {
                                    CONFIG.stage_select -= 1;
                                }
                            }
                            1 => {
                                // Kills
                                CONFIG.kills_to_win = match CONFIG.kills_to_win {
                                    7 => 5,
                                    5 => 3,
                                    _ => 7,
                                };
                            }
                            2 => {
                                // Time
                                CONFIG.round_time_seconds = match CONFIG.round_time_seconds {
                                    0 => 90,
                                    30 => 0,
                                    45 => 30,
                                    60 => 45,
                                    90 => 60,
                                    _ => 45,
                                };
                            }
                            3 => CONFIG.fill_bots = !CONFIG.fill_bots,
                            4 => {
                                if CONFIG.bot_difficulty == 0 {
                                    CONFIG.bot_difficulty = 2;
                                } else {
                                    CONFIG.bot_difficulty -= 1;
                                }
                            }
                            _ => {}
                        }
                    } else if button_pressed(0, BUTTON_RIGHT) != 0 {
                        match LOBBY_INDEX {
                            0 => {
                                // Stage select
                                CONFIG.stage_select =
                                    (CONFIG.stage_select + 1) % (STAGE_SELECT_ROTATE + 1);
                            }
                            1 => {
                                // Kills
                                CONFIG.kills_to_win = match CONFIG.kills_to_win {
                                    3 => 5,
                                    5 => 7,
                                    _ => 3,
                                };
                            }
                            2 => {
                                // Time
                                CONFIG.round_time_seconds = match CONFIG.round_time_seconds {
                                    0 => 30,
                                    30 => 45,
                                    45 => 60,
                                    60 => 90,
                                    90 => 0,
                                    _ => 45,
                                };
                            }
                            3 => CONFIG.fill_bots = !CONFIG.fill_bots,
                            4 => CONFIG.bot_difficulty = (CONFIG.bot_difficulty + 1) % 3,
                            _ => {}
                        }
                    }
                }

                // Start match
                let mut start_pressed = false;
                for i in 0..player_count() {
                    if button_pressed(i, BUTTON_START) != 0 {
                        start_pressed = true;
                        break;
                    }
                }
                if start_pressed {
                    start_match(false);
                    return;
                }
            }

            GamePhase::Countdown => {
                // Pause
                for i in 0..player_count() {
                    if button_pressed(i, BUTTON_START) != 0 {
                        GAME_STATE.paused_from = GamePhase::Countdown;
                        GAME_STATE.phase = GamePhase::Paused;
                        PAUSE_PAGE = PausePage::Main;
                        PAUSE_INDEX = 0;
                        return;
                    }
                }

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
                // Demo: any input exits back to lobby.
                if GAME_STATE.demo_mode && any_input_pressed() {
                    audio::stop_music();
                    enter_lobby();
                    return;
                }

                // Pause (only humans)
                for (i, p) in PLAYERS.iter().enumerate() {
                    if p.active && !p.is_bot && button_pressed(i as u32, BUTTON_START) != 0 {
                        GAME_STATE.paused_from = GamePhase::Playing;
                        GAME_STATE.phase = GamePhase::Paused;
                        PAUSE_PAGE = PausePage::Main;
                        PAUSE_INDEX = 0;
                        return;
                    }
                }

                // Update hit freeze first
                update_hit_freeze();

                // Update impact flash (visual effect)
                update_impact_flash();

                // Update camera FOV (zoom effect)
                update_camera_fov();

                // Update effect lights
                update_effect_lights();

                update_deflect_popup();

                // Only update game logic if not frozen
                if !is_frozen() {
                    // Round timer / overtime
                    if !GAME_STATE.overtime && GAME_STATE.round_time_left > 0 {
                        GAME_STATE.round_time_left -= 1;
                        if GAME_STATE.round_time_left == 0 {
                            GAME_STATE.overtime = true;
                            // Audible cue using existing countdown beep.
                            audio::play_countdown();
                            // Small shake to sell the transition.
                            game_state::trigger_shake(0.4);
                        }
                    }

                    update_overtime();

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

                        // When timer hits 0 after a kill, rotate stage (if configured) and reset round
                        if GAME_STATE.round_end_timer == 0 {
                            pick_stage_for_new_round();
                            ROUND_NUMBER += 1;
                            reset_round();
                        }
                    }
                }

                // Always update screen shake (visual effect during freeze)
                update_shake();
            }

            GamePhase::Paused => {
                // Navigate pause menu (P1)
                if player_count() == 0 {
                    return;
                }

                let up = button_pressed(0, BUTTON_UP) != 0;
                let down = button_pressed(0, BUTTON_DOWN) != 0;
                let left = button_pressed(0, BUTTON_LEFT) != 0;
                let right = button_pressed(0, BUTTON_RIGHT) != 0;
                let confirm = button_pressed(0, BUTTON_A) != 0;
                let back = button_pressed(0, BUTTON_B) != 0 || button_pressed(0, BUTTON_START) != 0;

                match PAUSE_PAGE {
                    PausePage::Main => {
                        const MAIN_ITEMS: u32 = 5;
                        if up {
                            PAUSE_INDEX = (PAUSE_INDEX + MAIN_ITEMS - 1) % MAIN_ITEMS;
                        } else if down {
                            PAUSE_INDEX = (PAUSE_INDEX + 1) % MAIN_ITEMS;
                        }

                        if confirm || back {
                            match PAUSE_INDEX {
                                0 => {
                                    // Resume
                                    GAME_STATE.phase = GAME_STATE.paused_from;
                                }
                                1 => {
                                    // Restart round
                                    reset_round();
                                }
                                2 => {
                                    // Restart match
                                    reset_match();
                                }
                                3 => {
                                    // Return to lobby
                                    audio::stop_music();
                                    enter_lobby();
                                }
                                4 => {
                                    // Options
                                    PAUSE_PAGE = PausePage::Options;
                                    PAUSE_INDEX = 0;
                                }
                                _ => {}
                            }
                        }
                    }
                    PausePage::Options => {
                        const OPT_ITEMS: u32 = 5;
                        if up {
                            PAUSE_INDEX = (PAUSE_INDEX + OPT_ITEMS - 1) % OPT_ITEMS;
                        } else if down {
                            PAUSE_INDEX = (PAUSE_INDEX + 1) % OPT_ITEMS;
                        }

                        match PAUSE_INDEX {
                            0 => {
                                if left {
                                    OPTIONS.music_volume = (OPTIONS.music_volume - 0.05).max(0.0);
                                    audio::set_music_volume(OPTIONS.music_volume);
                                } else if right {
                                    OPTIONS.music_volume = (OPTIONS.music_volume + 0.05).min(1.0);
                                    audio::set_music_volume(OPTIONS.music_volume);
                                }
                            }
                            1 => {
                                if left {
                                    OPTIONS.sfx_volume = (OPTIONS.sfx_volume - 0.05).max(0.0);
                                    audio::set_sfx_volume(OPTIONS.sfx_volume);
                                } else if right {
                                    OPTIONS.sfx_volume = (OPTIONS.sfx_volume + 0.05).min(1.0);
                                    audio::set_sfx_volume(OPTIONS.sfx_volume);
                                }
                            }
                            2 => {
                                if confirm || left || right {
                                    OPTIONS.screen_shake = !OPTIONS.screen_shake;
                                }
                            }
                            3 => {
                                if confirm || left || right {
                                    OPTIONS.screen_flash = !OPTIONS.screen_flash;
                                }
                            }
                            4 => {
                                if confirm || back {
                                    PAUSE_PAGE = PausePage::Main;
                                    PAUSE_INDEX = 0;
                                }
                            }
                            _ => {}
                        }

                        if back && PAUSE_INDEX != 4 {
                            PAUSE_PAGE = PausePage::Main;
                            PAUSE_INDEX = 0;
                        }
                    }
                }
            }

            GamePhase::RoundEnd => {
                // Currently unused - kills just cause brief pause
            }

            GamePhase::FinalKo => {
                update_hit_freeze();
                update_impact_flash();
                update_camera_fov();
                update_effect_lights();
                update_deflect_popup();

                // Let particles linger in slow motion (every other frame).
                if TICK % 2 == 0 {
                    particles::update_particles();
                }

                if GAME_STATE.final_ko_timer > 0 {
                    GAME_STATE.final_ko_timer -= 1;
                } else {
                    // Enter match end presentation
                    GAME_STATE.phase = GamePhase::MatchEnd;
                    game_state::reset_match_end_tick();
                    audio::play_victory();
                    particles::spawn_victory_confetti(
                        player::PLAYER_COLORS[GAME_STATE.winner_idx as usize],
                    );
                }
            }

            GamePhase::MatchEnd => {
                // Update match end animation tick
                update_match_end_tick();

                // Update particles (for victory confetti)
                particles::update_particles();

                if GAME_STATE.demo_mode {
                    // Any input -> lobby; otherwise return to title after a bit.
                    if any_input_pressed() {
                        audio::stop_music();
                        enter_lobby();
                        return;
                    }
                    if game_state::MATCH_END_TICK > 300 {
                        audio::stop_music();
                        enter_title();
                        return;
                    }
                } else {
                    // Rematch / back to lobby
                    if player_count() > 0 && button_pressed(0, BUTTON_B) != 0 {
                        audio::stop_music();
                        enter_lobby();
                        return;
                    }
                    for (i, player) in PLAYERS.iter().enumerate() {
                        if player.active
                            && !player.is_bot
                            && button_pressed(i as u32, BUTTON_START) != 0
                        {
                            // Rematch with same config/participants
                            reset_match();
                            return;
                        }
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
