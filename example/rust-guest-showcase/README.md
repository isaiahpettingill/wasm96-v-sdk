
# Wasm96 Rust Guest Showcase

This example demonstrates all available functionality of the Wasm96 SDK in a single comprehensive application. It showcases graphics rendering, audio playback, input handling, and asset loading.

## Features Demonstrated

### Graphics
- **Shapes**: Points, lines, rectangles (filled and outlined), circles (filled and outlined), triangles (filled and outlined), quadratic and cubic Bezier curves, pills (filled and outlined).
- **Assets**: SVG rendering, GIF animation, PNG images, custom TTF fonts, and built-in Spleen bitmap fonts.
- **Text**: Rendering text with loaded fonts and measuring text dimensions.

### Audio
- Plays `crickets.wav` (embedded asset) as a simple example of audio playback.

TODO:
- Add chiptune-style music playback (host-side synth voices + simple sequencing helpers).

### Input
- **Controllers**: Support for up to 4 game controllers, displaying button states in real-time.
- **Mouse**: Position tracking and button press detection.
- **Keyboard**: Basic key detection (though not fully utilized in this example).

### Assets
- **SVG**: man.svg - A vector graphic loaded from file.
- **GIF**: 200.gif - An animated GIF loaded from file.
- **PNG**: ink.png - An image loaded from file.
- **TTF**: UnifrakturMaguntia-Regular.ttf - A custom TTF font loaded from file.

Note: Assets are embedded from files in the src/assets directory using the include_bytes! macro at compile time.

## Building

To build the example for WebAssembly:

```bash
cargo build --package rust-guest-showcase --target wasm32-unknown-unknown
```

## Running

Load the generated WASM file (`target/wasm32-unknown-unknown/debug/rust_guest_showcase.wasm`) into a Wasm96-compatible runtime, such as the libretro core.

## Controls

- Use connected controllers to see button states displayed on-screen.
- Move the mouse to see its position and click to highlight.
- The application runs at 60 FPS with animated background and audio.

## Code Structure

- `setup()`: Initializes graphics, audio, and loads assets.
- `update()`: Generates audio samples.
- `draw()`: Renders all graphics elements, assets, text, and input status.

This example serves as a reference for using all Wasm96 SDK features in a Rust guest application.