//! Particle system for visual effects

use crate::ffi::*;

/// Maximum particles in pool (increased for trails and sparks)
pub const MAX_PARTICLES: usize = 128;

/// Individual particle
#[derive(Clone, Copy)]
pub struct Particle {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub lifetime: u32,
    pub max_lifetime: u32,
    pub color: u32,
    pub size: f32,
}

impl Particle {
    pub const fn new() -> Self {
        Particle {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            lifetime: 0,
            max_lifetime: 0,
            color: 0xFFFFFFFF,
            size: 0.2,
        }
    }
}

/// Global particle pool
pub static mut PARTICLES: [Particle; MAX_PARTICLES] = [Particle::new(); MAX_PARTICLES];

/// Spawn death explosion particles at position with color
pub fn spawn_death_particles(x: f32, y: f32, color: u32) {
    unsafe {
        let particle_count = 12; // Number of particles per death

        for i in 0..particle_count {
            // Find inactive particle
            for p in &mut PARTICLES {
                if !p.active {
                    p.active = true;
                    p.x = x;
                    p.y = y;

                    // Random velocity in all directions
                    let angle = (i as f32 / particle_count as f32) * core::f32::consts::TAU
                        + random_f32() * 0.5;
                    let speed = 0.1 + random_f32() * 0.15;
                    p.vx = libm::cosf(angle) * speed;
                    p.vy = libm::sinf(angle) * speed + 0.05; // Slight upward bias

                    p.lifetime = 30 + (random_f32() * 20.0) as u32; // 30-50 frames
                    p.max_lifetime = p.lifetime;
                    p.color = color;
                    p.size = 0.15 + random_f32() * 0.1;
                    break;
                }
            }
        }
    }
}

/// Update all particles
pub fn update_particles() {
    unsafe {
        for p in &mut PARTICLES {
            if p.active {
                // Apply velocity
                p.x += p.vx;
                p.y += p.vy;

                // Apply gravity
                p.vy -= 0.005;

                // Apply friction
                p.vx *= 0.98;
                p.vy *= 0.98;

                // Decrement lifetime
                if p.lifetime > 0 {
                    p.lifetime -= 1;
                } else {
                    p.active = false;
                }
            }
        }
    }
}

/// Clear all particles
pub fn clear_particles() {
    unsafe {
        for p in &mut PARTICLES {
            p.active = false;
        }
    }
}

/// Spawn landing dust particles
pub fn spawn_landing_dust(x: f32, y: f32) {
    unsafe {
        let particle_count = 4;
        for _ in 0..particle_count {
            for p in &mut PARTICLES {
                if !p.active {
                    p.active = true;
                    p.x = x + (random_f32() - 0.5) * 0.5;
                    p.y = y;
                    p.vx = (random_f32() - 0.5) * 0.1;
                    p.vy = random_f32() * 0.05;
                    p.lifetime = 10 + (random_f32() * 5.0) as u32;
                    p.max_lifetime = p.lifetime;
                    p.color = 0xAAAAAAAA; // Gray dust
                    p.size = 0.1;
                    break;
                }
            }
        }
    }
}

/// Spawn victory confetti across the screen
pub fn spawn_victory_confetti(winner_color: u32) {
    unsafe {
        // Spawn lots of confetti
        let particle_count = 40;
        let colors = [
            winner_color,
            0xFFFF00FF, // Yellow
            0x00FF00FF, // Green
            0xFF00FFFF, // Magenta
            0x00FFFFFF, // Cyan
            0xFFFFFFFF, // White
        ];

        for i in 0..particle_count {
            for p in &mut PARTICLES {
                if !p.active {
                    p.active = true;
                    // Spawn across top of screen
                    p.x = random_f32() * 16.0 - 8.0; // -8 to 8
                    p.y = 8.0 + random_f32() * 2.0; // Top of screen
                    p.vx = (random_f32() - 0.5) * 0.1;
                    p.vy = -random_f32() * 0.15 - 0.05; // Fall down
                    p.lifetime = 120 + (random_f32() * 60.0) as u32; // 2-3 seconds
                    p.max_lifetime = p.lifetime;
                    p.color = colors[i % colors.len()];
                    p.size = 0.1 + random_f32() * 0.1;
                    break;
                }
            }
        }
    }
}

/// Spawn bullet trail particle behind a moving bullet
pub fn spawn_bullet_trail(x: f32, y: f32) {
    unsafe {
        // Find an inactive particle
        for p in &mut PARTICLES {
            if !p.active {
                p.active = true;
                p.x = x + (random_f32() - 0.5) * 0.1;
                p.y = y + (random_f32() - 0.5) * 0.1;
                p.vx = (random_f32() - 0.5) * 0.02;
                p.vy = (random_f32() - 0.5) * 0.02;
                p.lifetime = 8;
                p.max_lifetime = 8;
                // Yellow/orange trail
                p.color = if random_f32() > 0.5 {
                    0xFFFF00FF
                } else {
                    0xFFAA00FF
                };
                p.size = 0.06;
                break;
            }
        }
    }
}

/// Spawn wall slide sparks when player slides down wall
/// wall_on_right: true if wall is on player's right side
pub fn spawn_wall_slide_sparks(x: f32, y: f32, wall_on_right: bool) {
    unsafe {
        // Only spawn occasionally (1 in 3 frames)
        if random_f32() > 0.33 {
            return;
        }

        for p in &mut PARTICLES {
            if !p.active {
                p.active = true;
                p.x = x;
                p.y = y + (random_f32() - 0.5) * 0.3;
                // Sparks fly away from wall
                let direction = if wall_on_right { -1.0 } else { 1.0 };
                p.vx = direction * (0.05 + random_f32() * 0.05);
                p.vy = random_f32() * 0.03;
                p.lifetime = 6;
                p.max_lifetime = 6;
                // White/gray sparks
                p.color = if random_f32() > 0.5 {
                    0xFFFFFFFF
                } else {
                    0xCCCCCCFF
                };
                p.size = 0.05;
                break;
            }
        }
    }
}
