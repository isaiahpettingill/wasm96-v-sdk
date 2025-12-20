//!
//! Audio/Video implementation for wasm96-core (Immediate Mode).
//!
//! This module implements the host-side drawing commands and audio handling.
//!
//! - Graphics: The host maintains a `Vec<u32>` framebuffer (XRGB8888).
//!   Guest commands modify this buffer.
//!   `video_present_host` sends it to libretro.
//!
//! - Audio:
//!   - Guests may push raw i16 samples (`audio_push_samples`) into `audio.host_queue`.
//!   - The host may also manage “channels/voices” (decoded assets and chiptune synth voices)
//!     stored in `state::AudioState` and mixed here.
//!   - `audio_drain_host` mixes everything into a single interleaved stereo i16 buffer and
//!     pads with silence as needed to satisfy the libretro backend.

// Needed for `alloc::` in this crate.
extern crate alloc;


// External crates for rendering

// External crates for asset decoding

// Storage ABI helpers

pub mod audio;
pub mod graphics;
pub mod resources;
pub mod storage;
pub mod tests;
pub mod utils;

// Re-export all public functions
pub use audio::*;
pub use graphics::*;
pub use resources::AvError;
pub use storage::*;
