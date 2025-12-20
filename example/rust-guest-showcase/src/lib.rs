#![no_std]

extern crate alloc;

// Comprehensive wasm96 Rust guest showcase example.
//
// This example demonstrates all available functionality:
// - Graphics: all shapes, images, SVGs, GIFs, fonts, text
// - Input: 4 controllers, keyboard, mouse
// - Audio: simple generated audio
//
// Assets are hardcoded small examples.

use wasm96_sdk::prelude::*;

// Load assets from files
const SVG_DATA: &[u8] = include_bytes!("assets/man.svg");
const GIF_DATA: &[u8] = include_bytes!("assets/200.gif");
const PNG_DATA: &[u8] = include_bytes!("assets/ink.png");
// Load TTF font from file
const TTF_DATA: &[u8] = include_bytes!("assets/UnifrakturMaguntia-Regular.ttf");
const WAV_DATA: &[u8] = include_bytes!("assets/crickets.wav");

static mut FRAME_COUNT: u32 = 0;

// Keyed resources: avoid numeric handles + global mutable IDs.
// These keys must be stable and are used by the host as identifiers.
const SVG_KEY: &str = "showcase/man.svg";
const GIF_KEY: &str = "showcase/200.gif";
const PNG_KEY: &str = "showcase/ink.png";
const FONT_KEY: &str = "showcase/unifraktur.ttf";

#[unsafe(no_mangle)]
pub extern "C" fn setup() {
    // Set screen size
    graphics::set_size(1200, 800);

    // Initialize audio
    audio::init(44100);

    // Play looping WAV
    audio::play_wav(WAV_DATA);

    // Register keyed resources with the host (no numeric handles).
    let _ = graphics::svg_register(SVG_KEY, SVG_DATA);
    let _ = graphics::gif_register(GIF_KEY, GIF_DATA);
    let _ = graphics::png_register(PNG_KEY, PNG_DATA);
    let _ = graphics::font_register_ttf(FONT_KEY, TTF_DATA);
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    unsafe {
        FRAME_COUNT += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn draw() {
    // Clear background with gradient
    let time = unsafe { FRAME_COUNT } as f32 * 0.01;
    let r = ((time.sin() + 1.0) * 127.5) as u8;
    let g = ((time.cos() + 1.0) * 127.5) as u8;
    let b = 100;

    graphics::background(r, g, b);

    // Draw keyed PNG (registered in setup)
    graphics::png_draw_key(PNG_KEY, 100, 100);

    // Draw raw PNG bytes directly (one-shot)
    graphics::image_png(250, 100, PNG_DATA);

    // Draw keyed SVG + GIF
    graphics::svg_draw_key(SVG_KEY, 420, 60, 320, 320);
    graphics::gif_draw_key_scaled(GIF_KEY, 40, 320, 160, 120);

    // Draw keyed text (registered font)
    graphics::text_key(24, 24, FONT_KEY, "wasm96 showcase");
}
