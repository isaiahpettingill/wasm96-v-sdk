//! Core-side shared state.
//!
//! This module owns the host-side state that bridges libretro callbacks and the
//! Wasmer host functions.
//!
//! In the "host provides buffers" ABI, the guest requests frame/audio buffers,
//! receives a pointer into guest memory, writes into it, then asks the host to
//! present/consume (which copies from guest memory into libretro).
//!
//! Design goals:
//! - Keep the raw pointers (`RuntimeHandle`, `wasmer::Memory`) isolated and synchronized.
//! - Track negotiated framebuffer + audio-ringbuffer specs and pointers.
//! - Store resolved guest exports (e.g. allocator) so host imports can call them.
//! - Provide a small, safe-ish API for other core modules (`abi`, `av`, `input`).

use libretro_backend::RuntimeHandle;
use std::sync::{Mutex, OnceLock};
use wasmer::{Function, Memory};

/// A pointer into guest linear memory (WASM32 offset).
///
/// We keep this as `u32` because WASM linear memory addressing is 32-bit.
pub type GuestPtr = u32;

/// Global core state accessed from:
/// - `Core::on_run` (to set the current `RuntimeHandle`)
/// - Wasmer host import functions (to read inputs and to present audio/video)
#[derive(Default)]
pub struct GlobalState {
    /// Current libretro runtime handle, set at the start of `on_run`.
    /// Raw pointer used to avoid lifetime issues across Wasmer host callbacks.
    pub handle: *mut RuntimeHandle,

    /// Guest linear memory export (`memory`).
    /// Populated after instantiation.
    pub memory: *mut Memory,

    /// Guest allocator export (required for buffer requests).
    ///
    /// Expected signature (WASM-side):
    /// `fn wasm96_alloc(size: u32, align: u32) -> u32`
    ///
    /// Stored here so host import functions can allocate frame/audio buffers without
    /// needing to borrow the core instance.
    pub guest_alloc: Option<Function>,

    /// Guest free export (optional).
    ///
    /// Expected signature:
    /// `fn wasm96_free(ptr: u32, size: u32, align: u32)`
    pub guest_free: Option<Function>,

    /// Negotiated framebuffer state (guest-owned memory, host-presented).
    pub video: VideoState,

    /// Negotiated audio ring buffer state (guest-owned memory, host-consumed).
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

/// Store guest allocator/free exports in global state.
///
/// Call this after instantiation, once you can resolve exports from the instance.
pub fn set_guest_allocator(alloc: Function, free: Option<Function>) {
    let mut s = global().lock().unwrap();
    s.guest_alloc = Some(alloc);
    s.guest_free = free;
}

/// Store guest allocator/free exports in global state (either/both may be None).
///
/// Call this after instantiation, once you can resolve exports from the instance.
///
/// This is intentionally tolerant: you can run guests that don't support `free`.
pub fn set_guest_allocators(alloc: Option<Function>, free: Option<Function>) {
    let mut s = global().lock().unwrap();
    s.guest_alloc = alloc;
    s.guest_free = free;
}

/// NOTE: Guest allocator calling has been removed from `state`.
///
/// Calling guest-exported functions (like `wasm96_alloc` / `wasm96_free`) requires access to
/// a `&mut wasmer::Store` (or `StoreMut`), which is not available from this global state module.
///
/// The correct fix is to move allocation to a place that *has* store access (e.g. the core
/// instance or an env struct carried by `FunctionEnv`).
///
/// For now, we only store the resolved guest allocator functions (if any) so other modules
/// can decide how to use them once a proper store access strategy is implemented.

/// Video configuration negotiated by the guest.
#[derive(Clone, Copy, Debug)]
pub struct VideoSpec {
    pub width: u32,
    pub height: u32,
    /// Bytes per row.
    pub pitch: u32,
    /// Pixel format enum value (ABI-defined). The core currently treats this as opaque
    /// but can use it to validate pitch and build libretro pixel format later.
    pub pixel_format: u32,
}

impl VideoSpec {
    /// Total framebuffer size in bytes (height * pitch), saturating.
    pub fn byte_len(&self) -> usize {
        (self.height as usize).saturating_mul(self.pitch as usize)
    }
}

/// Framebuffer state shared between guest and host.
#[derive(Debug)]
pub struct VideoState {
    /// Current negotiated spec. When `None`, the guest hasn't requested a buffer yet.
    pub spec: Option<VideoSpec>,

    /// Guest pointer to the framebuffer base.
    pub fb_ptr: Option<GuestPtr>,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            spec: None,
            fb_ptr: None,
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
    pub fn bytes_per_frame(&self) -> usize {
        // i16 * channels
        (self.channels as usize).saturating_mul(2)
    }
}

/// Audio ring buffer state.
///
/// The guest requests a buffer (capacity in frames), writes samples into it, and calls
/// `audio_commit(frames_written)` to indicate how many new frames are available.
/// The core then copies those samples out and passes them to libretro.
#[derive(Debug)]
pub struct AudioState {
    pub spec: Option<AudioSpec>,

    /// Guest pointer to ring buffer base (i16 samples).
    pub ring_ptr: Option<GuestPtr>,

    /// Capacity in frames (not bytes).
    pub capacity_frames: u32,

    /// Producer index in frames, written by the guest via `audio_commit`.
    pub write_index_frames: u32,

    /// Consumer index in frames, advanced by the core after uploading to libretro.
    pub read_index_frames: u32,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            spec: None,
            ring_ptr: None,
            capacity_frames: 0,
            write_index_frames: 0,
            read_index_frames: 0,
        }
    }
}

impl AudioState {
    pub fn is_configured(&self) -> bool {
        self.spec.is_some() && self.ring_ptr.is_some() && self.capacity_frames != 0
    }

    /// Number of readable frames currently in the ringbuffer.
    ///
    /// Uses modulo arithmetic over `capacity_frames`.
    pub fn available_frames(&self) -> u32 {
        let cap = self.capacity_frames;
        if cap == 0 {
            return 0;
        }
        let w = self.write_index_frames % cap;
        let r = self.read_index_frames % cap;
        if w >= r { w - r } else { cap - (r - w) }
    }

    /// Advance the consumer (read) index by `frames`, modulo capacity.
    pub fn consume_frames(&mut self, frames: u32) {
        let cap = self.capacity_frames;
        if cap == 0 {
            return;
        }
        self.read_index_frames = (self.read_index_frames.wrapping_add(frames)) % cap;
    }

    /// Set write index from guest commit (mod capacity).
    pub fn set_write_index(&mut self, new_write_index_frames: u32) {
        let cap = self.capacity_frames;
        if cap == 0 {
            self.write_index_frames = 0;
            return;
        }
        self.write_index_frames = new_write_index_frames % cap;
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

    s.guest_alloc = None;
    s.guest_free = None;

    s.video = VideoState::default();
    s.audio = AudioState::default();
    s.input = InputState::default();
}
