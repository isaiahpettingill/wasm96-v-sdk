// Wasm96 V SDK
module wasm96

// Joypad button ids.
pub enum Button as u32 {
	b = 0
	y = 1
	select = 2
	start = 3
	up = 4
	down = 5
	left = 6
	right = 7
	a = 8
	x = 9
	l1 = 10
	r1 = 11
	l2 = 12
	r2 = 13
	l3 = 14
	r3 = 15
}

// Text size dimensions.
pub struct TextSize {
	width u32
	height u32
}

// Low-level raw ABI imports.

// Graphics
fn C.wasm96_graphics_set_size(width u32, height u32)
fn C.wasm96_graphics_set_color(r u32, g u32, b u32, a u32)
fn C.wasm96_graphics_background(r u32, g u32, b u32)
fn C.wasm96_graphics_point(x int, y int)
fn C.wasm96_graphics_line(x1 int, y1 int, x2 int, y2 int)
fn C.wasm96_graphics_rect(x int, y int, w u32, h u32)
fn C.wasm96_graphics_rect_outline(x int, y int, w u32, h u32)
fn C.wasm96_graphics_circle(x int, y int, r u32)
fn C.wasm96_graphics_circle_outline(x int, y int, r u32)
fn C.wasm96_graphics_image(x int, y int, w u32, h u32, ptr &u8, len usize)
fn C.wasm96_graphics_image_png(x int, y int, ptr &u8, len usize)
fn C.wasm96_graphics_triangle(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int)
fn C.wasm96_graphics_triangle_outline(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int)
fn C.wasm96_graphics_bezier_quadratic(x1 int, y1 int, cx int, cy int, x2 int, y2 int, segments u32)
fn C.wasm96_graphics_bezier_cubic(x1 int, y1 int, cx1 int, cy1 int, cx2 int, cy2 int, x2 int, y2 int, segments u32)
fn C.wasm96_graphics_pill(x int, y int, w u32, h u32)
fn C.wasm96_graphics_pill_outline(x int, y int, w u32, h u32)
fn C.wasm96_graphics_svg_register(key u64, data_ptr &u8, data_len usize) u32
fn C.wasm96_graphics_svg_draw_key(key u64, x int, y int, w u32, h u32)
fn C.wasm96_graphics_svg_unregister(key u64)
fn C.wasm96_graphics_gif_register(key u64, data_ptr &u8, data_len usize) u32
fn C.wasm96_graphics_gif_draw_key(key u64, x int, y int)
fn C.wasm96_graphics_gif_draw_key_scaled(key u64, x int, y int, w u32, h u32)
fn C.wasm96_graphics_gif_unregister(key u64)
fn C.wasm96_graphics_png_register(key u64, data_ptr &u8, data_len usize) u32
fn C.wasm96_graphics_png_draw_key(key u64, x int, y int)
fn C.wasm96_graphics_png_draw_key_scaled(key u64, x int, y int, w u32, h u32)
fn C.wasm96_graphics_png_unregister(key u64)
fn C.wasm96_graphics_font_register_ttf(key u64, data_ptr &u8, data_len usize) u32
fn C.wasm96_graphics_font_register_bdf(key u64, data_ptr &u8, data_len usize) u32
fn C.wasm96_graphics_font_register_spleen(key u64, size u32) u32
fn C.wasm96_graphics_font_unregister(key u64)
fn C.wasm96_graphics_text_key(x int, y int, font_key u64, text_ptr &u8, text_len usize)
fn C.wasm96_graphics_text_measure_key(font_key u64, text_ptr &u8, text_len usize) u64

fn C.wasm96_graphics_set_3d(enable u32)
fn C.wasm96_graphics_camera_look_at(eye_x f32, eye_y f32, eye_z f32, target_x f32, target_y f32, target_z f32, up_x f32, up_y f32, up_z f32)
fn C.wasm96_graphics_camera_perspective(fovy f32, aspect f32, near f32, far f32)
fn C.wasm96_graphics_mesh_create(key u64, vertices_ptr &f32, vertices_len usize, indices_ptr &u32, indices_len usize)
fn C.wasm96_graphics_mesh_create_obj(key u64, data_ptr &u8, data_len usize)
fn C.wasm96_graphics_mesh_create_stl(key u64, data_ptr &u8, data_len usize)
fn C.wasm96_graphics_mesh_draw(key u64, pos_x f32, pos_y f32, pos_z f32, rot_x f32, rot_y f32, rot_z f32, scale_x f32, scale_y f32, scale_z f32)

// Input
fn C.wasm96_input_is_button_down(port u32, btn u32) u32
fn C.wasm96_input_is_key_down(key u32) u32
fn C.wasm96_input_get_mouse_x() int
fn C.wasm96_input_get_mouse_y() int
fn C.wasm96_input_is_mouse_down(btn u32) u32

// Audio
fn C.wasm96_audio_init(sample_rate u32) u32
fn C.wasm96_audio_push_samples(ptr &i16, len usize)
fn C.wasm96_audio_play_wav(ptr &u8, len usize)
fn C.wasm96_audio_play_qoa(ptr &u8, len usize)
fn C.wasm96_audio_play_xm(ptr &u8, len usize)

// System
fn C.wasm96_system_log(ptr &u8, len usize)
fn C.wasm96_system_millis() u64

// Graphics API.

fn hash_key(key []u8) u64 {
	mut hash := u64(0xcbf29ce484222325)
	for b in key {
		hash ^= u64(b)
		hash *= 0x100000001b3
	}
	return hash
}

// Set the screen dimensions.
pub fn graphics_set_size(width u32, height u32) {
	C.wasm96_graphics_set_size(width, height)
}

// Set the current drawing color (RGBA).
pub fn graphics_set_color(r u8, g u8, b u8, a u8) {
	C.wasm96_graphics_set_color(u32(r), u32(g), u32(b), u32(a))
}

// Clear the screen with a specific color (RGB).
pub fn graphics_background(r u8, g u8, b u8) {
	C.wasm96_graphics_background(u32(r), u32(g), u32(b))
}

// Draw a single pixel at (x, y).
pub fn graphics_point(x int, y int) {
	C.wasm96_graphics_point(x, y)
}

// Draw a line from (x1, y1) to (x2, y2).
pub fn graphics_line(x1 int, y1 int, x2 int, y2 int) {
	C.wasm96_graphics_line(x1, y1, x2, y2)
}

// Draw a filled rectangle.
pub fn graphics_rect(x int, y int, w u32, h u32) {
	C.wasm96_graphics_rect(x, y, w, h)
}

// Draw a rectangle outline.
pub fn graphics_rect_outline(x int, y int, w u32, h u32) {
	C.wasm96_graphics_rect_outline(x, y, w, h)
}

// Draw a filled circle.
pub fn graphics_circle(x int, y int, r u32) {
	C.wasm96_graphics_circle(x, y, r)
}

// Draw a circle outline.
pub fn graphics_circle_outline(x int, y int, r u32) {
	C.wasm96_graphics_circle_outline(x, y, r)
}

// Draw an image/sprite.
// data is a slice of RGBA bytes (4 bytes per pixel).
pub fn graphics_image(x int, y int, w u32, h u32, data []u8) {
	C.wasm96_graphics_image(x, y, w, h, &data[0], usize(data.len))
}

// Draw an image from raw PNG bytes.
pub fn graphics_image_png(x int, y int, data []u8) {
	C.wasm96_graphics_image_png(x, y, &data[0], usize(data.len))
}

// Draw a filled triangle.
pub fn graphics_triangle(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int) {
	C.wasm96_graphics_triangle(x1, y1, x2, y2, x3, y3)
}

// Draw a triangle outline.
pub fn graphics_triangle_outline(x1 int, y1 int, x2 int, y2 int, x3 int, y3 int) {
	C.wasm96_graphics_triangle_outline(x1, y1, x2, y2, x3, y3)
}

// Draw a quadratic Bezier curve.
pub fn graphics_bezier_quadratic(x1 int, y1 int, cx int, cy int, x2 int, y2 int, segments u32) {
	C.wasm96_graphics_bezier_quadratic(x1, y1, cx, cy, x2, y2, segments)
}

// Draw a cubic Bezier curve.
pub fn graphics_bezier_cubic(x1 int, y1 int, cx1 int, cy1 int, cx2 int, cy2 int, x2 int, y2 int, segments u32) {
	C.wasm96_graphics_bezier_cubic(x1, y1, cx1, cy1, cx2, cy2, x2, y2, segments)
}

// Draw a filled pill.
pub fn graphics_pill(x int, y int, w u32, h u32) {
	C.wasm96_graphics_pill(x, y, w, h)
}

// Draw a pill outline.
pub fn graphics_pill_outline(x int, y int, w u32, h u32) {
	C.wasm96_graphics_pill_outline(x, y, w, h)
}

// Register an SVG resource under a string key.
pub fn graphics_svg_register(key []u8, data []u8) bool {
	return C.wasm96_graphics_svg_register(hash_key(key), &data[0], usize(data.len)) != 0
}

// Draw a registered SVG by key.
pub fn graphics_svg_draw_key(key []u8, x int, y int, w u32, h u32) {
	C.wasm96_graphics_svg_draw_key(hash_key(key), x, y, w, h)
}

// Unregister an SVG by key.
pub fn graphics_svg_unregister(key []u8) {
	C.wasm96_graphics_svg_unregister(hash_key(key))
}

// Register a GIF resource under a string key.
pub fn graphics_gif_register(key []u8, data []u8) bool {
	return C.wasm96_graphics_gif_register(hash_key(key), &data[0], usize(data.len)) != 0
}

// Draw a registered GIF by key at natural size.
pub fn graphics_gif_draw_key(key []u8, x int, y int) {
	C.wasm96_graphics_gif_draw_key(hash_key(key), x, y)
}

// Draw a registered GIF by key scaled.
pub fn graphics_gif_draw_key_scaled(key []u8, x int, y int, w u32, h u32) {
	C.wasm96_graphics_gif_draw_key_scaled(hash_key(key), x, y, w, h)
}

// Unregister a GIF by key.
pub fn graphics_gif_unregister(key []u8) {
	C.wasm96_graphics_gif_unregister(hash_key(key))
}

// Register a PNG resource under a string key.
pub fn graphics_png_register(key []u8, data []u8) bool {
	return C.wasm96_graphics_png_register(hash_key(key), &data[0], usize(data.len)) != 0
}

// Draw a registered PNG by key at natural size.
pub fn graphics_png_draw_key(key []u8, x int, y int) {
	C.wasm96_graphics_png_draw_key(hash_key(key), x, y)
}

// Draw a registered PNG by key scaled.
pub fn graphics_png_draw_key_scaled(key []u8, x int, y int, w u32, h u32) {
	C.wasm96_graphics_png_draw_key_scaled(hash_key(key), x, y, w, h)
}

// Unregister a PNG by key.
pub fn graphics_png_unregister(key []u8) {
	C.wasm96_graphics_png_unregister(hash_key(key))
}

// Register a TTF font under a string key.
pub fn graphics_font_register_ttf(key []u8, data []u8) bool {
	return C.wasm96_graphics_font_register_ttf(hash_key(key), &data[0], usize(data.len)) != 0
}

// Register a BDF font under a string key.
pub fn graphics_font_register_bdf(key []u8, data []u8) bool {
	return C.wasm96_graphics_font_register_bdf(hash_key(key), &data[0], usize(data.len)) != 0
}

// Register a built-in Spleen font under a string key.
pub fn graphics_font_register_spleen(key []u8, size u32) bool {
	return C.wasm96_graphics_font_register_spleen(hash_key(key), size) != 0
}

// Unregister a font by key.
pub fn graphics_font_unregister(key []u8) {
	C.wasm96_graphics_font_unregister(hash_key(key))
}

// Draw text using a font referenced by key.
pub fn graphics_text_key(x int, y int, font_key []u8, str []u8) {
	C.wasm96_graphics_text_key(x, y, hash_key(font_key), &str[0], usize(str.len))
}

// Measure text using a font referenced by key.
pub fn graphics_text_measure_key(font_key []u8, str []u8) TextSize {
	result := C.wasm96_graphics_text_measure_key(hash_key(font_key), &str[0], usize(str.len))
	return TextSize{
		width: u32(result >> 32)
		height: u32(result & 0xFFFFFFFF)
	}
}

// 3D Graphics API.

// Enable or disable 3D rendering mode.
pub fn graphics_set_3d(enable bool) {
	C.wasm96_graphics_set_3d(if enable { 1 } else { 0 })
}

// Set the camera position and target.
pub fn graphics_camera_look_at(eye_x f32, eye_y f32, eye_z f32, target_x f32, target_y f32, target_z f32, up_x f32, up_y f32, up_z f32) {
	C.wasm96_graphics_camera_look_at(eye_x, eye_y, eye_z, target_x, target_y, target_z, up_x, up_y, up_z)
}

// Set the camera perspective projection.
pub fn graphics_camera_perspective(fovy f32, aspect f32, near f32, far f32) {
	C.wasm96_graphics_camera_perspective(fovy, aspect, near, far)
}

// Create a mesh from raw vertex and index data.
// vertices: [x, y, z, u, v, nx, ny, nz, ...]
pub fn graphics_mesh_create(key []u8, vertices []f32, indices []u32) {
	C.wasm96_graphics_mesh_create(hash_key(key), &vertices[0], usize(vertices.len), &indices[0], usize(indices.len))
}

// Create a mesh from OBJ file data.
pub fn graphics_mesh_create_obj(key []u8, data []u8) {
	C.wasm96_graphics_mesh_create_obj(hash_key(key), &data[0], usize(data.len))
}

// Create a mesh from STL file data.
pub fn graphics_mesh_create_stl(key []u8, data []u8) {
	C.wasm96_graphics_mesh_create_stl(hash_key(key), &data[0], usize(data.len))
}

// Draw a mesh with transformation.
pub fn graphics_mesh_draw(key []u8, pos_x f32, pos_y f32, pos_z f32, rot_x f32, rot_y f32, rot_z f32, scale_x f32, scale_y f32, scale_z f32) {
	C.wasm96_graphics_mesh_draw(hash_key(key), pos_x, pos_y, pos_z, rot_x, rot_y, rot_z, scale_x, scale_y, scale_z)
}

// Input API.

// Returns true if the specified button is currently held down.
pub fn input_is_button_down(port u32, btn Button) bool {
	return C.wasm96_input_is_button_down(port, u32(btn)) != 0
}

// Returns true if the specified key is currently held down.
pub fn input_is_key_down(key u32) bool {
	return C.wasm96_input_is_key_down(key) != 0
}

// Get current mouse X position.
pub fn input_get_mouse_x() int {
	return C.wasm96_input_get_mouse_x()
}

// Get current mouse Y position.
pub fn input_get_mouse_y() int {
	return C.wasm96_input_get_mouse_y()
}

// Returns true if the specified mouse button is held down.
// 0 = Left, 1 = Right, 2 = Middle.
pub fn input_is_mouse_down(btn u32) bool {
	return C.wasm96_input_is_mouse_down(btn) != 0
}

// Audio API.

// Initialize audio system.
pub fn audio_init(sample_rate u32) u32 {
	return C.wasm96_audio_init(sample_rate)
}

// Push a chunk of audio samples.
// Samples are interleaved stereo (L, R, L, R...) signed 16-bit integers.
pub fn audio_push_samples(samples []i16) {
	C.wasm96_audio_push_samples(&samples[0], usize(samples.len))
}

// Play a WAV file.
// The WAV data is decoded and played as a one-shot audio channel.
pub fn audio_play_wav(data []u8) {
	C.wasm96_audio_play_wav(&data[0], usize(data.len))
}

// Play a QOA file.
// The QOA data is decoded and played as a looping audio channel.
pub fn audio_play_qoa(data []u8) {
	C.wasm96_audio_play_qoa(&data[0], usize(data.len))
}

// Play an XM file.
// The XM data is decoded using xmrsplayer and played as a looping audio channel.
pub fn audio_play_xm(data []u8) {
	C.wasm96_audio_play_xm(&data[0], usize(data.len))
}

// System API.

// Log a message to the host console.
pub fn system_log(message []u8) {
	C.wasm96_system_log(&message[0], usize(message.len))
}

// Get the number of milliseconds since the app started.
pub fn system_millis() u64 {
	return C.wasm96_system_millis()
}
