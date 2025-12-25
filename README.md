# wasm96-v-sdk

A V SDK for building WebAssembly applications that run under the [wasm96](https://github.com/isaiahpettingill/wasm96) libretro core.

## Overview

wasm96-v-sdk provides safe, ergonomic bindings to the wasm96 ABI, allowing you to write games and applications in V that compile to WebAssembly and run in libretro frontends like RetroArch.

Key features:
- **Immediate Mode Graphics**: Issue drawing commands (rects, circles, text, etc.) without managing framebuffers.
- **Audio Playback**: Play WAV, QOA, and XM files with host-mixed channels.
- **Input Handling**: Query joypad, keyboard, and mouse state.
- **Resource Management**: Register and draw images (PNG, GIF, SVG), fonts, and other assets by key.
- **Storage**: Save/load persistent data.
- **System Utilities**: Logging and timing.
- **3D Graphics**: Support for 3D rendering with meshes, cameras, and transformations.

## Installation

### Option 1: Install from Git

```bash
v install isaiahpettingill.wasm96
```

### Option 2: Manual Installation

Clone this repository and copy the `wasm96.v` file to your V modules path:

```bash
# Assuming V modules are in ~/.vmodules
mkdir -p ~/.vmodules/isaiahpettingill
cp wasm96.v ~/.vmodules/isaiahpettingill/
```

## Usage

In your V project, create a `main.v` file:

```v
module main

import isaiahpettingill.wasm96

@[export: 'setup']
fn setup() {
    wasm96.graphics_set_size(640, 480)
    // Register assets, initialize state, etc.
}

@[export: 'draw']
fn draw() {
    wasm96.graphics_background(0, 0, 0) // Black background
    wasm96.graphics_set_color(255, 255, 255, 255) // White
    wasm96.graphics_rect(100, 100, 100, 100) // Draw a rectangle
}
```

Build for WebAssembly:

```bash
v -b wasm -enable-globals -o output.wasm main.v
```

The output `.wasm` file can be loaded into the wasm96 core in RetroArch.

## API Overview

### Graphics

```v
// Basic shapes
wasm96.graphics_rect(x, y, width, height)
wasm96.graphics_circle(x, y, radius)

// Colors and backgrounds
wasm96.graphics_set_color(r, g, b, a)
wasm96.graphics_background(r, g, b)

// Text
wasm96.graphics_font_register_spleen('font_key'.bytes(), 16)
wasm96.graphics_text_key(x, y, 'font_key'.bytes(), 'Hello World'.bytes())
```

### Input

```v
if wasm96.input_is_button_down(0, .a) {
    // A button pressed
}
```

### Audio

```v
wasm96.audio_init(44100)
wasm96.audio_play_wav(wav_data)
```

### 3D Graphics

```v
wasm96.graphics_set_3d(true)
wasm96.graphics_camera_perspective(fovy, aspect, near, far)
wasm96.graphics_mesh_create('cube'.bytes(), vertices, indices)
wasm96.graphics_mesh_draw('cube'.bytes(), pos_x, pos_y, pos_z, rot_x, rot_y, rot_z, scale_x, scale_y, scale_z)
```

## Examples

See the [wasm96 repository](https://github.com/isaiahpettingill/wasm96/tree/main/example) for complete examples:

- `v-guest-3d/`: 3D rotating cube demo (note: currently has compatibility issues)

## Known Issues

- The V SDK may have module import issues depending on your V installation and module paths.
- Ensure the module is correctly placed in `~/.vmodules/isaiahpettingill/wasm96.v`

## ABI Compatibility

This SDK targets the wasm96 ABI as defined in the [WIT interface](https://github.com/isaiahpettingill/wasm96/blob/main/wit/wasm96.wit). Ensure your wasm96-core version matches the SDK version for compatibility.

## License

MIT License - see [LICENSE](https://github.com/isaiahpettingill/wasm96/blob/main/LICENSE) for details.

## Contributing

Contributions are welcome! Please see the [main repository](https://github.com/isaiahpettingill/wasm96) for development guidelines.