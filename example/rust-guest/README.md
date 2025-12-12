# wasm96 Rust guest example

This is a minimal **Rust guest** WebAssembly module intended to run inside the `wasm96` libretro core.

It exports the required entrypoint:

- `wasm96_frame` (called once per frame by the host)

It uses the **handwritten** Rust SDK located at `wasm96/wasm96-sdk` (no WIT/codegen).

---

## Build (wasm32)

From this directory:

```sh
cargo build --release --target wasm32-unknown-unknown
```

The output `.wasm` will be at:

```text
target/wasm32-unknown-unknown/release/rust_guest.wasm
```

(Exact filename depends on your crate name; in this example it should match the package name.)

---

## Notes

- The wasm96 ABI uses **u32 offsets into guest linear memory** for buffers.
- The host may reject ABI mismatches; ensure the SDK’s `ABI_VERSION` matches the core.
- If you see framebuffer/audio requests failing (returning `0` / `None`), the host core may still be stubbing allocation APIs; the guest example can still compile, but you won’t get video/audio until the core implements allocation.

---

## Typical usage

1. Build the `.wasm` as above.
2. Load the resulting `.wasm` in your libretro frontend using the `wasm96` core (as you would load a ROM).