# wasm96 D SDK

This directory contains a small, handwritten D SDK for writing **guest** WebAssembly (wasm32) programs that run inside the `wasm96` libretro core.

It intentionally avoids WIT / component-model codegen and instead targets the current, C-like import ABI used by `wasm96-core` (upload-based: guest writes/upload-only; host owns system-memory A/V buffers).

## What you get

- A single D package module: `wasm96` (`source/wasm96/package.d`)
- Raw imported functions (host -> guest imports) declared as `extern(C)` with `pragma(mangle, "...")`
- A few typed enums/constants (pixel formats, joypad buttons, mouse/lightgun bitmasks)
- Thin convenience wrappers for video/audio/input

## ABI overview

The host provides imports under the module name `"env"` with the following symbol names:

- ABI:
  - `wasm96_abi_version() -> u32`

- Video (upload-based, full-frame-only):
  - `wasm96_video_config(width: u32, height: u32, pixel_format: u32) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_video_upload(ptr: u32, byte_len: u32, pitch_bytes: u32) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_video_present()`

- Audio (push-based, interleaved i16):
  - `wasm96_audio_config(sample_rate: u32, channels: u32) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_audio_push_i16(ptr: u32, frames: u32) -> u32` (returns frames accepted, 0 on failure)
  - `wasm96_audio_drain(max_frames: u32) -> u32`

- Input:
  - `wasm96_joypad_button_pressed(port: u32, button: u32) -> u32`
  - `wasm96_key_pressed(key: u32) -> u32`
  - `wasm96_mouse_x() -> i32`
  - `wasm96_mouse_y() -> i32`
  - `wasm96_mouse_buttons() -> u32`
  - `wasm96_lightgun_x(port: u32) -> i32`
  - `wasm96_lightgun_y(port: u32) -> i32`
  - `wasm96_lightgun_buttons(port: u32) -> u32`

### Required guest export

Your guest module must export:

- `wasm96_frame()`

Optional lifecycle exports:

- `wasm96_init()`
- `wasm96_deinit()`
- `wasm96_reset()`

## Important notes

- This ABI is upload-based:
  - The guest owns its allocations in WASM linear memory.
  - The host owns video/audio buffers in system memory.
- All “pointers” passed to the host are **u32 offsets into the guest linear memory**.
- `wasm96_video_upload` is full-frame-only: `byte_len` and `pitch_bytes` must match the configured framebuffer.
- The `wasm96` D package includes helpers that cast offsets to pointers; those are inherently unsafe if you pass invalid pointers/lengths.

## Using it

Add this directory to your D build include/import path so the `wasm96` package can be imported:

- `import wasm96;`

Then implement `extern(C) void wasm96_frame()` in your guest code and call into the `wasm96.*` helpers from there.

(Exact build steps differ depending on your D compiler and wasm toolchain; this SDK is intentionally toolchain-agnostic.)