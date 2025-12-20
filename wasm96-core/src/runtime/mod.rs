//! Wasmtime-backed runtime glue for wasm96-core.
//!
//! Responsibilities:
//! - Create a Wasmtime `Engine`/`Store` with feature flags enabled.
//! - Define host imports under module `"env"` matching the guest ABI.
//! - Instantiate a compiled `wasmtime::Module`.
//! - Register the guest memory export into global state for host-side helpers.
//!
//! Guest ABI is unchanged: imports are still `"env"` + `wasm96_*` symbols.
//!
//! Entrypoint resolution (setup/update/draw + WASI `_start`/`main` fallback) lives in
//! `crate::abi::GuestEntrypoints::resolve_wasmtime`.

pub mod imports;
pub mod runtime;

pub use runtime::WasmtimeRuntime;
