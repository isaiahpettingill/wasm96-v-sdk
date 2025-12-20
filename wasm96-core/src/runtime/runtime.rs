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

use crate::{abi, state};

use wasmtime::{Extern, Instance, Linker, Module, Store};

/// Host-side runtime container.
pub struct WasmtimeRuntime {
    pub engine: wasmtime::Engine,
    pub store: Store<()>,
    pub linker: Linker<()>,
}

impl WasmtimeRuntime {
    /// Create a new Wasmtime runtime with a broad set of WebAssembly features enabled.
    ///
    /// Notes:
    /// - We enable a wide range of Wasm proposal features to maximize guest compatibility.
    /// - Some proposals (notably threads/shared-memory) still require host-side integration
    ///   beyond flipping a config bit; we enable them here so modules can at least validate,
    ///   but guests must still be written with the embedding constraints in mind.
    pub fn new() -> Result<Self, anyhow::Error> {
        let mut cfg = wasmtime::Config::new();

        // Broadly supported/expected features for "modern" Wasm modules.
        cfg.wasm_multi_value(true);
        cfg.wasm_bulk_memory(true);
        cfg.wasm_reference_types(true);
        cfg.wasm_simd(true);

        // Additional proposal support.
        cfg.wasm_multi_memory(true);
        cfg.wasm_memory64(true);
        cfg.wasm_relaxed_simd(true);
        cfg.wasm_tail_call(true);
        cfg.wasm_function_references(true);
        cfg.wasm_gc(true);

        // Conservative but enabled, so guests using shared memories / atomics can at least load.
        // Full correctness/performance may require more embedding work (threads, shared memory limits, etc).
        cfg.wasm_threads(true);

        // Exception handling proposal is useful for some toolchains.
        cfg.wasm_exceptions(true);

        let engine = wasmtime::Engine::new(&cfg)?;
        let store = Store::new(&engine, ());
        let linker = Linker::new(&engine);

        Ok(Self {
            engine,
            store,
            linker,
        })
    }

    /// Define all host imports expected by guests under module `"env"`.
    ///
    /// Must be called before `instantiate`.
    pub fn define_imports(&mut self) -> Result<(), anyhow::Error> {
        super::imports::define_imports(&mut self.linker)
    }

    /// Instantiate a module and wire up exports/memory.
    pub fn instantiate(
        &mut self,
        module: &Module,
    ) -> Result<(Instance, abi::GuestEntrypoints), anyhow::Error> {
        let instance = self.linker.instantiate(&mut self.store, module)?;

        // Register memory in global state (best-effort).
        let memory = instance
            .get_export(&mut self.store, "memory")
            .and_then(Extern::into_memory);

        if let Some(mem) = memory.as_ref() {
            state::set_guest_memory_wasmtime(mem);
        }

        // Validate & resolve entrypoints via ABI helpers (single source of truth).
        abi::validate::required_exports_present_wasmtime(&instance, &mut self.store)
            .map_err(|e| anyhow::anyhow!("guest missing required export: {:?}", e))?;
        let entrypoints = abi::GuestEntrypoints::resolve_wasmtime(&instance, &mut self.store)?;

        Ok((instance, entrypoints))
    }
}
