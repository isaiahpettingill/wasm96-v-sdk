//! wasm96-core ABI module
//!
//! This module defines the ABI contract between:
//! - **Host**: `wasm96-core` (libretro core)
//! - **Guest**: the loaded WASM module (“game/app”)
//!
//! ## High-level model (buffer-based)
//! The host gives the guest pointers into *guest linear memory* for:
//! - a **framebuffer**
//! - an **audio ringbuffer**
//!
//! The guest **requests** these buffers with a desired size/spec, then writes into them.
//! The guest then **commits/presents** to let the host copy data out to libretro.
//!
//! This is intentionally simple and deterministic:
//! - The guest controls video size per game by requesting a framebuffer spec.
//! - The guest controls its update/draw loop by exporting a per-frame function that the host calls.
//!
//! ## Imports (guest -> host)
//! Imported from module `"env"`.
//!
//! ### ABI / lifecycle
//! - `wasm96_abi_version() -> u32`
//!
//! ### Memory management helpers
//! - `wasm96_alloc(size: u32, align: u32) -> u32`
//! - `wasm96_free(ptr: u32, size: u32, align: u32)`
//!
//! ### Video (framebuffer)
//! - `wasm96_video_request(width: u32, height: u32, pixel_format: u32) -> u32`
//!     - Returns framebuffer pointer (guest offset) on success, or 0 on failure.
//! - `wasm96_video_present()`
//!     - Host copies the configured framebuffer (height * pitch bytes) and uploads it.
//! - `wasm96_video_pitch() -> u32`
//!     - Bytes per row of the configured framebuffer (0 if not configured).
//!
//! ### Audio (ringbuffer)
//! - `wasm96_audio_request(sample_rate: u32, channels: u32, capacity_frames: u32) -> u32`
//!     - Returns ringbuffer pointer (guest offset) on success, or 0.
//! - `wasm96_audio_capacity_frames() -> u32`
//! - `wasm96_audio_write_index() -> u32`
//! - `wasm96_audio_read_index() -> u32`
//! - `wasm96_audio_commit(write_index_frames: u32)`
//!     - Guest sets its producer index (mod capacity). Host will drain available frames.
//! - `wasm96_audio_drain(max_frames: u32) -> u32`
//!     - Ask host to drain up to N frames from ringbuffer into libretro; returns drained frames.
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
//! Notes:
//! - “All controls” here means the core will expose a generalized set of input queries for
//!   joypad/keyboard/mouse/lightgun. The actual mapping to libretro devices lives in `crate::input`.
//!
//! ## Exports (host -> guest) required
//! The guest module **must** export at least:
//! - `wasm96_frame()`
//!   Called once per libretro `on_run` tick. The guest can implement its own update/draw loop here.
//!
//! Recommended exports (optional but highly useful):
//! - `wasm96_init()`: called once after instantiation / after the host sets up imports
//! - `wasm96_deinit()`: called on game unload
//! - `wasm96_reset()`: called on reset
//! - `wasm96_get_av_info(out_ptr: u32) -> u32`: write desired AV info struct (future)
//!
//! ## ABI Stability
//! We version this ABI with a single integer. Incompatible changes bump the number.
//!
//! The core should refuse to run guests that report a different ABI version.

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

    // Guest memory allocation (host calls guest malloc-like? no: guest calls host alloc)
    // The core will implement these by delegating to guest exports if provided, or by using a
    // simple bump allocator (future). For now we define the ABI.
    pub const ALLOC: &str = "wasm96_alloc";
    pub const FREE: &str = "wasm96_free";

    // Video (buffer-based)
    pub const VIDEO_REQUEST: &str = "wasm96_video_request";
    pub const VIDEO_PRESENT: &str = "wasm96_video_present";
    pub const VIDEO_PITCH: &str = "wasm96_video_pitch";

    // Audio (ringbuffer-based)
    pub const AUDIO_REQUEST: &str = "wasm96_audio_request";
    pub const AUDIO_CAPACITY_FRAMES: &str = "wasm96_audio_capacity_frames";
    pub const AUDIO_WRITE_INDEX: &str = "wasm96_audio_write_index";
    pub const AUDIO_READ_INDEX: &str = "wasm96_audio_read_index";
    pub const AUDIO_COMMIT: &str = "wasm96_audio_commit";
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

/// Pixel format values used by `wasm96_video_request`.
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
