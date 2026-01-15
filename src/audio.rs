//! Audio module - handles sound and music loading and playback
//!
//! Loads sound and music assets from ROM during init and provides
//! convenience functions for playing game sounds and stage music.

use crate::ffi::*;

// =============================================================================
// SOUND HANDLES
// =============================================================================

// Sound handles (loaded at init)
pub static mut SND_SHOOT: u32 = 0;
pub static mut SND_HIT: u32 = 0;
pub static mut SND_DEATH: u32 = 0;
pub static mut SND_DEFLECT: u32 = 0;
pub static mut SND_JUMP: u32 = 0;
pub static mut SND_COUNTDOWN: u32 = 0;
pub static mut SND_GO: u32 = 0;
pub static mut SND_SPAWN: u32 = 0;
pub static mut SND_VICTORY: u32 = 0;

// =============================================================================
// MUSIC HANDLES
// =============================================================================

// Music handles (loaded at init)
pub static mut MUSIC_MENU: u32 = 0;
pub static mut MUSIC_GRID: u32 = 0;
pub static mut MUSIC_SCATTER: u32 = 0;
pub static mut MUSIC_RING: u32 = 0;

// =============================================================================
// INITIALIZATION
// =============================================================================

/// Initialize all sound and music handles from ROM
///
/// Must be called during `init()` before any sounds or music can be played.
pub fn init_audio() {
    unsafe {
        // Load sound effects
        SND_SHOOT = rom_sound_str("shoot");
        SND_HIT = rom_sound_str("hit");
        SND_DEATH = rom_sound_str("death");
        SND_DEFLECT = rom_sound_str("deflect");
        SND_JUMP = rom_sound_str("jump");
        SND_COUNTDOWN = rom_sound_str("countdown");
        SND_GO = rom_sound_str("go");
        SND_SPAWN = rom_sound_str("spawn");
        SND_VICTORY = rom_sound_str("victory");

        // Load music tracks
        MUSIC_MENU = rom_tracker_str("music_menu");
        MUSIC_GRID = rom_tracker_str("music_grid");
        MUSIC_SCATTER = rom_tracker_str("music_scatter");
        MUSIC_RING = rom_tracker_str("music_ring");
    }
}

// =============================================================================
// GENERIC PLAYBACK
// =============================================================================

/// Play a sound with volume and pan
///
/// # Arguments
/// * `sound` - Sound handle from ROM
/// * `volume` - 0.0 to 1.0
/// * `pan` - -1.0 (left) to 1.0 (right), 0.0 = center
#[allow(dead_code)]
pub fn play(sound: u32, volume: f32, pan: f32) {
    unsafe {
        play_sound(sound, volume, pan);
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Play shoot sound with spatial panning
///
/// # Arguments
/// * `pan` - -1.0 (left) to 1.0 (right), based on player x position
pub fn play_shoot(pan: f32) {
    unsafe {
        play_sound(SND_SHOOT, 0.8, pan);
    }
}

/// Play hit sound (melee hit or bullet impact)
pub fn play_hit() {
    unsafe {
        play_sound(SND_HIT, 1.0, 0.0);
    }
}

/// Play death sound
pub fn play_death() {
    unsafe {
        play_sound(SND_DEATH, 1.0, 0.0);
    }
}

/// Play deflect sound (melee parry)
pub fn play_deflect() {
    unsafe {
        play_sound(SND_DEFLECT, 0.9, 0.0);
    }
}

/// Play jump sound with spatial panning
///
/// # Arguments
/// * `pan` - -1.0 (left) to 1.0 (right), based on player x position
pub fn play_jump(pan: f32) {
    unsafe {
        play_sound(SND_JUMP, 0.6, pan);
    }
}

/// Play countdown beep (3, 2, 1)
pub fn play_countdown() {
    unsafe {
        play_sound(SND_COUNTDOWN, 0.7, 0.0);
    }
}

/// Play GO sound (match start)
pub fn play_go() {
    unsafe {
        play_sound(SND_GO, 1.0, 0.0);
    }
}

/// Play spawn/respawn sound with spatial panning
///
/// # Arguments
/// * `pan` - -1.0 (left) to 1.0 (right), based on player x position
pub fn play_spawn(pan: f32) {
    unsafe {
        play_sound(SND_SPAWN, 0.8, pan);
    }
}

/// Play victory fanfare (match end celebration)
pub fn play_victory() {
    unsafe {
        play_sound(SND_VICTORY, 1.0, 0.0);
    }
}

// =============================================================================
// MUSIC CONTROL
// =============================================================================

/// Play menu/title screen music
pub fn play_menu_music() {
    unsafe {
        music_play(MUSIC_MENU, 0.6, 1); // volume 0.6, looping=1
    }
}

/// Play the appropriate music track for a stage
///
/// # Arguments
/// * `stage` - Stage index (0=Grid Arena, 1=Scatter Field, 2=Ring Void)
pub fn play_music_for_stage(stage: u32) {
    unsafe {
        let handle = match stage {
            0 => MUSIC_GRID,
            1 => MUSIC_SCATTER,
            2 => MUSIC_RING,
            _ => MUSIC_GRID, // Default to Grid Arena music
        };
        music_play(handle, 0.5, 1); // volume 0.5, looping=1
    }
}

/// Stop the currently playing music
pub fn stop_music() {
    unsafe {
        music_stop();
    }
}

/// Set the music volume
///
/// # Arguments
/// * `volume` - Volume level from 0.0 to 1.0
#[allow(dead_code)]
pub fn set_music_volume(volume: f32) {
    unsafe {
        music_set_volume(volume);
    }
}
