# wasm96-cpp-sdk

A C++ SDK for wasm96, a WebAssembly-based graphics framework.

## Usage

To use this SDK, copy this repository and create a new repo based on it. Modify `main.cpp` to implement your game or application logic.

## Building

Use `just build` to compile the project to WebAssembly using CMake and Zig.

## Current Example

The `main.cpp` file demonstrates basic drawing:
- Sets canvas size to 256x256
- Draws a white background
- Draws a red circle in the center