//! wasm96-core: a libretro core that loads and runs a guest WASM/WAT module.
//!
//! This crate implements an **Immediate Mode ABI**:
//! - The host owns the framebuffer and handles rendering.
//! - The guest issues drawing commands.
//! - The guest exports `setup`, and may export `update`/`draw`.
//! - WASI-style guests are supported: if `draw` is missing, `_start` or `main` may be used.
//!
//! The ABI surface is defined in `crate::abi` and mirrored by `wasm96-sdk`.
//!
//! Runtime backend: Wasmtime (see `crate::runtime`).

mod abi;
mod av;
mod input;
mod loader;
mod runtime;
mod state;

use libretro_backend::{Core, CoreInfo, RuntimeHandle, libretro_core};

use crate::abi::GuestEntrypoints;

/// The libretro core instance.
pub struct Wasm96Core {
    rt: Option<runtime::WasmtimeRuntime>,
    module: Option<wasmtime::Module>,
    instance: Option<wasmtime::Instance>,
    entrypoints: Option<GuestEntrypoints>,
    game_data: Option<libretro_backend::GameData>,
}

impl Default for Wasm96Core {
    fn default() -> Self {
        Self {
            rt: None,
            module: None,
            instance: None,
            entrypoints: None,
            game_data: None,
        }
    }
}

impl Wasm96Core {
    fn ensure_runtime(&mut self) -> Result<(), ()> {
        if self.rt.is_some() {
            return Ok(());
        }

        let mut rt = runtime::WasmtimeRuntime::new().map_err(|_| ())?;
        rt.define_imports().map_err(|_| ())?;
        self.rt = Some(rt);
        Ok(())
    }

    fn instantiate(&mut self) -> Result<(), ()> {
        self.ensure_runtime()?;
        let rt = self.rt.as_mut().ok_or(())?;
        let module = self.module.as_ref().ok_or(())?;

        let (instance, entrypoints) = rt.instantiate(module).map_err(|_| ())?;
        self.instance = Some(instance);
        self.entrypoints = Some(entrypoints);

        Ok(())
    }

    fn call_guest_setup(&mut self) {
        let Some(rt) = self.rt.as_mut() else { return };
        let Some(entry) = &self.entrypoints else {
            return;
        };

        // Wasmtime's `Func::call` requires an output buffer even if there are no returns.
        let mut results: [wasmtime::Val; 0] = [];
        let _ = entry.setup.call(&mut rt.store, &[], &mut results);
    }

    fn call_guest_update(&mut self) {
        let Some(rt) = self.rt.as_mut() else { return };
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let Some(update) = &entry.update else { return };

        let mut results: [wasmtime::Val; 0] = [];
        let _ = update.call(&mut rt.store, &[], &mut results);
    }

    fn call_guest_draw(&mut self) {
        let Some(rt) = self.rt.as_mut() else { return };
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let Some(draw) = &entry.draw else { return };

        let mut results: [wasmtime::Val; 0] = [];
        let _ = draw.call(&mut rt.store, &[], &mut results);
    }

    fn clear_guest(&mut self) {
        self.module = None;
        self.instance = None;
        self.entrypoints = None;
        // Keep `rt` allocated so subsequent loads are faster; it’s safe because imports are pure host fns.
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

        // Ensure runtime exists so we have an Engine to compile against.
        if self.ensure_runtime().is_err() {
            state::clear_on_unload();
            return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
        }

        // Copy game bytes out of `GameData` so we don't hold an immutable borrow of `self.game_data`
        // across calls that mutably borrow `self` (and so the slice doesn't outlive any temporary borrow).
        let data: Vec<u8> = match self.game_data.as_ref().and_then(|g| g.data()) {
            Some(d) => d.to_vec(),
            None => {
                return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
            }
        };

        let rt = self.rt.as_ref().unwrap();

        // Compile module (WASM or WAT) using Wasmtime Engine.
        let module = match loader::compile_module(&rt.engine, &data) {
            Ok(m) => m,
            Err(_) => {
                return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
            }
        };

        self.module = Some(module);

        // Instantiate module + resolve entrypoints/memory.
        if self.instantiate().is_err() {
            state::clear_on_unload();
            self.clear_guest();
            return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
        }

        // Call setup
        self.call_guest_setup();

        // Return default AV info.
        let av_info = libretro_backend::AudioVideoInfo::new()
            .video(320, 240, 60.0, libretro_backend::PixelFormat::ARGB8888)
            .audio(44100.0)
            .region(libretro_backend::Region::NTSC);

        libretro_backend::LoadGameResult::Success(av_info)
    }

    fn on_unload_game(&mut self) -> libretro_backend::GameData {
        self.clear_guest();
        state::clear_on_unload();
        self.game_data.take().unwrap()
    }

    fn on_run(&mut self, handle: &mut RuntimeHandle) {
        // Update global handle pointer first.
        state::set_runtime_handle(handle);

        // Snapshot inputs once per frame for determinism.
        input::snapshot_per_frame();

        // Run guest update loop.
        self.call_guest_update();

        // Run guest draw loop.
        self.call_guest_draw();

        // Present video and drain audio.
        av::video_present_host();
        av::audio_drain_host(0);
    }

    fn on_reset(&mut self) {
        self.call_guest_setup();
    }
}

libretro_core!(Wasm96Core);
