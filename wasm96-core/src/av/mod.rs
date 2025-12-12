//! Audio/Video helpers for wasm96-core.
//!
//! This module implements the **guest uploads -> host stores in system memory** model.
//!
//! - Video: guest configures a framebuffer spec, then uploads an entire frame from guest
//!   linear memory via `(ptr, byte_len, pitch_bytes)`. The host copies into a host-owned
//!   system-memory buffer and presents from that buffer.
//!
//! - Audio: guest configures an audio spec, then pushes interleaved stereo i16 frames from
//!   guest linear memory via `(ptr, frames)`. The host copies into a host-owned queue and
//!   drains from that queue into libretro.
//!
//! Notes / limitations (current):
//! - Full-frame-only video uploads.
//! - Audio sample format is fixed to interleaved stereo i16.
//! - We still copy from guest memory into host/system memory (intentionally).

use crate::abi::PixelFormat;
use crate::state::{self, AudioSpec, VideoSpec};
use wasmer::FunctionEnvMut;

/// Errors from AV operations.
#[derive(Debug)]
pub enum AvError {
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
    let pitch_bytes = compute_pitch(width, pixel_format).ok_or(AvError::InvalidSpec)?;
    let _ = (height as usize)
        .checked_mul(pitch_bytes as usize)
        .ok_or(AvError::InvalidSpec)?;

    Ok(VideoSpec {
        width,
        height,
        pitch_bytes,
        pixel_format,
    })
}

/// Validate an audio request and create an `AudioSpec`.
pub fn validate_audio_request(sample_rate: u32, channels: u32) -> Result<AudioSpec, AvError> {
    if sample_rate == 0 {
        return Err(AvError::InvalidSpec);
    }
    // For now we only support stereo on the host side.
    if channels != 2 {
        return Err(AvError::InvalidSpec);
    }

    Ok(AudioSpec {
        sample_rate,
        channels,
    })
}

/// Configure host-side video spec and resize host framebuffer storage.
pub fn video_config(width: u32, height: u32, pixel_format: u32) -> bool {
    let spec = match validate_video_request(width, height, pixel_format) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mut s = state::global().lock().unwrap();
    let need = spec.byte_len();

    s.video.spec = Some(spec);
    if s.video.host_fb.len() != need {
        s.video.host_fb = vec![0u8; need];
    }

    true
}

/// Configure host-side audio format.
pub fn audio_config(sample_rate: u32, channels: u32) -> Result<bool, AvError> {
    let spec = validate_audio_request(sample_rate, channels)?;
    let mut s = state::global().lock().unwrap();
    s.audio.spec = Some(spec);
    Ok(true)
}

/// Upload a full video frame from guest memory into host/system-memory buffer.
///
/// `ptr` and `byte_len` refer to a region in guest linear memory.
/// `pitch_bytes` must match the configured pitch (full-frame-only).
pub fn video_upload(
    env: &FunctionEnvMut<()>,
    ptr: u32,
    byte_len: u32,
    pitch_bytes: u32,
) -> Result<bool, AvError> {
    let (memory_ptr, spec, dst_ptr, dst_len) = {
        let mut s = state::global().lock().unwrap();
        let memory_ptr = s.memory;
        let spec = s.video.spec.ok_or(AvError::VideoNotConfigured)?;

        let need_len = spec.byte_len();
        if pitch_bytes != spec.pitch_bytes {
            return Err(AvError::InvalidSpec);
        }
        if byte_len as usize != need_len {
            return Err(AvError::InvalidSpec);
        }

        if s.video.host_fb.len() != need_len {
            s.video.host_fb = vec![0u8; need_len];
        }

        let dst_ptr = s.video.host_fb.as_mut_ptr();
        let dst_len = s.video.host_fb.len();
        (memory_ptr, spec, dst_ptr, dst_len)
    };

    let _ = spec; // spec is validated above; keep it for clarity/future use.

    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }
    if ptr == 0 {
        return Err(AvError::InvalidSpec);
    }

    // SAFETY: memory pointer checked; destination pointer/len come from a valid Vec.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    let mut tmp = vec![0u8; dst_len];
    view.read(ptr as u64, &mut tmp)
        .map_err(|_| AvError::MemoryReadFailed)?;

    // SAFETY: dst_ptr is valid for dst_len bytes, and tmp is exactly dst_len bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(tmp.as_ptr(), dst_ptr, dst_len);
    }

    Ok(true)
}

/// Present the last uploaded host/system-memory framebuffer to libretro (best-effort).
pub fn video_present_host() {
    let (handle_ptr, data) = {
        let s = state::global().lock().unwrap();
        (s.handle, s.video.host_fb.clone())
    };

    if handle_ptr.is_null() {
        return;
    }

    // SAFETY: handle pointer checked.
    let h = unsafe { &mut *handle_ptr };
    h.upload_video_frame(&data);
}

/// Push interleaved stereo i16 audio frames from guest memory into the host queue.
///
/// `ptr` points to `frames * channels` i16 samples (little-endian in guest memory).
pub fn audio_push_i16(env: &FunctionEnvMut<()>, ptr: u32, frames: u32) -> Result<u32, AvError> {
    let (handle_ptr, memory_ptr, spec) = {
        let s = state::global().lock().unwrap();
        (s.handle, s.memory, s.audio.spec)
    };

    if handle_ptr.is_null() {
        return Err(AvError::MissingHandle);
    }
    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }
    let spec = spec.ok_or(AvError::AudioNotConfigured)?;
    if spec.channels != 2 {
        return Err(AvError::InvalidSpec);
    }
    if ptr == 0 {
        return Err(AvError::InvalidSpec);
    }

    let samples_per_frame = spec.samples_per_frame();
    if samples_per_frame != 2 {
        return Err(AvError::InvalidSpec);
    }

    let samples_total = (frames as usize)
        .checked_mul(samples_per_frame)
        .ok_or(AvError::InvalidSpec)?;
    let byte_len = samples_total.checked_mul(2).ok_or(AvError::InvalidSpec)?;

    // SAFETY: pointers checked.
    let mem = unsafe { &*memory_ptr };
    let view = mem.view(env);

    let mut tmp = vec![0u8; byte_len];
    view.read(ptr as u64, &mut tmp)
        .map_err(|_| AvError::MemoryReadFailed)?;

    // Convert LE bytes -> i16 samples.
    let samples = tmp.len() / 2;
    let mut out = vec![0i16; samples];
    for i in 0..samples {
        let lo = tmp[i * 2] as u16;
        let hi = tmp[i * 2 + 1] as u16;
        out[i] = ((hi << 8) | lo) as i16;
    }

    // Append to host queue (system memory).
    {
        let mut s = state::global().lock().unwrap();
        // Guest could push without calling config; treat as not configured.
        if s.audio.spec.is_none() {
            return Err(AvError::AudioNotConfigured);
        }
        s.audio.host_queue.extend_from_slice(&out);
    }

    Ok(frames)
}

/// Drain up to `max_frames` from the host queue into libretro audio output.
/// Returns frames drained.
/// If `max_frames == 0`, drains everything currently queued.
pub fn audio_drain_host(max_frames: u32) -> u32 {
    let (handle_ptr, samples_per_frame) = {
        let s = state::global().lock().unwrap();
        let Some(spec) = s.audio.spec else {
            return 0;
        };
        (s.handle, spec.samples_per_frame())
    };

    if handle_ptr.is_null() {
        return 0;
    }
    if samples_per_frame == 0 {
        return 0;
    }

    // Drain from the host queue while holding the lock, but DO NOT call libretro while locked.
    let (frames_to_drain, drained) = {
        let mut s = state::global().lock().unwrap();

        let available_frames = (s.audio.host_queue.len() / samples_per_frame) as u32;
        if available_frames == 0 {
            return 0;
        }

        let n = if max_frames == 0 {
            available_frames
        } else {
            available_frames.min(max_frames)
        };

        let drain_samples = (n as usize).saturating_mul(samples_per_frame);
        let drained: Vec<i16> = s.audio.host_queue.drain(0..drain_samples).collect();
        (n, drained)
    };

    // SAFETY: handle pointer checked.
    let h = unsafe { &mut *handle_ptr };
    h.upload_audio_frame(drained.as_slice());

    frames_to_drain
}
