//! Core-side shared state.
//!
//! This module owns the host-side state that bridges libretro callbacks and the
//! Wasmer host functions.
//!
//! ABI model (Immediate Mode):
//! - Host owns the framebuffer and handles all rendering commands.
//! - Guest issues commands (draw rect, line, etc.) which modify the host framebuffer.
//! - Host presents the framebuffer to libretro at the end of the frame.

use libretro_backend::RuntimeHandle;
use std::sync::{Mutex, OnceLock};
use wasmer::Memory;

/// A single host-side “audio channel” (a.k.a. a mixing voice).
///
/// This is used for higher-level playback APIs (e.g. `play_wav`, `play_ogg`, etc.)
/// where the host decodes and mixes audio. Guests get back an `id` that can be
/// adjusted (volume/pan/loop/stop) without pushing raw samples every frame.
///
/// NOTE: Actual decoding/mixing logic lives elsewhere (e.g. `av`); this is only state.
#[derive(Debug, Clone)]
pub struct AudioChannel {
    /// Whether this channel is currently active/playing.
    pub active: bool,

    /// Channel volume in Q8.8 fixed-point (256 = 1.0x).
    pub volume_q8_8: u32,

    /// Pan in i16 domain: -32768 = full left, 0 = center, 32767 = full right.
    pub pan_i16: i32,

    /// Whether playback should loop when reaching end.
    pub loop_enabled: bool,

    /// Interleaved stereo PCM samples (i16) for this channel.
    ///
    /// This is a simple representation that enables mixing without requiring the
    /// guest to continuously feed audio. Decoders can fill this buffer and reset
    /// `position_frames` as needed.
    pub pcm_stereo: Vec<i16>,

    /// Current playback position in *frames* (not i16 samples).
    /// One frame = 2 i16 samples (L, R).
    pub position_frames: usize,

    /// Source sample rate for this channel's PCM.
    pub sample_rate: u32,
}

impl Default for AudioChannel {
    fn default() -> Self {
        Self {
            active: false,
            volume_q8_8: 256,
            pan_i16: 0,
            loop_enabled: false,
            pcm_stereo: Vec::new(),
            position_frames: 0,
            sample_rate: 44100,
        }
    }
}

/// Global core state accessed from:
/// - `Core::on_run` (to set the current `RuntimeHandle`)
/// - Wasmer host import functions
#[derive(Default)]
pub struct GlobalState {
    /// Current libretro runtime handle.
    pub handle: *mut RuntimeHandle,

    /// Guest linear memory export (`memory`).
    pub memory: *mut Memory,

    /// Host-owned video state (system memory).
    pub video: VideoState,

    /// Host-owned audio state (system memory).
    pub audio: AudioState,

    /// Cached input state.
    pub input: InputState,
}

// Raw pointers are used for `handle` and `memory`. We guard access with a mutex.
unsafe impl Send for GlobalState {}
unsafe impl Sync for GlobalState {}

static GLOBAL_STATE: OnceLock<Mutex<GlobalState>> = OnceLock::new();

pub fn global() -> &'static Mutex<GlobalState> {
    GLOBAL_STATE.get_or_init(|| Mutex::new(GlobalState::default()))
}

/// Host-owned framebuffer state for immediate mode drawing.
#[derive(Debug)]
pub struct VideoState {
    pub width: u32,
    pub height: u32,

    /// Framebuffer pixels (XRGB8888).
    /// Size is width * height.
    /// Stored as `u32` for easy pixel manipulation.
    /// Format: 0x00RRGGBB (little endian in memory: BB GG RR 00).
    pub framebuffer: Vec<u32>,

    /// Current drawing color (packed 0x00RRGGBB for XRGB8888).
    pub draw_color: u32,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            width: 320, // Default size until set_size is called
            height: 240,
            framebuffer: vec![0; 320 * 240],
            draw_color: 0x00FFFFFF, // Default white
        }
    }
}

/// Host-owned audio buffer state.
#[derive(Debug)]
pub struct AudioState {
    /// Output sample rate (what libretro expects).
    pub sample_rate: u32,

    /// Guest-provided staging buffer (interleaved i16 stereo).
    ///
    /// This is still supported for “raw push” style audio.
    pub host_queue: Vec<i16>,

    /// Host-mixed playback channels (decoded assets like WAV/QOA/M4A/OGG).
    ///
    /// Guests can trigger playback via higher-level audio APIs and the core will mix
    /// these channels into the output stream.
    pub channels: Vec<AudioChannel>,

    /// Next id to assign for a new `AudioChannel`.
    pub next_channel_id: u32,

    /// Mapping from public channel id to index in `channels`.
    ///
    /// NOTE: Kept as a simple vec-of-pairs to avoid pulling in a HashMap in state;
    /// the number of channels is expected to be small.
    pub channel_id_to_index: Vec<(u32, usize)>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            host_queue: Vec::new(),

            channels: Vec::new(),
            next_channel_id: 1,
            channel_id_to_index: Vec::new(),
        }
    }
}

/// Minimal cached input state.
#[derive(Default, Debug)]
pub struct InputState {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_buttons: u32,
}

pub fn set_runtime_handle(handle: &mut RuntimeHandle) {
    let mut s = global().lock().unwrap();
    s.handle = handle as *mut _;
}

pub fn set_guest_memory(memory: &Memory) {
    let mut s = global().lock().unwrap();
    s.memory = memory as *const _ as *mut _;
}

pub fn clear_on_unload() {
    let mut s = global().lock().unwrap();
    s.handle = std::ptr::null_mut();
    s.memory = std::ptr::null_mut();

    s.video = VideoState::default();
    s.audio = AudioState::default();
    s.input = InputState::default();
}
