# wasm96 C++ SDK

This directory contains a small, **handwritten**, **header-only** C++ SDK for writing **guest** WebAssembly modules that run inside the `wasm96` libretro core.

The SDK is intentionally minimal and avoids WIT/component-model code generation. It targets the current C-like ABI that the host exposes as imports under the WebAssembly import module `"env"`.

## ABI model (upload-based, write-only from guest)

- **Guest** manages its own allocations in WASM linear memory.
- **Host** owns video/audio buffers in **system memory**.
- The guest performs **write-only** uploads/pushes:
  - Video: `config` → `upload` (full frame) → `present`
  - Audio: `config` → `push_i16` (sample batch) → `drain` (optional)

## Files

- `include/wasm96.hpp` — the C++ header-only SDK

## What the header provides

- Raw ABI imports (declared as `extern "C"`):
  - `wasm96_abi_version`

  - Video (full-frame upload):
    - `wasm96_video_config`
    - `wasm96_video_upload`
    - `wasm96_video_present`

  - Audio (push interleaved i16):
    - `wasm96_audio_config`
    - `wasm96_audio_push_i16`
    - `wasm96_audio_drain`

  - Input:
    - `wasm96_joypad_button_pressed` / `wasm96_key_pressed`
    - `wasm96_mouse_x` / `wasm96_mouse_y` / `wasm96_mouse_buttons`
    - `wasm96_lightgun_x` / `wasm96_lightgun_y` / `wasm96_lightgun_buttons`

- Typed wrappers and helpers:
  - `wasm96::PixelFormat`, `wasm96::JoypadButton`, button bitmask constants
  - Convenience functions like:
    - `wasm96::video_config`, `wasm96::video_upload`, `wasm96::present`
    - `wasm96::audio_config`, `wasm96::audio_push_i16`, `wasm96::audio_drain`
    - input queries
  - `wasm96::abi_compatible()` to check ABI version

## Guest exports you must implement

Your WASM guest module must export at least:

- `void wasm96_frame();` *(required)*

Optionally:

- `void wasm96_init();`
- `void wasm96_deinit();`
- `void wasm96_reset();`

How you export these depends on your toolchain, but the function names must match exactly.

## Important notes / limitations

- ABI “pointers” are `u32` **offsets into the guest linear memory** (WASM32). Treat casts from offsets to pointers as unsafe unless you know the region is valid.
- The host **does not** allocate into guest memory (no request/returned-buffer model). Your guest must own its own pixel/sample storage and pass pointers to it.

## Compiler/toolchain notes

The header uses Clang-style WASM attributes when compiling with Clang:

- `__attribute__((import_module("env"), import_name("...")))`

If you are using a non-Clang toolchain, you may need to adapt the import declarations to your compiler or WASM pipeline.

## Minimal usage sketch

```cpp
#include "wasm96.hpp"

// Guest-owned framebuffer storage (example: 320x240 XRGB8888)
static std::uint8_t fb[320 * 240 * 4];

extern "C" void wasm96_frame() {
  if (!wasm96::abi_compatible()) return;

  const std::uint32_t width = 320;
  const std::uint32_t height = 240;
  const auto fmt = wasm96::PixelFormat::Xrgb8888;
  const std::uint32_t pitch = wasm96::pitch_bytes(width, fmt);
  const std::uint32_t byte_len = height * pitch;

  // Fill fb[...] here...

  if (wasm96::video_config(width, height, fmt) &&
      wasm96::video_upload(reinterpret_cast<std::uint32_t>(fb), byte_len, pitch)) {
    wasm96::present();
  }

  // Audio example (optional):
  // wasm96::audio_config(48000, 2);
  // wasm96::audio_push_i16(ptr_to_i16_samples, frames);
  // wasm96::audio_drain(0);
}
```
