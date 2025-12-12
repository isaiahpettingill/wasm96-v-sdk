//! Audio/Video helpers for wasm96-core.
//!
//! This module implements the "host presents/consumes guest-provided buffers" model.
//!
//! - Video: guest requests a framebuffer (size/spec), writes pixels into it,
//!   then calls `wasm96_video_present()`. The host copies bytes from guest memory
//!   and calls `RuntimeHandle::upload_video_frame`.
//!
//! - Audio: guest requests an audio ringbuffer, writes interleaved stereo i16
//!   samples into it, then calls `wasm96_audio_commit(write_index)` (producer index
//!   in frames). The host can drain available frames (optionally up to a max) and
//!   calls `RuntimeHandle::upload_audio_frame` with the drained samples.
//!
//! Notes / limitations (current):
//! - We always copy out of guest memory (no zero-copy).
//! - Audio sample format is currently fixed to interleaved stereo i16.
//! - Pixel format is tracked but not converted; `upload_video_frame` receives raw bytes.

use crate::abi::PixelFormat;
use crate::state::{self, AudioSpec, GuestPtr, VideoSpec};
use wasmer::FunctionEnvMut;

/// Errors from AV operations.
#[derive(Debug)]
pub enum AvError {
    NotReady,
    MissingHandle,
    MissingMemory,
    VideoNotConfigured,
    AudioNotConfigured,
    InvalidSpec,
    MemoryReadFailed,
}

impl core::fmt::Display for AvError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AvError::NotReady => write!(f, "AV not ready"),
            AvError::MissingHandle => write!(f, "missing libretro runtime handle"),
            AvError::MissingMemory => write!(f, "missing WASM guest memory"),
            AvError::VideoNotConfigured => write!(f, "video not configured"),
            AvError::AudioNotConfigured => write!(f, "audio not configured"),
            AvError::InvalidSpec => write!(f, "invalid A/V spec"),
            AvError::MemoryReadFailed => write!(f, "failed to read from guest memory"),
        }
    }
}

impl std::error::Error for AvError {}

/// Compute pitch for a requested framebuffer.
///
/// Current policy:
/// - pitch = width * bytes_per_pixel
/// - no extra alignment is enforced (yet)
pub fn compute_pitch(width: u32, pixel_format: u32) -> Option<u32> {
    let bpp = match pixel_format {
        x if x == PixelFormat::Xrgb8888 as u32 => PixelFormat::Xrgb8888.bytes_per_pixel(),
        x if x == PixelFormat::Rgb565 as u32 => PixelFormat::Rgb565.bytes_per_pixel(),
        _ => return None,
    };
    width.checked_mul(bpp)
}

/// Validate a requested framebuffer spec and compute the pitch.
/// Returns a full `VideoSpec` if valid.
pub fn validate_video_request(
    width: u32,
    height: u32,
    pixel_format: u32,
) -> Result<VideoSpec, AvError> {
    if width == 0 || height == 0 {
        return Err(AvError::InvalidSpec);
    }
    let pitch = compute_pitch(width, pixel_format).ok_or(AvError::InvalidSpec)?;
    // Ensure size calculation doesn't overflow usize too badly.
    let _ = (height as usize)
        .checked_mul(pitch as usize)
        .ok_or(AvError::InvalidSpec)?;

    Ok(VideoSpec {
        width,
        height,
        pitch,
        pixel_format,
    })
}

/// Validate an audio request and create an `AudioSpec`.
pub fn validate_audio_request(
    sample_rate: u32,
    channels: u32,
    capacity_frames: u32,
) -> Result<(AudioSpec, u32), AvError> {
    if sample_rate == 0 {
        return Err(AvError::InvalidSpec);
    }
    // For now we only support stereo on the host side.
    if channels != 2 {
        return Err(AvError::InvalidSpec);
    }
    if capacity_frames == 0 {
        return Err(AvError::InvalidSpec);
    }

    // Ensure the ring buffer size computation is sane:
    // capacity_frames * channels * sizeof(i16)
    let bytes_per_frame = (channels as usize)
        .checked_mul(2)
        .ok_or(AvError::InvalidSpec)?;
    let _bytes_total = (capacity_frames as usize)
        .checked_mul(bytes_per_frame)
        .ok_or(AvError::InvalidSpec)?;

    Ok((
        AudioSpec {
            sample_rate,
            channels,
        },
        capacity_frames,
    ))
}

/// Configure video in global state (spec + pointer).
///
/// This is typically called from the host import `wasm96_video_request`.
pub fn configure_video(spec: VideoSpec, fb_ptr: GuestPtr) {
    let mut s = state::global().lock().unwrap();
    s.video.spec = Some(spec);
    s.video.fb_ptr = Some(fb_ptr);
}

/// Configure audio in global state (spec + ring ptr + capacity).
///
/// This is typically called from the host import `wasm96_audio_request`.
pub fn configure_audio(spec: AudioSpec, ring_ptr: GuestPtr, capacity_frames: u32) {
    let mut s = state::global().lock().unwrap();
    s.audio.spec = Some(spec);
    s.audio.ring_ptr = Some(ring_ptr);
    s.audio.capacity_frames = capacity_frames;
    s.audio.write_index_frames = 0;
    s.audio.read_index_frames = 0;
}

/// Return video pitch from global state (0 if not configured).
pub fn video_pitch() -> u32 {
    let s = state::global().lock().unwrap();
    s.video.spec.map(|v| v.pitch).unwrap_or(0)
}

/// Return audio capacity frames from global state (0 if not configured).
pub fn audio_capacity_frames() -> u32 {
    let s = state::global().lock().unwrap();
    if s.audio.is_configured() {
        s.audio.capacity_frames
    } else {
        0
    }
}

/// Return audio write index (producer index) from global state.
pub fn audio_write_index_frames() -> u32 {
    let s = state::global().lock().unwrap();
    s.audio.write_index_frames
}

/// Return audio read index (consumer index) from global state.
pub fn audio_read_index_frames() -> u32 {
    let s = state::global().lock().unwrap();
    s.audio.read_index_frames
}

/// Called when the guest commits its updated producer index.
///
/// This updates the global state's `write_index_frames` (mod capacity).
pub fn audio_commit(write_index_frames: u32) {
    let mut s = state::global().lock().unwrap();
    s.audio.set_write_index(write_index_frames);
}

/// Present the currently configured framebuffer to libretro (copy out of guest memory).
///
/// This reads `height * pitch` bytes from guest memory at `fb_ptr` and uploads.
pub fn video_present(env: &FunctionEnvMut<()>) -> Result<(), AvError> {
    // Keep lock scope minimal: grab pointers/spec, then drop lock before reading memory.
    let (handle_ptr, memory_ptr, spec, fb_ptr) = {
        let s = state::global().lock().unwrap();
        (s.handle, s.memory, s.video.spec, s.video.fb_ptr)
    };

    if handle_ptr.is_null() {
        return Err(AvError::MissingHandle);
    }
    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    let spec = spec.ok_or(AvError::VideoNotConfigured)?;
    let fb_ptr = fb_ptr.ok_or(AvError::VideoNotConfigured)?;

    let byte_len = spec.byte_len();
    let mut data = vec![0u8; byte_len];

    // SAFETY: pointers were checked for null; reading is bounded by `byte_len`.
    let h = unsafe { &mut *handle_ptr };
    let mem = unsafe { &*memory_ptr };

    let view = mem.view(env);
    view.read(fb_ptr as u64, &mut data)
        .map_err(|_| AvError::MemoryReadFailed)?;

    h.upload_video_frame(&data);
    Ok(())
}

/// Drain up to `max_frames` audio frames from the ringbuffer and upload to libretro.
///
/// Returns number of frames drained.
///
/// If `max_frames == 0`, drains all currently available frames.
///
/// Audio is interleaved stereo i16:
/// - frames * 2 samples
pub fn audio_drain(env: &FunctionEnvMut<()>, max_frames: u32) -> Result<u32, AvError> {
    // Grab state snapshot first.
    let (handle_ptr, memory_ptr, ring_ptr, spec, capacity, write_idx, read_idx) = {
        let s = state::global().lock().unwrap();
        (
            s.handle,
            s.memory,
            s.audio.ring_ptr,
            s.audio.spec,
            s.audio.capacity_frames,
            s.audio.write_index_frames,
            s.audio.read_index_frames,
        )
    };

    if handle_ptr.is_null() {
        return Err(AvError::MissingHandle);
    }
    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    let ring_ptr = ring_ptr.ok_or(AvError::AudioNotConfigured)?;
    let spec = spec.ok_or(AvError::AudioNotConfigured)?;

    if capacity == 0 {
        return Err(AvError::AudioNotConfigured);
    }

    // Compute available frames.
    let available = {
        let cap = capacity;
        let w = write_idx % cap;
        let r = read_idx % cap;
        if w >= r { w - r } else { cap - (r - w) }
    };
    if available == 0 {
        return Ok(0);
    }

    let to_drain = if max_frames == 0 {
        available
    } else {
        available.min(max_frames)
    };

    // For simplicity, drain in up to two chunks (wrap-around).
    let cap = capacity;
    let r = read_idx % cap;

    let first_chunk = to_drain.min(cap - r);
    let second_chunk = to_drain - first_chunk;

    // bytes_per_frame (stereo i16) == 4
    let bpf = spec.bytes_per_frame();
    if bpf != 4 {
        // Should never happen given validation, but keep it defensive.
        return Err(AvError::InvalidSpec);
    }

    // SAFETY: pointers were checked for null.
    let h = unsafe { &mut *handle_ptr };
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    // Prepare a temporary Vec<i16> to avoid alignment/aliasing pitfalls.
    // total samples = frames * channels
    let total_samples = (to_drain as usize).saturating_mul(spec.channels as usize);
    let mut out = vec![0i16; total_samples];

    // Helper closure to read a chunk of frames into `out` as i16 LE.
    let mut read_frames_into =
        |dst_sample_offset: usize, frame_offset: u32, frames: u32| -> Result<(), AvError> {
            if frames == 0 {
                return Ok(());
            }

            let byte_offset = (frame_offset as u64).saturating_mul(bpf as u64);
            let byte_len = (frames as usize).saturating_mul(bpf);

            let mut tmp = vec![0u8; byte_len];
            view.read((ring_ptr as u64).saturating_add(byte_offset), &mut tmp)
                .map_err(|_| AvError::MemoryReadFailed)?;

            // Convert LE bytes -> i16 samples.
            // tmp len is multiple of 2.
            let samples = tmp.len() / 2;
            for i in 0..samples {
                let lo = tmp[i * 2] as u16;
                let hi = tmp[i * 2 + 1] as u16;
                let v = (hi << 8) | lo;
                out[dst_sample_offset + i] = v as i16;
            }

            Ok(())
        };

    // Read first chunk (from r to end).
    read_frames_into(0, r, first_chunk)?;

    // Read second chunk (from 0).
    if second_chunk != 0 {
        let dst_sample_offset = (first_chunk as usize).saturating_mul(spec.channels as usize);
        read_frames_into(dst_sample_offset, 0, second_chunk)?;
    }

    // Upload drained samples to libretro.
    h.upload_audio_frame(out.as_slice());

    // Advance read index in global state.
    {
        let mut s = state::global().lock().unwrap();
        s.audio.consume_frames(to_drain);
    }

    Ok(to_drain)
}
