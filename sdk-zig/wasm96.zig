//! wasm96 Zig SDK (handwritten)
//!
//! This module targets the wasm96 guest ABI exposed from the host under import module `"env"`.
//! The ABI is intentionally C-like and stable.
//!
//! Notes:
//! - Pointers are 32-bit offsets into guest linear memory (wasm32).
//! - ABI model is **upload-based**:
//!   - Guest owns allocations in linear memory.
//!   - Host owns video/audio buffers in system memory.
//!   - Guest performs **write-only** uploads (full-frame video, push audio samples).
//! - Guest must export at least: `export fn wasm96_frame() void`.
//! - Optional exports: `wasm96_init`, `wasm96_deinit`, `wasm96_reset`.

const std = @import("std");

pub const ABI_VERSION: u32 = 1;

/// Export names (for reference; Zig exports are named directly via `export fn`).
pub const export_names = struct {
    pub const init: []const u8 = "wasm96_init";
    pub const frame: []const u8 = "wasm96_frame";
    pub const deinit: []const u8 = "wasm96_deinit";
    pub const reset: []const u8 = "wasm96_reset";
};

/// Pixel formats: keep numeric values stable (part of the ABI).
pub const PixelFormat = enum(u32) {
    xrgb8888 = 0,
    rgb565 = 1,

    pub fn bytesPerPixel(self: PixelFormat) u32 {
        return switch (self) {
            .xrgb8888 => 4,
            .rgb565 => 2,
        };
    }
};

/// Joypad buttons: aligned to libretro joypad ids.
pub const JoypadButton = enum(u32) {
    b = 0,
    y = 1,
    select = 2,
    start = 3,
    up = 4,
    down = 5,
    left = 6,
    right = 7,
    a = 8,
    x = 9,
    l1 = 10,
    r1 = 11,
    l2 = 12,
    r2 = 13,
    l3 = 14,
    r3 = 15,
};

/// Mouse buttons bitmask.
pub const mouse_buttons = struct {
    pub const left: u32 = 1 << 0;
    pub const right: u32 = 1 << 1;
    pub const middle: u32 = 1 << 2;
    pub const button4: u32 = 1 << 3;
    pub const button5: u32 = 1 << 4;
};

/// Lightgun buttons bitmask.
pub const lightgun_buttons = struct {
    pub const trigger: u32 = 1 << 0;
    pub const reload: u32 = 1 << 1;
    pub const start: u32 = 1 << 2;
    pub const select: u32 = 1 << 3;
    pub const aux_a: u32 = 1 << 4;
    pub const aux_b: u32 = 1 << 5;
    pub const aux_c: u32 = 1 << 6;
    pub const offscreen: u32 = 1 << 7;
};

/// Low-level raw ABI declarations.
///
/// The host provides these under module `"env"`.
pub const sys = struct {
    extern "env" fn wasm96_abi_version() u32;

    // Video (upload-based)
    extern "env" fn wasm96_video_config(width: u32, height: u32, pixel_format: u32) u32;
    extern "env" fn wasm96_video_upload(ptr: u32, byte_len: u32, pitch_bytes: u32) u32;
    extern "env" fn wasm96_video_present() void;

    // Audio (push-based, interleaved i16)
    extern "env" fn wasm96_audio_config(sample_rate: u32, channels: u32) u32;
    extern "env" fn wasm96_audio_push_i16(ptr: u32, frames: u32) u32;
    extern "env" fn wasm96_audio_drain(max_frames: u32) u32;

    // Input
    extern "env" fn wasm96_joypad_button_pressed(port: u32, button: u32) u32;
    extern "env" fn wasm96_key_pressed(key: u32) u32;

    extern "env" fn wasm96_mouse_x() i32;
    extern "env" fn wasm96_mouse_y() i32;
    extern "env" fn wasm96_mouse_buttons() u32;

    extern "env" fn wasm96_lightgun_x(port: u32) i32;
    extern "env" fn wasm96_lightgun_y(port: u32) i32;
    extern "env" fn wasm96_lightgun_buttons(port: u32) u32;

    /// Optional/forward-looking interfaces (present in WIT spec but not necessarily implemented
    /// in the current core ABI). We do NOT declare them here to avoid link errors.
    /// - sram: get/mark-dirty
    /// - log: write(level, message)
};

/// ABI helper functions.
pub const abi = struct {
    /// Returns `(host_abi_version, sdk_abi_version)`.
    pub fn versions() struct { host: u32, sdk: u32 } {
        return .{ .host = sys.wasm96_abi_version(), .sdk = ABI_VERSION };
    }

    /// True if the host ABI matches this SDK's ABI version.
    pub fn compatible() bool {
        return sys.wasm96_abi_version() == ABI_VERSION;
    }
};

/// Video wrapper API.
pub const video = struct {
    /// Configure host-side video spec.
    /// Returns `true` on success.
    pub fn config(width: u32, height: u32, format: PixelFormat) bool {
        return sys.wasm96_video_config(width, height, @intFromEnum(format)) != 0;
    }

    /// Upload a full video frame from guest memory to the host.
    ///
    /// `ptr` is a u32 offset into linear memory pointing to `byte_len` bytes.
    /// `pitch_bytes` must match `width * bytesPerPixel(format)` for the configured spec.
    ///
    /// Returns `true` on success.
    pub fn upload(ptr: u32, byte_len: u32, pitch_bytes: u32) bool {
        return sys.wasm96_video_upload(ptr, byte_len, pitch_bytes) != 0;
    }

    /// Present the last uploaded framebuffer.
    pub fn present() void {
        sys.wasm96_video_present();
    }

    /// Convenience helper: compute pitch bytes for a given width/format.
    pub fn pitchBytes(width: u32, format: PixelFormat) u32 {
        return width * format.bytesPerPixel();
    }
};

/// Audio wrapper API.
///
/// Audio is interleaved i16 samples with `channels` (expected 2).
pub const audio = struct {
    /// Configure host-side audio format (interleaved i16).
    /// Returns `true` on success.
    pub fn config(sample_rate: u32, channels: u32) bool {
        return sys.wasm96_audio_config(sample_rate, channels) != 0;
    }

    /// Push interleaved i16 frames from guest memory to the host.
    ///
    /// `ptr` is a u32 offset into linear memory pointing to i16 LE samples.
    /// `frames` counts frames (1 frame = `channels` samples).
    ///
    /// Returns frames accepted (0 on failure).
    pub fn pushI16(ptr: u32, frames: u32) u32 {
        return sys.wasm96_audio_push_i16(ptr, frames);
    }

    /// Ask the host to drain up to `max_frames`.
    /// If `max_frames == 0`, host drains everything currently queued.
    pub fn drain(max_frames: u32) u32 {
        return sys.wasm96_audio_drain(max_frames);
    }
};

/// Input wrapper API.
pub const input = struct {
    pub fn joypadPressed(port: u32, button: JoypadButton) bool {
        return sys.wasm96_joypad_button_pressed(port, @intFromEnum(button)) != 0;
    }

    pub fn keyPressed(key: u32) bool {
        return sys.wasm96_key_pressed(key) != 0;
    }

    pub fn mouseX() i32 {
        return sys.wasm96_mouse_x();
    }

    pub fn mouseY() i32 {
        return sys.wasm96_mouse_y();
    }

    /// Mouse button bitmask (see `mouse_buttons`).
    pub fn mouseButtons() u32 {
        return sys.wasm96_mouse_buttons();
    }

    pub fn lightgunX(port: u32) i32 {
        return sys.wasm96_lightgun_x(port);
    }

    pub fn lightgunY(port: u32) i32 {
        return sys.wasm96_lightgun_y(port);
    }

    /// Lightgun button bitmask (see `lightgun_buttons`).
    pub fn lightgunButtons(port: u32) u32 {
        return sys.wasm96_lightgun_buttons(port);
    }
};

/// Allocator helpers were removed in the upload-based ABI.
/// The guest should manage its own allocations in linear memory (e.g. Zig allocators or static buffers).
