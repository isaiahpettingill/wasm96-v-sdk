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
1. Use the Rust SDK (`wasm96-sdk`), Zig SDK (`wasm96-zig-sdk`), or Go SDK (`wasm96-go-sdk`) for ergonomic bindings.
2. Implement the required export:
   - `setup()`: Initialize your application (e.g., set screen size)
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

## SDK

### Rust SDK (`wasm96-sdk/`)
- Handwritten bindings matching the WIT interface
- Safe wrappers around raw `extern "C"` imports
- Entry point: `wasm96_sdk::prelude::*`
- Supports `no_std` for minimal WASM builds
- Optional wee_alloc for custom allocator

### Zig SDK (`wasm96-zig-sdk/`)
- Handwritten bindings matching the WIT interface
- Safe wrappers around raw extern functions
- Entry point: `@import("wasm96")`
- Compiles directly to WASM32 (freestanding); produces a module exporting `setup`, `update`, and `draw` (no WASI `_start`)

### Go SDK (`wasm96-go-sdk/`)
- Handwritten bindings matching the WIT interface
- Safe wrappers around raw WebAssembly imports
- Entry point: `import "wasm96-go-sdk"`
- Compiles directly to WASM using WASI Preview 1 (wasip1)

## Examples

The `example/` directory contains guest applications:

- `rust-guest/`: Basic hello-world example (Rust)
- `rust-guest-mp-platformer/`: Multiplayer platformer game (Rust)
- `rust-guest-showcase/`: Comprehensive demo of all features (Rust)
- `zig-guest/`: Basic hello-world example (Zig)
- `go-guest/`: Basic hello-world example (Go)

To build a Rust example:
```bash
cargo build --package <example-name> --target wasm32-unknown-unknown
```

To build the Zig example:
```bash
cd example/zig-guest && zig build
```

To build the Go example:
```bash
cd example/go-guest && GOOS=wasip1 GOARCH=wasm go build -o go-guest.wasm
```

## Project Structure

```
wasm96/
├── wasm96-core/          # Libretro core implementation
├── wasm96-sdk/           # Handwritten Rust SDK
├── wasm96-zig-sdk/       # Handwritten Zig SDK
├── wasm96-go-sdk/        # Handwritten Go SDK
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
- The ABI is handwritten; update bindings in `wasm96-core/src/abi/mod.rs`, `wasm96-sdk/src/lib.rs`, `wasm96-zig-sdk/src/main.zig`, and `wasm96-go-sdk/wasm96.go` in lockstep
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

### GIF decoding + scaling (host/core)
The libretro core correctly decodes animated GIFs as indexed-color images and expands them to RGBA using the per-frame palette (or the global palette when the frame does not provide one). Scaled GIF rendering uses nearest-neighbor resampling so `gif_draw_scaled` produces correctly-sized output.

### PNG decoding + drawing (host/core)
The core now supports decoding **encoded PNG bytes** on the host and blitting the image at its natural size. This is intended for guests that embed PNG assets via `include_bytes!` and don’t want to ship a guest-side PNG decoder.

In the Rust SDK, use:
- `graphics::image_png(x, y, png_bytes)`

(Under the hood this maps to the import `wasm96_graphics_image_png(x, y, ptr, len)`.)

### Triangle rasterization (host/core)
Filled triangles are rasterized using a barycentric (edge-function) fill in the core. The implementation is winding-invariant (vertex order does not change filled results), deterministic, and clips to the framebuffer bounds.

### Audio channel mixing (host/core)
High-level audio playback (`play_wav`/`play_qoa`/`play_xm`) decodes assets and mixes them into the output stream via the host-managed channel mixer in `audio_drain_host`. The mixer supports:
- multiple channels
- per-channel volume (Q8.8)
- pan
- looping

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
- SDKs.md contains information about planned multi-language SDKs for an older ABI version and may not reflect the current state.