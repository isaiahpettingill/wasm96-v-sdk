// Needed for `alloc::` in this crate.
extern crate alloc;

use crate::state::global;
use wasmtime::Caller;

// External crates for rendering

// External crates for asset decoding

// Storage ABI helpers
use alloc::vec::Vec;

use super::resources::AvError;
use super::utils::sat_add_i16;

/// Helpers for mixing.
/// NOTE: Higher-level playback and chiptune APIs are stubbed for now; these helpers
/// are kept because `audio_drain_host` mixes guest-pushed audio and pads as needed.
#[inline]

pub fn audio_init(sample_rate: u32) -> u32 {
    let mut s = global().lock().unwrap();
    s.audio.sample_rate = sample_rate;
    // Return buffer size hint (e.g. 1 frame worth? or just 0).
    // The guest doesn't strictly need this if it pushes what it wants.
    1024
}

// --- Higher-level audio playback (stubs) ---
//
// These are intentionally stubbed so guests can link against the API.
// Full implementations will:
// - decode the encoded data (wav/ogg) into PCM,
// - mix it into the output stream (multi-channel / fire-and-forget).
//
// Fire-and-forget: no ids/handles are returned.

pub fn audio_play_wav(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read WAV bytes from guest memory.
    let mut wav_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut wav_bytes).is_err() {
        return;
    }

    // Decode WAV using hound.
    let cursor = std::io::Cursor::new(wav_bytes);
    let reader = match hound::WavReader::new(cursor) {
        Ok(r) => r,
        Err(_) => return,
    };

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    // Collect samples as i16, converting if necessary.
    let mut samples: Vec<i16> = Vec::new();
    for sample in reader.into_samples::<i16>() {
        match sample {
            Ok(s) => samples.push(s),
            Err(_) => return,
        }
    }

    // Convert to interleaved stereo if mono.
    let pcm_stereo: Vec<i16> = if spec.channels == 1 {
        // Mono: duplicate to stereo.
        samples.into_iter().flat_map(|s| [s, s]).collect()
    } else if spec.channels == 2 {
        // Stereo: already interleaved.
        samples
    } else {
        // Unsupported channel count.
        return;
    };

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_play_qoa(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read QOA bytes from guest memory.
    let mut qoa_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut qoa_bytes).is_err() {
        return;
    }

    // Decode QOA using qoaudio crate.
    let decoder = match qoaudio::QoaDecoder::new(&qoa_bytes) {
        Ok(d) => d,
        Err(_) => return,
    };

    let channels = decoder.channels() as usize;
    let sample_rate = decoder.sample_rate() as u32;
    let samples: Vec<i16> = if let Some(s) = decoder.decoded_samples() {
        s.into_iter().collect()
    } else {
        return;
    };

    let pcm_stereo: Vec<i16> = if channels == 1 {
        // Mono: duplicate to stereo.
        samples.into_iter().flat_map(|s| [s, s]).collect()
    } else if channels == 2 {
        // Stereo: already interleaved.
        samples
    } else {
        // Unsupported channel count.
        return;
    };

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_play_xm(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    let memory_ptr = {
        let s = crate::state::global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read XM bytes from guest memory.
    let mut xm_bytes = vec![0u8; len as usize];
    if mem.read(env, ptr as usize, &mut xm_bytes).is_err() {
        return;
    }

    // Load XM module using xmrs.
    let xm = match xmrs::import::xm::xmmodule::XmModule::load(&xm_bytes) {
        Ok(xm) => xm,
        Err(_) => return,
    };

    let module = xm.to_module();
    let module = Box::new(module);
    let module_ref: &'static xmrs::prelude::Module = Box::leak(module);

    // Get sample rate.
    let sample_rate = {
        let s = crate::state::global().lock().unwrap();
        s.audio.sample_rate
    };

    // Create player.
    let mut player =
        xmrsplayer::prelude::XmrsPlayer::new(module_ref, sample_rate as f32, 1024, false);
    player.set_max_loop_count(1); // Decode the song once

    // Decode the entire song into PCM.
    let mut pcm_stereo: Vec<i16> = Vec::new();

    loop {
        match player.sample(true) {
            Some((left, right)) => {
                let l_i16 = (left * 32767.0) as i16;
                let r_i16 = (right * 32767.0) as i16;
                pcm_stereo.push(l_i16);
                pcm_stereo.push(r_i16);
            }
            None => break,
        }
    }

    // Create a new audio channel and add to global state.
    let channel = crate::state::AudioChannel {
        active: true,
        volume_q8_8: 256, // 1.0
        pan_i16: 0,       // Center
        loop_enabled: true,
        pcm_stereo,
        position_frames: 0,
        sample_rate,
    };

    let mut s = crate::state::global().lock().unwrap();
    s.audio.channels.push(channel);
}

pub fn audio_push_samples(env: &mut Caller<'_, ()>, ptr: u32, count: u32) -> Result<(), AvError> {
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };

    if memory_ptr.is_null() {
        return Err(AvError::MissingMemory);
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    // Read i16 samples. count is number of i16 elements.
    let byte_len = count.checked_mul(2).ok_or(AvError::MemoryReadFailed)?;
    let mut tmp_bytes = vec![0u8; byte_len as usize];

    mem.read(env, ptr as usize, &mut tmp_bytes)
        .map_err(|_| AvError::MemoryReadFailed)?;

    // Convert bytes to i16
    let mut samples = Vec::with_capacity(count as usize);
    for chunk in tmp_bytes.chunks_exact(2) {
        let val = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(val);
    }

    // Append to host queue
    let mut s = global().lock().unwrap();
    s.audio.host_queue.extend(samples);

    Ok(())
}

pub fn audio_drain_host(max_frames: u32) -> u32 {
    let (handle_ptr, sample_rate) = {
        let s = global().lock().unwrap();
        (s.handle, s.audio.sample_rate)
    };

    if handle_ptr.is_null() {
        return 0;
    }

    // We must upload a minimum amount of audio each frame to satisfy libretro-backend.
    // The backend requires at least ~1 frame worth of stereo samples; for 44.1kHz @ 60fps:
    // 44100 / 60 = 735 frames (per channel) => 1470 i16 samples interleaved stereo.
    //
    // Even if the guest provides no audio (or too little), we pad with silence so the core
    // never panics due to insufficient audio uploads.
    let min_samples_per_run: usize = ((sample_rate as usize) / 60) * 2;

    // Stereo = 2 i16 samples per audio frame (L, R)
    let samples_per_frame: usize = 2;

    // How many frames are we going to output this run?
    // If max_frames == 0, use at least the backend minimum.
    let target_frames: usize = if max_frames == 0 {
        min_samples_per_run / samples_per_frame
    } else {
        (max_frames as usize).max(min_samples_per_run / samples_per_frame)
    };

    // Start with silence; we'll mix into this.
    let mut mixed: Vec<i16> = vec![0i16; target_frames * samples_per_frame];

    // Mix guest-pushed raw samples (host_queue).
    {
        let mut s = global().lock().unwrap();

        let available_samples = s.audio.host_queue.len();
        let available_frames = available_samples / samples_per_frame;

        let frames_to_take = available_frames.min(target_frames);
        let samples_to_take = frames_to_take * samples_per_frame;

        if samples_to_take != 0 {
            let drained: Vec<i16> = s.audio.host_queue.drain(0..samples_to_take).collect();
            for (dst, src) in mixed.iter_mut().zip(drained.iter()) {
                *dst = sat_add_i16(*dst, *src);
            }
        }

        // Mix audio channels (higher-level playback).
        for channel in &mut s.audio.channels {
            if !channel.active {
                continue;
            }

            let channel_frames = channel.pcm_stereo.len() / 2;
            if channel.position_frames >= channel_frames {
                if channel.loop_enabled {
                    channel.position_frames = 0;
                } else {
                    channel.active = false;
                    continue;
                }
            }

            let start_frame = channel.position_frames;
            let frames_to_mix = (channel_frames - start_frame).min(target_frames);

            let volume = channel.volume_q8_8 as f32 / 256.0;
            let pan_left = if channel.pan_i16 <= 0 {
                1.0
            } else {
                (32768 - channel.pan_i16) as f32 / 32768.0
            };
            let pan_right = if channel.pan_i16 >= 0 {
                1.0
            } else {
                (32768 + channel.pan_i16) as f32 / 32768.0
            };

            for i in 0..frames_to_mix {
                let src_idx = (start_frame + i) * 2;
                let l = (channel.pcm_stereo[src_idx] as f32 * volume * pan_left) as i16;
                let r = (channel.pcm_stereo[src_idx + 1] as f32 * volume * pan_right) as i16;

                let dst_idx = i * 2;
                mixed[dst_idx] = sat_add_i16(mixed[dst_idx], l);
                mixed[dst_idx + 1] = sat_add_i16(mixed[dst_idx + 1], r);
            }

            channel.position_frames += frames_to_mix;
        }
    }

    // SAFETY: handle pointer checked.
    let h = unsafe { &mut *handle_ptr };
    h.upload_audio_frame(&mixed);

    // Report how many *audio frames* we uploaded (stereo frames).
    (mixed.len() / samples_per_frame) as u32
}
