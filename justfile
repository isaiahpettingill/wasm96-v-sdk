build-examples:
    cargo build -p rust_guest --release --target wasm32-unknown-unknown
    cargo build -p rust-guest-showcase --release --target wasm32-unknown-unknown
    cargo build -p rust_guest_mp_platformer --release --target wasm32-unknown-unknown
    cargo build -p rust_guest_osmosis --release --target wasm32-unknown-unknown
    cd example/zig-guest && zig build

build-sdks:
    cargo build -p wasm96-sdk --release
    cd wasm96-go-sdk && go build .
    cd wasm96-zig-sdk && zig build

build-core:
    cargo build -p wasm96-core --release

run_command := if os_family() == "windows" { "/c/RetroArch/retroarch.exe -L ./target/release/wasm96_core.dll" } else { "retroarch -L ./target/release/libwasm96_core.so" }

run content-path: build-examples build-core
    RUST_BACKTRACE=1 {{ run_command }} {{ content-path }} --verbose

run-rust-guest:
    just run ./target/wasm32-unknown-unknown/release/rust_guest.wasm

run-rust-showcase:
    just run ./target/wasm32-unknown-unknown/release/rust_guest_showcase.wasm

run-rust-platformer:
    just run ./target/wasm32-unknown-unknown/release/rust_guest_mp_platformer.wasm

run-rust-osmosis:
    just run ./target/wasm32-unknown-unknown/release/rust_guest_osmosis.wasm

run-zig-guest:
    just run ./example/zig-guest/zig-out/bin/zig-guest.wasm
