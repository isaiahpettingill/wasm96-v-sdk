//! Core-side shared state.
//!
//! This module owns the host-side state that bridges libretro callbacks and the
//! Wasmer host functions.
//!
//! ABI model (current):
//! - Guest owns and manages its own allocations in WASM linear memory.
//! - Host owns and manages its own allocations in system memory.
//! - Guest uploads full-frame video and audio sample batches by passing pointers
//!   into guest linear memory; host copies into host-owned buffers.
//!
//! Design goals:
//! - Keep the raw pointers (`RuntimeHandle`, `wasmer::Memory`) isolated and synchronized.
//! - Track negotiated A/V configuration plus host-owned buffers.
//! - Provide a small, safe-ish API for other core modules (`abi`, `av`, `input`).

use libretro_backend::RuntimeHandle;
use std::sync::{Mutex, OnceLock};
use wasmer::Memory;

/// Global core state accessed from:
/// - `Core::on_run` (to set the current `RuntimeHandle`)
/// - Wasmer host import functions (to read inputs and to upload/present audio/video)
#[derive(Default)]
pub struct GlobalState {
    /// Current libretro runtime handle, set at the start of `on_run`.
    /// Raw pointer used to avoid lifetime issues across Wasmer host callbacks.
    pub handle: *mut RuntimeHandle,

    /// Guest linear memory export (`memory`).
    /// Populated after instantiation.
    pub memory: *mut Memory,

    /// Host-owned video state (system memory).
    pub video: VideoState,

    /// Host-owned audio state (system memory).
    pub audio: AudioState,

    /// Cached input state (optional; can also be queried directly from handle).
    pub input: InputState,
}

// Raw pointers are used for `handle` and `memory`. We guard access with a mutex.
unsafe impl Send for GlobalState {}
unsafe impl Sync for GlobalState {}

static GLOBAL_STATE: OnceLock<Mutex<GlobalState>> = OnceLock::new();

/// Get the singleton global state mutex.
pub fn global() -> &'static Mutex<GlobalState> {
    GLOBAL_STATE.get_or_init(|| Mutex::new(GlobalState::default()))
}

/// Video configuration negotiated by the guest / configured by the host.
#[derive(Clone, Copy, Debug)]
pub struct VideoSpec {
    pub width: u32,
    pub height: u32,
    /// Bytes per row of the uploaded framebuffer.
    pub pitch_bytes: u32,
    /// Pixel format enum value (ABI-defined).
    pub pixel_format: u32,
}

impl VideoSpec {
    /// Total framebuffer size in bytes (height * pitch), saturating.
    pub fn byte_len(&self) -> usize {
        (self.height as usize).saturating_mul(self.pitch_bytes as usize)
    }
}

/// Host-owned framebuffer state.
#[derive(Debug)]
pub struct VideoState {
    /// Current negotiated spec. When `None`, the guest hasn't configured video yet.
    pub spec: Option<VideoSpec>,

    /// Host-owned framebuffer bytes (system memory).
    pub host_fb: Vec<u8>,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            spec: None,
            host_fb: Vec::new(),
        }
    }
}

/// Audio format negotiated by the guest.
///
/// We fix the sample type to interleaved stereo i16 for now because that's what
/// `libretro-backend` expects via `upload_audio_frame(&[i16])`.
#[derive(Clone, Copy, Debug)]
pub struct AudioSpec {
    /// Sample rate in Hz (e.g. 44100, 48000).
    pub sample_rate: u32,
    /// Number of channels (currently expected to be 2).
    pub channels: u32,
}

impl AudioSpec {
    pub fn samples_per_frame(&self) -> usize {
        self.channels as usize
    }
}

/// Host-owned audio buffer state.
#[derive(Debug)]
pub struct AudioState {
    /// Configured audio spec.
    pub spec: Option<AudioSpec>,

    /// Host-owned audio staging buffer (interleaved i16).
    ///
    /// The guest pushes samples; the host later drains some/all into libretro.
    pub host_queue: Vec<i16>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            spec: None,
            host_queue: Vec::new(),
        }
    }
}

/// Minimal cached input state (optional).
///
/// The core can either:
/// - query `RuntimeHandle` directly during each host import call
/// - or snapshot inputs into this structure once per frame (preferred for determinism)
#[derive(Default, Debug)]
pub struct InputState {
    /// Mouse absolute X in pixels (or normalized scale, depending on your ABI decision).
    pub mouse_x: i32,
    /// Mouse absolute Y.
    pub mouse_y: i32,
    /// Mouse buttons bitmask (ABI-defined).
    pub mouse_buttons: u32,

    /// Lightgun state (ABI-defined).
    pub lightgun_x: i32,
    pub lightgun_y: i32,
    pub lightgun_buttons: u32,

    /// Keyboard state could be expanded to a bitset; for now just store "last key".
    pub last_key: i32,
}

/// Helper: set the current runtime handle pointer in global state.
///
/// Call this at the beginning of `Core::on_run`.
pub fn set_runtime_handle(handle: &mut RuntimeHandle) {
    let mut s = global().lock().unwrap();
    s.handle = handle as *mut _;
}

/// Helper: set the current guest memory pointer in global state.
///
/// Call this after instantiation, once you obtain the exported `memory`.
pub fn set_guest_memory(memory: &Memory) {
    let mut s = global().lock().unwrap();
    s.memory = memory as *const _ as *mut _;
}

/// Helper: clear transient pointers on unload.
pub fn clear_on_unload() {
    let mut s = global().lock().unwrap();
    s.handle = std::ptr::null_mut();
    s.memory = std::ptr::null_mut();

    s.video = VideoState::default();
    s.audio = AudioState::default();
    s.input = InputState::default();
}
