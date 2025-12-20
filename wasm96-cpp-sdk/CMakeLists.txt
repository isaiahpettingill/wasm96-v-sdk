cmake_minimum_required(VERSION 3.20)
project(wasm96-cpp-sdk)

# Use Zig toolchain
set(CMAKE_TOOLCHAIN_FILE ${CMAKE_SOURCE_DIR}/zig-toolchain.cmake)

# Set C++ standard
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Set target to WebAssembly WASI
set(CMAKE_CXX_FLAGS
    "${CMAKE_CXX_FLAGS} -target wasm32-wasi -Wall -Wextra -Wpedantic")

# Add executable
add_executable(game
    main.cpp
    wasm96.cpp
    wasm96_component_type.o
)

# Set output directory
set_target_properties(game PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/out"
    OUTPUT_NAME "game.wasm"
)
