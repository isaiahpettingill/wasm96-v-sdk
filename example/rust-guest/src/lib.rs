#![no_std]

// Minimal wasm96 Rust guest example (Immediate Mode).
//
// This crate is meant to be compiled to `wasm32-unknown-unknown` and loaded by `wasm96-core`.
//
// The host calls:
// - `setup()` once at startup.
// - `update()` once per frame.
// - `draw()` once per frame.

use wasm96_sdk::prelude::*;

static mut RECT_X: i32 = 10;
static mut RECT_Y: i32 = 10;
static mut VEL_X: i32 = 2;
static mut VEL_Y: i32 = 2;

// Keyed resources: the host identifies fonts by string keys.
const FONT_KEY_SPLEEN_16: &str = "font/spleen/16";

#[unsafe(no_mangle)]
pub extern "C" fn setup() {
    // Initialize screen size
    graphics::set_size(320, 240);

    // Register a built-in Spleen font under a stable key.
    // Guests can reuse the same key every run; the host manages the resource table.
    graphics::font_register_spleen(FONT_KEY_SPLEEN_16, 16);

    // Initialize audio (optional)
    audio::init(44100);
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    // Update game state
    unsafe {
        RECT_X += VEL_X;
        RECT_Y += VEL_Y;

        if RECT_X <= 0 || RECT_X >= 290 {
            VEL_X = -VEL_X;
        }
        if RECT_Y <= 0 || RECT_Y >= 210 {
            VEL_Y = -VEL_Y;
        }
    }

    // NOTE:
    // The core is responsible for padding/handling audio when the guest produces too little.
    // Guests shouldn't need to push silence just to keep the runtime happy.
}

#[unsafe(no_mangle)]
pub extern "C" fn draw() {
    // 1. Clear background
    graphics::background(20, 20, 40);
    graphics::text_key(100, 100, FONT_KEY_SPLEEN_16, "Hello");

    // 2. Draw moving rectangle
    graphics::set_color(255, 100, 100, 255);
    unsafe {
        graphics::rect(RECT_X, RECT_Y, 30, 30);

        // Draw outline
        graphics::set_color(255, 255, 255, 255);
        graphics::rect_outline(RECT_X, RECT_Y, 30, 30);
    }

    // 3. Draw circle at mouse position
    let mx = input::get_mouse_x();
    let my = input::get_mouse_y();

    if input::is_mouse_down(0) {
        graphics::set_color(255, 255, 0, 255); // Yellow if clicked
    } else {
        graphics::set_color(100, 255, 100, 255); // Green otherwise
    }
    graphics::circle(mx, my, 15);

    // Draw crosshair lines
    graphics::set_color(255, 255, 255, 100);
    graphics::line(mx - 20, my, mx + 20, my);
    graphics::line(mx, my - 20, mx, my + 20);

    // 4. Check joypad input
    if input::is_button_down(0, Button::A) {
        graphics::set_color(0, 0, 255, 255);
        graphics::rect(280, 200, 20, 20);
    }
}
