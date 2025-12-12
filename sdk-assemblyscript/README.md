# wasm96 AssemblyScript SDK

This SDK targets the wasm96 **upload-based** guest ABI:

- Guest owns allocations in WASM linear memory.
- Host owns video/audio buffers in **system memory**.
- Guest performs **write-only** uploads/pushes from guest memory to the host.


This package provides a **handwritten** AssemblyScript SDK for building **guest WebAssembly modules** that run under the `wasm96` libretro core.

It targets the current wasm96 **C-like ABI** (imports from module `"env"` with symbol names like `wasm96_video_config`, `wasm96_video_upload`, `wasm96_audio_config`, `wasm96_audio_push_i16`, etc.). There is **no WIT/component-model codegen** involved.

## What you get

- Typed enums and constants:
  - `PixelFormat`, `JoypadButton`
  - `MouseButtons.*`, `LightgunButtons.*`
- Low-level imported functions (declared with `@external("env", "...")`)
- Convenience wrappers:
  - `requestFramebuffer(...)` → `Framebuffer | null`
  - `requestRingBuffer(...)` → `RingBuffer | null`
  - `present()`, `audioDrain(...)`, input query helpers, ABI helpers

## Host/guest contract

### Host imports (guest calls)
The host provides imports under module name `"env"` with these symbol names:

- ABI: `wasm96_abi_version`
- Video (full-frame upload):
  - `wasm96_video_config(width, height, pixel_format) -> u32` (1 on success, 0 on failure)
  - `wasm96_video_upload(ptr, byte_len, pitch_bytes) -> u32` (1 on success, 0 on failure)
  - `wasm96_video_present()`

- Audio (push interleaved i16 frames):
  - `wasm96_audio_config(sample_rate, channels) -> u32` (1 on success, 0 on failure)
  - `wasm96_audio_push_i16(ptr, frames) -> u32` (frames accepted, 0 on failure)
  - `wasm96_audio_drain(max_frames) -> u32` (frames drained; if `max_frames==0`, drains everything queued)
- Input: `wasm96_joypad_button_pressed`, `wasm96_key_pressed`, `wasm96_mouse_x`, `wasm96_mouse_y`,
  `wasm96_mouse_buttons`, `wasm96_lightgun_x`, `wasm96_lightgun_y`, `wasm96_lightgun_buttons`

### Guest exports (host calls)
Your guest module must export:

- `wasm96_frame()` (required)

And may optionally export:

- `wasm96_init()`
- `wasm96_deinit()`
- `wasm96_reset()`

## Pointer model (important)

Most APIs return **u32 pointers**, which are **offsets into the guest linear memory**.
The SDK exposes these pointers as `u32` and returns raw pointer values (e.g. `Framebuffer.bytesPtr()`).

You are responsible for:
- checking for failure (`ptr == 0`)
- respecting row pitch (`pitchBytes`)
- treating returned pointers as unsafe/raw memory access

## Current limitations

Depending on the current state of `wasm96-core`, some allocation-related APIs may be stubbed out by the host (e.g. buffer requests returning `0`). Always handle `null` / `ptr == 0` gracefully.

## Source layout

- `assembly/index.ts` — SDK implementation

## License

MIT (see repository root `LICENSE`).