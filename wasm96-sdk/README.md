<file_path>
wasm96/wasm96-sdk/README.md
</file_path>

<edit_description>
Create README.md for wasm96-sdk
</edit_description>

# wasm96-sdk

A Rust SDK for building WebAssembly applications that run under the [wasm96](https://github.com/isaiahpettingill/wasm96) libretro core.

## Overview

wasm96-sdk provides safe, ergonomic bindings to the wasm96 ABI, allowing you to write games and applications in Rust that compile to WebAssembly and run in libretro frontends like RetroArch.

Key features:
- **Immediate Mode Graphics**: Issue drawing commands (rects, circles, text, etc.) without managing framebuffers.
- **Audio Playback**: Play WAV, QOA, and XM files with host-mixed channels.
- **Input Handling**: Query joypad, keyboard, and mouse state.
- **Resource Management**: Register and draw images (PNG, GIF, SVG), fonts, and other assets by key.
- **Storage**: Save/load persistent data.
- **System Utilities**: Logging and timing.

## Usage

Add this to your `Cargo.toml`:

```toml
[package]
name = "my-wasm96-app"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm96-sdk = "0.1.0"
```

In your `src/lib.rs`:

```rust
use wasm96_sdk::prelude::*;

// Required: Called once on startup
#[no_mangle]
pub extern "C" fn setup() {
    graphics::set_size(640, 480);
    // Register assets, initialize state, etc.
}

// Optional: Called once per frame to update logic
#[no_mangle]
pub extern "C" fn update() {
    // Handle input, update game state
}

// Optional: Called once per frame to draw
#[no_mangle]
pub extern "C" fn draw() {
    graphics::background(0, 0, 0); // Black background
    graphics::set_color(255, 255, 255, 255); // White
    graphics::rect(100, 100, 100, 100); // Draw a rectangle
}
```

Build for WebAssembly:

```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

The output `.wasm` file can be loaded into the wasm96 core in RetroArch.

## Features

- `std` (default): Enables standard library features for convenience.
- `wee_alloc`: Optional global allocator for `wasm32-unknown-unknown` targets.

## Examples

See the [wasm96 repository](https://github.com/isaiahpettingill/wasm96/tree/main/example) for complete examples:

- `rust-guest/`: Basic hello-world
- `rust-guest-showcase/`: Comprehensive feature demo

## Documentation

Generate and view docs locally:

```bash
cargo doc --open
```

## ABI Compatibility

This SDK targets the wasm96 ABI as defined in the [WIT interface](https://github.com/isaiahpettingill/wasm96/blob/main/wit/wasm96.wit). Ensure your wasm96-core version matches the SDK version for compatibility.

## License

MIT License - see [LICENSE](https://github.com/isaiahpettingill/wasm96/blob/main/LICENSE) for details.

## Contributing

Contributions are welcome! Please see the [main repository](https://github.com/isaiahpettingill/wasm96) for development guidelines.