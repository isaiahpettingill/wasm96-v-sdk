// Package wasm96 provides a handwritten guest SDK for the wasm96 libretro core.
//
// This package is intended to be compiled to WebAssembly (wasm32) as a *guest* module.
// The wasm96 host provides a small, C-like import surface under module name "env".
//
// ABI model (upload-based):
// - Guest owns allocations in WASM linear memory.
// - Host owns video/audio buffers in system memory.
// - Guest performs write-only uploads/pushes from guest memory into the host.
//
// Import symbols (provided by host):
//
//	wasm96_abi_version
//	wasm96_video_config / wasm96_video_upload / wasm96_video_present
//	wasm96_audio_config / wasm96_audio_push_i16 / wasm96_audio_drain
//	wasm96_joypad_button_pressed / wasm96_key_pressed / wasm96_mouse_x / wasm96_mouse_y / wasm96_mouse_buttons
//	wasm96_lightgun_x / wasm96_lightgun_y / wasm96_lightgun_buttons
//
// Required guest export (implemented by you in your guest module):
//
//	func wasm96_frame()
//
// Optional guest exports:
//
//	func wasm96_init()
//	func wasm96_deinit()
//	func wasm96_reset()
//
// Notes:
// - "Pointers" are u32 offsets into the guest linear memory.
// - This package intentionally does NOT attempt to provide any allocator hooks.
package wasm96

// ABI_VERSION must match the host/core ABI version.
const ABI_VERSION uint32 = 1

// PixelFormat is the pixel format enum used by the upload-based video ABI
// (wasm96_video_config / wasm96_video_upload).
type PixelFormat uint32

const (
	PixelFormatXRGB8888 PixelFormat = 0
	PixelFormatRGB565   PixelFormat = 1
)

func (pf PixelFormat) BytesPerPixel() uint32 {
	switch pf {
	case PixelFormatXRGB8888:
		return 4
	case PixelFormatRGB565:
		return 2
	default:
		return 0
	}
}

// JoypadButton is aligned with common libretro joypad ids.
type JoypadButton uint32

const (
	JoypadB      JoypadButton = 0
	JoypadY      JoypadButton = 1
	JoypadSelect JoypadButton = 2
	JoypadStart  JoypadButton = 3
	JoypadUp     JoypadButton = 4
	JoypadDown   JoypadButton = 5
	JoypadLeft   JoypadButton = 6
	JoypadRight  JoypadButton = 7
	JoypadA      JoypadButton = 8
	JoypadX      JoypadButton = 9
	JoypadL1     JoypadButton = 10
	JoypadR1     JoypadButton = 11
	JoypadL2     JoypadButton = 12
	JoypadR2     JoypadButton = 13
	JoypadL3     JoypadButton = 14
	JoypadR3     JoypadButton = 15
)

// Mouse button bitmask (returned by MouseButtons()).
const (
	MouseButtonLeft   uint32 = 1 << 0
	MouseButtonRight  uint32 = 1 << 1
	MouseButtonMiddle uint32 = 1 << 2
	MouseButton4      uint32 = 1 << 3
	MouseButton5      uint32 = 1 << 4
)

// Lightgun button bitmask (returned by LightgunButtons()).
const (
	LightgunButtonTrigger   uint32 = 1 << 0
	LightgunButtonReload    uint32 = 1 << 1
	LightgunButtonStart     uint32 = 1 << 2
	LightgunButtonSelect    uint32 = 1 << 3
	LightgunButtonAuxA      uint32 = 1 << 4
	LightgunButtonAuxB      uint32 = 1 << 5
	LightgunButtonAuxC      uint32 = 1 << 6
	LightgunButtonOffscreen uint32 = 1 << 7
)

// --------------------
// Raw imports (sys)
// --------------------
//
// In Go, "imports" for WASM are toolchain/runtime specific.
// The most portable approach is to declare stubs and have your build/runtime wire them.
//
// If you are using TinyGo, you can replace these with proper imports, for example:
//
//   //go:wasmimport env wasm96_abi_version
//   func wasm96_abi_version() uint32
//
// The declarations below are intentionally regular Go function declarations so this file
// remains usable across different toolchains; however, they will fail to link/run unless
// your environment provides these symbols.
//
// If you're using standard Go (not TinyGo), you likely need a WASM host that can map these
// names, or you will need to adjust this file to your host integration.

func wasm96_abi_version() uint32

func wasm96_video_config(width uint32, height uint32, pixelFormat uint32) uint32
func wasm96_video_upload(ptr uint32, byteLen uint32, pitchBytes uint32) uint32
func wasm96_video_present()

func wasm96_audio_config(sampleRate uint32, channels uint32) uint32
func wasm96_audio_push_i16(ptr uint32, frames uint32) uint32
func wasm96_audio_drain(maxFrames uint32) uint32

func wasm96_joypad_button_pressed(port uint32, button uint32) uint32
func wasm96_key_pressed(key uint32) uint32

func wasm96_mouse_x() int32
func wasm96_mouse_y() int32
func wasm96_mouse_buttons() uint32

func wasm96_lightgun_x(port uint32) int32
func wasm96_lightgun_y(port uint32) int32
func wasm96_lightgun_buttons(port uint32) uint32

// --------------------
// ABI helpers
// --------------------

// HostABIVersion returns the ABI version reported by the host.
func HostABIVersion() uint32 { return wasm96_abi_version() }

// Compatible reports whether the host ABI matches this SDK.
func Compatible() bool { return HostABIVersion() == ABI_VERSION }

// --------------------
// Video
// --------------------

// VideoConfig configures the host-side framebuffer spec.
// Returns true on success.
func VideoConfig(width, height uint32, format PixelFormat) bool {
	return wasm96_video_config(width, height, uint32(format)) != 0
}

// VideoPitchBytes computes the pitch for the given width/format.
// (Host uses the same policy: pitch = width * bytesPerPixel)
func VideoPitchBytes(width uint32, format PixelFormat) uint32 {
	return width * format.BytesPerPixel()
}

// VideoByteLen computes full-frame byte length.
func VideoByteLen(width, height uint32, format PixelFormat) uint32 {
	return height * VideoPitchBytes(width, format)
}

// VideoUpload uploads a full frame from guest linear memory into the host.
// ptr is a u32 offset into guest linear memory.
//
// Returns true on success.
func VideoUpload(ptr uint32, width, height uint32, format PixelFormat) bool {
	pitch := VideoPitchBytes(width, format)
	byteLen := height * pitch
	return wasm96_video_upload(ptr, byteLen, pitch) != 0
}

// Present presents the last uploaded framebuffer to the host.
func Present() { wasm96_video_present() }

// --------------------
// Audio
// --------------------

// AudioConfig configures host-side audio format.
// Returns true on success.
func AudioConfig(sampleRate, channels uint32) bool {
	return wasm96_audio_config(sampleRate, channels) != 0
}

// AudioPushI16 pushes interleaved i16 audio frames from guest linear memory into the host.
// ptr is a u32 offset into guest linear memory pointing to frames*channels i16 samples.
//
// Returns frames accepted (0 on failure).
func AudioPushI16(ptr uint32, frames uint32) uint32 {
	return wasm96_audio_push_i16(ptr, frames)
}

// AudioDrain asks the host to drain up to maxFrames from its internal queue.
// If maxFrames==0, the host drains everything it currently has queued.
// Returns drained frames.
func AudioDrain(maxFrames uint32) uint32 { return wasm96_audio_drain(maxFrames) }

// --------------------
// Input
// --------------------

// JoypadPressed returns true if the given joypad button is pressed for the port.
func JoypadPressed(port uint32, button JoypadButton) bool {
	return wasm96_joypad_button_pressed(port, uint32(button)) != 0
}

// KeyPressed returns true if the given key code is pressed.
// Key codes are implementation-defined; recommend using libretro key ids or USB HID.
func KeyPressed(key uint32) bool {
	return wasm96_key_pressed(key) != 0
}

// MouseX returns the mouse X coordinate.
func MouseX() int32 { return wasm96_mouse_x() }

// MouseY returns the mouse Y coordinate.
func MouseY() int32 { return wasm96_mouse_y() }

// MouseButtons returns a bitmask of mouse buttons pressed.
func MouseButtons() uint32 { return wasm96_mouse_buttons() }

// LightgunX returns the lightgun X coordinate for the port.
func LightgunX(port uint32) int32 { return wasm96_lightgun_x(port) }

// LightgunY returns the lightgun Y coordinate for the port.
func LightgunY(port uint32) int32 { return wasm96_lightgun_y(port) }

// LightgunButtons returns a bitmask of lightgun buttons pressed for the port.
func LightgunButtons(port uint32) uint32 { return wasm96_lightgun_buttons(port) }

// (No allocation helpers in the upload-based ABI; guest owns its own allocation strategy.)
