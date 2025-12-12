# wasm96 C SDK

This directory contains a small, handwritten C header for writing **guest** WebAssembly (wasm32) programs that run under the `wasm96` libretro core.

The SDK is intentionally **C-like** and **codegen-free** (no WIT/component model). It mirrors the current ABI implemented by the host core.

## Files

- `include/wasm96.h` — the C header you include from your guest project.

## ABI overview

ABI model (upload-based):

- Guest manages its own allocations in **WASM linear memory**
- Host owns video/audio buffers in **system memory**
- Guest performs **write-only** uploads/pushes by passing pointers (u32 offsets) into guest linear memory

The host provides a set of imported functions under the WASM import module `"env"` with these symbol names (see the header for the complete list):

- ABI / versioning:
  - `wasm96_abi_version() -> u32`

- Video (full-frame upload):
  - `wasm96_video_config(width, height, pixel_format) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_video_upload(ptr, byte_len, pitch_bytes) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_video_present()`

- Audio (push interleaved i16 frames):
  - `wasm96_audio_config(sample_rate, channels) -> u32` (returns 1 on success, 0 on failure)
  - `wasm96_audio_push_i16(ptr, frames) -> u32` (returns frames accepted, 0 on failure)
  - `wasm96_audio_drain(max_frames) -> u32` (returns frames drained; `0` means “drain everything available”)

- Input:
  - `wasm96_joypad_button_pressed(port, button) -> u32`
  - `wasm96_key_pressed(key) -> u32`
  - `wasm96_mouse_x() -> i32`, `wasm96_mouse_y() -> i32`, `wasm96_mouse_buttons() -> u32`
  - `wasm96_lightgun_x(port) -> i32`, `wasm96_lightgun_y(port) -> i32`, `wasm96_lightgun_buttons(port) -> u32`

### Guest exports

Your guest module must export:

- `void wasm96_frame(void);` (called once per frame)

Optional lifecycle exports:

- `void wasm96_init(void);`
- `void wasm96_deinit(void);`
- `void wasm96_reset(void);`

## Important notes

- **Pointers are `u32` offsets** into the guest linear memory (WASM32). The header provides `wasm96_ptr(offset)` helpers to cast offsets to pointers.
- The host **does not allocate into guest memory**.
- `wasm96_video_upload` is **full-frame-only**: `byte_len` must equal `height * pitch_bytes`, where `pitch_bytes` is typically `width * bytes_per_pixel`.
- Keep `WASM96_ABI_VERSION` in sync with the host (`wasm96-core`).

## Minimal usage sketch

Include the header and implement `wasm96_frame`.

This sketch assumes you own a guest-side framebuffer (static/global or heap) and upload it each frame:

```c
#include "wasm96.h"

#define WIDTH  320u
#define HEIGHT 240u

static uint8_t fb[WIDTH * HEIGHT * 4]; /* XRGB8888 */

void wasm96_frame(void) {
    if (!wasm96_abi_compatible()) {
        return;
    }

    /* Example: check input */
    if (wasm96_joypad_button_pressed(0, WASM96_JOYPAD_START)) {
        /* ... */
    }

    /* Configure host-side video once (ok to call every frame for simplicity) */
    if (!wasm96_video_config(WIDTH, HEIGHT, WASM96_PIXEL_FORMAT_XRGB8888)) {
        return;
    }

    const uint32_t pitch_bytes = WIDTH * 4u;
    const uint32_t byte_len = HEIGHT * pitch_bytes;

    /* Fill fb[...] here */

    /* Upload + present */
    if (wasm96_video_upload((uint32_t)(uintptr_t)fb, byte_len, pitch_bytes)) {
        wasm96_video_present();
    }
}
```

Your build system/toolchain must compile this as a wasm32 guest that imports symbols from `"env"` and exports `wasm96_frame`.