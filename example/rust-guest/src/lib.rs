#![no_std]

// Minimal wasm96 Rust guest example (upload-based ABI).
//
// This crate is meant to be compiled to `wasm32-unknown-unknown` and loaded by `wasm96-core`.
// The host calls `wasm96_frame()` once per frame.
//
// IMPORTANT (current ABI model):
// - Guest manages its own allocations in WASM linear memory.
// - Host owns video/audio buffers in system memory.
// - Guest uploads a full frame each tick by passing a pointer to its own pixel buffer.

use wasm96_sdk::prelude::*;

// 320x240 XRGB8888: 320 * 240 * 4 = 307200 bytes.
// This is guest-owned linear memory that we fill, then upload to the host.
static mut FRAMEBUFFER: [u8; 320 * 240 * 4] = [0u8; 320 * 240 * 4];

/// Called once per frame by the host.
#[unsafe(no_mangle)]
pub extern "C" fn wasm96_frame() {
    if !abi_compatible() {
        return;
    }

    let width: u32 = 320;
    let height: u32 = 240;
    let format = PixelFormat::Xrgb8888;
    let pitch_bytes: u32 = video::pitch_bytes(width, format);
    let byte_len: u32 = height.saturating_mul(pitch_bytes);

    // Configure the host-side framebuffer spec.
    if !video::config(width, height, format) {
        return;
    }

    // Fill guest framebuffer with a simple animated pattern.
    unsafe {
        let bytes = &mut FRAMEBUFFER[..];
        let pitch = pitch_bytes as usize;

        let t = (input::mouse_x() ^ input::mouse_y()) as i32;

        for y in 0..(height as usize) {
            let row = &mut bytes[y * pitch..y * pitch + (width as usize * 4)];
            for x in 0..(width as usize) {
                let r = ((x as i32 + t) & 0xFF) as u8;
                let g = ((y as i32 - t) & 0xFF) as u8;
                let b = (((x as i32 ^ y as i32) + (t >> 1)) & 0xFF) as u8;

                // Pack as 0x00RRGGBB (XRGB8888-ish).
                let pixel: u32 = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

                let o = x * 4;
                row[o + 0] = (pixel & 0xFF) as u8;
                row[o + 1] = ((pixel >> 8) & 0xFF) as u8;
                row[o + 2] = ((pixel >> 16) & 0xFF) as u8;
                row[o + 3] = 0; // X byte
            }
        }

        // Upload full frame from guest linear memory into host system memory, then present.
        let ptr = bytes.as_ptr() as u32;
        if video::upload(ptr, byte_len, pitch_bytes) {
            video::present();
        }
    }

    // Audio (optional example):
    // If you generate audio samples in guest memory, you'd do:
    //   audio::config(48000, 2);
    //   audio::push_i16(ptr_to_i16_samples, frames);
    //   audio::drain(0);
}

// Optional lifecycle hooks (host will call if exported).
#[unsafe(no_mangle)]
pub extern "C" fn wasm96_init() {}

#[unsafe(no_mangle)]
pub extern "C" fn wasm96_deinit() {}

#[unsafe(no_mangle)]
pub extern "C" fn wasm96_reset() {}
