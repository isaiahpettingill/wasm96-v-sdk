# wasm96 Go SDK

This directory contains a handwritten Go SDK for writing **guest** WebAssembly modules that run inside the `wasm96` libretro core.

It targets the current, C-like ABI the host exposes under the WebAssembly import module **`"env"`** (upload-based; write-only from guest). There is no WIT/component-model code generation.

## ABI model (upload-based)

- Guest owns allocations in WASM linear memory.
- Host owns video/audio buffers in system memory.
- Guest performs write-only uploads/pushes (by passing `u32` linear-memory offsets):
  - Video: `wasm96_video_config` → `wasm96_video_upload` (full frame) → `wasm96_video_present`
  - Audio: `wasm96_audio_config` → `wasm96_audio_push_i16` → `wasm96_audio_drain` (optional)

## Package layout

- `wasm96/wasm96.go`: the Go package (`package wasm96`) with:
  - typed enums/constants (`PixelFormat`, `JoypadButton`, button bitmasks)
  - ABI helpers (`HostABIVersion`, `Compatible`)
  - wrappers for video, audio, and input
  - raw imported symbol declarations (host functions)

## Important notes

- **Pointers are u32 offsets** into the guest linear memory (WASM32). The SDK uses `unsafe` to turn offsets into slices.
- The host **does not** allocate into guest memory. There is no request/returned-buffer model in this ABI.
- Always handle failure returns (`0` / `false`) from:
  - `VideoConfig(...)`
  - `VideoUpload(...)`
  - `AudioConfig(...)`
  - `AudioPushI16(...)`
- Go WebAssembly import wiring is **toolchain-specific**:
  - If you’re using **TinyGo**, you can annotate imports with `//go:wasmimport env <name>`.
  - If you’re using standard Go, you may need to adapt the import declarations to your build/runtime environment so the `env.wasm96_*` symbols resolve.

## Minimal usage (sketch)

Implement at least the required guest export:

- `func wasm96_frame()`

Then, from `wasm96_frame`, you typically:
1. Prepare/own a guest-side framebuffer in linear memory (static/global or your own allocator)
2. Call `wasm96.VideoConfig(width, height, format)`
3. Fill your guest framebuffer
4. Call `wasm96.VideoUpload(ptr, width, height, format)` and then `wasm96.Present()`
5. Optionally push audio samples (interleaved i16) via `wasm96.AudioConfig(...)`, `wasm96.AudioPushI16(ptr, frames)`, and `wasm96.AudioDrain(0)`
6. Read inputs via `JoypadPressed`, `MouseButtons`, etc.

## ABI stability

This SDK assumes `ABI_VERSION == 1` and must match the host core’s ABI version. Use `Compatible()` (and/or `HostABIVersion()`) to detect mismatch.