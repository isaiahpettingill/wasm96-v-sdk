//! wasm96-core: a libretro core that loads and runs a guest WASM/WAT module.
//!
//! This crate implements a **buffer-based ABI**:
//! - The guest requests a framebuffer and an audio ringbuffer (both live in guest memory).
//! - The guest writes into those buffers.
//! - The guest calls host imports to present video / commit+drain audio.
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

        // Allocation strategy:
        // - Host import closures call `crate::state::call_guest_alloc/free`.
        // - Those functions are wired after instantiation by resolving guest exports and storing
        //   them in global state (`state::set_guest_allocators`).

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

                // --- Allocation helpers ---
                //
                // These are currently stubbed out so the core builds cleanly.
                // The intended design is that the *core* owns framebuffer/audio allocations,
                // not the guest. That requires a handle-based API (not raw guest pointers).
                //
                // For now, allocation requests always fail (return 0) and free is a no-op.
                abi::host_imports::ALLOC => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, _size: u32, _align: u32| -> u32 { 0 }
                ),

                abi::host_imports::FREE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, _ptr: u32, _size: u32, _align: u32| { }
                ),

                // --- Video (buffer-based) ---

                // Guest requests a framebuffer for (w,h,fmt).
                //
                // NOTE: This currently returns 0 to indicate failure because the core is being
                // migrated to a host-owned allocation model. Host-owned allocations should be
                // exposed to the guest via handles + copy APIs (not raw guest pointers).
                abi::host_imports::VIDEO_REQUEST => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, _width: u32, _height: u32, _pixel_format: u32| -> u32 {
                        0
                    }
                ),

                // Present framebuffer to libretro.
                abi::host_imports::VIDEO_PRESENT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>| {
                        let _ = av::video_present(&env);
                    }
                ),

                // Get pitch in bytes.
                abi::host_imports::VIDEO_PITCH => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { av::video_pitch() }
                ),

                // --- Audio (ringbuffer-based) ---

                // Guest requests an audio ringbuffer.
                //
                // NOTE: This currently returns 0 to indicate failure because the core is being
                // migrated to a host-owned allocation model (handle-based).
                abi::host_imports::AUDIO_REQUEST => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, _sample_rate: u32, _channels: u32, _capacity_frames: u32| -> u32 {
                        0
                    }
                ),

                abi::host_imports::AUDIO_CAPACITY_FRAMES => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { av::audio_capacity_frames() }
                ),

                abi::host_imports::AUDIO_WRITE_INDEX => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { av::audio_write_index_frames() }
                ),

                abi::host_imports::AUDIO_READ_INDEX => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u32 { av::audio_read_index_frames() }
                ),

                abi::host_imports::AUDIO_COMMIT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, write_index_frames: u32| {
                        av::audio_commit(write_index_frames);
                    }
                ),

                abi::host_imports::AUDIO_DRAIN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, max_frames: u32| -> u32 {
                        av::audio_drain(&env, max_frames).unwrap_or(0)
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

        // Resolve guest allocator exports and store into global state (used by import closures).
        //
        // IMPORTANT: do not store these on `self` to avoid borrow conflicts during instantiation.
        let guest_alloc = instance
            .exports
            .get_function(abi::host_imports::ALLOC)
            .ok()
            .cloned();
        let guest_free = instance
            .exports
            .get_function(abi::host_imports::FREE)
            .ok()
            .cloned();

        state::set_guest_allocators(guest_alloc, guest_free);

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
