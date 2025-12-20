#include "wasm96_cpp.h"

namespace exports {
namespace wasm96 {
void Setup() { ::wasm96::core::SetSize(256, 256); }

void Update() {}

void Draw() {
  ::wasm96::core::Background(255, 255, 255);
  ::wasm96::core::SetColor(255, 0, 0, 255);
  ::wasm96::core::Circle(128, 128, 50);
}
} // namespace wasm96
} // namespace exports

int main() {}
