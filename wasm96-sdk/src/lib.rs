#![cfg_attr(not(feature = "std"), no_std)]

//! wasm96-sdk (handwritten)
//!
//! This crate is used by **guest** WASM apps that run inside the `wasm96` libretro core.
//!
//! ABI model (upload-based):
//! - Guest owns allocations in WASM linear memory.
//! - Host owns its video/audio buffers in system memory.
//! - Guest performs **write-only** uploads:
//!   - Video: configure -> upload full frame -> present
//!   - Audio: configure -> push i16 frames -> drain (optional)
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

        // Video (upload-based)
        #[link_name = "wasm96_video_config"]
        pub fn wasm96_video_config(width: u32, height: u32, pixel_format: u32) -> u32;
        #[link_name = "wasm96_video_upload"]
        pub fn wasm96_video_upload(ptr: u32, byte_len: u32, pitch_bytes: u32) -> u32;
        #[link_name = "wasm96_video_present"]
        pub fn wasm96_video_present();

        // Audio (push-based, interleaved i16)
        #[link_name = "wasm96_audio_config"]
        pub fn wasm96_audio_config(sample_rate: u32, channels: u32) -> u32;
        #[link_name = "wasm96_audio_push_i16"]
        pub fn wasm96_audio_push_i16(ptr: u32, frames: u32) -> u32;
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

    /// Configure the host-side framebuffer spec.
    ///
    /// Returns `true` on success.
    pub fn config(width: u32, height: u32, format: PixelFormat) -> bool {
        unsafe { sys::wasm96_video_config(width, height, format as u32) != 0 }
    }

    /// Upload a full frame to the host.
    ///
    /// `ptr` is a u32 offset into guest linear memory.
    /// `byte_len` must be exactly `height * pitch_bytes` for the configured framebuffer.
    ///
    /// Returns `true` on success.
    pub fn upload(ptr: u32, byte_len: u32, pitch_bytes: u32) -> bool {
        unsafe { sys::wasm96_video_upload(ptr, byte_len, pitch_bytes) != 0 }
    }

    /// Present the last uploaded framebuffer to the host.
    pub fn present() {
        unsafe { sys::wasm96_video_present() }
    }

    /// Convenience helper to compute pitch bytes for a given width+format.
    pub const fn pitch_bytes(width: u32, format: PixelFormat) -> u32 {
        width * format.bytes_per_pixel()
    }
}

/// Audio API (interleaved i16).
pub mod audio {
    use super::sys;

    /// Configure host-side audio output format.
    ///
    /// Returns `true` on success.
    pub fn config(sample_rate: u32, channels: u32) -> bool {
        unsafe { sys::wasm96_audio_config(sample_rate, channels) != 0 }
    }

    /// Push interleaved i16 samples to the host.
    ///
    /// `ptr` is a u32 offset into guest linear memory that points to `frames * channels`
    /// i16 samples (little-endian).
    ///
    /// Returns number of frames accepted (0 on failure).
    pub fn push_i16(ptr: u32, frames: u32) -> u32 {
        unsafe { sys::wasm96_audio_push_i16(ptr, frames) }
    }

    /// Drain up to `max_frames` from the host-side queue into libretro.
    ///
    /// If `max_frames == 0`, the host drains everything it currently has queued.
    /// Returns frames drained.
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
