#pragma once
/*
  wasm96 C++ guest SDK (header-only)

  This file is a typed wrapper around the wasm96 guest ABI.

  ABI model (upload-based, write-only from guest):
  - Host provides imports under module name "env" with these symbol names:
      wasm96_abi_version

      // Video (full-frame upload)
      wasm96_video_config
      wasm96_video_upload
      wasm96_video_present

      // Audio (push samples)
      wasm96_audio_config
      wasm96_audio_push_i16
      wasm96_audio_drain

      // Input
      wasm96_joypad_button_pressed / wasm96_key_pressed
      wasm96_mouse_x / wasm96_mouse_y / wasm96_mouse_buttons
      wasm96_lightgun_x / wasm96_lightgun_y / wasm96_lightgun_buttons

  - Guest must export (host calls):
      void wasm96_frame();
    Optional:
      void wasm96_init();
      void wasm96_deinit();
      void wasm96_reset();

  Notes:
  - "Pointers" are u32 offsets into the guest linear memory (wasm32).
  - Guest memory is owned/allocated by the guest; the host never allocates into guest linear memory.
  - This header does not depend on WIT/component model or any codegen.
*/

#include <cstdint>
#include <cstddef>
#include <type_traits>

#if defined(__clang__)
  #define WASM96_IMPORT(module, name) __attribute__((import_module(module), import_name(name)))
#else
  // For non-Clang toolchains, leave attributes empty. You may need to adapt imports
  // depending on your compiler/wasm pipeline.
  #define WASM96_IMPORT(module, name)
#endif

namespace wasm96 {

// Keep in sync with the host ABI version.
inline constexpr std::uint32_t ABI_VERSION = 1;

// --------------------
// Raw ABI imports
// --------------------
extern "C" {

// ABI
WASM96_IMPORT("env", "wasm96_abi_version")
std::uint32_t wasm96_abi_version();

// Video (upload-based)
WASM96_IMPORT("env", "wasm96_video_config")
std::uint32_t wasm96_video_config(std::uint32_t width, std::uint32_t height, std::uint32_t pixel_format);

WASM96_IMPORT("env", "wasm96_video_upload")
std::uint32_t wasm96_video_upload(std::uint32_t ptr, std::uint32_t byte_len, std::uint32_t pitch_bytes);

WASM96_IMPORT("env", "wasm96_video_present")
void wasm96_video_present();

// Audio (push-based, interleaved i16)
WASM96_IMPORT("env", "wasm96_audio_config")
std::uint32_t wasm96_audio_config(std::uint32_t sample_rate, std::uint32_t channels);

WASM96_IMPORT("env", "wasm96_audio_push_i16")
std::uint32_t wasm96_audio_push_i16(std::uint32_t ptr, std::uint32_t frames);

WASM96_IMPORT("env", "wasm96_audio_drain")
std::uint32_t wasm96_audio_drain(std::uint32_t max_frames);

// Input
WASM96_IMPORT("env", "wasm96_joypad_button_pressed")
std::uint32_t wasm96_joypad_button_pressed(std::uint32_t port, std::uint32_t button);

WASM96_IMPORT("env", "wasm96_key_pressed")
std::uint32_t wasm96_key_pressed(std::uint32_t key);

WASM96_IMPORT("env", "wasm96_mouse_x")
std::int32_t wasm96_mouse_x();

WASM96_IMPORT("env", "wasm96_mouse_y")
std::int32_t wasm96_mouse_y();

WASM96_IMPORT("env", "wasm96_mouse_buttons")
std::uint32_t wasm96_mouse_buttons();

WASM96_IMPORT("env", "wasm96_lightgun_x")
std::int32_t wasm96_lightgun_x(std::uint32_t port);

WASM96_IMPORT("env", "wasm96_lightgun_y")
std::int32_t wasm96_lightgun_y(std::uint32_t port);

WASM96_IMPORT("env", "wasm96_lightgun_buttons")
std::uint32_t wasm96_lightgun_buttons(std::uint32_t port);

} // extern "C"

// --------------------
// ABI types/constants
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

constexpr std::uint32_t pitch_bytes(std::uint32_t width, PixelFormat fmt) {
    return width * bytes_per_pixel(fmt);
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

// Convert a guest u32 offset into a pointer within linear memory.
// In wasm32 guests, linear memory is mapped starting at address 0, so this cast is typical.
template <class T>
inline T* ptr_from_offset(std::uint32_t offset) {
    return reinterpret_cast<T*>(static_cast<std::uintptr_t>(offset));
}

template <class T>
inline const T* ptr_from_offset_const(std::uint32_t offset) {
    return reinterpret_cast<const T*>(static_cast<std::uintptr_t>(offset));
}

// --------------------
// ABI helpers
// --------------------

inline std::uint32_t host_abi_version() { return wasm96_abi_version(); }

inline bool abi_compatible() { return host_abi_version() == ABI_VERSION; }

// --------------------
// Video API (full-frame upload)
//
// Guest owns the framebuffer in linear memory; host stores the configured framebuffer in system memory.
// --------------------

inline bool video_config(std::uint32_t width, std::uint32_t height, PixelFormat format) {
    return wasm96_video_config(width, height, static_cast<std::uint32_t>(format)) != 0;
}

// Upload a full frame to the host (write-only).
// - ptr: u32 offset into guest linear memory
// - byte_len: must equal height * pitch_bytes
// - pitch_bytes: bytes per row
inline bool video_upload(std::uint32_t ptr, std::uint32_t byte_len, std::uint32_t pitch_bytes_) {
    return wasm96_video_upload(ptr, byte_len, pitch_bytes_) != 0;
}

inline void present() {
    wasm96_video_present();
}

// --------------------
// Audio API (push interleaved i16)
//
// Guest owns the sample buffer in linear memory; host queues/drains to libretro.
// --------------------

inline bool audio_config(std::uint32_t sample_rate, std::uint32_t channels) {
    return wasm96_audio_config(sample_rate, channels) != 0;
}

// Push interleaved i16 samples (write-only).
// - ptr: u32 offset into guest linear memory pointing to i16 samples (little-endian)
// - frames: number of frames (one frame = channels samples)
inline std::uint32_t audio_push_i16(std::uint32_t ptr, std::uint32_t frames) {
    return wasm96_audio_push_i16(ptr, frames);
}

inline std::uint32_t audio_drain(std::uint32_t max_frames) {
    return wasm96_audio_drain(max_frames);
}

// --------------------
// Input API
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

} // namespace wasm96

#undef WASM96_IMPORT