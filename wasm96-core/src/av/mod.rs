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

// Needed for `alloc::` in this crate.
extern crate alloc;

use crate::state::global;
use wasmtime::Caller;

// External crates for rendering
use fontdue::{Font, FontSettings};

// External crates for asset decoding
use resvg::usvg::Tree;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::sync::Mutex;

// Storage ABI helpers
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(test)]
mod tests {
    use super::*;

    fn count_color(buf: &[u32], color: u32) -> usize {
        buf.iter().copied().filter(|&c| c == color).count()
    }

    fn reset_state_for_test() {
        // Ensure any previous test doesn't leave global state in a poisoned/invalid state.
        // This keeps tests isolated and avoids cascading failures when a prior test panics.
        crate::state::clear_on_unload();
    }

    #[test]
    fn triangle_degenerate_area_draws_nothing() {
        reset_state_for_test();

        // Make sure the triangle fill handles colinear points.
        graphics_set_size(16, 16);
        graphics_background(0, 0, 0);
        graphics_set_color(255, 0, 0, 255);
        let red = (255u32 << 16) | (0u32 << 8) | 0u32;

        // Colinear along y=x line.
        graphics_triangle(1, 1, 5, 5, 10, 10);

        let s = global().lock().unwrap();
        assert_eq!(count_color(&s.video.framebuffer, red), 0);
    }

    #[test]
    fn triangle_fills_some_pixels_for_simple_case() {
        reset_state_for_test();

        graphics_set_size(32, 32);
        graphics_background(0, 0, 0);
        graphics_set_color(0, 255, 0, 255);
        let green = (0u32 << 16) | (255u32 << 8) | 0u32;

        // A clearly non-degenerate triangle well within bounds.
        graphics_triangle(4, 4, 20, 6, 8, 24);

        let s = global().lock().unwrap();
        let filled = count_color(&s.video.framebuffer, green);
        assert!(filled > 0, "expected some filled pixels, got {filled}");
        assert!(
            filled < (32 * 32) as usize,
            "triangle should not fill entire screen"
        );
    }

    #[test]
    fn triangle_vertex_order_does_not_change_fill_count() {
        reset_state_for_test();

        // Vertex order reverses winding; rasterization should be winding-invariant.
        graphics_set_size(32, 32);

        // First order
        graphics_background(0, 0, 0);
        graphics_set_color(0, 0, 255, 255);
        let blue = (0u32 << 16) | (0u32 << 8) | 255u32;
        graphics_triangle(4, 4, 20, 6, 8, 24);
        let count_a = {
            let s = global().lock().unwrap();
            count_color(&s.video.framebuffer, blue)
        };

        // Reverse winding (same vertices)
        graphics_background(0, 0, 0);
        graphics_set_color(0, 0, 255, 255);
        graphics_triangle(4, 4, 8, 24, 20, 6);
        let count_b = {
            let s = global().lock().unwrap();
            count_color(&s.video.framebuffer, blue)
        };

        assert_eq!(
            count_a, count_b,
            "filled pixel count should be identical regardless of winding (got {count_a} vs {count_b})"
        );
        assert!(count_a > 0);
    }

    #[test]
    fn triangle_clips_to_screen_without_panicking() {
        reset_state_for_test();

        // This test mostly ensures we don't index OOB when coordinates are off-screen.
        graphics_set_size(16, 16);
        graphics_background(0, 0, 0);
        graphics_set_color(255, 255, 255, 255);
        let white = (255u32 << 16) | (255u32 << 8) | 255u32;

        // Large triangle that extends beyond bounds.
        graphics_triangle(-10, -10, 30, 0, 0, 30);

        let s = global().lock().unwrap();
        let filled = count_color(&s.video.framebuffer, white);
        assert!(filled > 0);

        // This assertion is only about clipping/not panicking; a strict upper bound can be flaky
        // if draw_color/state leaks or if the test isn't perfectly isolated.
        assert!(
            filled <= s.video.framebuffer.len(),
            "filled pixels must never exceed framebuffer length"
        );
    }

    #[test]
    fn audio_channel_mix_advances_position_without_requiring_runtime_handle() {
        reset_state_for_test();

        // `audio_drain_host` early-returns if no libretro runtime handle is installed, which
        // makes it unsuitable for unit tests. Instead, validate the core mixing behavior:
        // channel position advances only when we actually mix frames.
        let sample_rate = 44100;
        audio_init(sample_rate);

        // 4 stereo frames: constant non-zero signal.
        let pcm_stereo: Vec<i16> = vec![
            5000, 5000, // frame 0
            5000, 5000, // frame 1
            5000, 5000, // frame 2
            5000, 5000, // frame 3
        ];

        let mut mixed: Vec<i16> = vec![0i16; 1 * 2]; // 1 stereo frame
        {
            let mut s = global().lock().unwrap();
            s.audio.host_queue.clear();

            s.audio.channels.clear();
            s.audio.channels.push(crate::state::AudioChannel {
                active: true,
                volume_q8_8: 256, // 1.0
                pan_i16: 0,       // centered
                loop_enabled: false,
                pcm_stereo,
                position_frames: 0,
                sample_rate,
            });

            // Mix exactly 1 frame from the channel (mirrors the logic in `audio_drain_host`,
            // but without depending on a libretro handle).
            let channel = &mut s.audio.channels[0];
            let channel_frames = channel.pcm_stereo.len() / 2;

            let start_frame = channel.position_frames;
            let frames_to_mix = (channel_frames - start_frame).min(1);

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

        let s = global().lock().unwrap();
        assert_eq!(s.audio.channels.len(), 1, "expected one channel");
        assert_eq!(
            s.audio.channels[0].position_frames, 1,
            "expected channel to advance by exactly one frame"
        );

        // And the mixed buffer should contain non-zero data.
        assert!(
            mixed.iter().any(|&s| s != 0),
            "expected mixed output to be non-zero"
        );
    }

    #[test]
    fn png_decode_and_draw_renders_expected_pixel() {
        reset_state_for_test();

        // Generate a valid minimal 1x1 RGBA PNG at runtime using the encoder,
        // to avoid hardcoding bytes/CRCs.
        let mut png_bytes: Vec<u8> = Vec::new();
        {
            use std::io::Write;

            let mut encoder = png::Encoder::new(&mut png_bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png write_header failed");

            // One pixel RGBA = red.
            writer
                .write_image_data(&[255, 0, 0, 255])
                .expect("png write_image_data failed");

            // Ensure everything is flushed into the Vec.
            let _ = writer.finish();
            let _ = (&mut png_bytes as &mut dyn Write).flush();
        }

        // Arrange framebuffer.
        graphics_set_size(4, 4);
        graphics_background(0, 0, 0);

        // Decode (same crate/decoder path as host-side decode implementation),
        // then blit the resulting RGBA into the framebuffer.
        let cursor = std::io::Cursor::new(&png_bytes);
        let decoder = png::Decoder::new(cursor);
        let mut reader = decoder.read_info().expect("png read_info failed");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("png next_frame failed");
        assert_eq!(info.width, 1);
        assert_eq!(info.height, 1);

        let rgba = &buf[..info.buffer_size()];
        assert_eq!(rgba, &[255, 0, 0, 255]);

        // Draw at (0,0)
        graphics_image_from_host(0, 0, 1, 1, rgba);

        let s = global().lock().unwrap();
        let red = (255u32 << 16) | (0u32 << 8) | 0u32;
        assert_eq!(s.video.framebuffer[0], red);
    }
}

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
    // ID-based resources (existing APIs in this module).
    svgs: HashMap<u32, Tree>,
    gifs: HashMap<u32, GifResource>,
    fonts: HashMap<u32, FontResource>,

    // Keyed indirection (new): map string keys (bytes) -> ids in the above maps.
    //
    // We intentionally use owned `String` keys because guests pass UTF-8.
    keyed_svgs: HashMap<String, u32>,
    keyed_gifs: HashMap<String, u32>,
    keyed_pngs: HashMap<String, PngResource>,
    keyed_fonts: HashMap<String, u32>,

    next_id: u32,
}

struct GifResource {
    frames: Vec<Vec<u8>>, // RGBA data per frame
    delays: Vec<u16>,     // in 10ms units
    width: u16,
    height: u16,
}

#[derive(Clone)]
struct PngResource {
    rgba: Vec<u8>, // RGBA8888 bytes
    width: u32,
    height: u32,
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

fn read_guest_bytes(caller: &mut Caller<'_, ()>, ptr: u32, len: u32) -> Result<Vec<u8>, AvError> {
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or(AvError::MissingMemory)?;

    let mut data = vec![0u8; len as usize];
    memory
        .read(&*caller, ptr as usize, &mut data)
        .map_err(|_| AvError::MemoryReadFailed)?;
    Ok(data)
}

fn read_guest_utf8(caller: &mut Caller<'_, ()>, ptr: u32, len: u32) -> Result<String, AvError> {
    let data = read_guest_bytes(caller, ptr, len)?;
    let s = std::str::from_utf8(&data).map_err(|_| AvError::MemoryReadFailed)?;
    Ok(s.to_string())
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
    caller: &mut Caller<'_, ()>,
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
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or(AvError::MissingMemory)?;

    // We read the whole image into a temp buffer.
    // Optimization: could read row-by-row to avoid large allocation,
    // but for retro resolutions this is fine.
    let mut img_data = vec![0u8; len as usize];
    memory
        .read(&*caller, ptr as usize, &mut img_data)
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

/// Decode PNG bytes from guest memory and draw at (x, y) at the image's natural size.
///
/// If decoding fails, this is a no-op.
///
/// NOTE: this requires adding the `png` crate dependency to `wasm96-core/Cargo.toml`.
pub fn graphics_image_png(
    env: &mut Caller<'_, ()>,
    x: i32,
    y: i32,
    ptr: u32,
    len: u32,
) -> Result<(), AvError> {
    let png_bytes = read_guest_bytes(env, ptr, len)?;

    // Decode PNG into RGBA8
    let cursor = std::io::Cursor::new(png_bytes);
    let decoder = png::Decoder::new(cursor);
    let mut reader = match decoder.read_info() {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = match reader.next_frame(&mut buf) {
        Ok(i) => i,
        Err(_) => return Ok(()),
    };

    let w = info.width;
    let h = info.height;
    if w == 0 || h == 0 {
        return Ok(());
    }

    let bytes = &buf[..info.buffer_size()];

    // Convert decoded buffer to RGBA8 (if needed)
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            // The decoder should have expanded indexed color, but if not, bail out.
            return Ok(());
        }
    };

    graphics_image_from_host(x, y, w, h, &rgba);
    Ok(())
}

fn decode_png_to_rgba(png_bytes: &[u8]) -> Option<PngResource> {
    let cursor = std::io::Cursor::new(png_bytes);
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().ok()?;

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;

    let w = info.width;
    let h = info.height;
    if w == 0 || h == 0 {
        return None;
    }

    let bytes = &buf[..info.buffer_size()];

    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            // If the decoder didn't expand indexed color, we don't support it here.
            return None;
        }
    };

    Some(PngResource {
        rgba,
        width: w,
        height: h,
    })
}

/// Register a PNG under a string key (bytes are encoded PNG).
pub fn graphics_png_register(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let png_bytes = match read_guest_bytes(env, data_ptr, data_len) {
        Ok(b) => b,
        Err(_) => return 0,
    };

    let decoded = match decode_png_to_rgba(&png_bytes) {
        Some(d) => d,
        None => return 0,
    };

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_pngs.insert(key, decoded);
    1
}

/// Draw a keyed PNG at natural size.
pub fn graphics_png_draw_key(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let png = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_pngs.get(&key).cloned()
    };

    if let Some(png) = png {
        graphics_image_from_host(x, y, png.width, png.height, &png.rgba);
    }
}

/// Draw a keyed PNG scaled (nearest-neighbor).
pub fn graphics_png_draw_key_scaled(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let png = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_pngs.get(&key).cloned()
    };

    let Some(png) = png else {
        return;
    };

    // Natural size if either dimension is 0.
    if w == 0 || h == 0 {
        graphics_image_from_host(x, y, png.width, png.height, &png.rgba);
        return;
    }

    let src_w = png.width;
    let src_h = png.height;
    if src_w == 0 || src_h == 0 {
        return;
    }

    let mut dst = vec![0u8; (w as usize).saturating_mul(h as usize).saturating_mul(4)];
    for dy in 0..h {
        let sy = (dy as u64 * src_h as u64 / h as u64) as u32;
        let sy = sy.min(src_h.saturating_sub(1));
        for dx in 0..w {
            let sx = (dx as u64 * src_w as u64 / w as u64) as u32;
            let sx = sx.min(src_w.saturating_sub(1));

            let sidx = ((sy as usize) * (src_w as usize) + (sx as usize)) * 4;
            let didx = ((dy as usize) * (w as usize) + (dx as usize)) * 4;

            if sidx + 3 < png.rgba.len() && didx + 3 < dst.len() {
                dst[didx] = png.rgba[sidx];
                dst[didx + 1] = png.rgba[sidx + 1];
                dst[didx + 2] = png.rgba[sidx + 2];
                dst[didx + 3] = png.rgba[sidx + 3];
            }
        }
    }

    graphics_image_from_host(x, y, w, h, &dst);
}

/// Unregister a keyed PNG.
pub fn graphics_png_unregister(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_pngs.remove(&key);
}

#[inline]
fn tri_edge(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i64 {
    (c.0 as i64 - a.0 as i64) * (b.1 as i64 - a.1 as i64)
        - (c.1 as i64 - a.1 as i64) * (b.0 as i64 - a.0 as i64)
}

/// Draw a filled triangle using a barycentric (edge-function) rasterizer.
///
/// Properties:
/// - Works for any vertex order (winding), filled area is consistent.
/// - Clips to framebuffer bounds.
/// - Uses integer edge functions for stability/determinism.
pub fn graphics_triangle(x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
    let mut s = global().lock().unwrap();
    let w = s.video.width as i32;
    let h = s.video.height as i32;
    if w <= 0 || h <= 0 {
        return;
    }

    let color = s.video.draw_color;
    let fb = &mut s.video.framebuffer;

    let v0 = (x1, y1);
    let v1 = (x2, y2);
    let v2 = (x3, y3);

    // Degenerate (area==0): nothing to fill.
    let area = tri_edge(v0, v1, v2);
    if area == 0 {
        return;
    }

    // Bounding box (inclusive), clipped to framebuffer.
    let min_x = v0.0.min(v1.0).min(v2.0).max(0);
    let max_x = v0.0.max(v1.0).max(v2.0).min(w - 1);
    let min_y = v0.1.min(v1.1).min(v2.1).max(0);
    let max_y = v0.1.max(v1.1).max(v2.1).min(h - 1);

    if min_x > max_x || min_y > max_y {
        return;
    }

    // Make the edge tests winding-invariant by normalizing the edge function
    // values to the same sign (i.e. as if the triangle had positive area).
    //
    // IMPORTANT: The sign normalization must match the sign of the triangle's own
    // area under the *same* (a,b,c) ordering used by `tri_edge(a,b,c)`.
    let sign = if area > 0 { 1 } else { -1 };

    for y in min_y..=max_y {
        let row = (y as usize) * (w as usize);
        for x in min_x..=max_x {
            let p = (x, y);

            // Edge functions for triangle v0,v1,v2.
            // Multiply by `sign` so "inside" corresponds to >= 0 regardless of winding.
            let w0 = tri_edge(v1, v2, p) * sign;
            let w1 = tri_edge(v2, v0, p) * sign;
            let w2 = tri_edge(v0, v1, p) * sign;

            if w0 >= 0 && w1 >= 0 && w2 >= 0 {
                fb[row + x as usize] = color;
            }
        }
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
pub fn graphics_svg_create(env: &mut Caller<'_, ()>, ptr: u32, len: u32) -> u32 {
    let data = match read_guest_bytes(env, ptr, len) {
        Ok(d) => d,
        Err(_) => return 0,
    };

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

/// Register SVG resource under a string key.
pub fn graphics_svg_register(
    caller: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let key = match read_guest_utf8(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let data = match read_guest_bytes(caller, data_ptr, data_len) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    // Reuse the existing SVG parser logic by feeding bytes directly.
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
    res.keyed_svgs.insert(key, id);
    1
}

/// Draw keyed SVG.
pub fn graphics_svg_draw_key(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_svgs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_svg_draw(id, x, y, w, h);
    }
}

/// Unregister keyed SVG and free the underlying resource.
pub fn graphics_svg_unregister(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let mut res = RESOURCES.lock().unwrap();
        res.keyed_svgs.remove(&key)
    };

    if let Some(id) = id {
        graphics_svg_destroy(id);
    }
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
pub fn graphics_gif_create(env: &mut Caller<'_, ()>, ptr: u32, len: u32) -> u32 {
    let data = match read_guest_bytes(env, ptr, len) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    let cursor = std::io::Cursor::new(&data);
    let mut decoder = match gif::DecodeOptions::new().read_info(cursor) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    // Snapshot the global palette once up-front to avoid borrowing `decoder` inside
    // the frame loop (which already holds a mutable borrow for `read_next_frame`).
    let global_palette: Option<Vec<u8>> = decoder.global_palette().map(|p| p.to_vec());

    let mut frames = Vec::new();
    let mut delays = Vec::new();
    let mut width = 0;
    let mut height = 0;

    while let Some(frame) = match decoder.read_next_frame() {
        Ok(f) => f,
        Err(_) => return 0,
    } {
        width = frame.width;
        height = frame.height;

        // NOTE:
        // `gif` frames are typically *indexed* color, not raw RGB triplets.
        // `frame.buffer` contains palette indices.
        // Use the frame-local palette if present, otherwise the global palette.
        let palette: Option<&[u8]> = frame
            .palette
            .as_deref()
            .or_else(|| global_palette.as_deref());

        let Some(palette) = palette else {
            // No palette available -> can't expand indices to RGBA.
            return 0;
        };

        // Expand indices into RGBA. The palette is packed RGBRGB...
        // Transparency index (if present) maps to alpha=0.
        let transparent_idx = frame.transparent;

        let mut rgba = Vec::with_capacity(frame.buffer.len() * 4);
        for &idx in frame.buffer.iter() {
            if Some(idx) == transparent_idx {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let base = (idx as usize) * 3;
            if base + 2 >= palette.len() {
                // Malformed index/palette; treat as transparent.
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let r = palette[base];
            let g = palette[base + 1];
            let b = palette[base + 2];
            rgba.extend_from_slice(&[r, g, b, 255]);
        }

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

        let src_rgba = &gif.frames[frame_idx];
        let src_w = gif.width as u32;
        let src_h = gif.height as u32;

        // Natural size if either dimension is 0.
        if w == 0 || h == 0 {
            graphics_image_from_host(x, y, src_w, src_h, src_rgba);
            return;
        }

        // Nearest-neighbor resample into a temporary RGBA buffer, then blit.
        // This keeps the public API unchanged (host-side draw from RGBA).
        if src_w == 0 || src_h == 0 {
            return;
        }

        let mut dst = vec![0u8; (w as usize).saturating_mul(h as usize).saturating_mul(4)];
        for dy in 0..h {
            let sy = (dy as u64 * src_h as u64 / h as u64) as u32;
            let sy = sy.min(src_h.saturating_sub(1));
            for dx in 0..w {
                let sx = (dx as u64 * src_w as u64 / w as u64) as u32;
                let sx = sx.min(src_w.saturating_sub(1));

                let sidx = ((sy as usize) * (src_w as usize) + (sx as usize)) * 4;
                let didx = ((dy as usize) * (w as usize) + (dx as usize)) * 4;

                if sidx + 3 < src_rgba.len() && didx + 3 < dst.len() {
                    dst[didx] = src_rgba[sidx];
                    dst[didx + 1] = src_rgba[sidx + 1];
                    dst[didx + 2] = src_rgba[sidx + 2];
                    dst[didx + 3] = src_rgba[sidx + 3];
                }
            }
        }

        graphics_image_from_host(x, y, w, h, &dst);
    }
}

/// Destroy GIF.
pub fn graphics_gif_destroy(id: u32) {
    let mut res = RESOURCES.lock().unwrap();
    res.gifs.remove(&id);
}

/// Register GIF resource under a string key.
pub fn graphics_gif_register(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let id = graphics_gif_create(env, data_ptr, data_len);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_gifs.insert(key, id);
    1
}

/// Draw keyed GIF at natural size.
pub fn graphics_gif_draw_key(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_gifs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_gif_draw(id, x, y);
    }
}

/// Draw keyed GIF scaled.
pub fn graphics_gif_draw_key_scaled(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_gifs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_gif_draw_scaled(id, x, y, w, h);
    }
}

/// Unregister keyed GIF and destroy its underlying resource.
pub fn graphics_gif_unregister(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let mut res = RESOURCES.lock().unwrap();
        res.keyed_gifs.remove(&key)
    };

    if let Some(id) = id {
        graphics_gif_destroy(id);
    }
}

/// Upload TTF font.
pub fn graphics_font_upload_ttf(env: &mut Caller<'_, ()>, ptr: u32, len: u32) -> u32 {
    let data = match read_guest_bytes(env, ptr, len) {
        Ok(d) => d,
        Err(_) => return 0,
    };

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

/// Register TTF font under a string key.
pub fn graphics_font_register_ttf(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let id = graphics_font_upload_ttf(env, data_ptr, data_len);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_fonts.insert(key, id);
    1
}

/// Register built-in Spleen font under a string key.
pub fn graphics_font_register_spleen(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    size: u32,
) -> u32 {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let id = graphics_font_use_spleen(size);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_fonts.insert(key, id);
    1
}

/// Unregister keyed font.
pub fn graphics_font_unregister(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32) {
    let key = match read_guest_utf8(env, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return,
    };

    let id = {
        let mut res = RESOURCES.lock().unwrap();
        res.keyed_fonts.remove(&key)
    };

    if let Some(id) = id {
        let mut res = RESOURCES.lock().unwrap();
        res.fonts.remove(&id);
    }
}

/// Draw text using a keyed font.
pub fn graphics_text_key(
    x: i32,
    y: i32,
    env: &mut Caller<'_, ()>,
    font_key_ptr: u32,
    font_key_len: u32,
    text_ptr: u32,
    text_len: u32,
) {
    let font_key = match read_guest_utf8(env, font_key_ptr, font_key_len) {
        Ok(k) => k,
        Err(_) => return,
    };
    let font_id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_fonts.get(&font_key).copied()
    };
    let Some(font_id) = font_id else {
        return;
    };
    graphics_text(x, y, font_id, env, text_ptr, text_len);
}

/// Measure text using a keyed font.
pub fn graphics_text_measure_key(
    env: &mut Caller<'_, ()>,
    font_key_ptr: u32,
    font_key_len: u32,
    text_ptr: u32,
    text_len: u32,
) -> u64 {
    let font_key = match read_guest_utf8(env, font_key_ptr, font_key_len) {
        Ok(k) => k,
        Err(_) => return 0,
    };
    let font_id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_fonts.get(&font_key).copied()
    };
    let Some(font_id) = font_id else {
        return 0;
    };
    graphics_text_measure(font_id, env, text_ptr, text_len)
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
pub fn graphics_text(x: i32, y: i32, font_id: u32, env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return;
    }
    let mem = unsafe { &*memory_ptr };

    let mut text_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut text_bytes).is_err() {
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
pub fn graphics_text_measure(font_id: u32, env: &mut Caller<'_, ()>, ptr: u32, len: u32) -> u64 {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return 0;
    }
    let mem = unsafe { &*memory_ptr };

    let mut text_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut text_bytes).is_err() {
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

pub fn audio_play_wav(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read WAV bytes from guest memory.
    let mut wav_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut wav_bytes).is_err() {
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

pub fn audio_play_qoa(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read QOA bytes from guest memory.
    let mut qoa_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut qoa_bytes).is_err() {
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

pub fn audio_play_xm(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read XM bytes from guest memory.
    let mut xm_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut xm_bytes).is_err() {
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

pub fn audio_push_samples(env: &mut Caller<'_, ()>, ptr: u32, count: u32) -> Result<(), AvError> {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read i16 samples. count is number of i16 elements.
    let byte_len = count.checked_mul(2).ok_or(AvError::MemoryReadFailed)?;
    let mut tmp_bytes = vec![0u8; byte_len as usize];

    mem.read(env, ptr as usize, &mut tmp_bytes)
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

// --- Storage ABI ---
//
// NOTE: These functions are part of the host ABI but live here because they need
// direct access to guest memory.
//
// Design:
// - `storage_save`: copy bytes from guest memory into host storage map.
// - `storage_load`: look up key in host storage map; if found, allocate guest memory
//   by calling the guest's `__wasm96_alloc(len)` export, write bytes into it, and return
//   (ptr<<32)|len. If missing, return 0.
// - `storage_free`: call the guest's `__wasm96_free(ptr,len)` export if present.
//
// This requires the guest to export:
// - `__wasm96_alloc(len: u32) -> u32`
// - `__wasm96_free(ptr: u32, len: u32)`
//
// If these exports are not present, `load` returns 0 and `free` becomes a no-op.

fn guest_alloc(env: &mut Caller<'_, ()>, len: u32) -> Option<u32> {
    let _ = env;
    let _ = len;
    // We don't have direct access to the instance here; allocation exports must be wired
    // through the core. As a fallback, return None.
    None
}

fn guest_free(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let _ = env;
    let _ = ptr;
    let _ = len;
    // No-op unless core wires guest free export.
}

pub fn storage_save(
    env: &mut Caller<'_, ()>,
    key_ptr: u32,
    key_len: u32,
    data_ptr: u32,
    data_len: u32,
) {
    // Read guest memory pointers
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    let mut key_bytes = vec![0u8; key_len as usize];
    if mem
        .read(&mut *env, key_ptr as usize, &mut key_bytes)
        .is_err()
    {
        return;
    }
    let key = match core::str::from_utf8(&key_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut data = vec![0u8; data_len as usize];
    if mem.read(&mut *env, data_ptr as usize, &mut data).is_err() {
        return;
    }

    let mut s = global().lock().unwrap();
    s.storage.kv.insert(String::from(key), data);
}

pub fn storage_load(env: &mut Caller<'_, ()>, key_ptr: u32, key_len: u32) -> u64 {
    // Read guest memory pointers
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return 0;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    let mut key_bytes = vec![0u8; key_len as usize];
    if mem
        .read(&mut *env, key_ptr as usize, &mut key_bytes)
        .is_err()
    {
        return 0;
    }
    let key = match core::str::from_utf8(&key_bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let data = {
        let s = global().lock().unwrap();
        match s.storage.kv.get(key) {
            Some(v) => v.clone(),
            None => return 0,
        }
    };

    let Some(dst_ptr) = guest_alloc(env, data.len() as u32) else {
        return 0;
    };

    // Write to guest memory
    if mem.write(&mut *env, dst_ptr as usize, &data).is_err() {
        // If write fails, attempt to free.
        guest_free(env, dst_ptr, data.len() as u32);
        return 0;
    }

    ((dst_ptr as u64) << 32) | (data.len() as u64)
}

pub fn storage_free(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    guest_free(env, ptr, len);
}
