use libretro_backend::{Core, CoreInfo, JoypadButton, RuntimeHandle, libretro_core};
use std::sync::{Mutex, OnceLock};
use wasmer::{FunctionEnv, FunctionEnvMut, Store};

#[derive(Default)]
struct GlobalState {
    handle: *mut RuntimeHandle,
    memory: *mut wasmer::Memory,
}

unsafe impl Send for GlobalState {}
unsafe impl Sync for GlobalState {}

static GLOBAL_STATE: OnceLock<Mutex<GlobalState>> = OnceLock::new();

fn get_global_state() -> &'static Mutex<GlobalState> {
    GLOBAL_STATE.get_or_init(|| Mutex::new(GlobalState::default()))
}

#[derive(Default)]
pub struct Wasm96Core {
    store: Store,
    module: Option<wasmer::Module>,
    instance: Option<wasmer::Instance>,
    game_data: Option<libretro_backend::GameData>,
    env: Option<FunctionEnv<()>>,
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

    fn info() -> libretro_backend::CoreInfo {
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
        let module = match wasmer::Module::new(&self.store, data) {
            Ok(m) => m,
            Err(_e) => {
                return libretro_backend::LoadGameResult::Failed(self.game_data.take().unwrap());
            }
        };
        self.module = Some(module);

        // Create env and imports for instantiation
        self.env = Some(FunctionEnv::new(&mut self.store, ()));
        let env = self.env.as_ref().unwrap().clone();

        let imports = wasmer::imports! {
            "env" => {
                "video_refresh" => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: i32, _width: i32, height: i32, pitch: i32| {
                        let state = get_global_state();
                        let mut s = state.lock().unwrap();
                        if !s.handle.is_null() && !s.memory.is_null() {
                            let h = unsafe { &mut *s.handle };
                            let mem = unsafe { &*s.memory };
                            let view = mem.view(&env);
                            let size = (height as usize) * (pitch as usize);
                            let mut data = vec![0u8; size];
                            view.read(ptr as u64, &mut data).unwrap();
                            h.upload_video_frame(&data);
                        }
                    },
                ),
                "audio_sample_batch" => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |env: FunctionEnvMut<()>, ptr: i32, frames: i32| {
                        let state = get_global_state();
                        let mut s = state.lock().unwrap();
                        if !s.handle.is_null() && !s.memory.is_null() {
                            let h = unsafe { &mut *s.handle };
                            let mem = unsafe { &*s.memory };
                            let view = mem.view(&env);
                            let size = frames as usize * 4;
                            let mut data = vec![0u8; size];
                            view.read(ptr as u64, &mut data).unwrap();
                            let audio: &[i16] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const i16, frames as usize * 2) };
                            h.upload_audio_frame(audio);
                        }
                    },
                ),
                "get_input_state" => wasmer::Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    |_env: FunctionEnvMut<()>, port: i32, device: i32, _index: i32, id: i32| -> i32 {
                        let state = get_global_state();
                        let mut s = state.lock().unwrap();
                        if !s.handle.is_null() {
                            let h = unsafe { &mut *s.handle };
                            match device {
                                1 => { // Joypad
                                    let pressed = h.is_joypad_button_pressed(port as u32, match id {
                                        0 => JoypadButton::B,
                                        1 => JoypadButton::Y,
                                        2 => JoypadButton::Select,
                                        3 => JoypadButton::Start,
                                        4 => JoypadButton::Up,
                                        5 => JoypadButton::Down,
                                        6 => JoypadButton::Left,
                                        7 => JoypadButton::Right,
                                        8 => JoypadButton::A,
                                        9 => JoypadButton::X,
                                        10 => JoypadButton::L1,
                                        11 => JoypadButton::R1,
                                        12 => JoypadButton::L2,
                                        13 => JoypadButton::R2,
                                        14 => JoypadButton::L3,
                                        15 => JoypadButton::R3,
                                        _ => return 0,
                                    });
                                    if pressed { 1 } else { 0 }
                                }
                                _ => 0,
                            }
                        } else {
                            0
                        }
                    },
                ),
            }
        };

        self.instance = Some(
            wasmer::Instance::new(&mut self.store, self.module.as_ref().unwrap(), &imports)
                .unwrap(),
        );
        {
            let state = get_global_state();
            let mut s = state.lock().unwrap();
            if let Some(inst) = &self.instance {
                if let Ok(mem) = inst.exports.get_memory("memory") {
                    s.memory = mem as *const _ as *mut _;
                }
            }
        }

        // Commented out custom AudioVideoInfo construction to allow compilation
        // let av_info = if let Some(inst) = &self.instance {
        //     let width = if let Ok(f) = inst.exports.get_function("get_width") {
        //         f.call(&mut self.store, &[]).unwrap()[0].unwrap_i32() as u32
        //     } else {
        //         320
        //     };
        //     let height = if let Ok(f) = inst.exports.get_function("get_height") {
        //         f.call(&mut self.store, &[]).unwrap()[0].unwrap_i32() as u32
        //     } else {
        //         240
        //     };
        //     let fps = if let Ok(f) = inst.exports.get_function("get_fps") {
        //         f.call(&mut self.store, &[]).unwrap()[0].unwrap_f32()
        //     } else {
        //         60.0
        //     };
        //     let sample_rate = if let Ok(f) = inst.exports.get_function("get_sample_rate") {
        //         f.call(&mut self.store, &[]).unwrap()[0].unwrap_i32() as f64
        //     } else {
        //         44100.0
        //     };
        //     libretro_backend::AudioVideoInfo {
        //         base_width: width,
        //         base_height: height,
        //         max_width: width,
        //         max_height: height,
        //         aspect_ratio: Some(width as f32 / height as f32),
        //         fps,
        //         sample_rate,
        //         pixel_format: libretro_backend::PixelFormat::ARGB8888,
        //     }
        // } else {
        //     libretro_backend::AudioVideoInfo::new()
        // };
        libretro_backend::LoadGameResult::Success(libretro_backend::AudioVideoInfo::new())
    }

    fn on_unload_game(&mut self) -> libretro_backend::GameData {
        self.module = None;
        self.instance = None;
        self.env = None;
        self.game_data.take().unwrap()
    }

    fn on_run(&mut self, handle: &mut libretro_backend::RuntimeHandle) {
        {
            let state = get_global_state();
            let mut s = state.lock().unwrap();
            s.handle = handle as *mut _;
        }

        if let Some(instance) = &self.instance {
            if let Ok(run) = instance.exports.get_function("run") {
                run.call(&mut self.store, &[]).unwrap();
            }
        }
    }

    fn on_reset(&mut self) {
        // Reset logic if needed
    }
}

libretro_core!(Wasm96Core);
