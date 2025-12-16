//! wasm96-core: a libretro core that loads and runs a guest WASM/WAT module.
//!
//! This crate implements an **Immediate Mode ABI**:
//! - The host owns the framebuffer and handles rendering.
//! - The guest issues drawing commands.
//! - The guest exports `setup`, `update`, and `draw`.
//!
//! The ABI surface is defined in `crate::abi` and mirrored by `wasm96-sdk`.

mod abi;
mod av;
mod input;
mod loader;
mod state;

use crate::abi::{GuestEntrypoints, IMPORT_MODULE};
use crate::state::global;
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
                // --- Graphics ---

                abi::host_imports::GRAPHICS_SET_SIZE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, width: u32, height: u32| {
                        av::graphics_set_size(width, height);
                    }
                ),

                abi::host_imports::GRAPHICS_SET_COLOR => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, r: u32, g: u32, b: u32, a: u32| {
                        av::graphics_set_color(r, g, b, a);
                    }
                ),

                abi::host_imports::GRAPHICS_BACKGROUND => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, r: u32, g: u32, b: u32| {
                        av::graphics_background(r, g, b);
                    }
                ),

                abi::host_imports::GRAPHICS_POINT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32| {
                        av::graphics_point(x, y);
                    }
                ),

                abi::host_imports::GRAPHICS_LINE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x1: i32, y1: i32, x2: i32, y2: i32| {
                        av::graphics_line(x1, y1, x2, y2);
                    }
                ),

                abi::host_imports::GRAPHICS_RECT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_rect(x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_RECT_OUTLINE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_rect_outline(x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_CIRCLE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, r: u32| {
                        av::graphics_circle(x, y, r);
                    }
                ),

                abi::host_imports::GRAPHICS_CIRCLE_OUTLINE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, r: u32| {
                        av::graphics_circle_outline(x, y, r);
                    }
                ),

                abi::host_imports::GRAPHICS_IMAGE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, x: i32, y: i32, w: u32, h: u32, ptr: u32, len: u32| {
                        let _ = av::graphics_image(&env, x, y, w, h, ptr, len);
                    }
                ),

                abi::host_imports::GRAPHICS_TRIANGLE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32| {
                        av::graphics_triangle(x1, y1, x2, y2, x3, y3);
                    }
                ),

                abi::host_imports::GRAPHICS_TRIANGLE_OUTLINE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32| {
                        av::graphics_triangle_outline(x1, y1, x2, y2, x3, y3);
                    }
                ),

                abi::host_imports::GRAPHICS_BEZIER_QUADRATIC => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x1: i32, y1: i32, cx: i32, cy: i32, x2: i32, y2: i32, segments: u32| {
                        av::graphics_bezier_quadratic(x1, y1, cx, cy, x2, y2, segments);
                    }
                ),

                abi::host_imports::GRAPHICS_BEZIER_CUBIC => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x1: i32, y1: i32, cx1: i32, cy1: i32, cx2: i32, cy2: i32, x2: i32, y2: i32, segments: u32| {
                        av::graphics_bezier_cubic(x1, y1, cx1, cy1, cx2, cy2, x2, y2, segments);
                    }
                ),

                abi::host_imports::GRAPHICS_PILL => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_pill(x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_PILL_OUTLINE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_pill_outline(x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_SVG_CREATE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| -> u32 {
                        av::graphics_svg_create(&env, ptr, len)
                    }
                ),

                abi::host_imports::GRAPHICS_SVG_DRAW => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, id: u32, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_svg_draw(id, x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_SVG_DESTROY => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, id: u32| {
                        av::graphics_svg_destroy(id);
                    }
                ),

                abi::host_imports::GRAPHICS_GIF_CREATE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| -> u32 {
                        av::graphics_gif_create(&env, ptr, len)
                    }
                ),

                abi::host_imports::GRAPHICS_GIF_DRAW => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, id: u32, x: i32, y: i32| {
                        av::graphics_gif_draw(id, x, y);
                    }
                ),

                abi::host_imports::GRAPHICS_GIF_DRAW_SCALED => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, id: u32, x: i32, y: i32, w: u32, h: u32| {
                        av::graphics_gif_draw_scaled(id, x, y, w, h);
                    }
                ),

                abi::host_imports::GRAPHICS_GIF_DESTROY => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, id: u32| {
                        av::graphics_gif_destroy(id);
                    }
                ),

                abi::host_imports::GRAPHICS_FONT_UPLOAD_TTF => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| -> u32 {
                        av::graphics_font_upload_ttf(&env, ptr, len)
                    }
                ),

                abi::host_imports::GRAPHICS_FONT_USE_SPLEEN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, size: u32| -> u32 {
                        av::graphics_font_use_spleen(size)
                    }
                ),

                abi::host_imports::GRAPHICS_TEXT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, x: i32, y: i32, font: u32, ptr: u32, len: u32| {
                        av::graphics_text(x, y, font, &env, ptr, len);
                    }
                ),

                abi::host_imports::GRAPHICS_TEXT_MEASURE => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, font: u32, ptr: u32, len: u32| -> u64 {
                        av::graphics_text_measure(font, &env, ptr, len)
                    }
                ),

                // --- Audio ---

                abi::host_imports::AUDIO_INIT => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, sample_rate: u32| -> u32 {
                        av::audio_init(sample_rate)
                    }
                ),

                abi::host_imports::AUDIO_PUSH_SAMPLES => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| {
                        let _ = av::audio_push_samples(&env, ptr, len);
                    }
                ),

                abi::host_imports::AUDIO_PLAY_WAV => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| {
                        let _ = av::audio_play_wav(&env, ptr, len);
                    }
                ),

                abi::host_imports::AUDIO_PLAY_QOA => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| {
                        let _ = av::audio_play_qoa(&env, ptr, len);
                    }
                ),

                abi::host_imports::AUDIO_PLAY_XM => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| {
                        let _ = av::audio_play_xm(&env, ptr, len);
                    }
                ),

                // --- Input ---

                abi::host_imports::INPUT_IS_BUTTON_DOWN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: u32, btn: u32| -> u32 {
                        input::joypad_button_pressed(port, btn)
                    }
                ),

                abi::host_imports::INPUT_IS_KEY_DOWN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, key: u32| -> u32 {
                        input::key_pressed(key)
                    }
                ),

                abi::host_imports::INPUT_GET_MOUSE_X => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> i32 { input::mouse_x() }
                ),

                abi::host_imports::INPUT_GET_MOUSE_Y => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> i32 { input::mouse_y() }
                ),

                abi::host_imports::INPUT_IS_MOUSE_DOWN => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, btn: u32| -> u32 {
                        let mask = input::mouse_buttons();
                        let requested = 1u32 << btn;
                        if (mask & requested) != 0 { 1 } else { 0 }
                    }
                ),

                // --- System ---

                abi::host_imports::SYSTEM_LOG => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: u32, len: u32| {
                        // Best-effort: read UTF-8 from guest memory and log to stdout.
                        //
                        // If memory isn't available or the guest passes garbage, we ignore.
                        let memory_ptr = {
                            let s = global().lock().unwrap();
                            s.memory
                        };
                        if memory_ptr.is_null() {
                            return;
                        }

                        let mem = unsafe { &*memory_ptr };
                        let view = mem.view(&env);

                        let mut buf = vec![0u8; len as usize];
                        if view.read(ptr as u64, &mut buf).is_ok() {
                            if let Ok(msg) = core::str::from_utf8(&buf) {
                                println!("[wasm96] {msg}");
                            }
                        }
                    }
                ),

                abi::host_imports::SYSTEM_MILLIS => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>| -> u64 {
                        // Host time in milliseconds (monotonic-ish).
                        //
                        // This uses libretro's monotonic time if available elsewhere in the future,
                        // but for now use std time since UNIX epoch.
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                        now.as_millis() as u64
                    }
                ),

                // --- New graphics APIs (stubs for now; wired up in av/graphics extensions) ---
                // These are added so guest WASM modules can link, even before the renderer is finalized.
                //
                // NOTE: The actual ABI constants must exist in `abi::host_imports` for these to be reachable.

                // abi::host_imports::GRAPHICS_TRIANGLE => wasmer::Function::new_typed_with_env(
                //     &mut self.store,
                //     &env,
                //     |_env: FunctionEnvMut<()>, _x1: i32, _y1: i32, _x2: i32, _y2: i32, _x3: i32, _y3: i32| {
                //         // TODO: av::graphics_triangle_filled(...)
                //     }
                // ),
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

        // Store instance/entrypoints.
        self.instance = Some(instance);
        self.entrypoints = Some(entrypoints);

        Ok(())
    }

    fn call_guest_setup(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let _ = entry.setup.call(&mut self.store, &[]);
    }

    fn call_guest_update(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let _ = entry.update.call(&mut self.store, &[]);
    }

    fn call_guest_draw(&mut self) {
        let Some(entry) = &self.entrypoints else {
            return;
        };
        let _ = entry.draw.call(&mut self.store, &[]);
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

        // Call setup
        self.call_guest_setup();

        // Return default AV info.
        // We set 60 FPS and 44100 Hz audio as a baseline.
        let av_info = libretro_backend::AudioVideoInfo::new()
            .video(320, 240, 60.0, libretro_backend::PixelFormat::ARGB8888)
            .audio(44100.0)
            .region(libretro_backend::Region::NTSC);

        libretro_backend::LoadGameResult::Success(av_info)
    }

    fn on_unload_game(&mut self) -> libretro_backend::GameData {
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
        // Re-run setup on reset? Or add a reset export?
        // For now, let's just re-run setup.
        self.call_guest_setup();
    }
}

libretro_core!(Wasm96Core);
