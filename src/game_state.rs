//! Game state management
//!
//! Contains GamePhase enum and GameState struct for match flow control.

/// Represents the current phase of the game
#[derive(Clone, Copy, PartialEq)]
pub enum GamePhase {
    Title,     // Main menu / title screen
    Lobby,     // Join/ready + match settings
    Countdown, // 3-2-1 before round starts
    Playing,   // Active gameplay
    Paused,    // Pause menu / options
    FinalKo,   // Match-winning hit slow-mo
    #[allow(dead_code)]
    RoundEnd, // Someone got a kill, brief pause (reserved for future use)
    MatchEnd,  // Someone won the match
}

// =============================================================================
// CONFIG + OPTIONS
// =============================================================================

/// Stage select setting:
/// - 0..NUM_STAGES-1: fixed stage
/// - NUM_STAGES: random each round
/// - NUM_STAGES+1: rotate each round
pub const STAGE_SELECT_RANDOM: u32 = NUM_STAGES;
pub const STAGE_SELECT_ROTATE: u32 = NUM_STAGES + 1;

#[derive(Clone, Copy)]
pub struct GameConfig {
    pub stage_select: u32,
    pub kills_to_win: u32,
    pub round_time_seconds: u32, // 0 = infinite
    pub fill_bots: bool,
    pub bot_difficulty: u32, // 0=Easy, 1=Normal, 2=Hard
}

impl GameConfig {
    pub const fn new() -> Self {
        Self {
            stage_select: STAGE_SELECT_ROTATE,
            kills_to_win: 5,
            round_time_seconds: 45,
            fill_bots: true,
            bot_difficulty: 1,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Options {
    pub music_volume: f32, // 0.0 - 1.0
    pub sfx_volume: f32,   // 0.0 - 1.0
    pub screen_shake: bool,
    pub screen_flash: bool,
}

impl Options {
    pub const fn new() -> Self {
        Self {
            music_volume: 0.6,
            sfx_volume: 0.85,
            screen_shake: true,
            screen_flash: true,
        }
    }
}

pub static mut CONFIG: GameConfig = GameConfig::new();
pub static mut OPTIONS: Options = Options::new();

/// Main game state tracking match progress
#[derive(Clone, Copy)]
pub struct GameState {
    pub phase: GamePhase,
    pub countdown: u32,
    pub round_end_timer: u32,
    pub current_stage: u32,
    pub round_time_left: u32,
    pub overtime: bool,
    pub arena_left: f32,
    pub arena_right: f32,
    pub winner_idx: u32,
    pub final_ko_timer: u32,
    pub demo_mode: bool,
    pub paused_from: GamePhase,
}

impl GameState {
    pub const fn new() -> Self {
        Self {
            phase: GamePhase::Title,
            countdown: 180, // 3 seconds
            round_end_timer: 0,
            current_stage: 0,
            round_time_left: 0,
            overtime: false,
            arena_left: -10.0,
            arena_right: 10.0,
            winner_idx: 0,
            final_ko_timer: 0,
            demo_mode: false,
            paused_from: GamePhase::Playing,
        }
    }
}

// =============================================================================
// GLOBAL STATE
// =============================================================================

pub static mut GAME_STATE: GameState = GameState::new();
pub static mut TICK: u32 = 0;
pub static mut ROUND_NUMBER: u32 = 1;

/// Number of stages in the game
pub const NUM_STAGES: u32 = 3;

/// Title idle counter for attract/demo mode.
pub static mut TITLE_IDLE_TICKS: u32 = 0;

/// Deflect popup (short UI feedback when someone parries a bullet)
pub static mut DEFLECT_POPUP_TICKS: u32 = 0;
pub static mut DEFLECT_PLAYER: u32 = 0;

// =============================================================================
// MENU STATE
// =============================================================================

#[derive(Clone, Copy, PartialEq)]
pub enum PausePage {
    Main,
    Options,
}

pub static mut PAUSE_PAGE: PausePage = PausePage::Main;
pub static mut PAUSE_INDEX: u32 = 0;
pub static mut LOBBY_INDEX: u32 = 0;

pub fn round_time_limit_ticks() -> u32 {
    unsafe { CONFIG.round_time_seconds.saturating_mul(60) }
}

// =============================================================================
// HIT FREEZE STATE
// =============================================================================

/// Hit freeze countdown (game pauses when > 0)
pub static mut HIT_FREEZE: u32 = 0;

// =============================================================================
// HIT FREEZE FUNCTIONS
// =============================================================================

/// Trigger hit freeze for given duration
pub fn trigger_hit_freeze(frames: u32) {
    unsafe {
        // Don't override a longer freeze
        if frames > HIT_FREEZE {
            HIT_FREEZE = frames;
        }
    }
}

/// Check if game is in hit freeze
pub fn is_frozen() -> bool {
    unsafe { HIT_FREEZE > 0 }
}

/// Update hit freeze (decrement each frame)
pub fn update_hit_freeze() {
    unsafe {
        HIT_FREEZE = HIT_FREEZE.saturating_sub(1);
    }
}

// =============================================================================
// SCREEN SHAKE STATE
// =============================================================================

/// Current shake intensity (0.0 - 1.0)
pub static mut SCREEN_SHAKE: f32 = 0.0;
/// Current X offset from shake
pub static mut SCREEN_SHAKE_X: f32 = 0.0;
/// Current Y offset from shake
pub static mut SCREEN_SHAKE_Y: f32 = 0.0;

// =============================================================================
// SCREEN SHAKE FUNCTIONS
// =============================================================================

/// Trigger screen shake with given intensity (0.0 - 1.0)
pub fn trigger_shake(intensity: f32) {
    unsafe {
        if !OPTIONS.screen_shake {
            return;
        }
        SCREEN_SHAKE = intensity.min(1.0);
    }
}

/// Update shake state (call each frame during Playing phase)
pub fn update_shake() {
    unsafe {
        if SCREEN_SHAKE > 0.01 {
            // Random offset based on intensity
            let shake_amount = SCREEN_SHAKE * 0.5; // Max 0.5 world units
            SCREEN_SHAKE_X = (crate::ffi::random_f32() - 0.5) * 2.0 * shake_amount;
            SCREEN_SHAKE_Y = (crate::ffi::random_f32() - 0.5) * 2.0 * shake_amount;
            // Decay shake over time
            SCREEN_SHAKE *= 0.85; // Quick falloff
        } else {
            SCREEN_SHAKE = 0.0;
            SCREEN_SHAKE_X = 0.0;
            SCREEN_SHAKE_Y = 0.0;
        }
    }
}

// =============================================================================
// IMPACT FLASH STATE
// =============================================================================

/// Impact flash countdown (white screen overlay when > 0)
pub static mut IMPACT_FLASH: u32 = 0;

// =============================================================================
// IMPACT FLASH FUNCTIONS
// =============================================================================

/// Trigger impact flash for 3 frames
pub fn trigger_impact_flash() {
    unsafe {
        if !OPTIONS.screen_flash {
            return;
        }
        IMPACT_FLASH = 3; // 3 frame flash
    }
}

/// Update impact flash (decrement each frame)
pub fn update_impact_flash() {
    unsafe {
        IMPACT_FLASH = IMPACT_FLASH.saturating_sub(1);
    }
}

// =============================================================================
// CAMERA ZOOM STATE
// =============================================================================

/// Default camera FOV
pub const CAMERA_FOV_DEFAULT: f32 = 50.0;
/// Zoomed in FOV for kill impact
pub const CAMERA_FOV_MIN: f32 = 40.0;

/// Current camera FOV
pub static mut CAMERA_FOV: f32 = CAMERA_FOV_DEFAULT;
/// Target camera FOV (for smooth interpolation)
pub static mut CAMERA_FOV_TARGET: f32 = CAMERA_FOV_DEFAULT;

/// Trigger camera zoom for kill impact
pub fn trigger_camera_zoom() {
    unsafe {
        CAMERA_FOV_TARGET = CAMERA_FOV_MIN;
        CAMERA_FOV = CAMERA_FOV_MIN; // Instant zoom on impact
    }
}

/// Update camera FOV (smooth return to default)
pub fn update_camera_fov() {
    unsafe {
        // When not frozen, gradually return FOV to default
        if !is_frozen() {
            CAMERA_FOV_TARGET = CAMERA_FOV_DEFAULT;
        }
        // Interpolate current FOV toward target
        CAMERA_FOV += (CAMERA_FOV_TARGET - CAMERA_FOV) * 0.15;
        // Snap to target if close enough
        if (CAMERA_FOV - CAMERA_FOV_TARGET).abs() < 0.1 {
            CAMERA_FOV = CAMERA_FOV_TARGET;
        }
    }
}

// =============================================================================
// EFFECT LIGHTS
// =============================================================================

/// Maximum number of dynamic effect lights
pub const MAX_EFFECT_LIGHTS: usize = 4;

/// A temporary point light for visual effects
#[derive(Clone, Copy)]
pub struct EffectLight {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: u32,
    pub intensity: f32,
    pub decay: f32, // Multiplied each frame (0.9 = slow, 0.7 = fast)
}

impl EffectLight {
    pub const fn new() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            color: 0xFFFFFFFF,
            intensity: 0.0,
            decay: 0.85,
        }
    }
}

/// Global effect lights pool
pub static mut EFFECT_LIGHTS: [EffectLight; MAX_EFFECT_LIGHTS] =
    [EffectLight::new(); MAX_EFFECT_LIGHTS];

/// Spawn an effect light at position with color
pub fn spawn_effect_light(x: f32, y: f32, color: u32, intensity: f32, decay: f32) {
    unsafe {
        for light in &mut EFFECT_LIGHTS {
            if !light.active {
                light.active = true;
                light.x = x;
                light.y = y;
                light.z = 1.0; // Slightly in front
                light.color = color;
                light.intensity = intensity;
                light.decay = decay;
                break;
            }
        }
    }
}

/// Update effect lights (decay and deactivate)
pub fn update_effect_lights() {
    unsafe {
        for light in &mut EFFECT_LIGHTS {
            if light.active {
                light.intensity *= light.decay;
                if light.intensity < 0.05 {
                    light.active = false;
                }
            }
        }
    }
}

// =============================================================================
// MATCH END ANIMATION STATE
// =============================================================================

/// Tick counter for match end screen animations
pub static mut MATCH_END_TICK: u32 = 0;

/// Reset match end tick (call when entering MatchEnd phase)
pub fn reset_match_end_tick() {
    unsafe {
        MATCH_END_TICK = 0;
    }
}

/// Increment match end tick
pub fn update_match_end_tick() {
    unsafe {
        MATCH_END_TICK += 1;
    }
}

pub fn register_deflect(player_idx: u32) {
    unsafe {
        DEFLECT_PLAYER = player_idx.min(3);
        DEFLECT_POPUP_TICKS = 45;
    }
}

pub fn update_deflect_popup() {
    unsafe {
        DEFLECT_POPUP_TICKS = DEFLECT_POPUP_TICKS.saturating_sub(1);
    }
}

// =============================================================================
// STAGE TRANSITION STATE
// =============================================================================

/// Transition state for stage changes
#[derive(Clone, Copy, PartialEq)]
pub enum TransitionPhase {
    None,
    FadeOut,
    FadeIn,
}

/// Current transition phase
pub static mut TRANSITION_PHASE: TransitionPhase = TransitionPhase::None;
/// Transition progress (0.0 to 1.0)
pub static mut TRANSITION_PROGRESS: f32 = 0.0;
/// Transition speed (progress per frame)
pub const TRANSITION_SPEED: f32 = 0.05;

/// Start a fade-out transition
pub fn start_transition_out() {
    unsafe {
        TRANSITION_PHASE = TransitionPhase::FadeOut;
        TRANSITION_PROGRESS = 0.0;
    }
}

/// Start a fade-in transition
pub fn start_transition_in() {
    unsafe {
        TRANSITION_PHASE = TransitionPhase::FadeIn;
        TRANSITION_PROGRESS = 1.0;
    }
}

/// Update transition state
pub fn update_transition() -> bool {
    unsafe {
        match TRANSITION_PHASE {
            TransitionPhase::None => false,
            TransitionPhase::FadeOut => {
                TRANSITION_PROGRESS += TRANSITION_SPEED;
                if TRANSITION_PROGRESS >= 1.0 {
                    TRANSITION_PROGRESS = 1.0;
                    TRANSITION_PHASE = TransitionPhase::None;
                    true // Fade out complete
                } else {
                    false
                }
            }
            TransitionPhase::FadeIn => {
                TRANSITION_PROGRESS -= TRANSITION_SPEED;
                if TRANSITION_PROGRESS <= 0.0 {
                    TRANSITION_PROGRESS = 0.0;
                    TRANSITION_PHASE = TransitionPhase::None;
                    true // Fade in complete
                } else {
                    false
                }
            }
        }
    }
}

/// Check if currently transitioning
pub fn is_transitioning() -> bool {
    unsafe { TRANSITION_PHASE != TransitionPhase::None }
}
