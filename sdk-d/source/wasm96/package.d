module wasm96;

///
/// wasm96 guest SDK (D)
///
/// This module targets the wasm96 guest ABI exposed by the host under import module `"env"`.
/// It is intentionally C-like and stable.
///
/// Host provides these imports (module `"env"`):
///   wasm96_abi_version
///   wasm96_video_config / wasm96_video_upload / wasm96_video_present
///   wasm96_audio_config / wasm96_audio_push_i16 / wasm96_audio_drain
///   wasm96_joypad_button_pressed / wasm96_key_pressed
///   wasm96_mouse_x / wasm96_mouse_y / wasm96_mouse_buttons
///   wasm96_lightgun_x / wasm96_lightgun_y / wasm96_lightgun_buttons
///
/// Required guest export (host calls):
///   extern(C) void wasm96_frame();
///
/// Optional guest exports:
///   extern(C) void wasm96_init();
///   extern(C) void wasm96_deinit();
///   extern(C) void wasm96_reset();
///
/// Notes:
/// - All pointers are 32-bit offsets into the guest's linear memory (WASM32).
/// - Some hosts may currently stub out allocation/buffer-request APIs (returning 0). Handle failures.
/// - This file avoids WIT/component-model dependencies.
///

import core.stdc.stdint : uint32_t, int32_t;
import core.stdc.stddef : size_t;
import core.stdc.string : memset;

enum uint32_t ABI_VERSION = 1;

/// Pixel formats for the framebuffer request ABI.
/// Keep numeric values stable; they are part of the ABI.
enum PixelFormat : uint32_t
{
    Xrgb8888 = 0,
    Rgb565   = 1,
}

uint32_t bytesPerPixel(PixelFormat fmt) @nogc nothrow @safe
{
    final switch (fmt)
    {
        case PixelFormat.Xrgb8888: return 4;
        case PixelFormat.Rgb565:   return 2;
    }
}

/// Joypad button ids (aligned with libretro joypad ids).
enum JoypadButton : uint32_t
{
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
}

/// Mouse button bitmask values returned by `wasm96_mouse_buttons`.
enum uint32_t MouseButtonsLeft   = 1u << 0;
enum uint32_t MouseButtonsRight  = 1u << 1;
enum uint32_t MouseButtonsMiddle = 1u << 2;
enum uint32_t MouseButtonsButton4 = 1u << 3;
enum uint32_t MouseButtonsButton5 = 1u << 4;

/// Lightgun button bitmask values returned by `wasm96_lightgun_buttons(port)`.
enum uint32_t LightgunButtonsTrigger   = 1u << 0;
enum uint32_t LightgunButtonsReload    = 1u << 1;
enum uint32_t LightgunButtonsStart     = 1u << 2;
enum uint32_t LightgunButtonsSelect    = 1u << 3;
enum uint32_t LightgunButtonsAuxA      = 1u << 4;
enum uint32_t LightgunButtonsAuxB      = 1u << 5;
enum uint32_t LightgunButtonsAuxC      = 1u << 6;
enum uint32_t LightgunButtonsOffscreen = 1u << 7;

/// Low-level extern declarations for the host ABI.
/// These are imported from the WASM import module `"env"`.
extern (C) @nogc nothrow
{
    // ABI
    pragma(mangle, "wasm96_abi_version")
    uint32_t wasm96_abi_version();

    // Video (upload-based)
    pragma(mangle, "wasm96_video_config")
    uint32_t wasm96_video_config(uint32_t width, uint32_t height, uint32_t pixel_format);

    pragma(mangle, "wasm96_video_upload")
    uint32_t wasm96_video_upload(uint32_t ptr, uint32_t byte_len, uint32_t pitch_bytes);

    pragma(mangle, "wasm96_video_present")
    void wasm96_video_present();

    // Audio (push-based, interleaved i16)
    pragma(mangle, "wasm96_audio_config")
    uint32_t wasm96_audio_config(uint32_t sampleRate, uint32_t channels);

    pragma(mangle, "wasm96_audio_push_i16")
    uint32_t wasm96_audio_push_i16(uint32_t ptr, uint32_t frames);

    pragma(mangle, "wasm96_audio_drain")
    uint32_t wasm96_audio_drain(uint32_t maxFrames);

    // Input
    pragma(mangle, "wasm96_joypad_button_pressed")
    uint32_t wasm96_joypad_button_pressed(uint32_t port, uint32_t button);

    pragma(mangle, "wasm96_key_pressed")
    uint32_t wasm96_key_pressed(uint32_t key);

    pragma(mangle, "wasm96_mouse_x")
    int32_t wasm96_mouse_x();

    pragma(mangle, "wasm96_mouse_y")
    int32_t wasm96_mouse_y();

    pragma(mangle, "wasm96_mouse_buttons")
    uint32_t wasm96_mouse_buttons();

    pragma(mangle, "wasm96_lightgun_x")
    int32_t wasm96_lightgun_x(uint32_t port);

    pragma(mangle, "wasm96_lightgun_y")
    int32_t wasm96_lightgun_y(uint32_t port);

    pragma(mangle, "wasm96_lightgun_buttons")
    uint32_t wasm96_lightgun_buttons(uint32_t port);
}

/// ABI helpers.
bool abiCompatible() @nogc nothrow @safe
{
    return wasm96_abi_version() == ABI_VERSION;
}

/// Convert a guest linear-memory offset into a typed pointer.
///
/// In wasm32 guests, linear memory starts at address 0, so this is a common pattern.
/// Still treat the result as unsafe unless you know it points to a valid region.
T* ptrFromOffset(T)(uint32_t offset) @nogc nothrow @trusted
{
    return cast(T*)cast(size_t)offset;
}

/// Video helpers (upload-based).
///
/// The guest owns its framebuffer in linear memory and uploads a full frame each tick.
struct Framebuffer
{
    uint32_t ptr;        // offset into linear memory (guest-owned)
    uint32_t width;
    uint32_t height;
    uint32_t pitchBytes; // bytes per row in guest buffer
    PixelFormat format;

    bool valid() const @nogc nothrow @safe
    {
        return ptr != 0;
    }

    uint32_t byteLen() const @nogc nothrow @safe
    {
        return height * pitchBytes;
    }

    /// Mutable view of framebuffer bytes.
    ///
    /// Safety: the returned slice must point to valid linear memory.
    ubyte[] bytesMut() @nogc nothrow @trusted
    {
        auto p = cast(ubyte*)cast(size_t)ptr;
        return p[0 .. byteLen()];
    }
}

/// Configure the host-side framebuffer spec.
/// Returns true on success.
bool videoConfig(uint32_t width, uint32_t height, PixelFormat format) @nogc nothrow @safe
{
    return wasm96_video_config(width, height, cast(uint32_t)format) != 0;
}

/// Upload a full frame from guest linear memory into host system memory.
/// Returns true on success.
bool videoUpload(uint32_t ptr, uint32_t byteLen, uint32_t pitchBytes) @nogc nothrow @safe
{
    return wasm96_video_upload(ptr, byteLen, pitchBytes) != 0;
}

void presentFramebuffer() @nogc nothrow @safe
{
    wasm96_video_present();
}

/// Audio helpers (push-based).
///
/// The guest pushes interleaved i16 samples from linear memory to the host.
struct AudioBuffer
{
    uint32_t ptr;        // offset in linear memory (guest-owned)
    uint32_t frames;     // number of frames (one frame = channels samples)
    uint32_t channels;   // expected 2

    bool valid() const @nogc nothrow @safe
    {
        return ptr != 0 && frames != 0 && channels != 0;
    }

    uint32_t sampleCount() const @nogc nothrow @safe
    {
        return frames * channels;
    }

    uint32_t byteLen() const @nogc nothrow @safe
    {
        return sampleCount() * 2u; // i16
    }

    /// Mutable view of interleaved samples.
    ///
    /// Safety: `ptr` must point to valid memory for byteLen() bytes.
    short[] samplesMut() @nogc nothrow @trusted
    {
        auto p = cast(short*)cast(size_t)ptr;
        return p[0 .. sampleCount()];
    }
}

/// Configure host-side audio output format.
/// Returns true on success.
bool audioConfig(uint32_t sampleRate, uint32_t channels) @nogc nothrow @safe
{
    return wasm96_audio_config(sampleRate, channels) != 0;
}

/// Push interleaved i16 frames from guest linear memory into the host queue.
/// Returns number of frames accepted (0 on failure).
uint32_t audioPushI16(uint32_t ptr, uint32_t frames) @nogc nothrow @safe
{
    return wasm96_audio_push_i16(ptr, frames);
}

/// Drain up to `maxFrames` frames from the host-side queue into libretro.
/// `maxFrames == 0` means drain everything available.
/// Returns frames drained.
uint32_t audioDrain(uint32_t maxFrames = 0) @nogc nothrow @safe
{
    return wasm96_audio_drain(maxFrames);
}

/// Input helpers.
bool joypadPressed(uint32_t port, JoypadButton button) @nogc nothrow @safe
{
    return wasm96_joypad_button_pressed(port, cast(uint32_t)button) != 0;
}

bool keyPressed(uint32_t key) @nogc nothrow @safe
{
    return wasm96_key_pressed(key) != 0;
}

int32_t mouseX() @nogc nothrow @safe { return wasm96_mouse_x(); }
int32_t mouseY() @nogc nothrow @safe { return wasm96_mouse_y(); }
uint32_t mouseButtons() @nogc nothrow @safe { return wasm96_mouse_buttons(); }

int32_t lightgunX(uint32_t port) @nogc nothrow @safe { return wasm96_lightgun_x(port); }
int32_t lightgunY(uint32_t port) @nogc nothrow @safe { return wasm96_lightgun_y(port); }
uint32_t lightgunButtons(uint32_t port) @nogc nothrow @safe { return wasm96_lightgun_buttons(port); }

/// Optional: helper to clear a framebuffer region.
/// This is purely a convenience wrapper.
void clearFramebuffer(ref Framebuffer fb, ubyte value = 0) @nogc nothrow @trusted
{
    if (!fb.valid()) return;
    auto bytes = fb.bytesMut();
    memset(bytes.ptr, value, bytes.length);
}
