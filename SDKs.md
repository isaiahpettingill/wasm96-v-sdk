```wasm96 SDKs

This repo contains **handwritten**, **codegen-free** guest SDKs for the `wasm96` libretro core.

They all target the same C-like WebAssembly import surface (module `"env"`).

ABI model (upload-based)

- Guest owns and manages its own allocations in **WASM linear memory**.
- Host owns and manages video/audio buffers in **system memory**.
- Guest performs **write-only** uploads/pushes from guest memory to host buffers.

Host exports imports under module name `"env"` with symbol names like:

- ABI:
  - `wasm96_abi_version`

- Video (full-frame upload):
  - `wasm96_video_config`
  - `wasm96_video_upload`
  - `wasm96_video_present`

- Audio (push samples, interleaved i16):
  - `wasm96_audio_config`
  - `wasm96_audio_push_i16`
  - `wasm96_audio_drain`

- Input:
  - `wasm96_joypad_button_pressed`, `wasm96_key_pressed`, `wasm96_mouse_x`, `wasm96_mouse_y`,
    `wasm96_mouse_buttons`, `wasm96_lightgun_x`, `wasm96_lightgun_y`, `wasm96_lightgun_buttons`

Guest must export (host calls):

- `wasm96_frame()` (required)
- `wasm96_init()`, `wasm96_deinit()`, `wasm96_reset()` (optional)

Important ABI notes

- “Pointers” passed **to the host** (e.g. video/audio upload pointers) are **u32 offsets into guest linear memory** (WASM32). Treat them as raw/unsafe.
- `wasm96_video_config` / `wasm96_video_upload` / `wasm96_audio_config` may return **0** on failure. Handle this path.
- ABI version in these SDKs is `1`. You should check `wasm96_abi_version()` and refuse to run if it doesn’t match.

SDK list

Rust (guest SDK)
- Path: `wasm96-sdk/`
- What: Rust crate providing a handwritten guest SDK (no WIT).
- Entry point:
  - `wasm96_sdk::prelude::*`
- Notes: Mirrors the core ABI with safe-ish wrappers and raw `extern "C"` bindings.

C
- Path: `sdk-c/`
- Files:
  - `sdk-c/include/wasm96.h`
  - `sdk-c/README.md`
- What: C header defining imports, ABI enums/constants, and minimal helpers.
- Best for: C, C-compatible languages, or building your own bindings in other toolchains.

C++
- Path: `sdk-cpp/`
- Files:
  - `sdk-cpp/include/wasm96.hpp`
  - `sdk-cpp/README.md`
- What: Header-only C++ wrapper with typed enums and helper structs.
- Toolchain note: Uses Clang wasm import attributes when compiling with Clang; other toolchains may require adapting import declarations.

Zig
- Path: `sdk-zig/`
- Files:
  - `sdk-zig/wasm96.zig`
  - `sdk-zig/README.md`
  - `sdk-zig/build.zig.zon`
- What: Zig module exposing raw `extern "env"` imports (`sys`) plus wrapper APIs (`video`, `audio`, `input`, `abi`).
- Best for: Zig wasm32 guest projects that want a thin, typed layer.

D
- Path: `sdk-d/`
- Files:
  - `sdk-d/source/wasm96/package.d`
  - `sdk-d/README.md`
  - `sdk-d/dub.json`
- What: D package module `wasm96` exposing ABI imports via `extern(C)` and `pragma(mangle, ...)` plus simple wrappers.

Go
- Path: `sdk-go/`
- Files:
  - `sdk-go/wasm96/wasm96.go`
  - `sdk-go/README.md`
  - `sdk-go/go.mod`
- What: Go package `wasm96` with typed wrappers and unsafe memory views.
- Toolchain note: WebAssembly import wiring is toolchain-specific:
  - TinyGo users may want `//go:wasmimport env <name>` annotations.
  - Standard Go users may need to adapt the import declarations / build pipeline so `env.wasm96_*` symbols resolve.

AssemblyScript
- Path: `sdk-assemblyscript/`
- Files:
  - `sdk-assemblyscript/assembly/index.ts`
  - `sdk-assemblyscript/README.md`
  - `sdk-assemblyscript/package.json`
  - `sdk-assemblyscript/asconfig.json`
  - `sdk-assemblyscript/tsconfig.json`
- What: AssemblyScript module using `@external("env", "...")` imports and thin wrapper classes for video/audio.
- Best for: TypeScript-like guest development targeting wasm32.

Choosing an SDK

- If you want maximum portability: start with the C header (`sdk-c/include/wasm96.h`) and build up.
- If you want immediate ergonomics: use Rust (`wasm96-sdk`) or Zig (`sdk-zig`).
- If you already have an ecosystem preference: C++, D, Go, and AssemblyScript SDKs are provided as thin wrappers with minimal assumptions.

Versioning

All SDKs assume ABI version `1`. If `wasm96-core` bumps its ABI version, these SDKs must be updated in lockstep.

Contributing

If you add new ABI imports to the core, please update:
- `wasm96-core/src/abi/mod.rs` (canonical ABI documentation)
- the C header (`sdk-c/include/wasm96.h`)
- each language SDK wrapper so they remain aligned
- this file (`SDKs.md`) if a new SDK is added or paths change
