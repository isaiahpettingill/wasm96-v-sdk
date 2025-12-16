//! Audio/Video implementation for wasm96-core (Immediate Mode).
//!
//! This module implements the host-side drawing commands and audio handling.
//!
//! - Graphics: The host maintains a `Vec<u32>` framebuffer (XRGB8888).
//!   Guest commands modify this buffer.
//!   `video_present_host` sends it to libretro.
//!
//! - Audio:
//!   - Guests may push raw i16 samples (`audio_push_samples`) into `audio.host_queue`.
//!   - The host may also manage “channels/voices” (decoded assets and chiptune synth voices)
//!     stored in `state::AudioState` and mixed here.
//!   - `audio_drain_host` mixes everything into a single interleaved stereo i16 buffer and
//!     pads with silence as needed to satisfy the libretro backend.

use crate::state::global;
use wasmer::FunctionEnvMut;

// External crates for rendering
use fontdue::{Font, FontSettings};

// External crates for audio

use resvg::usvg::Tree;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::sync::Mutex;

/// Text size dimensions.
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TextSize {
    pub width: u32,
    pub height: u32,
}

// Embedded Spleen font data
static SPLEEN_5X8: &[u8] = include_bytes!("../assets/spleen-5x8.bdf");
static SPLEEN_8X16: &[u8] = include_bytes!("../assets/spleen-8x16.bdf");
static SPLEEN_12X24: &[u8] = include_bytes!("../assets/spleen-12x24.bdf");
static SPLEEN_16X32: &[u8] = include_bytes!("../assets/spleen-16x32.bdf");
static SPLEEN_32X64: &[u8] = include_bytes!("../assets/spleen-32x64.bdf");

// Global resource storage (lazy_static or similar, but using Mutex for simplicity)
lazy_static::lazy_static! {
    static ref RESOURCES: Mutex<Resources> = Mutex::new(Resources::default());
}

#[derive(Default)]
struct Resources {
    svgs: HashMap<u32, Tree>,
    gifs: HashMap<u32, GifResource>,
    fonts: HashMap<u32, FontResource>,
    next_id: u32,
}

struct GifResource {
    frames: Vec<Vec<u8>>, // RGBA data per frame
    delays: Vec<u16>,     // in 10ms units
    width: u16,
    height: u16,
}

enum FontResource {
    Ttf(Font),
    Spleen {
        width: u32,
        height: u32,
        glyphs: HashMap<char, Vec<u8>>, // char -> bitmap rows
    },
}

/// Errors from AV operations.
#[derive(Debug)]
pub enum AvError {
    MissingMemory,
    MemoryReadFailed,
}

// --- Graphics ---

/// Set the screen dimensions. Resizes the host framebuffer.
pub fn graphics_set_size(width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    let mut s = global().lock().unwrap();
    s.video.width = width;
    s.video.height = height;
    s.video.framebuffer.resize((width * height) as usize, 0);
    // Clear to black on resize
    s.video.framebuffer.fill(0);
}

/// Set the current drawing color.
pub fn graphics_set_color(r: u32, g: u32, b: u32, _a: u32) {
    let mut s = global().lock().unwrap();
    // Pack as 0x00RRGGBB (XRGB8888). We ignore Alpha for the framebuffer format usually,
    // but we might use it for blending later. For now, simple overwrite.
    // Libretro XRGB8888 expects 0x00RRGGBB.
    let color = ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF);
    s.video.draw_color = color;
}

/// Clear the screen to a specific color.
pub fn graphics_background(r: u32, g: u32, b: u32) {
    let mut s = global().lock().unwrap();
    let color = ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF);
    s.video.framebuffer.fill(color);
}

/// Draw a single pixel.
pub fn graphics_point(x: i32, y: i32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;

    if x >= 0 && x < w && y >= 0 && y < h {
        let idx = (y * w + x) as usize;
        s.video.framebuffer[idx] = s.video.draw_color;
    }
}

/// Draw a line using Bresenham's algorithm.
pub fn graphics_line(mut x0: i32, mut y0: i32, x1: i32, y1: i32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;
    let color = s.video.draw_color;
    let fb = &mut s.video.framebuffer;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
            fb[(y0 * w + x0) as usize] = color;
        }

        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Draw a filled rectangle.
pub fn graphics_rect(x: i32, y: i32, w: u32, h: u32) {
    let mut s = global().lock().unwrap();
    let screen_w = s.video.width as i32;
    let screen_h = s.video.height as i32;
    let color = s.video.draw_color;

    let x_start = x.max(0);
    let y_start = y.max(0);
    let x_end = (x + w as i32).min(screen_w);
    let y_end = (y + h as i32).min(screen_h);

    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let fb_w = s.video.width as usize;
    let fb = &mut s.video.framebuffer;

    for curr_y in y_start..y_end {
        let start_idx = (curr_y as usize) * fb_w + (x_start as usize);
        let end_idx = (curr_y as usize) * fb_w + (x_end as usize);
        fb[start_idx..end_idx].fill(color);
    }
}

/// Draw a rectangle outline.
pub fn graphics_rect_outline(x: i32, y: i32, w: u32, h: u32) {
    // Top
    graphics_line_internal(x, y, x + w as i32, y);
    // Bottom
    graphics_line_internal(x, y + h as i32, x + w as i32, y + h as i32);
    // Left
    graphics_line_internal(x, y, x, y + h as i32);
    // Right
    graphics_line_internal(x + w as i32, y, x + w as i32, y + h as i32);
}

/// Helper for internal line drawing without locking every pixel (if we had a way to pass the lock).
/// Since we don't want to complicate locking, we'll just call the public one which locks.
/// It's slightly inefficient but fine for this scale.
/// Actually, `graphics_rect_outline` calls `graphics_line` 4 times, so 4 locks. Acceptable.
fn graphics_line_internal(x1: i32, y1: i32, x2: i32, y2: i32) {
    graphics_line(x1, y1, x2, y2);
}

/// Draw a filled circle.
pub fn graphics_circle(cx: i32, cy: i32, r: u32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;
    let color = s.video.draw_color;
    let fb = &mut s.video.framebuffer;

    let r_sq = (r * r) as i32;
    let r_i32 = r as i32;

    let x_min = (cx - r_i32).max(0);
    let x_max = (cx + r_i32).min(w);
    let y_min = (cy - r_i32).max(0);
    let y_max = (cy + r_i32).min(h);

    for y in y_min..y_max {
        for x in x_min..x_max {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r_sq {
                fb[(y * w + x) as usize] = color;
            }
        }
    }
}

/// Draw a circle outline (Bresenham's circle algorithm).
pub fn graphics_circle_outline(cx: i32, cy: i32, r: u32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;
    let color = s.video.draw_color;
    let fb = &mut s.video.framebuffer;

    let mut x = 0;
    let mut y = r as i32;
    let mut d = 3 - 2 * r as i32;

    let mut plot = |x: i32, y: i32| {
        if x >= 0 && x < w && y >= 0 && y < h {
            fb[(y * w + x) as usize] = color;
        }
    };

    while y >= x {
        plot(cx + x, cy + y);
        plot(cx - x, cy + y);
        plot(cx + x, cy - y);
        plot(cx - x, cy - y);
        plot(cx + y, cy + x);
        plot(cx - y, cy + x);
        plot(cx + y, cy - x);
        plot(cx - y, cy - x);

        x += 1;
        if d > 0 {
            y -= 1;
            d = d + 4 * (x - y) + 10;
        } else {
            d = d + 4 * x + 6;
        }
    }
}

/// Draw an image from guest memory.
/// `ptr` points to RGBA bytes (4 bytes per pixel).
pub fn graphics_image(
    env: &FunctionEnvMut<()>,
    x: i32,
    y: i32,
    img_w: u32,
    img_h: u32,
    ptr: u32,
    len: u32,
) -> Result<(), AvError> {
    // Basic validation
    let expected_len = img_w.checked_mul(img_h).and_then(|s| s.checked_mul(4));
    if let Some(req) = expected_len {
        if len < req {
            // Not enough data provided
            return Ok(());
        }
    } else {
        return Ok(());
    }

    // Read guest memory
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // We read the whole image into a temp buffer.
    // Optimization: could read row-by-row to avoid large allocation,
    // but for retro resolutions this is fine.
    let mut img_data = vec![0u8; len as usize];
    view.read(ptr as u64, &mut img_data)
        .map_err(|_| AvError::MemoryReadFailed)?;

    // Lock and draw
    let mut s = global().lock().unwrap();
    let screen_w = s.video.width as i32;
    let screen_h = s.video.height as i32;
    let fb = &mut s.video.framebuffer;

    // Clipping
    let x_start = x.max(0);
    let y_start = y.max(0);
    let x_end = (x + img_w as i32).min(screen_w);
    let y_end = (y + img_h as i32).min(screen_h);

    if x_start >= x_end || y_start >= y_end {
        return Ok(());
    }

    for curr_y in y_start..y_end {
        let src_y = curr_y - y; // relative to image
        let src_row_start = (src_y as usize) * (img_w as usize) * 4;

        let dst_row_start = (curr_y as usize) * (screen_w as usize);

        for curr_x in x_start..x_end {
            let src_x = curr_x - x; // relative to image
            let src_idx = src_row_start + (src_x as usize) * 4;

            let r = img_data[src_idx];
            let g = img_data[src_idx + 1];
            let b = img_data[src_idx + 2];
            let a = img_data[src_idx + 3];

            if a > 0 {
                // Simple alpha check (0 = transparent, >0 = opaque).
                // Real blending would be: result = alpha * src + (1-alpha) * dst
                // For now, just overwrite if not fully transparent.
                let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                fb[dst_row_start + (curr_x as usize)] = color;
            }
        }
    }

    Ok(())
}

/// Draw a filled triangle.
pub fn graphics_triangle(x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;
    let color = s.video.draw_color;
    let fb = &mut s.video.framebuffer;

    // Sort vertices by y
    let mut verts = [(x1, y1), (x2, y2), (x3, y3)];
    verts.sort_by_key(|&(_, y)| y);

    let (x1, y1) = verts[0];
    let (x2, y2) = verts[1];
    let (x3, y3) = verts[2];

    // Helper to draw horizontal line
    let mut draw_hline = |y: i32, x_start: i32, x_end: i32| {
        if y < 0 || y >= h {
            return;
        }
        let start = x_start.max(0).min(w - 1);
        let end = x_end.max(0).min(w - 1);
        if start > end {
            return;
        }
        let row_start = (y as usize) * (w as usize);
        for x in start..=end {
            fb[row_start + x as usize] = color;
        }
    };

    // Scanline fill
    if y1 == y3 {
        return;
    } // degenerate

    for y in y1..=y3 {
        if y < y1 || y > y3 {
            continue;
        }
        let mut x_left = w;
        let mut x_right = -1;

        // Interpolate edges
        for &(xa, ya, xb, yb) in &[(x1, y1, x2, y2), (x2, y2, x3, y3), (x3, y3, x1, y1)] {
            if (ya <= y && y <= yb) || (yb <= y && y <= ya) {
                if ya == yb {
                    continue;
                }
                let t = (y - ya) as f32 / (yb - ya) as f32;
                let x = xa as f32 + t * (xb - xa) as f32;
                x_left = x_left.min(x as i32);
                x_right = x_right.max(x as i32);
            }
        }

        draw_hline(y, x_left, x_right);
    }
}

/// Draw a triangle outline.
pub fn graphics_triangle_outline(x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
    graphics_line(x1, y1, x2, y2);
    graphics_line(x2, y2, x3, y3);
    graphics_line(x3, y3, x1, y1);
}

/// Draw a quadratic Bezier curve.
pub fn graphics_bezier_quadratic(
    x1: i32,
    y1: i32,
    cx: i32,
    cy: i32,
    x2: i32,
    y2: i32,
    segments: u32,
) {
    if segments == 0 {
        return;
    }
    let mut prev_x = x1 as f32;
    let mut prev_y = y1 as f32;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let x =
            (1.0 - t).powi(2) * x1 as f32 + 2.0 * (1.0 - t) * t * cx as f32 + t.powi(2) * x2 as f32;
        let y =
            (1.0 - t).powi(2) * y1 as f32 + 2.0 * (1.0 - t) * t * cy as f32 + t.powi(2) * y2 as f32;
        graphics_line(prev_x as i32, prev_y as i32, x as i32, y as i32);
        prev_x = x;
        prev_y = y;
    }
}

/// Draw a cubic Bezier curve.
pub fn graphics_bezier_cubic(
    x1: i32,
    y1: i32,
    cx1: i32,
    cy1: i32,
    cx2: i32,
    cy2: i32,
    x2: i32,
    y2: i32,
    segments: u32,
) {
    if segments == 0 {
        return;
    }
    let mut prev_x = x1 as f32;
    let mut prev_y = y1 as f32;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let x = (1.0 - t).powi(3) * x1 as f32
            + 3.0 * (1.0 - t).powi(2) * t * cx1 as f32
            + 3.0 * (1.0 - t) * t.powi(2) * cx2 as f32
            + t.powi(3) * x2 as f32;
        let y = (1.0 - t).powi(3) * y1 as f32
            + 3.0 * (1.0 - t).powi(2) * t * cy1 as f32
            + 3.0 * (1.0 - t) * t.powi(2) * cy2 as f32
            + t.powi(3) * y2 as f32;
        graphics_line(prev_x as i32, prev_y as i32, x as i32, y as i32);
        prev_x = x;
        prev_y = y;
    }
}

/// Draw a filled pill.
pub fn graphics_pill(x: i32, y: i32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    let r = (w.min(h) / 2) as i32;
    // Draw center rect
    graphics_rect(x + r, y, w - 2 * r as u32, h);
    // Draw left cap
    graphics_circle(x + r, y + r, r as u32);
    // Draw right cap
    graphics_circle(x + w as i32 - r, y + r, r as u32);
}

/// Draw a pill outline.
pub fn graphics_pill_outline(x: i32, y: i32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    let r = (w.min(h) / 2) as i32;
    // Outline center rect
    graphics_rect_outline(x + r, y, w - 2 * r as u32, h);
    // Outline left cap
    graphics_circle_outline(x + r, y + r, r as u32);
    // Outline right cap
    graphics_circle_outline(x + w as i32 - r, y + r, r as u32);
}

/// Create SVG resource.
pub fn graphics_svg_create(env: &FunctionEnvMut<()>, ptr: u32, len: u32) -> u32 {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return 0;
    }
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);
    let mut data = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut data).is_err() {
        return 0;
    }
    let svg_str = match std::str::from_utf8(&data) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let tree = match Tree::from_str(svg_str, &usvg::Options::default()) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let mut res = RESOURCES.lock().unwrap();
    let id = res.next_id;
    res.next_id += 1;
    res.svgs.insert(id, tree);
    id
}

/// Draw SVG.
pub fn graphics_svg_draw(id: u32, x: i32, y: i32, w: u32, h: u32) {
    let res = RESOURCES.lock().unwrap();
    if let Some(tree) = res.svgs.get(&id) {
        let pixmap_size = tiny_skia::IntSize::from_wh(w as u32, h as u32).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
        resvg::render(tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        // Now draw pixmap as image
        let rgba_data: Vec<u8> = pixmap
            .data()
            .chunks_exact(4)
            .flat_map(|rgba| [rgba[0], rgba[1], rgba[2], rgba[3]])
            .collect();
        graphics_image_from_host(x, y, w, h, &rgba_data);
    }
}

/// Destroy SVG.
pub fn graphics_svg_destroy(id: u32) {
    let mut res = RESOURCES.lock().unwrap();
    res.svgs.remove(&id);
}

/// Create GIF resource.
pub fn graphics_gif_create(env: &FunctionEnvMut<()>, ptr: u32, len: u32) -> u32 {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return 0;
    }
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);
    let mut data = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut data).is_err() {
        return 0;
    }
    let cursor = std::io::Cursor::new(&data);
    let mut decoder = gif::DecodeOptions::new().read_info(cursor).unwrap();
    let mut frames = Vec::new();
    let mut delays = Vec::new();
    let mut width = 0;
    let mut height = 0;
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        width = frame.width;
        height = frame.height;
        let rgba: Vec<u8> = frame
            .buffer
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect();
        frames.push(rgba);
        delays.push(frame.delay);
    }
    let mut res = RESOURCES.lock().unwrap();
    let id = res.next_id;
    res.next_id += 1;
    res.gifs.insert(
        id,
        GifResource {
            frames,
            delays,
            width,
            height,
        },
    );
    id
}

/// Draw GIF at natural size.
pub fn graphics_gif_draw(id: u32, x: i32, y: i32) {
    graphics_gif_draw_scaled(id, x, y, 0, 0); // 0 means natural
}

/// Draw GIF scaled.
pub fn graphics_gif_draw_scaled(id: u32, x: i32, y: i32, w: u32, h: u32) {
    let res = RESOURCES.lock().unwrap();
    if let Some(gif) = res.gifs.get(&id) {
        let millis = system_millis();
        let total_delay = gif.delays.iter().sum::<u16>() as u64 * 10; // 10ms per unit
        let frame_idx = if total_delay > 0 {
            ((millis % total_delay) / 10) as usize % gif.frames.len()
        } else {
            0
        };
        let rgba_data = &gif.frames[frame_idx];
        let img_w = gif.width as u32;
        let img_h = gif.height as u32;
        if w == 0 || h == 0 {
            graphics_image_from_host(x, y, img_w, img_h, rgba_data);
        } else {
            // Simple scaling (placeholder - real scaling needed)
            graphics_image_from_host(x, y, w, h, rgba_data);
        }
    }
}

/// Destroy GIF.
pub fn graphics_gif_destroy(id: u32) {
    let mut res = RESOURCES.lock().unwrap();
    res.gifs.remove(&id);
}

/// Upload TTF font.
pub fn graphics_font_upload_ttf(env: &FunctionEnvMut<()>, ptr: u32, len: u32) -> u32 {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return 0;
    }
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);
    let mut data = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut data).is_err() {
        return 0;
    }
    let font = match Font::from_bytes(data, FontSettings::default()) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut res = RESOURCES.lock().unwrap();
    let id = res.next_id;
    res.next_id += 1;
    res.fonts.insert(id, FontResource::Ttf(font));
    id
}

/// Parse BDF font data into glyph map.
fn parse_bdf(bdf_data: &[u8]) -> Option<HashMap<char, Vec<u8>>> {
    let text = core::str::from_utf8(bdf_data).ok()?;
    let mut glyphs = HashMap::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("STARTCHAR") {
            let mut encoding = None;
            let mut bitmap = Vec::new();
            let mut in_bitmap = false;
            while let Some(inner_line) = lines.next() {
                if inner_line.starts_with("ENCODING") {
                    if let Some(enc_str) = inner_line.split_whitespace().nth(1) {
                        encoding = enc_str.parse::<u32>().ok().and_then(|e| char::from_u32(e));
                    }
                } else if inner_line == "BITMAP" {
                    in_bitmap = true;
                } else if inner_line == "ENDCHAR" {
                    break;
                } else if in_bitmap {
                    if let Ok(byte) = u8::from_str_radix(inner_line.trim(), 16) {
                        bitmap.push(byte);
                    }
                }
            }
            if let Some(ch) = encoding {
                glyphs.insert(ch, bitmap);
            }
        }
    }
    Some(glyphs)
}

/// Use Spleen font.
pub fn graphics_font_use_spleen(size: u32) -> u32 {
    let (data, w, h) = match size {
        8 => (SPLEEN_5X8, 5, 8),
        16 => (SPLEEN_8X16, 8, 16),
        24 => (SPLEEN_12X24, 12, 24),
        32 => (SPLEEN_16X32, 16, 32),
        64 => (SPLEEN_32X64, 32, 64),
        _ => return 0,
    };
    let Some(glyphs) = parse_bdf(data) else {
        return 0;
    };
    let mut res = RESOURCES.lock().unwrap();
    let id = res.next_id;
    res.next_id += 1;
    res.fonts.insert(
        id,
        FontResource::Spleen {
            width: w,
            height: h,
            glyphs,
        },
    );
    id
}

/// Draw text.
pub fn graphics_text(x: i32, y: i32, font_id: u32, env: &FunctionEnvMut<()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return;
    }
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);
    let mut text_bytes = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut text_bytes).is_err() {
        return;
    }
    let text = match std::str::from_utf8(&text_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };
    let res = RESOURCES.lock().unwrap();
    if let Some(font) = res.fonts.get(&font_id) {
        match font {
            FontResource::Ttf(f) => {
                let mut px = x as f32;
                for ch in text.chars() {
                    let (metrics, bitmap) = f.rasterize(ch, 16.0); // fixed size
                    for (i, &alpha) in bitmap.iter().enumerate() {
                        if alpha > 0 {
                            let gx = px as i32 + (i % metrics.width as usize) as i32;
                            let gy = y + (i / metrics.width as usize) as i32;
                            graphics_point(gx, gy);
                        }
                    }
                    px += metrics.advance_width;
                }
            }
            FontResource::Spleen {
                width,
                height: _,
                glyphs,
            } => {
                let mut px = x;
                for ch in text.chars() {
                    if let Some(bitmap) = glyphs.get(&ch) {
                        for (row, &byte) in bitmap.iter().enumerate() {
                            for col in 0..*width as usize {
                                if (byte & (1 << (7 - col))) != 0 {
                                    graphics_point(px + col as i32, y + row as i32);
                                }
                            }
                        }
                    }
                    px += *width as i32;
                }
            }
        }
    }
}

/// Measure text.
pub fn graphics_text_measure(font_id: u32, env: &FunctionEnvMut<()>, ptr: u32, len: u32) -> u64 {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };
    if memory_ptr.is_null() {
        return 0;
    }
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);
    let mut text_bytes = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut text_bytes).is_err() {
        return 0;
    }
    let text = match std::str::from_utf8(&text_bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let res = RESOURCES.lock().unwrap();
    let (width, height) = if let Some(font) = res.fonts.get(&font_id) {
        match font {
            FontResource::Ttf(f) => {
                let mut width = 0.0;
                let mut height: f32 = 0.0;
                for ch in text.chars() {
                    let (metrics, _) = f.rasterize(ch, 16.0);
                    width += metrics.advance_width;
                    height = height.max(metrics.height as f32);
                }
                (width as u32, height as u32)
            }
            FontResource::Spleen { width, height, .. } => {
                (text.chars().count() as u32 * *width, *height)
            }
        }
    } else {
        (0, 0)
    };
    ((width as u64) << 32) | (height as u64)
}

// Helper to draw image from host memory (RGBA vec)
fn graphics_image_from_host(x: i32, y: i32, w: u32, h: u32, data: &[u8]) {
    let mut s = global().lock().unwrap();
    let screen_w = s.video.width as i32;
    let screen_h = s.video.height as i32;
    let fb = &mut s.video.framebuffer;

    let x_start = x.max(0);
    let y_start = y.max(0);
    let x_end = (x + w as i32).min(screen_w);
    let y_end = (y + h as i32).min(screen_h);

    for curr_y in y_start..y_end {
        let src_y = curr_y - y;
        let src_row_start = (src_y as usize) * (w as usize) * 4;
        let dst_row_start = (curr_y as usize) * (screen_w as usize);
        for curr_x in x_start..x_end {
            let src_x = curr_x - x;
            let src_idx = src_row_start + (src_x as usize) * 4;
            let r = data[src_idx];
            let g = data[src_idx + 1];
            let b = data[src_idx + 2];
            let a = data[src_idx + 3];
            if a > 0 {
                let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                fb[dst_row_start + (curr_x as usize)] = color;
            }
        }
    }
}

// Get current time in milliseconds
fn system_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Present the framebuffer to libretro.
pub fn video_present_host() {
    let (handle_ptr, _width, _height, fb) = {
        let s = global().lock().unwrap();
        (
            s.handle,
            s.video.width,
            s.video.height,
            s.video.framebuffer.clone(),
        )
    };

    if handle_ptr.is_null() {
        return;
    }

    // Convert Vec<u32> to &[u8] for libretro.
    // XRGB8888 is 4 bytes per pixel.
    // We can cast the slice safely because the layout is compatible (little endian).
    let data_ptr = fb.as_ptr() as *const u8;
    let data_len = fb.len() * 4;
    let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };

    // SAFETY: handle pointer checked.
    let h = unsafe { &mut *handle_ptr };
    h.upload_video_frame(data_slice);
}

// --- Audio ---
//
// Helpers for mixing.
// NOTE: Higher-level playback and chiptune APIs are stubbed for now; these helpers
// are kept because `audio_drain_host` mixes guest-pushed audio and pads as needed.
#[inline]
fn sat_add_i16(a: i16, b: i16) -> i16 {
    let s = a as i32 + b as i32;
    if s > i16::MAX as i32 {
        i16::MAX
    } else if s < i16::MIN as i32 {
        i16::MIN
    } else {
        s as i16
    }
}

pub fn audio_init(sample_rate: u32) -> u32 {
    let mut s = global().lock().unwrap();
    s.audio.sample_rate = sample_rate;
    // Return buffer size hint (e.g. 1 frame worth? or just 0).
    // The guest doesn't strictly need this if it pushes what it wants.
    1024
}

// --- Higher-level audio playback (stubs) ---
//
// These are intentionally stubbed so guests can link against the API.
// Full implementations will:
// - decode the encoded data (wav/ogg) into PCM,
// - mix it into the output stream (multi-channel / fire-and-forget).
//
// Fire-and-forget: no ids/handles are returned.

pub fn audio_play_wav(env: &FunctionEnvMut<()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // Read WAV bytes from guest memory.
    let mut wav_bytes = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut wav_bytes).is_err() {
        return;
    }

    // Decode WAV using hound.
    let cursor = std::io::Cursor::new(wav_bytes);
    let reader = match hound::WavReader::new(cursor) {
        Ok(r) => r,
        Err(_) => return,
    };

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    // Collect samples as i16, converting if necessary.
    let mut samples: Vec<i16> = Vec::new();
    for sample in reader.into_samples::<i16>() {
        match sample {
            Ok(s) => samples.push(s),
            Err(_) => return,
        }
    }

    // Convert to interleaved stereo if mono.
    let pcm_stereo: Vec<i16> = if spec.channels == 1 {
        // Mono: duplicate to stereo.
        samples.into_iter().flat_map(|s| [s, s]).collect()
    } else if spec.channels == 2 {
        // Stereo: already interleaved.
        samples
    } else {
        // Unsupported channel count.
        return;
    };

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_play_qoa(env: &FunctionEnvMut<()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // Read QOA bytes from guest memory.
    let mut qoa_bytes = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut qoa_bytes).is_err() {
        return;
    }

    // Decode QOA using qoaudio crate.
    let decoder = match qoaudio::QoaDecoder::new(&qoa_bytes) {
        Ok(d) => d,
        Err(_) => return,
    };

    let channels = decoder.channels() as usize;
    let sample_rate = decoder.sample_rate() as u32;
    let samples: Vec<i16> = if let Some(s) = decoder.decoded_samples() {
        s.into_iter().collect()
    } else {
        return;
    };

    let pcm_stereo: Vec<i16> = if channels == 1 {
        // Mono: duplicate to stereo.
        samples.into_iter().flat_map(|s| [s, s]).collect()
    } else if channels == 2 {
        // Stereo: already interleaved.
        samples
    } else {
        // Unsupported channel count.
        return;
    };

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_play_xm(env: &FunctionEnvMut<()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // Read XM bytes from guest memory.
    let mut xm_bytes = vec![0u8; len as usize];
    if view.read(ptr as u64, &mut xm_bytes).is_err() {
        return;
    }

    // Load XM module using xmrs.
    let xm = match xmrs::import::xm::xmmodule::XmModule::load(&xm_bytes) {
        Ok(xm) => xm,
        Err(_) => return,
    };

    let module = xm.to_module();
    let module = Box::new(module);
    let module_ref: &'static xmrs::prelude::Module = Box::leak(module);

    // Get sample rate.
    let sample_rate = {
        let s = crate::state::global().lock().unwrap();
        s.audio.sample_rate
    };

    // Create player.
    let mut player =
        xmrsplayer::prelude::XmrsPlayer::new(module_ref, sample_rate as f32, 1024, false);
    player.set_max_loop_count(1); // Decode the song once

    // Decode the entire song into PCM.
    let mut pcm_stereo: Vec<i16> = Vec::new();

    loop {
        match player.sample(true) {
            Some((left, right)) => {
                let l_i16 = (left * 32767.0) as i16;
                let r_i16 = (right * 32767.0) as i16;
                pcm_stereo.push(l_i16);
                pcm_stereo.push(r_i16);
            }
            None => break,
        }
    }

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_push_samples(env: &FunctionEnvMut<()>, ptr: u32, count: u32) -> Result<(), AvError> {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory
    };

    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // Read i16 samples. count is number of i16 elements.
    let byte_len = count.checked_mul(2).ok_or(AvError::MemoryReadFailed)?;
    let mut tmp_bytes = vec![0u8; byte_len as usize];

    view.read(ptr as u64, &mut tmp_bytes)
        .map_err(|_| AvError::MemoryReadFailed)?;

    // Convert bytes to i16
    let mut samples = Vec::with_capacity(count as usize);
    for chunk in tmp_bytes.chunks_exact(2) {
        let val = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(val);
    }

    // Append to host queue
    let mut s = global().lock().unwrap();
    s.audio.host_queue.extend(samples);

    Ok(())
}

pub fn audio_drain_host(max_frames: u32) -> u32 {
    let (handle_ptr, sample_rate) = {
        let s = global().lock().unwrap();
        (s.handle, s.audio.sample_rate)
    };

    if handle_ptr.is_null() {
        return 0;
    }

    // We must upload a minimum amount of audio each frame to satisfy libretro-backend.
    // The backend requires at least ~1 frame worth of stereo samples; for 44.1kHz @ 60fps:
    // 44100 / 60 = 735 frames (per channel) => 1470 i16 samples interleaved stereo.
    //
    // Even if the guest provides no audio (or too little), we pad with silence so the core
    // never panics due to insufficient audio uploads.
    let min_samples_per_run: usize = ((sample_rate as usize) / 60) * 2;

    // Stereo = 2 i16 samples per audio frame (L, R)
    let samples_per_frame: usize = 2;

    // How many frames are we going to output this run?
    // If max_frames == 0, use at least the backend minimum.
    let target_frames: usize = if max_frames == 0 {
        min_samples_per_run / samples_per_frame
    } else {
        (max_frames as usize).max(min_samples_per_run / samples_per_frame)
    };

    // Start with silence; we'll mix into this.
    let mut mixed: Vec<i16> = vec![0i16; target_frames * samples_per_frame];

    // Mix guest-pushed raw samples (host_queue).
    {
        let mut s = global().lock().unwrap();

        let available_samples = s.audio.host_queue.len();
        let available_frames = available_samples / samples_per_frame;

        let frames_to_take = available_frames.min(target_frames);
        let samples_to_take = frames_to_take * samples_per_frame;

        if samples_to_take != 0 {
            let drained: Vec<i16> = s.audio.host_queue.drain(0..samples_to_take).collect();
            for (dst, src) in mixed.iter_mut().zip(drained.iter()) {
                *dst = sat_add_i16(*dst, *src);
            }
        }

        // Mix audio channels (higher-level playback).
        for channel in &mut s.audio.channels {
            if !channel.active {
                continue;
            }

            let channel_frames = channel.pcm_stereo.len() / 2;
            if channel.position_frames >= channel_frames {
                if channel.loop_enabled {
                    channel.position_frames = 0;
                } else {
                    channel.active = false;
                    continue;
                }
            }

            let start_frame = channel.position_frames;
            let frames_to_mix = (channel_frames - start_frame).min(target_frames);

            let volume = channel.volume_q8_8 as f32 / 256.0;
            let pan_left = if channel.pan_i16 <= 0 {
                1.0
            } else {
                (32768 - channel.pan_i16) as f32 / 32768.0
            };
            let pan_right = if channel.pan_i16 >= 0 {
                1.0
            } else {
                (32768 + channel.pan_i16) as f32 / 32768.0
            };

            for i in 0..frames_to_mix {
                let src_idx = (start_frame + i) * 2;
                let l = (channel.pcm_stereo[src_idx] as f32 * volume * pan_left) as i16;
                let r = (channel.pcm_stereo[src_idx + 1] as f32 * volume * pan_right) as i16;

                let dst_idx = i * 2;
                mixed[dst_idx] = sat_add_i16(mixed[dst_idx], l);
                mixed[dst_idx + 1] = sat_add_i16(mixed[dst_idx + 1], r);
            }

            channel.position_frames += frames_to_mix;
        }
    }

    // SAFETY: handle pointer checked.
    let h = unsafe { &mut *handle_ptr };
    h.upload_audio_frame(&mixed);

    // Report how many *audio frames* we uploaded (stereo frames).
    (mixed.len() / samples_per_frame) as u32
}
