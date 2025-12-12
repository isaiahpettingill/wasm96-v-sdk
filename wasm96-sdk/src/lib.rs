#![cfg_attr(not(feature = "std"), no_std)]

//! wasm96-sdk (handwritten)
//!
//! This crate is used by **guest** WASM apps that run inside the `wasm96` libretro core.
//!
//! The host ABI is a small set of `extern "C"` imports from module `"env"` plus a few
//! required guest exports (`wasm96_frame`, optional `wasm96_init/deinit/reset`).
//!
//! This file intentionally contains **no WIT** and **no codegen**.

#[cfg(not(feature = "std"))]
extern crate alloc;

use core::ffi::c_void;

/// ABI version expected by the host/core.
///
/// Keep this in sync with `wasm96-core/src/abi/mod.rs`.
pub const ABI_VERSION: u32 = 1;

/// Guest entrypoint export names (for reference).
pub mod export_names {
    pub const INIT: &str = "wasm96_init";
    pub const FRAME: &str = "wasm96_frame";
    pub const DEINIT: &str = "wasm96_deinit";
    pub const RESET: &str = "wasm96_reset";
}

/// Pixel formats supported by the ABI.
///
/// Keep numeric values stable; they are part of the ABI.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Xrgb8888 = 0,
    Rgb565 = 1,
}

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            PixelFormat::Xrgb8888 => 4,
            PixelFormat::Rgb565 => 2,
        }
    }
}

/// Joypad button ids (aligned to libretro joypad ids).
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JoypadButton {
    B = 0,
    Y = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
    A = 8,
    X = 9,
    L1 = 10,
    R1 = 11,
    L2 = 12,
    R2 = 13,
    L3 = 14,
    R3 = 15,
}

/// Mouse buttons bitmask.
pub mod mouse_buttons {
    pub const LEFT: u32 = 1 << 0;
    pub const RIGHT: u32 = 1 << 1;
    pub const MIDDLE: u32 = 1 << 2;
    pub const BUTTON4: u32 = 1 << 3;
    pub const BUTTON5: u32 = 1 << 4;
}

/// Lightgun buttons bitmask.
pub mod lightgun_buttons {
    pub const TRIGGER: u32 = 1 << 0;
    pub const RELOAD: u32 = 1 << 1;
    pub const START: u32 = 1 << 2;
    pub const SELECT: u32 = 1 << 3;
    pub const AUX_A: u32 = 1 << 4;
    pub const AUX_B: u32 = 1 << 5;
    pub const AUX_C: u32 = 1 << 6;
    pub const OFFSCREEN: u32 = 1 << 7;
}

/// Low-level raw ABI imports.
///
/// These functions are provided by the host under module `"env"`.
/// Prefer using the safe wrappers in this crate instead.
#[allow(non_camel_case_types)]
pub mod sys {
    unsafe extern "C" {
        // ABI
        #[link_name = "wasm96_abi_version"]
        pub fn wasm96_abi_version() -> u32;

        // Allocation helpers (currently stubbed on host; may return 0).
        #[link_name = "wasm96_alloc"]
        pub fn wasm96_alloc(size: u32, align: u32) -> u32;
        #[link_name = "wasm96_free"]
        pub fn wasm96_free(ptr: u32, size: u32, align: u32);

        // Video
        #[link_name = "wasm96_video_request"]
        pub fn wasm96_video_request(width: u32, height: u32, pixel_format: u32) -> u32;
        #[link_name = "wasm96_video_present"]
        pub fn wasm96_video_present();
        #[link_name = "wasm96_video_pitch"]
        pub fn wasm96_video_pitch() -> u32;

        // Audio
        #[link_name = "wasm96_audio_request"]
        pub fn wasm96_audio_request(sample_rate: u32, channels: u32, capacity_frames: u32) -> u32;
        #[link_name = "wasm96_audio_capacity_frames"]
        pub fn wasm96_audio_capacity_frames() -> u32;
        #[link_name = "wasm96_audio_write_index"]
        pub fn wasm96_audio_write_index() -> u32;
        #[link_name = "wasm96_audio_read_index"]
        pub fn wasm96_audio_read_index() -> u32;
        #[link_name = "wasm96_audio_commit"]
        pub fn wasm96_audio_commit(write_index_frames: u32);
        #[link_name = "wasm96_audio_drain"]
        pub fn wasm96_audio_drain(max_frames: u32) -> u32;

        // Input
        #[link_name = "wasm96_joypad_button_pressed"]
        pub fn wasm96_joypad_button_pressed(port: u32, button: u32) -> u32;
        #[link_name = "wasm96_key_pressed"]
        pub fn wasm96_key_pressed(key: u32) -> u32;

        #[link_name = "wasm96_mouse_x"]
        pub fn wasm96_mouse_x() -> i32;
        #[link_name = "wasm96_mouse_y"]
        pub fn wasm96_mouse_y() -> i32;
        #[link_name = "wasm96_mouse_buttons"]
        pub fn wasm96_mouse_buttons() -> u32;

        #[link_name = "wasm96_lightgun_x"]
        pub fn wasm96_lightgun_x(port: u32) -> i32;
        #[link_name = "wasm96_lightgun_y"]
        pub fn wasm96_lightgun_y(port: u32) -> i32;
        #[link_name = "wasm96_lightgun_buttons"]
        pub fn wasm96_lightgun_buttons(port: u32) -> u32;
    }
}

/// Video API.
pub mod video {
    use super::{PixelFormat, sys};

    /// A configured framebuffer in guest memory.
    ///
    /// The pointer is a 32-bit offset into guest linear memory.
    #[derive(Copy, Clone, Debug)]
    pub struct Framebuffer {
        pub ptr: u32,
        pub width: u32,
        pub height: u32,
        pub pitch_bytes: u32,
        pub format: PixelFormat,
    }

    impl Framebuffer {
        /// Total framebuffer byte length (height * pitch).
        pub const fn byte_len(&self) -> u32 {
            self.height.saturating_mul(self.pitch_bytes)
        }

        /// Get a mutable byte slice view for the framebuffer.
        ///
        /// # Safety
        /// - `ptr` must point to valid guest linear memory for `byte_len()` bytes.
        /// - You must respect `pitch_bytes` when writing rows.
        pub unsafe fn as_bytes_mut<'a>(&self) -> &'a mut [u8] {
            unsafe {
                core::slice::from_raw_parts_mut(self.ptr as *mut u8, self.byte_len() as usize)
            }
        }

        /// Get a mutable slice view for 32bpp pixels (XRGB8888).
        ///
        /// # Panics
        /// Panics if the format is not `Xrgb8888` or pitch is not a multiple of 4.
        ///
        /// # Safety
        /// Same as `as_bytes_mut`.
        pub unsafe fn as_u32_pixels_mut<'a>(&self) -> &'a mut [u32] {
            assert_eq!(self.format, PixelFormat::Xrgb8888);
            assert_eq!(self.pitch_bytes % 4, 0);
            let len_u32 = (self.byte_len() / 4) as usize;
            unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut u32, len_u32) }
        }

        /// Get a mutable slice view for RGB565 pixels.
        ///
        /// # Panics
        /// Panics if the format is not `Rgb565` or pitch is not a multiple of 2.
        ///
        /// # Safety
        /// Same as `as_bytes_mut`.
        pub unsafe fn as_u16_pixels_mut<'a>(&self) -> &'a mut [u16] {
            assert_eq!(self.format, PixelFormat::Rgb565);
            assert_eq!(self.pitch_bytes % 2, 0);
            let len_u16 = (self.byte_len() / 2) as usize;
            unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut u16, len_u16) }
        }
    }

    /// Request a framebuffer from the host.
    ///
    /// Returns `None` if the request failed (`ptr == 0`).
    ///
    /// Note: The current host implementation may stub this out and always fail.
    pub fn request(width: u32, height: u32, format: PixelFormat) -> Option<Framebuffer> {
        let ptr = unsafe { sys::wasm96_video_request(width, height, format as u32) };
        if ptr == 0 {
            return None;
        }
        let pitch_bytes = unsafe { sys::wasm96_video_pitch() };
        Some(Framebuffer {
            ptr,
            width,
            height,
            pitch_bytes,
            format,
        })
    }

    /// Present the last requested framebuffer to the host.
    pub fn present() {
        unsafe { sys::wasm96_video_present() }
    }

    /// Query the current pitch in bytes (0 means not configured).
    pub fn pitch_bytes() -> u32 {
        unsafe { sys::wasm96_video_pitch() }
    }
}

/// Audio API (interleaved i16).
pub mod audio {
    use super::sys;

    /// A configured audio ringbuffer in guest memory.
    ///
    /// The pointer is a 32-bit offset into guest linear memory and should be treated
    /// as an `i16` sample buffer (interleaved).
    #[derive(Copy, Clone, Debug)]
    pub struct RingBuffer {
        pub ptr: u32,
        pub sample_rate: u32,
        pub channels: u32,
        pub capacity_frames: u32,
    }

    impl RingBuffer {
        /// Total number of i16 samples stored in the buffer.
        pub const fn capacity_samples(&self) -> u32 {
            self.capacity_frames.saturating_mul(self.channels)
        }

        /// Total size in bytes of the buffer.
        pub const fn byte_len(&self) -> u32 {
            self.capacity_samples().saturating_mul(2)
        }

        /// View the ringbuffer as a mutable i16 slice.
        ///
        /// # Safety
        /// - `ptr` must point to valid guest linear memory for `byte_len()` bytes.
        pub unsafe fn as_i16_mut<'a>(&self) -> &'a mut [i16] {
            unsafe {
                core::slice::from_raw_parts_mut(
                    self.ptr as *mut i16,
                    self.capacity_samples() as usize,
                )
            }
        }
    }

    /// Request an audio ringbuffer from the host.
    ///
    /// Returns `None` if the request failed (`ptr == 0`).
    ///
    /// Note: The current host implementation may stub this out and always fail.
    pub fn request(sample_rate: u32, channels: u32, capacity_frames: u32) -> Option<RingBuffer> {
        let ptr = unsafe { sys::wasm96_audio_request(sample_rate, channels, capacity_frames) };
        if ptr == 0 {
            return None;
        }
        let cap = unsafe { sys::wasm96_audio_capacity_frames() };
        Some(RingBuffer {
            ptr,
            sample_rate,
            channels,
            capacity_frames: cap,
        })
    }

    /// Get producer write index in frames.
    pub fn write_index() -> u32 {
        unsafe { sys::wasm96_audio_write_index() }
    }

    /// Get consumer read index in frames.
    pub fn read_index() -> u32 {
        unsafe { sys::wasm96_audio_read_index() }
    }

    /// Commit a new producer write index (frames, modulo capacity).
    pub fn commit(write_index_frames: u32) {
        unsafe { sys::wasm96_audio_commit(write_index_frames) }
    }

    /// Ask the host to drain up to `max_frames` into libretro audio output.
    ///
    /// If `max_frames == 0`, the host drains as much as it wants.
    pub fn drain(max_frames: u32) -> u32 {
        unsafe { sys::wasm96_audio_drain(max_frames) }
    }
}

/// Input API.
pub mod input {
    use super::{JoypadButton, sys};

    /// True if `button` is pressed on `port`.
    pub fn joypad_pressed(port: u32, button: JoypadButton) -> bool {
        unsafe { sys::wasm96_joypad_button_pressed(port, button as u32) != 0 }
    }

    /// True if `key` is pressed.
    ///
    /// `key` is an implementation-defined key code (recommend: libretro key ids or USB HID).
    pub fn key_pressed(key: u32) -> bool {
        unsafe { sys::wasm96_key_pressed(key) != 0 }
    }

    /// Mouse X coordinate.
    pub fn mouse_x() -> i32 {
        unsafe { sys::wasm96_mouse_x() }
    }

    /// Mouse Y coordinate.
    pub fn mouse_y() -> i32 {
        unsafe { sys::wasm96_mouse_y() }
    }

    /// Mouse buttons bitmask (see `mouse_buttons::*`).
    pub fn mouse_buttons() -> u32 {
        unsafe { sys::wasm96_mouse_buttons() }
    }

    /// Lightgun X coordinate for `port`.
    pub fn lightgun_x(port: u32) -> i32 {
        unsafe { sys::wasm96_lightgun_x(port) }
    }

    /// Lightgun Y coordinate for `port`.
    pub fn lightgun_y(port: u32) -> i32 {
        unsafe { sys::wasm96_lightgun_y(port) }
    }

    /// Lightgun buttons bitmask (see `lightgun_buttons::*`).
    pub fn lightgun_buttons(port: u32) -> u32 {
        unsafe { sys::wasm96_lightgun_buttons(port) }
    }
}

/// ABI helpers.
pub mod abi {
    use super::{ABI_VERSION, sys};

    /// Returns `(host_abi_version, sdk_abi_version)`.
    pub fn versions() -> (u32, u32) {
        let host = unsafe { sys::wasm96_abi_version() };
        (host, ABI_VERSION)
    }

    /// True if the host ABI matches this SDK's ABI version.
    pub fn compatible() -> bool {
        let host = unsafe { sys::wasm96_abi_version() };
        host == ABI_VERSION
    }
}

/// Convenience prelude for guest apps.
pub mod prelude {
    pub use crate::abi::{compatible as abi_compatible, versions as abi_versions};
    pub use crate::audio;
    pub use crate::input;
    pub use crate::lightgun_buttons;
    pub use crate::mouse_buttons;
    pub use crate::video;
    pub use crate::{ABI_VERSION, JoypadButton, PixelFormat};
}

// Keep `c_void` referenced so it doesn't look unused in some configurations.
#[allow(dead_code)]
const _C_VOID: *const c_void = core::ptr::null();
