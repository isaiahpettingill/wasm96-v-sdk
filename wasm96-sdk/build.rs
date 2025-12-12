//! Build script for `wasm96-sdk`.
//!
//! This crate no longer performs any WIT (wit-bindgen) code generation at build time.
//! The SDK is intended to be implemented by hand against the stable, C-like import/export
//! ABI defined by the `wasm96-core` runtime.
//
// Intentionally minimal: keep only `rerun-if-changed` hints for Cargo hygiene.

fn main() {
    // If the repository still contains WIT files, treat them as documentation/spec inputs.
    // This does NOT trigger codegen anymore; it only helps Cargo decide when to rerun.
    println!("cargo:rerun-if-changed=../wit");
    println!("cargo:rerun-if-changed=../wit/wasm96.wit");

    // No other actions. Avoid invoking external tools or generating Rust sources.
}
