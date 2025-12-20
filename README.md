# Clone the repository
git clone https://github.com/isaiahpettingill/wasm96.git
cd wasm96

## Runtime
The core runs guest modules using **Wasmtime**.

Wasmtime configuration is set up to enable a broad set of WebAssembly feature flags (in both Cargo features and `wasmtime::Config`) for maximum guest compatibility.

# Build the libretro core
cargo build --release --package wasm96-core
```

The core library will be at `target/release/libwasm96_core.so` (or equivalent for your platform).

### Writing a Guest
1. Use the Rust SDK (`wasm96-sdk`) or Zig SDK (`wasm96-zig-sdk`) for ergonomic bindings.
2. Implement the required export:
   - `setup()`: Initialize your application (e.g., set screen size, register assets)
3. Implement optional exports (preferred runtime entrypoints):
   - `update()`: Update game logic (called once per frame)
   - `draw()`: Issue drawing commands (called once per frame)
4. (Optional) WASI-style exports are also supported:
   - If `draw()` is not exported, the core will treat `_start()` as the draw function.
   - If `draw()` and `_start()` are not exported, the core will treat `main()` as the draw function.
5. Compile to WASM32 target
6. Load the `.wasm` file into the wasm96 core via your libretro frontend

### Entrypoint precedence rules
- `setup()` is required.
- `draw()` takes precedence over `_start()` and `main()`.
- `_start()` takes precedence over `main()` (only used when `draw()` is missing).
- `update()` is optional; if missing, update is treated as a no-op.

### Running
Load the wasm96 core in your libretro frontend and select a `.wasm` file as the "game". The core will instantiate the WASM module and start calling the guest entrypoints according to the precedence rules above.

## ABI notes: keyed resources (hashed strings)
The core uses **keyed resources** for images and fonts. Instead of receiving numeric handles from `*_create(...)`, guests register assets under a stable key and later draw/use them by that key.

At the ABI level, keys are `u64` values. The SDKs (Rust/Zig) automatically hash string keys (using a stable hash) to `u64` when calling the host, so you can use human-readable strings in your code.

This avoids global mutable “resource id” state in guests and makes resource usage explicit.

### PNG (encoded bytes)
- Direct draw (one-shot):
  - `graphics::image_png(x, y, png_bytes)`
- Register once (typically in `setup()`):
  - `graphics::png_register("ui/logo", png_bytes)`
- Draw by key (in `draw()`):
  - `graphics::png_draw_key("ui/logo", x, y)`
  - `graphics::png_draw_key_scaled("ui/logo", x, y, w, h)`
- Unregister (optional):
  - `graphics::png_unregister("ui/logo")`

### SVG (encoded bytes)
- Register:
  - `graphics::svg_register("icons/player", svg_bytes)`
- Draw:
  - `graphics::svg_draw_key("icons/player", x, y, w, h)`
- Unregister (optional):
  - `graphics::svg_unregister("icons/player")`

### GIF (encoded bytes)
- Register:
  - `graphics::gif_register("fx/explosion", gif_bytes)`
- Draw:
  - `graphics::gif_draw_key("fx/explosion", x, y)`
  - `graphics::gif_draw_key_scaled("fx/explosion", x, y, w, h)`
- Unregister (optional):
  - `graphics::gif_unregister("fx/explosion")`

### Fonts + text (keyed)
- Register a font under a key:
  - Built-in Spleen:
    - `graphics::font_register_spleen("font/spleen/16", 16)`
  - TTF bytes:
    - `graphics::font_register_ttf("font/title", ttf_bytes)`
- Draw text using the font key:
  - `graphics::text_key(x, y, "font/spleen/16", "Hello")`
- Measure text:
  - `graphics::text_measure_key("font/spleen/16", "Hello")`

## SDK

### Rust SDK (`wasm96-sdk/`)
- Handwritten bindings matching the core ABI
- Safe wrappers around raw `extern "C"` imports
- Entry point: `wasm96_sdk::prelude::*`
- Supports `no_std` for minimal WASM builds
- Optional wee_alloc for custom allocator

### Zig SDK (`wasm96-zig-sdk/`)
- Handwritten bindings matching the core ABI
- Safe wrappers around raw extern functions
- Entry point: `@import("wasm96")`
- Compiles directly to WASM32 (freestanding); produces a module exporting `setup`, `update`, and `draw` (no WASI `_start`)

## Examples

The `example/` directory contains guest applications:

- `rust-guest/`: Basic hello-world example (Rust)
- `rust-guest-mp-platformer/`: Multiplayer platformer game (Rust)
- `rust-guest-showcase/`: Comprehensive demo of all features (Rust)
- `zig-guest/`: Basic hello-world example (Zig)

To build a Rust example:
```bash
cargo build --package <example-name> --target wasm32-unknown-unknown
```

To build the Zig example:
```bash
cd example/zig-guest && zig build
```

## Project Structure

```
wasm96/
├── wasm96-core/          # Libretro core implementation
├── wasm96-sdk/           # Handwritten Rust SDK
├── wasm96-zig-sdk/       # Handwritten Zig SDK
├── wit/                  # WIT interface definitions
├── example/              # Guest examples
├── Cargo.toml            # Workspace configuration
├── SDKs.md               # Outdated; describes planned multi-language SDKs for old ABI
├── AGENTS.md             # Development guidelines
└── README.md             # This file
```

## Development

### Guidelines
- Follow test-driven development (TDD)
- Ensure all code compiles and passes tests
- See `AGENTS.md` for agent-specific rules

### Contributing
- The ABI is handwritten; update bindings in `wasm96-core/src/abi/mod.rs`, `wasm96-sdk/src/lib.rs`, and `wasm96-zig-sdk/src/main.zig` in lockstep
- Update `wit/wasm96.wit` to reflect interface changes
- SDKs.md is outdated and describes a different (upload-based) ABI; it may be removed or updated in the future

### Building Everything
```bash
# Build all workspace members
cargo build --workspace

# Run tests (default feature sets)
cargo test --workspace

# Run tests with all crate feature flags enabled (recommended for CI)
cargo test --workspace --all-features
```

## Recent Fixes

### AV Module Refactor (host/core)
The large `av/mod.rs` file has been split into multiple modules (`audio.rs`, `graphics.rs`, `resources.rs`, `storage.rs`, `utils.rs`, `tests.rs`) for better organization and maintainability. All public functions are re-exported from `av` to maintain API compatibility.

### Runtime Module Refactor (host/core)
The large `runtime/mod.rs` file has been split into multiple modules (`runtime.rs`, `imports.rs`) for better organization and maintainability. All public functions are re-exported from `runtime` to maintain API compatibility.

### GIF decoding + scaling (host/core)
The libretro core correctly decodes animated GIFs as indexed-color images and expands them to RGBA using the per-frame palette. Scaled GIF rendering uses nearest-neighbor resampling. Frame delays are now correctly respected (accumulated), and 0-delay frames default to 100ms, fixing playback speed issues. Frame composition (disposal methods) and interlacing are now fully supported, eliminating visual artifacts.

### PNG decoding + drawing (host/core)
The core supports decoding **encoded PNG bytes** on the host. Guests can draw PNGs directly via `image_png` or register them as keyed resources for repeated use (see “ABI notes: keyed resources (hashed strings)” above).

### SDK Parity
Both Rust and Zig SDKs now implement the full set of core features, including all drawing primitives, audio playback, and input handling.

### Triangle rasterization (host/core)
Filled triangles are rasterized using a barycentric (edge-function) fill in the core. The implementation is winding-invariant (vertex order does not change filled results), deterministic, and clips to the framebuffer bounds.

### Audio channel mixing (host/core)
High-level audio playback (`play_wav`/`play_qoa`/`play_xm`) decodes assets and mixes them into the output stream via the host-managed channel mixer in `audio_drain_host`. The mixer supports:
- multiple channels
- per-channel volume (Q8.8)
- pan
- looping

### Resolution configuration (host/core)
The core now correctly respects the resolution set by the guest via `graphics::set_size()` during `setup()`. Previously, the resolution was hardcoded to 320x240 in the libretro AV info, causing display issues if the guest requested a different size.

### SVG scaling (host/core)
SVG rendering now correctly respects the target width and height passed to `svg_draw_key`, scaling the vector graphic to fit the requested dimensions instead of cropping it.

### Font blending (host/core)
TTF font rendering now performs proper alpha blending with the background, eliminating artifacts where text would overwrite the background with black pixels in transparent regions of the glyph.

### ABI Update: u64 keys (host/core/sdk)
The resource ABI has been updated to use `u64` keys instead of string pointers. This improves portability and performance at the boundary. The Rust and Zig SDKs have been updated to automatically hash string keys to `u64` so application code remains unchanged.

## License

MIT License - see `LICENSE` for details.

## Repository

- **GitHub**: https://github.com/isaiahpettingill/wasm96
- **Author**: isaiahpettingill

## Notes

- The project is in active development; some WIT-defined features (e.g., storage) are not yet implemented in the SDK.
- WAV playback is implemented using the hound library for decoding and mixing.
- QOA playback is implemented using the qoaudio library for decoding and mixing.
- XM playback is implemented using the xmrsplayer library for decoding and mixing.
