// Needed for `alloc::` in this crate.
extern crate alloc;

use crate::state::global;
use wasmtime::Caller;

// External crates for rendering

// External crates for asset decoding

// Storage ABI helpers
use alloc::vec::Vec;

use super::AvError;

pub fn read_guest_bytes(
    caller: &mut Caller<'_, ()>,
    ptr: u32,
    len: u32,
) -> Result<Vec<u8>, AvError> {
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

pub fn graphics_line_internal(x1: i32, y1: i32, x2: i32, y2: i32) {
    super::graphics::graphics_line(x1, y1, x2, y2);
}

#[inline]
pub fn tri_edge(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i64 {
    (c.0 as i64 - a.0 as i64) * (b.1 as i64 - a.1 as i64)
        - (c.1 as i64 - a.1 as i64) * (b.0 as i64 - a.0 as i64)
}

pub fn graphics_image_from_host(x: i32, y: i32, w: u32, h: u32, data: &[u8]) {
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
pub fn system_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[inline]
pub fn sat_add_i16(a: i16, b: i16) -> i16 {
    let s = a as i32 + b as i32;
    if s > i16::MAX as i32 {
        i16::MAX
    } else if s < i16::MIN as i32 {
        i16::MIN
    } else {
        s as i16
    }
}

pub fn guest_alloc(env: &mut Caller<'_, ()>, len: u32) -> Option<u32> {
    let _ = env;
    let _ = len;
    // We don't have direct access to the instance here; allocation exports must be wired
    // through the core. As a fallback, return None.
    None
}

pub fn guest_free(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let _ = env;
    let _ = ptr;
    let _ = len;
    // No-op unless core wires guest free export.
}
