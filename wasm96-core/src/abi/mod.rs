//! wasm96-core ABI module
//!
//! This module defines the ABI contract between:
//! - **Host**: `wasm96-core` (libretro core)
//! - **Guest**: the loaded WASM module (“game/app”)
//!
//! ## High-level model (write-only uploads)
//! The host owns **all** video/audio buffers in **system memory**.
//!
//! The guest owns its own allocations in **WASM linear memory**, and submits data to the host
//! via *write-only* upload calls:
//! - Video: guest configures a framebuffer spec, uploads a full frame, then presents.
//! - Audio: guest configures audio format, pushes interleaved i16 samples, and may request a drain.
//!
//! This keeps memory ownership clear:
//! - Host does not allocate into guest memory.
//! - Guest does not receive host pointers or handles.
//!
//! ## Imports (guest -> host)
//! Imported from module `"env"`.
//!
//! ### ABI / lifecycle
//! - `wasm96_abi_version() -> u32`
//!
//! ### Video (full-frame upload)
//! - `wasm96_video_config(width: u32, height: u32, pixel_format: u32) -> u32`
//!     - Returns 1 on success, 0 on failure.
//! - `wasm96_video_upload(ptr: u32, byte_len: u32, pitch_bytes: u32) -> u32`
//!     - Guest pointer (`ptr`) is an offset into guest linear memory.
//!     - Host copies `byte_len` bytes into its system-memory framebuffer (full-frame only).
//!     - Returns 1 on success, 0 on failure.
//! - `wasm96_video_present()`
//!
//! ### Audio (push samples)
//! - `wasm96_audio_config(sample_rate: u32, channels: u32) -> u32`
//!     - Returns 1 on success, 0 on failure.
//! - `wasm96_audio_push_i16(ptr: u32, frames: u32) -> u32`
//!     - Guest pointer (`ptr`) is an offset into guest linear memory.
//!     - Samples are interleaved i16, `frames` counts *frames* (one frame = `channels` samples).
//!     - Returns number of frames accepted (0 on failure).
//! - `wasm96_audio_drain(max_frames: u32) -> u32`
//!     - Drains up to `max_frames` frames from the host-side queue/ringbuffer into libretro.
//!
//! ### Input queries
//! - `wasm96_joypad_button_pressed(port: u32, button: u32) -> u32`
//! - `wasm96_key_pressed(key: u32) -> u32`
//! - `wasm96_mouse_x() -> i32`
//! - `wasm96_mouse_y() -> i32`
//! - `wasm96_mouse_buttons() -> u32`
//! - `wasm96_lightgun_x(port: u32) -> i32`
//! - `wasm96_lightgun_y(port: u32) -> i32`
//! - `wasm96_lightgun_buttons(port: u32) -> u32`
//!
//! ## Exports (host -> guest) required
//! The guest module **must** export at least:
//! - `wasm96_frame()`
//!
//! Optional exports:
//! - `wasm96_init()`
//! - `wasm96_deinit()`
//! - `wasm96_reset()`
//!
//! ## ABI Stability
//! We version this ABI with a single integer. Incompatible changes bump the number.

use wasmer::Function;

/// Current ABI version expected by the host.
///
/// Bump this only for breaking ABI changes.
pub const ABI_VERSION: u32 = 1;

/// Wasmer import module name used by the guest.
pub const IMPORT_MODULE: &str = "env";

/// Guest export names (entrypoints).
///
/// The guest must export at least `FRAME`.
pub mod guest_exports {
    /// Called once after instantiation (optional).
    pub const INIT: &str = "wasm96_init";
    /// Called once per libretro frame (required).
    pub const FRAME: &str = "wasm96_frame";
    /// Called when unloading (optional).
    pub const DEINIT: &str = "wasm96_deinit";
    /// Called on reset (optional).
    pub const RESET: &str = "wasm96_reset";
}

/// Host import names provided to the guest.
///
/// These are the string names under module [`IMPORT_MODULE`].
pub mod host_imports {
    // ABI
    pub const ABI_VERSION: &str = "wasm96_abi_version";

    // Video (write-only full-frame upload)
    pub const VIDEO_CONFIG: &str = "wasm96_video_config";
    pub const VIDEO_UPLOAD: &str = "wasm96_video_upload";
    pub const VIDEO_PRESENT: &str = "wasm96_video_present";

    // Audio (write-only push)
    pub const AUDIO_CONFIG: &str = "wasm96_audio_config";
    pub const AUDIO_PUSH_I16: &str = "wasm96_audio_push_i16";
    pub const AUDIO_DRAIN: &str = "wasm96_audio_drain";

    // Input (joypad/keyboard/mouse/lightgun)
    pub const JOYPAD_BUTTON_PRESSED: &str = "wasm96_joypad_button_pressed";
    pub const KEY_PRESSED: &str = "wasm96_key_pressed";
    pub const MOUSE_X: &str = "wasm96_mouse_x";
    pub const MOUSE_Y: &str = "wasm96_mouse_y";
    pub const MOUSE_BUTTONS: &str = "wasm96_mouse_buttons";
    pub const LIGHTGUN_X: &str = "wasm96_lightgun_x";
    pub const LIGHTGUN_Y: &str = "wasm96_lightgun_y";
    pub const LIGHTGUN_BUTTONS: &str = "wasm96_lightgun_buttons";
}

/// Pixel format values used by `wasm96_video_config` (and `wasm96_video_upload`).
///
/// Keep these stable; they are part of the ABI. The core can extend this over time.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    /// Packed 32-bit pixels (4 bytes per pixel). Channel order is currently treated as opaque bytes
    /// by the core; guests should default to XRGB8888 style packing (common for libretro 32bpp).
    Xrgb8888 = 0,

    /// 16-bit RGB565 packed pixels (2 bytes per pixel); optional for the core to support.
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

/// Joypad button ids used by `wasm96_joypad_button_pressed`.
///
/// These are aligned with common libretro joypad ids.
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

/// Mouse button bitmask returned by `wasm96_mouse_buttons`.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MouseButtons {
    Left = 1 << 0,
    Right = 1 << 1,
    Middle = 1 << 2,
    Button4 = 1 << 3,
    Button5 = 1 << 4,
}

/// Lightgun button bitmask returned by `wasm96_lightgun_buttons(port)`.
///
/// This is a superset-style bitmask; the core maps whatever libretro provides.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LightgunButtons {
    Trigger = 1 << 0,
    Reload = 1 << 1,
    Start = 1 << 2,
    Select = 1 << 3,
    AuxA = 1 << 4,
    AuxB = 1 << 5,
    AuxC = 1 << 6,
    Offscreen = 1 << 7,
}

/// Helpers for validating guest exports.
pub mod validate {
    use super::guest_exports;
    use wasmer::Instance;

    /// Validate that a guest instance exports the required entrypoints for this ABI.
    ///
    /// Currently required:
    /// - `wasm96_frame`
    pub fn required_exports_present(instance: &Instance) -> Result<(), MissingExport> {
        if instance.exports.get_function(guest_exports::FRAME).is_err() {
            return Err(MissingExport::Frame);
        }
        Ok(())
    }

    #[derive(Debug)]
    pub enum MissingExport {
        Frame,
    }
}

/// A small view of a guest's entrypoints as `wasmer::Function`s.
///
/// The core can resolve these once after instantiation and call them each frame.
#[derive(Clone)]
pub struct GuestEntrypoints {
    pub init: Option<Function>,
    pub frame: Function,
    pub deinit: Option<Function>,
    pub reset: Option<Function>,
}

impl GuestEntrypoints {
    /// Resolve entrypoint exports from an instance.
    pub fn resolve(instance: &wasmer::Instance) -> Result<Self, wasmer::ExportError> {
        let frame = instance.exports.get_function(guest_exports::FRAME)?.clone();

        let init = instance
            .exports
            .get_function(guest_exports::INIT)
            .ok()
            .cloned();
        let deinit = instance
            .exports
            .get_function(guest_exports::DEINIT)
            .ok()
            .cloned();
        let reset = instance
            .exports
            .get_function(guest_exports::RESET)
            .ok()
            .cloned();

        Ok(Self {
            init,
            frame,
            deinit,
            reset,
        })
    }
}
