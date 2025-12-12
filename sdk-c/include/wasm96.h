#ifndef WASM96_H_INCLUDED
#define WASM96_H_INCLUDED

/*
 * wasm96 guest SDK (C)
 *
 * This header defines the stable, C-like ABI between:
 *  - Host: wasm96 libretro core
 *  - Guest: your WebAssembly module (wasm32)
 *
 * ABI model (upload-based):
 *  - Guest manages its own allocations in WASM linear memory.
 *  - Host owns video/audio buffers in system memory.
 *  - Guest performs write-only full-frame video uploads and pushes audio sample batches.
 *
 * The host provides imports under module name "env" with these symbol names:
 *   wasm96_abi_version
 *
 *   Video:
 *     wasm96_video_config
 *     wasm96_video_upload
 *     wasm96_video_present
 *
 *   Audio:
 *     wasm96_audio_config
 *     wasm96_audio_push_i16
 *     wasm96_audio_drain
 *
 *   Input:
 *     wasm96_joypad_button_pressed / wasm96_key_pressed
 *     wasm96_mouse_x / wasm96_mouse_y / wasm96_mouse_buttons
 *     wasm96_lightgun_x / wasm96_lightgun_y / wasm96_lightgun_buttons
 *
 * Required guest export:
 *   void wasm96_frame(void);
 *
 * Optional guest exports:
 *   void wasm96_init(void);
 *   void wasm96_deinit(void);
 *   void wasm96_reset(void);
 *
 * Notes:
 * - Pointers are 32-bit offsets into the guest's linear memory (WASM32).
 * - The host does not allocate into guest memory.
 */

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

#define WASM96_ABI_VERSION 1u

/* =========================
 * Enums / constants (ABI)
 * ========================= */

/* Pixel formats for wasm96_video_config */
typedef enum wasm96_pixel_format {
    WASM96_PIXEL_FORMAT_XRGB8888 = 0u, /* 4 bytes per pixel, 32bpp packed */
    WASM96_PIXEL_FORMAT_RGB565   = 1u  /* 2 bytes per pixel, 16bpp packed */
} wasm96_pixel_format_t;

/* Joypad button ids (aligned with libretro joypad ids) */
typedef enum wasm96_joypad_button {
    WASM96_JOYPAD_B      = 0u,
    WASM96_JOYPAD_Y      = 1u,
    WASM96_JOYPAD_SELECT = 2u,
    WASM96_JOYPAD_START  = 3u,
    WASM96_JOYPAD_UP     = 4u,
    WASM96_JOYPAD_DOWN   = 5u,
    WASM96_JOYPAD_LEFT   = 6u,
    WASM96_JOYPAD_RIGHT  = 7u,
    WASM96_JOYPAD_A      = 8u,
    WASM96_JOYPAD_X      = 9u,
    WASM96_JOYPAD_L1     = 10u,
    WASM96_JOYPAD_R1     = 11u,
    WASM96_JOYPAD_L2     = 12u,
    WASM96_JOYPAD_R2     = 13u,
    WASM96_JOYPAD_L3     = 14u,
    WASM96_JOYPAD_R3     = 15u
} wasm96_joypad_button_t;

/* Mouse buttons bitmask (returned by wasm96_mouse_buttons) */
enum {
    WASM96_MOUSE_BUTTON_LEFT   = 1u << 0,
    WASM96_MOUSE_BUTTON_RIGHT  = 1u << 1,
    WASM96_MOUSE_BUTTON_MIDDLE = 1u << 2,
    WASM96_MOUSE_BUTTON_4      = 1u << 3,
    WASM96_MOUSE_BUTTON_5      = 1u << 4
};

/* Lightgun buttons bitmask (returned by wasm96_lightgun_buttons) */
enum {
    WASM96_LIGHTGUN_BUTTON_TRIGGER   = 1u << 0,
    WASM96_LIGHTGUN_BUTTON_RELOAD    = 1u << 1,
    WASM96_LIGHTGUN_BUTTON_START     = 1u << 2,
    WASM96_LIGHTGUN_BUTTON_SELECT    = 1u << 3,
    WASM96_LIGHTGUN_BUTTON_AUX_A     = 1u << 4,
    WASM96_LIGHTGUN_BUTTON_AUX_B     = 1u << 5,
    WASM96_LIGHTGUN_BUTTON_AUX_C     = 1u << 6,
    WASM96_LIGHTGUN_BUTTON_OFFSCREEN = 1u << 7
};

/* =========================
 * Host imports (guest -> host)
 * =========================
 *
 * These are provided by the host under module "env".
 *
 * Export mechanics are toolchain-specific. In C, you typically just declare them `extern`
 * and your wasm compiler + linker will emit imports for unresolved symbols (when targeting wasm).
 */

extern uint32_t wasm96_abi_version(void);

/* Video (upload-based) */

/* Configure the host-side framebuffer spec. Returns 1 on success, 0 on failure. */
extern uint32_t wasm96_video_config(uint32_t width, uint32_t height, uint32_t pixel_format);

/* Upload a full frame from guest linear memory. Returns 1 on success, 0 on failure.
 * - ptr: guest linear memory offset to framebuffer bytes
 * - byte_len: must be exactly height * pitch_bytes
 * - pitch_bytes: bytes per row; must match the configured pitch
 */
extern uint32_t wasm96_video_upload(uint32_t ptr, uint32_t byte_len, uint32_t pitch_bytes);

/* Present the last uploaded frame. */
extern void     wasm96_video_present(void);

/* Audio (push-based, interleaved i16 stereo; counts are in frames) */

/* Configure host-side audio output format. Returns 1 on success, 0 on failure. */
extern uint32_t wasm96_audio_config(uint32_t sample_rate, uint32_t channels);

/* Push interleaved i16 samples from guest linear memory. Returns frames accepted (0 on failure).
 * - ptr: guest linear memory offset to i16 samples (little-endian)
 * - frames: number of frames (one frame = channels samples)
 */
extern uint32_t wasm96_audio_push_i16(uint32_t ptr, uint32_t frames);

/* Drain up to max_frames from host-side queue into libretro. Returns frames drained.
 * If max_frames == 0, drains everything available.
 */
extern uint32_t wasm96_audio_drain(uint32_t max_frames);

/* Input */
extern uint32_t wasm96_joypad_button_pressed(uint32_t port, uint32_t button);
extern uint32_t wasm96_key_pressed(uint32_t key);

extern int32_t  wasm96_mouse_x(void);
extern int32_t  wasm96_mouse_y(void);
extern uint32_t wasm96_mouse_buttons(void);

extern int32_t  wasm96_lightgun_x(uint32_t port);
extern int32_t  wasm96_lightgun_y(uint32_t port);
extern uint32_t wasm96_lightgun_buttons(uint32_t port);

/* =========================
 * Guest exports (host -> guest)
 * =========================
 *
 * Your module should export these symbols so the host can call them.
 * How you export depends on your toolchain (attributes, linker flags, etc).
 *
 * This header only declares the functions; it does not apply toolchain-specific attributes.
 */

void wasm96_frame(void);

/* Optional lifecycle exports */
void wasm96_init(void);
void wasm96_deinit(void);
void wasm96_reset(void);

/* =========================
 * Convenience helpers
 * ========================= */

static inline int wasm96_abi_compatible(void) {
    return wasm96_abi_version() == (uint32_t)WASM96_ABI_VERSION;
}

/* Convert a guest linear-memory offset into a C pointer.
 * In a wasm32 guest, linear memory starts at address 0, so this cast is typical.
 */
static inline void* wasm96_ptr(uint32_t offset) {
    return (void*)(uintptr_t)offset;
}

static inline const void* wasm96_ptr_const(uint32_t offset) {
    return (const void*)(uintptr_t)offset;
}

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* WASM96_H_INCLUDED */