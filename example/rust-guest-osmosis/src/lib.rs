use libm::{cosf, logf, sinf};
use std::sync::Mutex;
use wasm96_sdk::prelude::*;

// --- Constants ---
const WORLD_WIDTH: f32 = 2000.0;
const WORLD_HEIGHT: f32 = 2000.0;
const VIEWPORT_WIDTH: u32 = 800;
const VIEWPORT_HEIGHT: u32 = 600;

const PLAYER_START_RADIUS: f32 = 30.0;
const EJECT_RADIUS: f32 = 2.0;
const EJECT_SPEED: f32 = 300.0; // Speed of ejected particle relative to player
const MIN_PLAYER_RADIUS: f32 = 3.0;

const MIN_ENEMY_RADIUS: f32 = 1.0;

const FONT_KEY: &str = "font/spleen/16";
const DEBUG_FONT_KEY: &str = "font/spleen/16";
const CENTER_X: f32 = WORLD_WIDTH / 2.0;
const CENTER_Y: f32 = WORLD_HEIGHT / 2.0;

// --- Game State ---

#[derive(Clone, Copy, PartialEq)]
struct Circle {
    id: u32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    radius: f32,
    color: u32, // 0xRRGGBBAA
    is_player: bool,
    to_remove: bool,
}

struct GameState {
    circles: Vec<Circle>,
    next_id: u32,
    camera_x: f32,
    camera_y: f32,
    game_over: bool,
    win: bool,
    rng_seed: u32,
    aim_dx: f32,
    aim_dy: f32,
    zoom: f32,
    cursor_angle: f32,
}

// Global state protected by Mutex
static STATE: Mutex<Option<GameState>> = Mutex::new(None);

// --- RNG Helpers ---

fn rand(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    *seed
}

fn rand_f32(seed: &mut u32) -> f32 {
    (rand(seed) as f32) / (u32::MAX as f32)
}

fn rand_range(seed: &mut u32, min: f32, max: f32) -> f32 {
    min + rand_f32(seed) * (max - min)
}

fn rand_color(seed: &mut u32) -> u32 {
    let r = rand_range(seed, 50.0, 255.0) as u32;
    let g = rand_range(seed, 50.0, 255.0) as u32;
    let b = rand_range(seed, 50.0, 255.0) as u32;
    (r << 24) | (g << 16) | (b << 8) | 255
}

fn rand_normal(seed: &mut u32, mean: f32, std_dev: f32) -> f32 {
    // Box-Muller transform
    let u1 = rand_f32(seed);
    let u2 = rand_f32(seed);
    let z0 = (-2.0 * logf(u1)).sqrt() * cosf(2.0 * core::f32::consts::PI * u2);
    mean + z0 * std_dev
}

// --- Implementation ---

impl Circle {
    fn mass(&self) -> f32 {
        self.radius * self.radius
    }

    fn update(&mut self, dt: f32) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // Friction
        self.vx *= 0.995;
        self.vy *= 0.995;

        // Wall bouncing
        if self.x < self.radius {
            self.x = self.radius;
            self.vx = -self.vx;
        } else if self.x > WORLD_WIDTH - self.radius {
            self.x = WORLD_WIDTH - self.radius;
            self.vx = -self.vx;
        }

        if self.y < self.radius {
            self.y = self.radius;
            self.vy = -self.vy;
        } else if self.y > WORLD_HEIGHT - self.radius {
            self.y = WORLD_HEIGHT - self.radius;
            self.vy = -self.vy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setup() {
    graphics::set_size(VIEWPORT_WIDTH, VIEWPORT_HEIGHT);
    // Register font, but the game must still be operable if this fails.
    let _font_ok = graphics::font_register_spleen(FONT_KEY, 16);

    // Initialize Game State - start directly in gameplay
    let mut state = GameState {
        circles: Vec::new(),
        next_id: 0,
        camera_x: WORLD_WIDTH / 2.0,
        camera_y: WORLD_HEIGHT / 2.0,
        game_over: false,
        win: false,
        rng_seed: 987654321,
        aim_dx: 1.0,
        aim_dy: 0.0,
        zoom: 1.0,
        cursor_angle: 0.0,
    };

    // Start the game immediately
    unsafe {
        setup_game(&mut state);
    }

    *STATE.lock().unwrap() = Some(state);
}

unsafe fn setup_game(state: &mut GameState) {
    // Font is registered once in setup(). Do not re-register here.
    state.circles.clear();
    state.next_id = 0;

    // Spawn Player
    let player_x = WORLD_WIDTH / 2.0;
    let player_y = WORLD_HEIGHT / 2.0;
    state.circles.push(Circle {
        id: state.next_id,
        x: player_x,
        y: player_y,
        vx: 0.0,
        vy: 0.0,
        radius: PLAYER_START_RADIUS,
        color: 0x00AAFFFF, // Cyan
        is_player: true,
        to_remove: false,
    });
    state.next_id += 1;

    // Spawn Enemies
    for _ in 0..50 {
        let x = rand_range(&mut state.rng_seed, 0.0, WORLD_WIDTH);
        let y = rand_range(&mut state.rng_seed, 0.0, WORLD_HEIGHT);
        let vx = rand_range(&mut state.rng_seed, -50.0, 50.0);
        let vy = rand_range(&mut state.rng_seed, -50.0, 50.0);
        let radius = rand_normal(&mut state.rng_seed, 25.0, 10.0).clamp(MIN_ENEMY_RADIUS, 50.0);
        let color = rand_color(&mut state.rng_seed);

        state.circles.push(Circle {
            id: state.next_id,
            x,
            y,
            vx,
            vy,
            radius,
            color,
            is_player: false,
            to_remove: false,
        });
        state.next_id += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let mut lock = STATE.lock().unwrap();
    let state = match lock.as_mut() {
        Some(s) => s,
        None => return,
    };

    let dt = 0.016; // Approx 60 FPS

    // Handle zoom always (except in menu)
    if input::is_button_down(0, Button::X) {
        state.zoom *= 1.05;
        if state.zoom > 4.0 {
            state.zoom = 4.0;
        }
    }
    if input::is_button_down(0, Button::Y) {
        state.zoom /= 1.05;
        if state.zoom < 0.25 {
            state.zoom = 0.25;
        }
    }

    if state.game_over || state.win {
        if input::is_button_down(0, Button::A) {
            // Restart the game
            state.game_over = false;
            state.win = false;
            state.zoom = 1.0;
            state.aim_dx = 1.0;
            state.aim_dy = 0.0;
            state.cursor_angle = 0.0;
            unsafe {
                setup_game(state);
            }
        }
        return;
    }

    // 1. Handle Controller Input
    // D-pad Left/Right for rotating cursor
    let delta_angle = 0.1;
    if input::is_button_down(0, Button::Left) {
        state.cursor_angle -= delta_angle;
    }
    if input::is_button_down(0, Button::Right) {
        state.cursor_angle += delta_angle;
    }
    // Update aim direction
    state.aim_dx = cosf(state.cursor_angle);
    state.aim_dy = sinf(state.cursor_angle);

    // 2. Handle Player Input (Ejection)
    // We need to find the player index first
    let mut player_idx = None;
    for (i, c) in state.circles.iter().enumerate() {
        if c.is_player {
            player_idx = Some(i);
            break;
        }
    }

    let mut new_particles = Vec::new();

    if let Some(pidx) = player_idx {
        let p = &mut state.circles[pidx];

        // Determine aim direction: controller or mouse
        let (dir_x, dir_y) = if state.aim_dx != 0.0 || state.aim_dy != 0.0 {
            (state.aim_dx, state.aim_dy)
        } else {
            // Fallback to mouse
            let mx = input::get_mouse_x() as f32;
            let my = input::get_mouse_y() as f32;
            let screen_cx = (VIEWPORT_WIDTH / 2) as f32;
            let screen_cy = (VIEWPORT_HEIGHT / 2) as f32;
            let dx = mx - screen_cx;
            let dy = my - screen_cy;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                (dx / len, dy / len)
            } else {
                (0.0, 1.0)
            }
        };

        // Eject or brake
        let eject_forward = input::is_button_down(0, Button::A)
            || input::is_mouse_down(0)
            || input::is_key_down(32);
        let eject_backward = input::is_button_down(0, Button::B);
        let eject = eject_forward || eject_backward;

        if eject && p.radius > MIN_PLAYER_RADIUS + EJECT_RADIUS {
            let actual_dir_x = if eject_backward { -dir_x } else { dir_x };
            let actual_dir_y = if eject_backward { -dir_y } else { dir_y };

            // Physics: Conservation of Momentum & Mass
            let m_e = EJECT_RADIUS * EJECT_RADIUS;
            let m_old = p.radius * p.radius;
            let m_new = m_old - m_e;
            let r_new = m_new.sqrt();

            // Eject velocity: backwards relative to aim
            let eject_speed = EJECT_SPEED;
            let v_e_x = p.vx - actual_dir_x * eject_speed;
            let v_e_y = p.vy - actual_dir_y * eject_speed;

            // New player velocity
            let v_new_x = (m_old * p.vx - m_e * v_e_x) / m_new;
            let v_new_y = (m_old * p.vy - m_e * v_e_y) / m_new;

            // Update Player
            p.radius = r_new;
            p.vx = v_new_x;
            p.vy = v_new_y;

            // Spawn Particle
            let spawn_dist = p.radius + EJECT_RADIUS + 2.0;
            new_particles.push(Circle {
                id: state.next_id,
                x: p.x - actual_dir_x * spawn_dist,
                y: p.y - actual_dir_y * spawn_dist,
                vx: v_e_x,
                vy: v_e_y,
                radius: EJECT_RADIUS,
                color: p.color, // Same color as player
                is_player: false,
                to_remove: false,
            });
            state.next_id += 1;
        }
    } else {
        state.game_over = true;
    }

    state.circles.append(&mut new_particles);

    // 2. Update Physics & AI
    // Split borrow to allow RNG usage
    let rng_seed = &mut state.rng_seed;
    let circles = &mut state.circles;

    for c in circles.iter_mut() {
        // Simple AI for enemies: drift towards center if too far, otherwise random drift
        if !c.is_player {
            // Very dumb AI: just keep moving, maybe accelerate slightly randomly
            c.vx += rand_range(rng_seed, -1.0, 1.0);
            c.vy += rand_range(rng_seed, -1.0, 1.0);

            // Cap speed
            let speed = (c.vx * c.vx + c.vy * c.vy).sqrt();
            if speed > 100.0 {
                c.vx = (c.vx / speed) * 100.0;
                c.vy = (c.vy / speed) * 100.0;
            }
        }
        c.update(dt);

        // Circular Arena Bounce
        let dx = c.x - CENTER_X;
        let dy = c.y - CENTER_Y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist + c.radius > 900.0 {
            if dist > 0.001 {
                let nx = dx / dist;
                let ny = dy / dist;
                // Reflect velocity
                let dot = c.vx * nx + c.vy * ny;
                c.vx -= 2.0 * dot * nx;
                c.vy -= 2.0 * dot * ny;
                // Push back inside
                let overlap = (dist + c.radius) - 900.0;
                c.x -= nx * overlap;
                c.y -= ny * overlap;
            }
        }
    }

    // 3. Collision Detection (Absorption)
    // O(N^2) is fine for N=50-100
    let len = state.circles.len();
    for i in 0..len {
        for j in (i + 1)..len {
            let (c1, c2) = unsafe {
                let ptr = state.circles.as_mut_ptr();
                (&mut *ptr.add(i), &mut *ptr.add(j))
            };

            if c1.to_remove || c2.to_remove {
                continue;
            }

            let dx = c1.x - c2.x;
            let dy = c1.y - c2.y;
            let dist_sq = dx * dx + dy * dy;
            let r_sum = c1.radius + c2.radius;

            if dist_sq < r_sum * r_sum {
                // Collision!
                if c1.radius > c2.radius {
                    absorb(c1, c2);
                } else if c2.radius > c1.radius {
                    absorb(c2, c1);
                } else {
                    // Same size, absorb based on id
                    if c1.id > c2.id {
                        absorb(c1, c2);
                    } else {
                        absorb(c2, c1);
                    }
                }
            }
        }
    }

    // Remove dead circles
    state.circles.retain(|c| !c.to_remove);

    // 4. Update Camera
    // Find player again
    let mut player_exists = false;
    let mut biggest_radius = 0.0;
    let mut player_radius = 0.0;

    for c in &state.circles {
        if c.radius > biggest_radius {
            biggest_radius = c.radius;
        }
        if c.is_player {
            player_exists = true;
            player_radius = c.radius;
            // Smooth follow
            state.camera_x = state.camera_x + (c.x - state.camera_x) * 0.1;
            state.camera_y = state.camera_y + (c.y - state.camera_y) * 0.1;
        }
    }

    if !player_exists {
        state.game_over = true;
    } else if player_radius >= biggest_radius && state.circles.len() > 1 {
        // Winning condition check
    }

    if state.circles.len() == 1 && player_exists {
        state.win = true;
    }
}

fn absorb(eater: &mut Circle, eaten: &mut Circle) {
    // Conservation of Mass: Area adds up
    // R_new = sqrt(R1^2 + R2^2)
    let m1 = eater.mass();
    let m2 = eaten.mass();
    let m_new = m1 + m2;
    eater.radius = m_new.sqrt();

    // Conservation of Momentum (Inelastic)
    // V_new = (m1*v1 + m2*v2) / (m1+m2)
    eater.vx = (m1 * eater.vx + m2 * eaten.vx) / m_new;
    eater.vy = (m1 * eater.vy + m2 * eaten.vy) / m_new;

    eaten.to_remove = true;
}

#[unsafe(no_mangle)]
pub extern "C" fn draw() {
    let lock = STATE.lock().unwrap();
    let state = match lock.as_ref() {
        Some(s) => s,
        None => return,
    };

    graphics::background(20, 20, 30);

    let cx = state.camera_x;
    let cy = state.camera_y;
    let zoom = state.zoom;
    let half_w = VIEWPORT_WIDTH as f32 / 2.0;
    let half_h = VIEWPORT_HEIGHT as f32 / 2.0;

    // Draw Grid
    graphics::set_color(40, 40, 50, 255);
    let grid_size = 100.0;
    let world_half_w = half_w / zoom;
    let world_half_h = half_h / zoom;
    let start_x = ((cx - world_half_w) / grid_size).floor() * grid_size;
    let start_y = ((cy - world_half_h) / grid_size).floor() * grid_size;

    let mut x = start_x;
    while x < cx + world_half_w + grid_size {
        let screen_x = ((x - cx) * zoom + half_w) as i32;
        graphics::line(screen_x, 0, screen_x, VIEWPORT_HEIGHT as i32);
        x += grid_size;
    }
    let mut y = start_y;
    while y < cy + world_half_h + grid_size {
        let screen_y = ((y - cy) * zoom + half_h) as i32;
        graphics::line(0, screen_y, VIEWPORT_WIDTH as i32, screen_y);
        y += grid_size;
    }

    // Draw Arena Bounds
    graphics::set_color(255, 0, 0, 255);
    let arena_center_x = (CENTER_X - cx) * zoom + half_w;
    let arena_center_y = (CENTER_Y - cy) * zoom + half_h;
    let arena_r = (900.0 * zoom) as u32;
    graphics::circle_outline(arena_center_x as i32, arena_center_y as i32, arena_r);

    // Draw Circles
    // Find player radius for color comparison
    let player_r = state
        .circles
        .iter()
        .find(|c| c.is_player)
        .map(|c| c.radius)
        .unwrap_or(0.0);

    for c in &state.circles {
        let world_x = c.x - cx;
        let world_y = c.y - cy;
        let screen_x = (world_x * zoom + half_w) as i32;
        let screen_y = (world_y * zoom + half_h) as i32;
        let r_scaled = (c.radius * zoom) as u32;
        let r_i32 = r_scaled as i32;

        // Optimization: Don't draw if off screen
        if screen_x + r_i32 < 0
            || screen_x - r_i32 > VIEWPORT_WIDTH as i32
            || screen_y + r_i32 < 0
            || screen_y - r_i32 > VIEWPORT_HEIGHT as i32
        {
            continue;
        }

        if c.is_player {
            graphics::set_color(0, 255, 255, 255); // Bright cyan
        } else {
            // Color code based on danger
            if c.radius > player_r {
                graphics::set_color(255, 50, 50, 255); // Dangerous
            } else {
                graphics::set_color(50, 255, 50, 255); // Edible
            }
        }

        graphics::circle(screen_x, screen_y, r_scaled);

        // Outline - brighter for player
        if c.is_player {
            graphics::set_color(255, 255, 255, 255);
            graphics::circle_outline(screen_x, screen_y, r_scaled);
            // Draw thicker outline for player
            graphics::circle_outline(screen_x, screen_y, (r_scaled as i32 - 2).max(1) as u32);
        } else {
            graphics::set_color(255, 255, 255, 100);
            graphics::circle_outline(screen_x, screen_y, r_scaled);
        }

        // Draw cursor if this is player
        if c.is_player {
            let cursor_length = (c.radius * zoom * 1.5) as i32;
            let cursor_x = screen_x + (state.aim_dx * cursor_length as f32) as i32;
            let cursor_y = screen_y + (state.aim_dy * cursor_length as f32) as i32;
            graphics::set_color(255, 255, 0, 255); // Yellow cursor
            graphics::circle(cursor_x, cursor_y, 5);
            // Draw line from player to cursor
            graphics::set_color(255, 255, 0, 200);
            graphics::line(screen_x, screen_y, cursor_x, cursor_y);
        }
    }

    // UI
    graphics::set_color(255, 255, 255, 255);
    if state.game_over {
        graphics::text_key(300, 250, DEBUG_FONT_KEY, "GAME OVER");
        graphics::text_key(260, 280, DEBUG_FONT_KEY, "Press A to Restart");
    } else if state.win {
        graphics::text_key(300, 250, DEBUG_FONT_KEY, "YOU WIN!");
        graphics::text_key(260, 280, DEBUG_FONT_KEY, "Press A to Restart");
    } else {
        graphics::text_key(10, 10, DEBUG_FONT_KEY, "Osmosis Clone");
        graphics::text_key(10, 30, DEBUG_FONT_KEY, "D-pad L/R: Rotate Aim");
        graphics::text_key(10, 50, DEBUG_FONT_KEY, "A: Eject, B: Brake");
        graphics::text_key(10, 70, DEBUG_FONT_KEY, "X/Y: Zoom In/Out");
        graphics::text_key(10, 90, DEBUG_FONT_KEY, "Absorb smaller Green circles");
        graphics::text_key(10, 110, DEBUG_FONT_KEY, "Avoid larger Red circles");
    }
}
