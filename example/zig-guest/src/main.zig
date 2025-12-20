const wasm96 = @import("wasm96");

export fn setup() void {
    wasm96.graphics.setSize(640, 480);
    _ = wasm96.graphics.fontRegisterSpleen("font/spleen/16", 16);
}

export fn update() void {
    // Update logic here
}

export fn draw() void {
    wasm96.graphics.background(0, 0, 0); // Black background
    wasm96.graphics.setColor(255, 255, 255, 255); // White
    wasm96.graphics.rect(100, 100, 100, 100); // Draw a white rectangle
    wasm96.graphics.textKey(10, 10, "font/spleen/16", "Hello from Zig!");
}
