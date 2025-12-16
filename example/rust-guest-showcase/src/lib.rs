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

static mut SVG_ID: u32 = 0;
static mut GIF_ID: u32 = 0;
static mut FONT_ID: u32 = 0;

static mut FRAME_COUNT: u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn setup() {
    // Set screen size
    graphics::set_size(800, 600);

    // Initialize audio
    audio::init(44100);

    // Play looping WAV
    audio::play_wav(WAV_DATA);

    // Load SVG
    unsafe {
        SVG_ID = graphics::svg_create(SVG_DATA);
    }

    // Load GIF
    unsafe {
        GIF_ID = graphics::gif_create(GIF_DATA);
    }

    // Load TTF font
    unsafe {
        FONT_ID = graphics::font_upload_ttf(TTF_DATA);
    }
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

    // Draw all shapes
    draw_shapes();

    // Draw images and assets
    draw_assets();

    // Draw text
    draw_text();

    // Draw input status
    draw_input_status();
}

fn draw_shapes() {
    // Points
    graphics::set_color(255, 255, 255, 255);
    for i in 0..100 {
        let x = (i * 8) % 800;
        let y = 50 + (i / 100) * 10;
        graphics::point(x as i32, y as i32);
    }

    // Lines
    graphics::set_color(255, 0, 0, 255);
    graphics::line(10, 70, 100, 120);
    graphics::line(100, 70, 10, 120);

    // Rectangles
    graphics::set_color(0, 255, 0, 255);
    graphics::rect(120, 70, 50, 50);
    graphics::set_color(255, 255, 0, 255);
    graphics::rect_outline(180, 70, 50, 50);

    // Circles
    graphics::set_color(0, 0, 255, 255);
    graphics::circle(250, 95, 25);
    graphics::set_color(255, 0, 255, 255);
    graphics::circle_outline(320, 95, 25);

    // Triangles
    graphics::set_color(255, 165, 0, 255);
    graphics::triangle(380, 70, 430, 70, 405, 120);
    graphics::set_color(0, 255, 255, 255);
    graphics::triangle_outline(440, 70, 490, 70, 465, 120);

    // Bezier curves
    graphics::set_color(128, 0, 128, 255);
    graphics::bezier_quadratic(10, 140, 50, 160, 90, 140, 20);
    graphics::bezier_cubic(100, 140, 120, 160, 140, 160, 160, 140, 20);

    // Pills
    graphics::set_color(255, 192, 203, 255);
    graphics::pill(200, 140, 80, 40);
    graphics::set_color(0, 128, 0, 255);
    graphics::pill_outline(300, 140, 80, 40);
}

fn draw_assets() {
    // Draw SVG
    unsafe {
        if SVG_ID != 0 {
            graphics::svg_draw(SVG_ID, 400, 200, 100, 100);
        }
    }

    // Draw GIF
    unsafe {
        if GIF_ID != 0 {
            graphics::gif_draw_scaled(GIF_ID, 520, 200, 100, 100);
        }
    }

    // Draw PNG as image (assuming small size, adjust as needed)
    graphics::image(10, 200, 100, 100, PNG_DATA);
}

fn draw_text() {
    // Use loaded TTF font
    unsafe {
        if FONT_ID != 0 {
            graphics::set_color(255, 255, 255, 255);
            graphics::text(10, 320, FONT_ID, "Hello from wasm96!");
            graphics::text(10, 350, FONT_ID, "This is a TTF font.");
        }
    }

    // Use built-in Spleen font
    let spleen_font = graphics::font_use_spleen(16);
    graphics::set_color(255, 255, 0, 255);
    graphics::text(
        10,
        380,
        spleen_font,
        "Spleen font: ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    );
    graphics::text(
        10,
        400,
        spleen_font,
        "abcdefghijklmnopqrstuvwxyz 0123456789",
    );
}

fn draw_input_status() {
    graphics::set_color(255, 255, 255, 255);
    let spleen_font = graphics::font_use_spleen(8);

    for port in 0..4 {
        let y = 430 + port * 40;
        graphics::text(10, y, spleen_font, &alloc::format!("Controller {}:", port));

        let buttons = [
            (Button::A, "A"),
            (Button::B, "B"),
            (Button::X, "X"),
            (Button::Y, "Y"),
            (Button::Start, "Start"),
            (Button::Select, "Select"),
            (Button::Up, "Up"),
            (Button::Down, "Down"),
            (Button::Left, "Left"),
            (Button::Right, "Right"),
            (Button::L1, "L1"),
            (Button::R1, "R1"),
            (Button::L2, "L2"),
            (Button::R2, "R2"),
            (Button::L3, "L3"),
            (Button::R3, "R3"),
        ];

        let mut x = 100;
        for (btn, name) in buttons.iter() {
            if input::is_button_down(port as u32, *btn) {
                graphics::set_color(0, 255, 0, 255);
            } else {
                graphics::set_color(128, 128, 128, 255);
            }
            graphics::text(x, y, spleen_font, name);
            x += 30;
        }
    }

    // Mouse
    let mx = input::get_mouse_x();
    let my = input::get_mouse_y();
    graphics::set_color(255, 0, 0, 255);
    graphics::text(
        10,
        590,
        spleen_font,
        &alloc::format!("Mouse: ({}, {})", mx, my),
    );
    if input::is_mouse_down(0) {
        graphics::text(200, 590, spleen_font, "Left Click");
    }
    if input::is_mouse_down(1) {
        graphics::text(280, 590, spleen_font, "Right Click");
    }
}
