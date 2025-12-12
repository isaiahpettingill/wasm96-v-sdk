# wasm96 Zig SDK

This directory contains a handwritten Zig SDK for writing **guest** WebAssembly (wasm32) programs that run inside the `wasm96` libretro core.

The SDK exposes the same low-level ABI that the core provides under import module `"env"` (upload-based; write-only from guest) and wraps it with a small set of Zig-friendly types.

## Files

- `wasm96.zig` — the Zig module. It contains:
  - `pub const sys` — raw `extern "env"` imports
  - `pub const video`, `audio`, `input`, `abi` — convenience wrappers and typed enums
  - constants/enums that must match the core ABI (e.g. `ABI_VERSION`, `PixelFormat`, `JoypadButton`)

## ABI notes

- “Pointers” are **u32 offsets** into the guest’s linear memory (WASM32).
- Host APIs may fail by returning `0` (for example, buffer requests or allocation hooks).
- The current host implementation in `wasm96-core` may stub out some allocation-related functions; always handle failure.

## Guest exports you must provide

Your guest module must export at least:

- `export fn wasm96_frame() void` — called once per frame by the host.

Optional lifecycle exports:

- `export fn wasm96_init() void`
- `export fn wasm96_deinit() void`
- `export fn wasm96_reset() void`

## Using the module

In your Zig guest project:

- Copy or reference `wasm96.zig`
- Import it from your code (example):

```zig
const wasm96 = @import("wasm96.zig");

export fn wasm96_frame() void {
    // Example: poll input
    const pressed = wasm96.input.joypadPressed(0, .a);

    // Example: present (if you previously requested a framebuffer and wrote pixels)
    // wasm96.video.present();

    _ = pressed;
}
```

(How you compile Zig to wasm32 and how you package the resulting `.wasm` for the core depends on your build pipeline.)

## Versioning / compatibility

- `wasm96.zig` defines `ABI_VERSION` and expects the host to report the same version.
- You can check compatibility at runtime via `wasm96.abi.compatible()`.

Keep `ABI_VERSION` in sync with the core’s ABI version in `wasm96-core`.