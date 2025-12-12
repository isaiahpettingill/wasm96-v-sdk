#pragma once
/*
  wasm96 C++ SDK (header-only)

  This is a thin, typed C++ wrapper around the wasm96 guest->host ABI.

  ABI model (upload-based):
  - Guest manages its own allocations in WASM linear memory.
  - Host owns video/audio buffers in system memory.
  - Guest performs write-only uploads:
      Video: configure -> upload full frame -> present
      Audio: configure -> push interleaved i16 -> drain (optional)

  Notes:
  - This header is intended to be compiled for wasm32 guests.
  - The host provides the imported functions under module "env" using the
    symbol names listed below (e.g. "wasm96_video_config").
  - Pointers are 32-bit offsets in guest linear memory (WASM32).
*/

#include <cstdint>
#include <cstddef>
#include <type_traits>

namespace wasm96 {

// Keep in sync with wasm96-core/src/abi/mod.rs
inline constexpr std::uint32_t ABI_VERSION = 1;

// --------------------
// Low-level C ABI
// --------------------
extern "C" {

// ABI
__attribute__((import_module("env"), import_name("wasm96_abi_version")))
std::uint32_t wasm96_abi_version();



// Video
__attribute__((import_module("env"), import_name("wasm96_video_config")))
std::uint32_t wasm96_video_config(std::uint32_t width, std::uint32_t height, std::uint32_t pixel_format);

__attribute__((import_module("env"), import_name("wasm96_video_present")))
void wasm96_video_present();

__attribute__((import_module("env"), import_name("wasm96_video_upload")))
std::uint32_t wasm96_video_upload(std::uint32_t ptr, std::uint32_t byte_len, std::uint32_t pitch_bytes);

// Audio
__attribute__((import_module("env"), import_name("wasm96_audio_config")))
std::uint32_t wasm96_audio_config(std::uint32_t sample_rate, std::uint32_t channels);

__attribute__((import_module("env"), import_name("wasm96_audio_push_i16")))
std::uint32_t wasm96_audio_push_i16(std::uint32_t ptr, std::uint32_t frames);



__attribute__((import_module("env"), import_name("wasm96_audio_drain")))
std::uint32_t wasm96_audio_drain(std::uint32_t max_frames);

// Input
__attribute__((import_module("env"), import_name("wasm96_joypad_button_pressed")))
std::uint32_t wasm96_joypad_button_pressed(std::uint32_t port, std::uint32_t button);

__attribute__((import_module("env"), import_name("wasm96_key_pressed")))
std::uint32_t wasm96_key_pressed(std::uint32_t key);

__attribute__((import_module("env"), import_name("wasm96_mouse_x")))
std::int32_t wasm96_mouse_x();

__attribute__((import_module("env"), import_name("wasm96_mouse_y")))
std::int32_t wasm96_mouse_y();

__attribute__((import_module("env"), import_name("wasm96_mouse_buttons")))
std::uint32_t wasm96_mouse_buttons();

__attribute__((import_module("env"), import_name("wasm96_lightgun_x")))
std::int32_t wasm96_lightgun_x(std::uint32_t port);

__attribute__((import_module("env"), import_name("wasm96_lightgun_y")))
std::int32_t wasm96_lightgun_y(std::uint32_t port);

__attribute__((import_module("env"), import_name("wasm96_lightgun_buttons")))
std::uint32_t wasm96_lightgun_buttons(std::uint32_t port);

} // extern "C"

// --------------------
// Typed enums / flags
// --------------------

enum class PixelFormat : std::uint32_t {
  Xrgb8888 = 0,
  Rgb565   = 1,
};

constexpr std::uint32_t bytes_per_pixel(PixelFormat fmt) {
  switch (fmt) {
    case PixelFormat::Xrgb8888: return 4;
    case PixelFormat::Rgb565:   return 2;
  }
  return 0;
}

enum class JoypadButton : std::uint32_t {
  B      = 0,
  Y      = 1,
  Select = 2,
  Start  = 3,
  Up     = 4,
  Down   = 5,
  Left   = 6,
  Right  = 7,
  A      = 8,
  X      = 9,
  L1     = 10,
  R1     = 11,
  L2     = 12,
  R2     = 13,
  L3     = 14,
  R3     = 15,
};

struct MouseButtons {
  static inline constexpr std::uint32_t Left    = 1u << 0;
  static inline constexpr std::uint32_t Right   = 1u << 1;
  static inline constexpr std::uint32_t Middle  = 1u << 2;
  static inline constexpr std::uint32_t Button4 = 1u << 3;
  static inline constexpr std::uint32_t Button5 = 1u << 4;
};

struct LightgunButtons {
  static inline constexpr std::uint32_t Trigger   = 1u << 0;
  static inline constexpr std::uint32_t Reload    = 1u << 1;
  static inline constexpr std::uint32_t Start     = 1u << 2;
  static inline constexpr std::uint32_t Select    = 1u << 3;
  static inline constexpr std::uint32_t AuxA      = 1u << 4;
  static inline constexpr std::uint32_t AuxB      = 1u << 5;
  static inline constexpr std::uint32_t AuxC      = 1u << 6;
  static inline constexpr std::uint32_t Offscreen = 1u << 7;
};

// --------------------
// Video
// --------------------

struct Framebuffer {
  std::uint32_t ptr = 0;          // guest linear memory offset
  std::uint32_t width = 0;
  std::uint32_t height = 0;
  std::uint32_t pitch_bytes = 0;
  PixelFormat format = PixelFormat::Xrgb8888;

  constexpr bool valid() const { return ptr != 0; }

  constexpr std::uint32_t byte_len_u32() const {
    return height * pitch_bytes;
  }

  constexpr std::size_t byte_len() const {
    return static_cast<std::size_t>(byte_len_u32());
  }

  // View raw bytes.
  // SAFETY: caller must ensure ptr points to a valid allocation for byte_len().
  std::uint8_t* bytes_mut() const {
    return reinterpret_cast<std::uint8_t*>(static_cast<std::uintptr_t>(ptr));
  }

  // View pixels as u32 (XRGB8888).
  // Requires pitch divisible by 4.
  std::uint32_t* pixels_xrgb8888_mut() const {
    return reinterpret_cast<std::uint32_t*>(static_cast<std::uintptr_t>(ptr));
  }

  // View pixels as u16 (RGB565).
  // Requires pitch divisible by 2.
  std::uint16_t* pixels_rgb565_mut() const {
    return reinterpret_cast<std::uint16_t*>(static_cast<std::uintptr_t>(ptr));
  }
};

inline bool video_config(std::uint32_t width, std::uint32_t height, PixelFormat format) {
  return wasm96_video_config(width, height, static_cast<std::uint32_t>(format)) != 0;
}

inline bool video_upload(std::uint32_t ptr, std::uint32_t byte_len, std::uint32_t pitch_bytes_) {
  return wasm96_video_upload(ptr, byte_len, pitch_bytes_) != 0;
}

inline void present() {
  wasm96_video_present();
}

inline std::uint32_t pitch_bytes(std::uint32_t width, PixelFormat format) {
  return width * bytes_per_pixel(format);
}

// --------------------
// Audio
// --------------------

/* Ringbuffer-based audio API removed in upload-based ABI */

inline bool audio_config(std::uint32_t sample_rate, std::uint32_t channels) {
  return wasm96_audio_config(sample_rate, channels) != 0;
}

inline std::uint32_t audio_push_i16(std::uint32_t ptr, std::uint32_t frames) {
  return wasm96_audio_push_i16(ptr, frames);
}

inline std::uint32_t audio_drain(std::uint32_t max_frames) {
  return wasm96_audio_drain(max_frames);
}

// --------------------
// Input
// --------------------

inline bool joypad_pressed(std::uint32_t port, JoypadButton button) {
  return wasm96_joypad_button_pressed(port, static_cast<std::uint32_t>(button)) != 0;
}

inline bool key_pressed(std::uint32_t key) {
  return wasm96_key_pressed(key) != 0;
}

inline std::int32_t mouse_x() { return wasm96_mouse_x(); }
inline std::int32_t mouse_y() { return wasm96_mouse_y(); }
inline std::uint32_t mouse_buttons() { return wasm96_mouse_buttons(); }

inline std::int32_t lightgun_x(std::uint32_t port) { return wasm96_lightgun_x(port); }
inline std::int32_t lightgun_y(std::uint32_t port) { return wasm96_lightgun_y(port); }
inline std::uint32_t lightgun_buttons(std::uint32_t port) { return wasm96_lightgun_buttons(port); }

// --------------------
// ABI helpers
// --------------------

inline std::uint32_t host_abi_version() { return wasm96_abi_version(); }

inline bool abi_compatible() { return host_abi_version() == ABI_VERSION; }

} // namespace wasm96