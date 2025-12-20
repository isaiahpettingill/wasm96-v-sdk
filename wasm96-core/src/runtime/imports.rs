//! Host import definitions for the Wasmtime runtime.
//!
//! This module defines all the host functions imported by guest modules under the "env" module.

use crate::{
    abi::{IMPORT_MODULE, host_imports},
    av, input,
};

use wasmtime::{Caller, Linker};

/// Define all host imports expected by guests under module `"env"`.
///
/// Must be called before instantiating the module.
pub fn define_imports(linker: &mut Linker<()>) -> Result<(), anyhow::Error> {
    // --- Graphics ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_SET_SIZE,
        |_caller: Caller<'_, ()>, width: u32, height: u32| {
            av::graphics_set_size(width, height);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_SET_COLOR,
        |_caller: Caller<'_, ()>, r: u32, g: u32, b: u32, a: u32| {
            av::graphics_set_color(r, g, b, a);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_BACKGROUND,
        |_caller: Caller<'_, ()>, r: u32, g: u32, b: u32| {
            av::graphics_background(r, g, b);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_POINT,
        |_caller: Caller<'_, ()>, x: i32, y: i32| {
            av::graphics_point(x, y);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_LINE,
        |_caller: Caller<'_, ()>, x1: i32, y1: i32, x2: i32, y2: i32| {
            av::graphics_line(x1, y1, x2, y2);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_RECT,
        |_caller: Caller<'_, ()>, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_rect(x, y, w, h);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_RECT_OUTLINE,
        |_caller: Caller<'_, ()>, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_rect_outline(x, y, w, h);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_CIRCLE,
        |_caller: Caller<'_, ()>, x: i32, y: i32, r: u32| {
            av::graphics_circle(x, y, r);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_CIRCLE_OUTLINE,
        |_caller: Caller<'_, ()>, x: i32, y: i32, r: u32| {
            av::graphics_circle_outline(x, y, r);
        },
    )?;

    // Raw RGBA blit: (x,y,w,h,ptr,len)
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_IMAGE,
        |mut caller: Caller<'_, ()>, x: i32, y: i32, w: u32, h: u32, ptr: u32, len: u32| {
            let _ = av::graphics_image(&mut caller, x, y, w, h, ptr, len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_IMAGE_PNG,
        |mut caller: Caller<'_, ()>, x: i32, y: i32, ptr: u32, len: u32| {
            let _ = av::graphics_image_png(&mut caller, x, y, ptr, len);
        },
    )?;

    // --- Keyed resources (SVG/GIF/PNG) ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_SVG_REGISTER,
        |mut caller: Caller<'_, ()>,
         key_ptr: u32,
         key_len: u32,
         data_ptr: u32,
         data_len: u32|
         -> u32 {
            av::graphics_svg_register(&mut caller, key_ptr, key_len, data_ptr, data_len)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_SVG_DRAW_KEY,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_svg_draw_key(&mut caller, key_ptr, key_len, x, y, w, h)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_SVG_UNREGISTER,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32| {
            av::graphics_svg_unregister(&mut caller, key_ptr, key_len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_GIF_REGISTER,
        |mut caller: Caller<'_, ()>,
         key_ptr: u32,
         key_len: u32,
         data_ptr: u32,
         data_len: u32|
         -> u32 {
            av::graphics_gif_register(&mut caller, key_ptr, key_len, data_ptr, data_len)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_GIF_DRAW_KEY,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32| {
            av::graphics_gif_draw_key(&mut caller, key_ptr, key_len, x, y);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_GIF_DRAW_KEY_SCALED,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_gif_draw_key_scaled(&mut caller, key_ptr, key_len, x, y, w, h)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_GIF_UNREGISTER,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32| {
            av::graphics_gif_unregister(&mut caller, key_ptr, key_len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PNG_REGISTER,
        |mut caller: Caller<'_, ()>,
         key_ptr: u32,
         key_len: u32,
         data_ptr: u32,
         data_len: u32|
         -> u32 {
            av::graphics_png_register(&mut caller, key_ptr, key_len, data_ptr, data_len)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PNG_DRAW_KEY,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32| {
            av::graphics_png_draw_key(&mut caller, key_ptr, key_len, x, y);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PNG_DRAW_KEY_SCALED,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_png_draw_key_scaled(&mut caller, key_ptr, key_len, x, y, w, h)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PNG_UNREGISTER,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32| {
            av::graphics_png_unregister(&mut caller, key_ptr, key_len);
        },
    )?;

    // --- Keyed fonts + text ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_FONT_REGISTER_TTF,
        |mut caller: Caller<'_, ()>,
         key_ptr: u32,
         key_len: u32,
         data_ptr: u32,
         data_len: u32|
         -> u32 {
            av::graphics_font_register_ttf(&mut caller, key_ptr, key_len, data_ptr, data_len)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_FONT_REGISTER_SPLEEN,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, size: u32| -> u32 {
            av::graphics_font_register_spleen(&mut caller, key_ptr, key_len, size)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_FONT_UNREGISTER,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32| {
            av::graphics_font_unregister(&mut caller, key_ptr, key_len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_TEXT_KEY,
        |mut caller: Caller<'_, ()>,
         x: i32,
         y: i32,
         font_key_ptr: u32,
         font_key_len: u32,
         text_ptr: u32,
         text_len: u32| {
            av::graphics_text_key(
                x,
                y,
                &mut caller,
                font_key_ptr,
                font_key_len,
                text_ptr,
                text_len,
            );
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_TEXT_MEASURE_KEY,
        |mut caller: Caller<'_, ()>,
         font_key_ptr: u32,
         font_key_len: u32,
         text_ptr: u32,
         text_len: u32|
         -> u64 {
            av::graphics_text_measure_key(
                &mut caller,
                font_key_ptr,
                font_key_len,
                text_ptr,
                text_len,
            )
        },
    )?;

    // --- Shapes ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_TRIANGLE,
        |_caller: Caller<'_, ()>, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32| {
            av::graphics_triangle(x1, y1, x2, y2, x3, y3);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_TRIANGLE_OUTLINE,
        |_caller: Caller<'_, ()>, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32| {
            av::graphics_triangle_outline(x1, y1, x2, y2, x3, y3);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_BEZIER_QUADRATIC,
        |_caller: Caller<'_, ()>,
         x1: i32,
         y1: i32,
         cx: i32,
         cy: i32,
         x2: i32,
         y2: i32,
         segments: u32| {
            av::graphics_bezier_quadratic(x1, y1, cx, cy, x2, y2, segments);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_BEZIER_CUBIC,
        |_caller: Caller<'_, ()>,
         x1: i32,
         y1: i32,
         cx1: i32,
         cy1: i32,
         cx2: i32,
         cy2: i32,
         x2: i32,
         y2: i32,
         segments: u32| {
            av::graphics_bezier_cubic(x1, y1, cx1, cy1, cx2, cy2, x2, y2, segments);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PILL,
        |_caller: Caller<'_, ()>, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_pill(x, y, w, h);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::GRAPHICS_PILL_OUTLINE,
        |_caller: Caller<'_, ()>, x: i32, y: i32, w: u32, h: u32| {
            av::graphics_pill_outline(x, y, w, h);
        },
    )?;

    // --- Audio ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::AUDIO_INIT,
        |_caller: Caller<'_, ()>, sample_rate: u32| -> u32 { av::audio_init(sample_rate) },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::AUDIO_PUSH_SAMPLES,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            let _ = av::audio_push_samples(&mut caller, ptr, len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::AUDIO_PLAY_WAV,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            av::audio_play_wav(&mut caller, ptr, len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::AUDIO_PLAY_QOA,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            av::audio_play_qoa(&mut caller, ptr, len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::AUDIO_PLAY_XM,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            av::audio_play_xm(&mut caller, ptr, len);
        },
    )?;

    // --- Input ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::INPUT_IS_BUTTON_DOWN,
        |_caller: Caller<'_, ()>, port: u32, btn: u32| -> u32 {
            input::joypad_button_pressed(port, btn)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::INPUT_IS_KEY_DOWN,
        |_caller: Caller<'_, ()>, key: u32| -> u32 { input::key_pressed(key) },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::INPUT_GET_MOUSE_X,
        |_caller: Caller<'_, ()>| -> i32 { input::mouse_x() },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::INPUT_GET_MOUSE_Y,
        |_caller: Caller<'_, ()>| -> i32 { input::mouse_y() },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::INPUT_IS_MOUSE_DOWN,
        |_caller: Caller<'_, ()>, btn: u32| -> u32 {
            let mask = input::mouse_buttons();
            let requested = 1u32 << btn;
            if (mask & requested) != 0 { 1 } else { 0 }
        },
    )?;

    // --- System ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::SYSTEM_LOG,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            let Some(memory) = memory else {
                return;
            };

            let mut buf = vec![0u8; len as usize];
            if memory.read(&caller, ptr as usize, &mut buf).is_ok()
                && let Ok(msg) = core::str::from_utf8(&buf)
            {
                println!("[wasm96] {msg}");
            }
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::SYSTEM_MILLIS,
        |_caller: Caller<'_, ()>| -> u64 {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            now.as_millis() as u64
        },
    )?;

    // --- Storage ---
    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::STORAGE_SAVE,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32, data_ptr: u32, data_len: u32| {
            av::storage_save(&mut caller, key_ptr, key_len, data_ptr, data_len);
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::STORAGE_LOAD,
        |mut caller: Caller<'_, ()>, key_ptr: u32, key_len: u32| -> u64 {
            av::storage_load(&mut caller, key_ptr, key_len)
        },
    )?;

    linker.func_wrap(
        IMPORT_MODULE,
        host_imports::STORAGE_FREE,
        |mut caller: Caller<'_, ()>, ptr: u32, len: u32| {
            av::storage_free(&mut caller, ptr, len);
        },
    )?;

    Ok(())
}
