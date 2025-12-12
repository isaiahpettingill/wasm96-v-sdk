//! Input module for wasm96-core.
//!
//! Responsibilities:
//! - Provide a stable ABI-facing set of input queries (joypad/keyboard/mouse/lightgun).
//! - Implement those queries by calling into `libretro_backend::RuntimeHandle`.
//! - Optionally cache/snapshot inputs per-frame for determinism (future).
//!
//! Notes / constraints:
//! - `libretro_backend::RuntimeHandle` does not expose all libretro device APIs in a uniform way.
//!   In particular, this project currently only uses `is_joypad_button_pressed` directly.
//! - For keyboard/mouse/lightgun, this module provides a *best-effort* implementation.
//!   Where the backend doesn't expose the needed primitives, these APIs return default values.
//!
//! If you later extend `libretro-backend` usage (e.g., to query mouse/lightgun/keyboard from
//! libretro directly), you can wire those calls in at the `TODO(libretro)` markers below.

use crate::abi::{JoypadButton, LightgunButtons, MouseButtons};
use crate::state;
use libretro_backend::{JoypadButton as LrJoypadButton, RuntimeHandle};

/// Convert ABI joypad button id into libretro-backend joypad button enum.
fn map_joypad_button(button: u32) -> Option<LrJoypadButton> {
    match button {
        x if x == JoypadButton::B as u32 => Some(LrJoypadButton::B),
        x if x == JoypadButton::Y as u32 => Some(LrJoypadButton::Y),
        x if x == JoypadButton::Select as u32 => Some(LrJoypadButton::Select),
        x if x == JoypadButton::Start as u32 => Some(LrJoypadButton::Start),
        x if x == JoypadButton::Up as u32 => Some(LrJoypadButton::Up),
        x if x == JoypadButton::Down as u32 => Some(LrJoypadButton::Down),
        x if x == JoypadButton::Left as u32 => Some(LrJoypadButton::Left),
        x if x == JoypadButton::Right as u32 => Some(LrJoypadButton::Right),
        x if x == JoypadButton::A as u32 => Some(LrJoypadButton::A),
        x if x == JoypadButton::X as u32 => Some(LrJoypadButton::X),
        x if x == JoypadButton::L1 as u32 => Some(LrJoypadButton::L1),
        x if x == JoypadButton::R1 as u32 => Some(LrJoypadButton::R1),
        x if x == JoypadButton::L2 as u32 => Some(LrJoypadButton::L2),
        x if x == JoypadButton::R2 as u32 => Some(LrJoypadButton::R2),
        x if x == JoypadButton::L3 as u32 => Some(LrJoypadButton::L3),
        x if x == JoypadButton::R3 as u32 => Some(LrJoypadButton::R3),
        _ => None,
    }
}

/// Return a mutable reference to the current RuntimeHandle if available.
fn with_handle<R>(f: impl FnOnce(&mut RuntimeHandle) -> R) -> Option<R> {
    let mut s = state::global().lock().unwrap();
    if s.handle.is_null() {
        return None;
    }
    // SAFETY: handle pointer is set at start of `on_run` and guarded by the mutex.
    let h = unsafe { &mut *s.handle };
    Some(f(h))
}

/// Query whether a given joypad button is pressed.
///
/// Returns 1 if pressed, else 0.
pub fn joypad_button_pressed(port: u32, button: u32) -> u32 {
    let Some(btn) = map_joypad_button(button) else {
        return 0;
    };

    with_handle(|h| {
        if h.is_joypad_button_pressed(port, btn) {
            1
        } else {
            0
        }
    })
    .unwrap_or(0)
}

/// Query whether a given key is pressed.
///
/// ABI note:
/// - `key` is an ABI-defined keycode. For long-term stability, you should pick a stable
///   code set (e.g., “libretro key” ids or USB HID usage ids).
///
/// Current implementation:
/// - `libretro_backend::RuntimeHandle` does not expose keyboard querying in the codebase
///   we have here, so this returns 0 by default.
///
/// TODO(libretro): wire to real keyboard input via libretro if/when exposed.
pub fn key_pressed(_key: u32) -> u32 {
    // You can also consider caching a keyboard bitset per frame in `state::InputState`.
    0
}

/// Mouse X coordinate.
///
/// Current implementation:
/// - Returns cached state (`state::InputState.mouse_x`) which is not yet populated by the core,
///   so it defaults to 0.
///
/// TODO(libretro): populate per-frame from libretro mouse input.
pub fn mouse_x() -> i32 {
    let s = state::global().lock().unwrap();
    s.input.mouse_x
}

/// Mouse Y coordinate.
///
/// TODO(libretro): populate per-frame from libretro mouse input.
pub fn mouse_y() -> i32 {
    let s = state::global().lock().unwrap();
    s.input.mouse_y
}

/// Mouse buttons bitmask.
///
/// Bits correspond to [`MouseButtons`].
///
/// TODO(libretro): populate per-frame from libretro mouse input.
pub fn mouse_buttons() -> u32 {
    let s = state::global().lock().unwrap();
    s.input.mouse_buttons
}

/// Convenience: check if mouse left button is pressed (bit test).
pub fn mouse_left_pressed() -> u32 {
    (mouse_buttons() & (MouseButtons::Left as u32) != 0) as u32
}

/// Convenience: check if mouse right button is pressed (bit test).
pub fn mouse_right_pressed() -> u32 {
    (mouse_buttons() & (MouseButtons::Right as u32) != 0) as u32
}

/// Lightgun X coordinate for a given port.
///
/// Current implementation:
/// - Returns cached state (`state::InputState.lightgun_x`) which is not yet populated by the core,
///   so it defaults to 0.
///
/// TODO(libretro): populate per-frame from libretro lightgun input for each port.
pub fn lightgun_x(_port: u32) -> i32 {
    let s = state::global().lock().unwrap();
    s.input.lightgun_x
}

/// Lightgun Y coordinate for a given port.
///
/// TODO(libretro): populate per-frame from libretro lightgun input for each port.
pub fn lightgun_y(_port: u32) -> i32 {
    let s = state::global().lock().unwrap();
    s.input.lightgun_y
}

/// Lightgun buttons bitmask for a given port.
///
/// Bits correspond to [`LightgunButtons`].
///
/// TODO(libretro): populate per-frame from libretro lightgun input for each port.
pub fn lightgun_buttons(_port: u32) -> u32 {
    let s = state::global().lock().unwrap();
    s.input.lightgun_buttons
}

/// Convenience: check if lightgun trigger is pressed.
pub fn lightgun_trigger_pressed(port: u32) -> u32 {
    let _ = port;
    (lightgun_buttons(port) & (LightgunButtons::Trigger as u32) != 0) as u32
}

/// Convenience: check if lightgun offscreen is active.
pub fn lightgun_offscreen(port: u32) -> u32 {
    (lightgun_buttons(port) & (LightgunButtons::Offscreen as u32) != 0) as u32
}

/// Snapshot inputs for the current frame into `state::InputState`.
///
/// This is optional but recommended for determinism: host import queries during a frame
/// should see a consistent state.
///
/// Current implementation:
/// - Joypad is queried on-demand (directly) because we have a stable call available.
/// - Keyboard/mouse/lightgun are not available from `RuntimeHandle` here, so we keep defaults.
///
/// Call this once per `on_run` before invoking guest `wasm96_frame`.
pub fn snapshot_per_frame() {
    // Keep a single lock for updating `state::InputState`.
    let mut s = state::global().lock().unwrap();

    // If you later add real device querying, do it here. For now, keep defaults.
    // s.input.mouse_x = ...
    // s.input.mouse_y = ...
    // s.input.mouse_buttons = ...
    // s.input.lightgun_x = ...
    // s.input.lightgun_y = ...
    // s.input.lightgun_buttons = ...

    // Keep last_key as-is; could be updated on key events.
    let _ = &mut *s;
}
