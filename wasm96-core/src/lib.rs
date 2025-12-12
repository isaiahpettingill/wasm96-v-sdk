//! wasm96-core: a libretro core that loads and runs a guest WASM/WAT module.
//!
//! This crate implements an **upload-based ABI**:
//! - The guest owns its allocations in WASM linear memory.
//! - The host owns its video/audio buffers in system memory.
//! - The guest uploads full-frame video and pushes audio samples by passing pointers into
//!   guest linear memory; the host copies into system memory and presents/drains to libretro.
//!
//! Required guest export:
//! - `wasm96_frame()`
//!
//! Optional guest exports:
//! - `wasm96_init()`
//! - `wasm96_deinit()`
//! - `wasm96_reset()`
//!
//! The ABI surface is defined in `crate::abi` and mirrored by `wasm96-sdk`.

mod abi;
mod av;
mod input;
mod loader;
mod state;

use crate::abi::{ABI_VERSION, GuestEntrypoints, IMPORT_MODULE};
use libretro_backend::{Core, CoreInfo, RuntimeHandle, libretro_core};
use wasmer::{FunctionEnv, FunctionEnvMut, Imports, Store};

/// The libretro core instance.
pub struct Wasm96Core {
    store: Store,
    module: Option<wasmer::Module>,
    instance: Option<wasmer::Instance>,
    entrypoints: Option<GuestEntrypoints>,
    env: Option<FunctionEnv<()>>,
    game_data: Option<libretro_backend::GameData>,
}

impl Default for Wasm96Core {
    fn default() -> Self {
        Self {
            store: Store::default(),
            module: None,
            instance: None,
            entrypoints: None,
            env: None,
            game_data: None,
        }
    }
}

impl Wasm96Core {
    fn build_imports(&mut self) -> Imports {
        // Wasmer needs an env to pass to host functions that read guest memory views.
        self.env = Some(FunctionEnv::new(&mut self.store, ()));
        let env = self.env.as_ref().unwrap().clone();

        // Note: all imports are under module `env` (see abi::IMPORT_MODULE),
        // because wasm32 targets typically expect `"env"` for imports.
        wasmer::imports! {
            IMPORT_MODULE => {
                // ABI version
                abi::host_imports::ABI_VERSION => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { ABI_VERSION }
                ),

                // --- Video (upload-based) ---

                // Configure host-side system-memory framebuffer spec.
                abi::host_imports::VIDEO_CONFIG => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, width: u32, height: u32, pixel_format: u32| -> u32 {
                        av::video_config(width, height, pixel_format) as u32
                    }
                ),

                // Upload full frame from guest linear memory into host-side system-memory buffer.
                abi::host_imports::VIDEO_UPLOAD => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, src_ptr: u32, byte_len: u32, pitch_bytes: u32| -> u32 {
                        av::video_upload(&env, src_ptr, byte_len, pitch_bytes).unwrap_or(false) as u32
                    }
                ),

                // Present last uploaded host-side framebuffer to libretro.
                abi::host_imports::VIDEO_PRESENT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| {
                        av::video_present_host();
                    }
                ),

                // --- Audio (upload-based) ---

                // Configure host-side audio format.
                abi::host_imports::AUDIO_CONFIG => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, sample_rate: u32, channels: u32| -> u32 {
                        av::audio_config(sample_rate, channels).unwrap_or(false) as u32
                    }
                ),

                // Push interleaved i16 frames from guest linear memory into host-side buffer.
                abi::host_imports::AUDIO_PUSH_I16 => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, src_ptr: u32, frames: u32| -> u32 {
                        av::audio_push_i16(&env, src_ptr, frames).unwrap_or(0)
                    }
                ),

                // Drain host-side audio buffer into libretro. Returns frames drained.
                abi::host_imports::AUDIO_DRAIN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, max_frames: u32| -> u32 {
                        av::audio_drain_host(max_frames)
                    }
                ),

                // --- Input ---

                abi::host_imports::JOYPAD_BUTTON_PRESSED => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: u32, button: u32| -> u32 {
                        input::joypad_button_pressed(port, button)
                    }
                ),

                abi::host_imports::KEY_PRESSED => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, key: u32| -> u32 {
                        input::key_pressed(key)
                    }
                ),

                abi::host_imports::MOUSE_X => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> i32 { input::mouse_x() }
                ),

                abi::host_imports::MOUSE_Y => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> i32 { input::mouse_y() }
                ),

                abi::host_imports::MOUSE_BUTTONS => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { input::mouse_buttons() }
                ),

                abi::host_imports::LIGHTGUN_X => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: u32| -> i32 { input::lightgun_x(port) }
                ),

                abi::host_imports::LIGHTGUN_Y => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: u32| -> i32 { input::lightgun_y(port) }
                ),

                abi::host_imports::LIGHTGUN_BUTTONS => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: u32| -> u32 { input::lightgun_buttons(port) }
                ),
            }
        }
    }

    fn instantiate(&mut self) -> Result<(), ()> {
        // Take ownership of the module temporarily to avoid holding an immutable borrow
        // across `self.build_imports()` (which needs `&mut self`).
        let module = self.module.take().ok_or(())?;

        // Install imports and instantiate.
        let imports = self.build_imports();
        let instance = wasmer::Instance::new(&mut self.store, &module, &imports).map_err(|_| ())?;

        // Put the module back now that instantiation succeeded.
        self.module = Some(module);

        // Validate required exports + resolve entrypoints.
        abi::validate::required_exports_present(&instance).map_err(|_| ())?;
        let entrypoints = GuestEntrypoints::resolve(&instance).map_err(|_| ())?;

        // Register exported memory in global state.
        let mem = instance.exports.get_memory("memory").map_err(|_| ())?;
        state::set_guest_memory(mem);

        // Upload-based ABI: the host does not allocate inside guest memory.
        // The guest manages its own allocations; the host only reads guest memory
        // when the guest uploads/pushes buffers.

        // Store instance/entrypoints.
        self.instance = Some(instance);
        self.entrypoints = Some(entrypoints);

        Ok(())
    }

    fn call_guest_init_if_present(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        if let Some(init) = &entry.init {
            let _ = init.call(&mut self.store, &[]);
        }
    }

    fn call_guest_deinit_if_present(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        if let Some(deinit) = &entry.deinit {
            let _ = deinit.call(&mut self.store, &[]);
        }
    }

    fn call_guest_reset_if_present(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        if let Some(reset) = &entry.reset {
            let _ = reset.call(&mut self.store, &[]);
        }
    }

    fn call_guest_frame(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let _ = entry.frame.call(&mut self.store, &[]);
    }
}

impl Core for Wasm96Core {
    fn save_memory(&mut self) -> Option<&mut [u8]> {
        None
    }

    fn rtc_memory(&mut self) -> Option<&mut [u8]> {
        None
    }

    fn system_memory(&mut self) -> Option<&mut [u8]> {
        None
    }

    fn video_memory(&mut self) -> Option<&mut [u8]> {
        None
    }

    fn info() -> CoreInfo {
        CoreInfo::new("Wasm96", "1.0.0")
            .supports_roms_with_extension("wasm")
            .supports_roms_with_extension("wat")
    }

    fn on_load_game(
        &mut self,
        game_data: libretro_backend::GameData,
    ) -> libretro_backend::LoadGameResult {
        self.game_data = Some(game_data);

        let data = match self.game_data.as_ref().unwrap().data() {
            Some(d) => d,
            None => {
                return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
            }
        };

        // Compile module (WASM or WAT).
        let module = match loader::compile_module(&self.store, data) {
            Ok(m) => m,
            Err(_) => {
                return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
            }
        };

        self.module = Some(module);

        // Instantiate module + resolve entrypoints/memory.
        if self.instantiate().is_err() {
            state::clear_on_unload();
            self.module = None;
            self.instance = None;
            self.entrypoints = None;
            self.env = None;
            return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
        }

        // Call optional guest init hook.
        self.call_guest_init_if_present();

        // For now we return default AV info. The guest controls the actual buffer size via ABI calls.
        libretro_backend::LoadGameResult::Success(libretro_backend::AudioVideoInfo::new())
    }

    fn on_unload_game(&mut self) -> libretro_backend::GameData {
        // Call optional guest deinit hook.
        self.call_guest_deinit_if_present();

        self.module = None;
        self.instance = None;
        self.entrypoints = None;
        self.env = None;

        state::clear_on_unload();

        self.game_data.take().unwrap()
    }

    fn on_run(&mut self, handle: &mut RuntimeHandle) {
        // Update global handle pointer first.
        state::set_runtime_handle(handle);

        // Snapshot inputs once per frame for determinism (currently mostly defaults).
        input::snapshot_per_frame();

        // Run guest frame.
        self.call_guest_frame();
    }

    fn on_reset(&mut self) {
        self.call_guest_reset_if_present();
    }
}

libretro_core!(Wasm96Core);
