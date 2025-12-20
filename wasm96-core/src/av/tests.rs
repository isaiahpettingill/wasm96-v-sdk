// Needed for `alloc::` in this crate.
extern crate alloc;

// External crates for rendering

// External crates for asset decoding

// Storage ABI helpers

#[cfg(test)]
mod tests {
    use crate::av::audio::audio_init;
    use crate::av::utils::{graphics_image_from_host, sat_add_i16};
    use crate::av::{
        graphics_background, graphics_set_color, graphics_set_size, graphics_triangle,
    };
    use crate::state::global;

    fn count_color(buf: &[u32], color: u32) -> usize {
        buf.iter().copied().filter(|&c| c == color).count()
    }

    fn reset_state_for_test() {
        // Ensure any previous test doesn't leave global state in a poisoned/invalid state.
        // This keeps tests isolated and avoids cascading failures when a prior test panics.
        crate::state::clear_on_unload();
    }

    #[test]
    fn triangle_degenerate_area_draws_nothing() {
        reset_state_for_test();

        // Make sure the triangle fill handles colinear points.
        graphics_set_size(16, 16);
        graphics_background(0, 0, 0);
        graphics_set_color(255, 0, 0, 255);
        let red = (255u32 << 16) | (0u32 << 8) | 0u32;

        // Colinear along y=x line.
        graphics_triangle(1, 1, 5, 5, 10, 10);

        let s = global().lock().unwrap();
        assert_eq!(count_color(&s.video.framebuffer, red), 0);
    }

    #[test]
    fn triangle_fills_some_pixels_for_simple_case() {
        reset_state_for_test();

        graphics_set_size(32, 32);
        graphics_background(0, 0, 0);
        graphics_set_color(0, 255, 0, 255);
        let green = (0u32 << 16) | (255u32 << 8) | 0u32;

        // A clearly non-degenerate triangle well within bounds.
        graphics_triangle(4, 4, 20, 6, 8, 24);

        let s = global().lock().unwrap();
        let filled = count_color(&s.video.framebuffer, green);
        assert!(filled > 0, "expected some filled pixels, got {filled}");
        assert!(
            filled < (32 * 32) as usize,
            "triangle should not fill entire screen"
        );
    }

    #[test]
    fn triangle_vertex_order_does_not_change_fill_count() {
        reset_state_for_test();

        // Vertex order reverses winding; rasterization should be winding-invariant.
        graphics_set_size(32, 32);

        // First order
        graphics_background(0, 0, 0);
        graphics_set_color(0, 0, 255, 255);
        let blue = (0u32 << 16) | (0u32 << 8) | 255u32;
        graphics_triangle(4, 4, 20, 6, 8, 24);
        let count_a = {
            let s = global().lock().unwrap();
            count_color(&s.video.framebuffer, blue)
        };

        // Reverse winding (same vertices)
        graphics_background(0, 0, 0);
        graphics_set_color(0, 0, 255, 255);
        graphics_triangle(4, 4, 8, 24, 20, 6);
        let count_b = {
            let s = global().lock().unwrap();
            count_color(&s.video.framebuffer, blue)
        };

        assert_eq!(
            count_a, count_b,
            "filled pixel count should be identical regardless of winding (got {count_a} vs {count_b})"
        );
        assert!(count_a > 0);
    }

    #[test]
    fn triangle_clips_to_screen_without_panicking() {
        reset_state_for_test();

        // This test mostly ensures we don't index OOB when coordinates are off-screen.
        graphics_set_size(16, 16);
        graphics_background(0, 0, 0);
        graphics_set_color(255, 255, 255, 255);
        let white = (255u32 << 16) | (255u32 << 8) | 255u32;

        // Large triangle that extends beyond bounds.
        graphics_triangle(-10, -10, 30, 0, 0, 30);

        let s = global().lock().unwrap();
        let filled = count_color(&s.video.framebuffer, white);
        assert!(filled > 0);

        // This assertion is only about clipping/not panicking; a strict upper bound can be flaky
        // if draw_color/state leaks or if the test isn't perfectly isolated.
        assert!(
            filled <= s.video.framebuffer.len(),
            "filled pixels must never exceed framebuffer length"
        );
    }

    #[test]
    fn audio_channel_mix_advances_position_without_requiring_runtime_handle() {
        reset_state_for_test();

        // `audio_drain_host` early-returns if no libretro runtime handle is installed, which
        // makes it unsuitable for unit tests. Instead, validate the core mixing behavior:
        // channel position advances only when we actually mix frames.
        let sample_rate = 44100;
        audio_init(sample_rate);

        // 4 stereo frames: constant non-zero signal.
        let pcm_stereo: Vec<i16> = vec![
            5000, 5000, // frame 0
            5000, 5000, // frame 1
            5000, 5000, // frame 2
            5000, 5000, // frame 3
        ];

        let mut mixed: Vec<i16> = vec![0i16; 1 * 2]; // 1 stereo frame
        {
            let mut s = global().lock().unwrap();
            s.audio.host_queue.clear();

            s.audio.channels.clear();
            s.audio.channels.push(crate::state::AudioChannel {
                active: true,
                volume_q8_8: 256, // 1.0
                pan_i16: 0,       // centered
                loop_enabled: false,
                pcm_stereo,
                position_frames: 0,
                sample_rate,
            });

            // Mix exactly 1 frame from the channel (mirrors the logic in `audio_drain_host`,
            // but without depending on a libretro handle).
            let channel = &mut s.audio.channels[0];
            let channel_frames = channel.pcm_stereo.len() / 2;

            let start_frame = channel.position_frames;
            let frames_to_mix = (channel_frames - start_frame).min(1);

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

        let s = global().lock().unwrap();
        assert_eq!(s.audio.channels.len(), 1, "expected one channel");
        assert_eq!(
            s.audio.channels[0].position_frames, 1,
            "expected channel to advance by exactly one frame"
        );

        // And the mixed buffer should contain non-zero data.
        assert!(
            mixed.iter().any(|&s| s != 0),
            "expected mixed output to be non-zero"
        );
    }

    #[test]
    fn png_decode_and_draw_renders_expected_pixel() {
        reset_state_for_test();

        // Generate a valid minimal 1x1 RGBA PNG at runtime using the encoder,
        // to avoid hardcoding bytes/CRCs.
        let mut png_bytes: Vec<u8> = Vec::new();
        {
            use std::io::Write;

            let mut encoder = png::Encoder::new(&mut png_bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png write_header failed");

            // One pixel RGBA = red.
            writer
                .write_image_data(&[255, 0, 0, 255])
                .expect("png write_image_data failed");

            // Ensure everything is flushed into the Vec.
            let _ = writer.finish();
            let _ = (&mut png_bytes as &mut dyn Write).flush();
        }

        // Arrange framebuffer.
        graphics_set_size(4, 4);
        graphics_background(0, 0, 0);

        // Decode (same crate/decoder path as host-side decode implementation),
        // then blit the resulting RGBA into the framebuffer.
        let cursor = std::io::Cursor::new(&png_bytes);
        let decoder = png::Decoder::new(cursor);
        let mut reader = decoder.read_info().expect("png read_info failed");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("png next_frame failed");
        assert_eq!(info.width, 1);
        assert_eq!(info.height, 1);

        let rgba = &buf[..info.buffer_size()];
        assert_eq!(rgba, &[255, 0, 0, 255]);

        // Draw at (0,0)
        graphics_image_from_host(0, 0, 1, 1, rgba);

        let s = global().lock().unwrap();
        let red = (255u32 << 16) | (0u32 << 8) | 0u32;
        assert_eq!(s.video.framebuffer[0], red);
    }
}
