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

// Storage ABI helpers
use alloc::vec::Vec;

use super::resources::{AvError, FontResource, GifResource, PngResource, RESOURCES};
use super::utils::{
    graphics_image_from_host, graphics_line_internal, read_guest_bytes, system_millis, tri_edge,
};

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
    key: u64,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
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
pub fn graphics_png_draw_key(key: u64, x: i32, y: i32) {
    let png = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_pngs.get(&key).cloned()
    };

    if let Some(png) = png {
        graphics_image_from_host(x, y, png.width, png.height, &png.rgba);
    }
}

/// Draw a keyed PNG scaled (nearest-neighbor).
pub fn graphics_png_draw_key_scaled(key: u64, x: i32, y: i32, w: u32, h: u32) {
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
pub fn graphics_png_unregister(key: u64) {
    let mut res = RESOURCES.lock().unwrap();
    res.keyed_pngs.remove(&key);
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
/// Register SVG resource under a string key.
pub fn graphics_svg_register(
    caller: &mut Caller<'_, ()>,
    key: u64,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
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
pub fn graphics_svg_draw_key(key: u64, x: i32, y: i32, w: u32, h: u32) {
    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_svgs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_svg_draw(id, x, y, w, h);
    }
}

/// Unregister keyed SVG and free the underlying resource.
pub fn graphics_svg_unregister(key: u64) {
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
        let pixmap_size = tiny_skia::IntSize::from_wh(w, h).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        let sx = w as f32 / tree.size().width();
        let sy = h as f32 / tree.size().height();
        let transform = tiny_skia::Transform::from_scale(sx, sy);

        resvg::render(tree, transform, &mut pixmap.as_mut());
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

    let width = decoder.width();
    let height = decoder.height();
    let global_palette: Option<Vec<u8>> = decoder.global_palette().map(|p| p.to_vec());

    let mut frames = Vec::new();
    let mut delays = Vec::new();

    // Canvas for composition (RGBA)
    let mut canvas = vec![0u8; width as usize * height as usize * 4];
    // Backup for "Restore to Previous" disposal
    let mut previous_canvas = canvas.clone();

    let mut last_disposal = gif::DisposalMethod::Any;
    let mut last_rect = (0u16, 0u16, 0u16, 0u16); // left, top, width, height

    while let Some(frame) = match decoder.read_next_frame() {
        Ok(f) => f,
        Err(_) => return 0,
    } {
        // 1. Handle disposal of the *previous* frame
        match last_disposal {
            gif::DisposalMethod::Any | gif::DisposalMethod::Keep => {
                // Do nothing, draw on top
            }
            gif::DisposalMethod::Background => {
                // Restore background (transparent) for the area of the previous frame
                let (lx, ly, lw, lh) = last_rect;
                for y in ly..(ly + lh) {
                    if y >= height {
                        break;
                    }
                    for x in lx..(lx + lw) {
                        if x >= width {
                            break;
                        }
                        let idx = ((y as usize) * (width as usize) + (x as usize)) * 4;
                        if idx + 3 < canvas.len() {
                            canvas[idx] = 0;
                            canvas[idx + 1] = 0;
                            canvas[idx + 2] = 0;
                            canvas[idx + 3] = 0;
                        }
                    }
                }
            }
            gif::DisposalMethod::Previous => {
                // Restore to state before previous frame
                canvas = previous_canvas.clone();
            }
        }

        // Save state if *current* frame says "Restore to Previous" (for the next iteration)
        if frame.dispose == gif::DisposalMethod::Previous {
            previous_canvas = canvas.clone();
        }

        last_disposal = frame.dispose;
        last_rect = (frame.left, frame.top, frame.width, frame.height);

        // 2. Draw current frame onto canvas
        let palette: Option<&[u8]> = frame.palette.as_deref().or(global_palette.as_deref());
        if let Some(palette) = palette {
            let transparent_idx = frame.transparent;
            let fw = frame.width as usize;
            let fh = frame.height as usize;
            let fl = frame.left as usize;
            let ft = frame.top as usize;

            // Helper to write a pixel
            let mut put_pixel = |x: usize, y: usize, color_idx: u8| {
                if Some(color_idx) == transparent_idx {
                    return;
                }
                let base = (color_idx as usize) * 3;
                if base + 2 >= palette.len() {
                    return;
                }
                let r = palette[base];
                let g = palette[base + 1];
                let b = palette[base + 2];

                let cx = fl + x;
                let cy = ft + y;
                if cx < width as usize && cy < height as usize {
                    let idx = (cy * (width as usize) + cx) * 4;
                    canvas[idx] = r;
                    canvas[idx + 1] = g;
                    canvas[idx + 2] = b;
                    canvas[idx + 3] = 255;
                }
            };

            if frame.interlaced {
                let mut offset = 0;
                // Pass 1: Every 8th row, starting at 0
                for y in (0..fh).step_by(8) {
                    for x in 0..fw {
                        if offset < frame.buffer.len() {
                            put_pixel(x, y, frame.buffer[offset]);
                            offset += 1;
                        }
                    }
                }
                // Pass 2: Every 8th row, starting at 4
                for y in (4..fh).step_by(8) {
                    for x in 0..fw {
                        if offset < frame.buffer.len() {
                            put_pixel(x, y, frame.buffer[offset]);
                            offset += 1;
                        }
                    }
                }
                // Pass 3: Every 4th row, starting at 2
                for y in (2..fh).step_by(4) {
                    for x in 0..fw {
                        if offset < frame.buffer.len() {
                            put_pixel(x, y, frame.buffer[offset]);
                            offset += 1;
                        }
                    }
                }
                // Pass 4: Every 2nd row, starting at 1
                for y in (1..fh).step_by(2) {
                    for x in 0..fw {
                        if offset < frame.buffer.len() {
                            put_pixel(x, y, frame.buffer[offset]);
                            offset += 1;
                        }
                    }
                }
            } else {
                // Normal
                for (i, &idx) in frame.buffer.iter().enumerate() {
                    let x = i % fw;
                    let y = i / fw;
                    put_pixel(x, y, idx);
                }
            }
        }

        frames.push(canvas.clone());
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
        let total_delay_ms: u64 = gif.delays.iter().map(|&d| d as u64 * 10).sum();

        let mut frame_idx = 0;
        if total_delay_ms > 0 {
            let mut rem = millis % total_delay_ms;
            for (i, &d) in gif.delays.iter().enumerate() {
                let d_ms = d as u64 * 10;
                // Treat 0 delay as 100ms (common GIF viewer behavior)
                let effective_delay = if d_ms == 0 { 100 } else { d_ms };
                if rem < effective_delay {
                    frame_idx = i;
                    break;
                }
                rem = rem.saturating_sub(effective_delay);
            }
        }

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
    key: u64,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let id = graphics_gif_create(env, data_ptr, data_len);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_gifs.insert(key, id);
    1
}

/// Draw keyed GIF at natural size.
pub fn graphics_gif_draw_key(key: u64, x: i32, y: i32) {
    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_gifs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_gif_draw(id, x, y);
    }
}

/// Draw keyed GIF scaled.
pub fn graphics_gif_draw_key_scaled(key: u64, x: i32, y: i32, w: u32, h: u32) {
    let id = {
        let res = RESOURCES.lock().unwrap();
        res.keyed_gifs.get(&key).copied()
    };

    if let Some(id) = id {
        graphics_gif_draw_scaled(id, x, y, w, h);
    }
}

/// Unregister keyed GIF and destroy its underlying resource.
pub fn graphics_gif_unregister(key: u64) {
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
    key: u64,
    data_ptr: u32,
    data_len: u32,
) -> u32 {
    let id = graphics_font_upload_ttf(env, data_ptr, data_len);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_fonts.insert(key, id);
    1
}

/// Register built-in Spleen font under a string key.
pub fn graphics_font_register_spleen(key: u64, size: u32) -> u32 {
    let id = graphics_font_use_spleen(size);
    if id == 0 {
        return 0;
    }

    let mut res = RESOURCES.lock().unwrap();
    res.keyed_fonts.insert(key, id);
    1
}

/// Unregister keyed font.
pub fn graphics_font_unregister(key: u64) {
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
    font_key: u64,
    text_ptr: u32,
    text_len: u32,
) {
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
    font_key: u64,
    text_ptr: u32,
    text_len: u32,
) -> u64 {
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
            for inner_line in lines.by_ref() {
                if inner_line.starts_with("ENCODING") {
                    if let Some(enc_str) = inner_line.split_whitespace().nth(1) {
                        encoding = enc_str.parse::<u32>().ok().and_then(char::from_u32);
                    }
                } else if inner_line == "BITMAP" {
                    in_bitmap = true;
                } else if inner_line == "ENDCHAR" {
                    break;
                } else if in_bitmap && let Ok(byte) = u8::from_str_radix(inner_line.trim(), 16) {
                    bitmap.push(byte);
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
        8 => (super::resources::SPLEEN_5X8, 5, 8),
        16 => (super::resources::SPLEEN_8X16, 8, 16),
        24 => (super::resources::SPLEEN_12X24, 12, 24),
        32 => (super::resources::SPLEEN_16X32, 16, 32),
        64 => (super::resources::SPLEEN_32X64, 32, 64),
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
                // Lock global state once for the whole string to enable blending
                let mut s = global().lock().unwrap();
                let width = s.video.width as i32;
                let height = s.video.height as i32;
                let draw_color = s.video.draw_color;
                let r_fg = ((draw_color >> 16) & 0xFF) as u32;
                let g_fg = ((draw_color >> 8) & 0xFF) as u32;
                let b_fg = (draw_color & 0xFF) as u32;

                let mut px = x as f32;
                for ch in text.chars() {
                    let (metrics, bitmap) = f.rasterize(ch, 16.0); // fixed size
                    for (i, &alpha) in bitmap.iter().enumerate() {
                        if alpha > 0 {
                            let gx = px as i32 + (i % metrics.width) as i32;
                            let gy = y + (i / metrics.width) as i32;

                            if gx >= 0 && gx < width && gy >= 0 && gy < height {
                                let idx = (gy * width + gx) as usize;
                                let bg = s.video.framebuffer[idx];

                                // Alpha blend
                                let a = alpha as u32;
                                let inv_a = 255 - a;

                                let r_bg = (bg >> 16) & 0xFF;
                                let g_bg = (bg >> 8) & 0xFF;
                                let b_bg = bg & 0xFF;

                                let r = (r_fg * a + r_bg * inv_a) / 255;
                                let g = (g_fg * a + g_bg * inv_a) / 255;
                                let b = (b_fg * a + b_bg * inv_a) / 255;

                                s.video.framebuffer[idx] = (r << 16) | (g << 8) | b;
                            }
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
            FontResource::Spleen {
                width,
                height,
                glyphs: _,
            } => (text.chars().count() as u32 * *width, *height),
        }
    } else {
        (0, 0)
    };

    ((width as u64) << 32) | (height as u64)
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
