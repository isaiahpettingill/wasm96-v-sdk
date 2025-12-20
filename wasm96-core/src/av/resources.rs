// Needed for `alloc::` in this crate.
extern crate alloc;

// External crates for rendering
use fontdue::Font;

// External crates for asset decoding
use resvg::usvg::Tree;
use std::collections::HashMap;
use std::sync::Mutex;

// Storage ABI helpers
use alloc::vec::Vec;

// Embedded Spleen font data
pub static SPLEEN_5X8: &[u8] = include_bytes!("../assets/spleen-5x8.bdf");
pub static SPLEEN_8X16: &[u8] = include_bytes!("../assets/spleen-8x16.bdf");
pub static SPLEEN_12X24: &[u8] = include_bytes!("../assets/spleen-12x24.bdf");
pub static SPLEEN_16X32: &[u8] = include_bytes!("../assets/spleen-16x32.bdf");
pub static SPLEEN_32X64: &[u8] = include_bytes!("../assets/spleen-32x64.bdf");

// Global resource storage (lazy_static or similar, but using Mutex for simplicity)
lazy_static::lazy_static! {
    pub static ref RESOURCES: Mutex<Resources> = Mutex::new(Resources::default());
}

#[derive(Default)]
pub struct Resources {
    // ID-based resources (existing APIs in this module).
    pub svgs: HashMap<u32, Tree>,
    pub gifs: HashMap<u32, GifResource>,
    pub fonts: HashMap<u32, FontResource>,

    // Keyed indirection (new): map u64 keys (hashed strings) -> ids in the above maps.
    pub keyed_svgs: HashMap<u64, u32>,
    pub keyed_gifs: HashMap<u64, u32>,
    pub keyed_pngs: HashMap<u64, PngResource>,
    pub keyed_fonts: HashMap<u64, u32>,

    pub next_id: u32,
}

pub struct GifResource {
    pub frames: Vec<Vec<u8>>, // RGBA data per frame
    pub delays: Vec<u16>,     // in 10ms units
    pub width: u16,
    pub height: u16,
}

#[derive(Clone)]
pub struct PngResource {
    pub rgba: Vec<u8>, // RGBA8888 bytes
    pub width: u32,
    pub height: u32,
}

pub enum FontResource {
    Ttf(Font),
    Spleen {
        width: u32,
        height: u32,
        glyphs: HashMap<char, Vec<u8>>, // char -> bitmap rows
    },
}

/// Errors from AV operations.
#[derive(Debug)]
pub enum AvError {
    MissingMemory,
    MemoryReadFailed,
}
