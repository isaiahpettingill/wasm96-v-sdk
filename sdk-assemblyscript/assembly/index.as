/**
 * wasm96 AssemblyScript SDK (handwritten)
 *
 * This module targets the wasm96 guest ABI exposed by the host under import module "env".
 *
 * ABI model (upload-based):
 * - Guest owns its allocations in WASM linear memory.
 * - Host owns its video/audio buffers in *system memory*.
 * - Guest performs write-only uploads:
 *   - Video: configure -> upload full frame -> present
 *   - Audio: configure -> push i16 frames -> drain (optional)
 *
 * ABI notes:
 * - Pointers are 32-bit offsets into the guest's linear memory (WebAssembly linear memory).
 * - Some host functions may return 0 / do nothing; always handle failure.
 * - The host calls guest exports: `wasm96_frame` (required), and optionally `wasm96_init`,
 *   `wasm96_deinit`, `wasm96_reset`.
 *
 * This SDK intentionally does NOT use WIT/component-model codegen.
 */

// Keep in sync with wasm96-core ABI version.
export const ABI_VERSION: u32 = 1;

// --------------------
// Enums / constants
// --------------------

export enum PixelFormat {
  Xrgb8888 = 0,
  Rgb565 = 1,
}

export function bytesPerPixel(format: PixelFormat): u32 {
  return format == PixelFormat.Xrgb8888 ? 4 : 2;
}

export enum JoypadButton {
  B = 0,
  Y = 1,
  Select = 2,
  Start = 3,
  Up = 4,
  Down = 5,
  Left = 6,
  Right = 7,
  A = 8,
  X = 9,
  L1 = 10,
  R1 = 11,
  L2 = 12,
  R2 = 13,
  L3 = 14,
  R3 = 15,
}

export namespace MouseButtons {
  export const Left: u32 = 1 << 0;
  export const Right: u32 = 1 << 1;
  export const Middle: u32 = 1 << 2;
  export const Button4: u32 = 1 << 3;
  export const Button5: u32 = 1 << 4;
}

export namespace LightgunButtons {
  export const Trigger: u32 = 1 << 0;
  export const Reload: u32 = 1 << 1;
  export const Start: u32 = 1 << 2;
  export const Select: u32 = 1 << 3;
  export const AuxA: u32 = 1 << 4;
  export const AuxB: u32 = 1 << 5;
  export const AuxC: u32 = 1 << 6;
  export const Offscreen: u32 = 1 << 7;
}

// --------------------
// Host imports (env)
// --------------------

// ABI
@external("env", "wasm96_abi_version")
declare function _wasm96_abi_version(): u32;

// Video (upload-based)
@external("env", "wasm96_video_config")
declare function _wasm96_video_config(width: u32, height: u32, pixel_format: u32): u32;

@external("env", "wasm96_video_upload")
declare function _wasm96_video_upload(ptr: u32, byte_len: u32, pitch_bytes: u32): u32;

@external("env", "wasm96_video_present")
declare function _wasm96_video_present(): void;

// Audio (push-based, interleaved i16)
@external("env", "wasm96_audio_config")
declare function _wasm96_audio_config(sample_rate: u32, channels: u32): u32;

@external("env", "wasm96_audio_push_i16")
declare function _wasm96_audio_push_i16(ptr: u32, frames: u32): u32;

@external("env", "wasm96_audio_drain")
declare function _wasm96_audio_drain(max_frames: u32): u32;

// Input
@external("env", "wasm96_joypad_button_pressed")
declare function _wasm96_joypad_button_pressed(port: u32, button: u32): u32;

@external("env", "wasm96_key_pressed")
declare function _wasm96_key_pressed(key: u32): u32;

@external("env", "wasm96_mouse_x")
declare function _wasm96_mouse_x(): i32;

@external("env", "wasm96_mouse_y")
declare function _wasm96_mouse_y(): i32;

@external("env", "wasm96_mouse_buttons")
declare function _wasm96_mouse_buttons(): u32;

@external("env", "wasm96_lightgun_x")
declare function _wasm96_lightgun_x(port: u32): i32;

@external("env", "wasm96_lightgun_y")
declare function _wasm96_lightgun_y(port: u32): i32;

@external("env", "wasm96_lightgun_buttons")
declare function _wasm96_lightgun_buttons(port: u32): u32;

// --------------------
// ABI helpers
// --------------------

export function hostAbiVersion(): u32 {
  return _wasm96_abi_version();
}

export function abiCompatible(): bool {
  return _wasm96_abi_version() == ABI_VERSION;
}

// --------------------
// Video helpers
// --------------------

/**
 * Configure the host-side framebuffer spec.
 * Returns `true` on success.
 */
export function videoConfig(width: u32, height: u32, format: PixelFormat): bool {
  return _wasm96_video_config(width, height, <u32>format) != 0;
}

/**
 * Upload a full frame from guest linear memory into the host system-memory framebuffer.
 *
 * - `ptr`: u32 offset into guest linear memory
 * - `byteLen`: must be exactly `height * pitchBytes` for the configured framebuffer
 * - `pitchBytes`: bytes per row used by the guest buffer
 *
 * Returns `true` on success.
 */
export function videoUpload(ptr: u32, byteLen: u32, pitchBytes: u32): bool {
  return _wasm96_video_upload(ptr, byteLen, pitchBytes) != 0;
}

/**
 * Present the last uploaded frame.
 */
export function present(): void {
  _wasm96_video_present();
}

/**
 * Convenience helper: compute pitch bytes for width+format (no padding).
 */
export function videoPitchBytes(width: u32, format: PixelFormat): u32 {
  return width * bytesPerPixel(format);
}

// --------------------
// Audio helpers
// --------------------

/**
 * Configure host-side audio output format.
 * Returns `true` on success.
 */
export function audioConfig(sampleRate: u32, channels: u32): bool {
  return _wasm96_audio_config(sampleRate, channels) != 0;
}

/**
 * Push interleaved i16 audio frames from guest linear memory into the host queue.
 *
 * - `ptr`: u32 offset into guest linear memory
 * - `frames`: number of frames (one frame = `channels` samples)
 *
 * Returns number of frames accepted (0 on failure).
 */
export function audioPushI16(ptr: u32, frames: u32): u32 {
  return _wasm96_audio_push_i16(ptr, frames);
}

/**
 * Drain up to `maxFrames` frames from host queue into libretro.
 * If `maxFrames == 0`, drains everything currently queued.
 */
export function audioDrain(maxFrames: u32 = 0): u32 {
  return _wasm96_audio_drain(maxFrames);
}

// --------------------
// Input helpers
// --------------------

export function joypadPressed(port: u32, button: JoypadButton): bool {
  return _wasm96_joypad_button_pressed(port, <u32>button) != 0;
}

export function keyPressed(key: u32): bool {
  return _wasm96_key_pressed(key) != 0;
}

export function mouseX(): i32 {
  return _wasm96_mouse_x();
}

export function mouseY(): i32 {
  return _wasm96_mouse_y();
}

/** Returns bitmask; see `MouseButtons.*` constants. */
export function mouseButtons(): u32 {
  return _wasm96_mouse_buttons();
}

export function lightgunX(port: u32): i32 {
  return _wasm96_lightgun_x(port);
}

export function lightgunY(port: u32): i32 {
  return _wasm96_lightgun_y(port);
}

/** Returns bitmask; see `LightgunButtons.*` constants. */
export function lightgunButtons(port: u32): u32 {
  return _wasm96_lightgun_buttons(port);
}

// --------------------
// Optional allocator helpers
// --------------------
//
// The upload-based ABI does not require any host allocator hooks. Guests should manage
// their own allocations in linear memory. Therefore, allocator helpers are removed.
