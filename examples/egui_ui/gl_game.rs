use egui_sdl2_gl::gl;
use egui_sdl2_gl::gl::types::*;
use std::ffi::CString;
use std::ptr;

const VS_SRC: &str = r#"#version 150
in vec2 a_pos;
in vec2 a_uv;
out vec2 v_uv;
void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

const FS_SRC: &str = r#"#version 150
in vec2 v_uv;
out vec4 o_color;
uniform sampler2D u_tex;
void main() {
    o_color = texture(u_tex, v_uv);
}
"#;

// Fullscreen quad: position (x,y) + texcoord (u,v)
// Texture V is flipped: our framebuffer row 0 = GL texture V=0 (bottom),
// so screen-top maps to V=0 and screen-bottom maps to V=1.
#[rustfmt::skip]
const QUAD: [f32; 16] = [
    // x,    y,    u,   v
    -1.0, -1.0,  0.0, 1.0,  // bottom-left
     1.0, -1.0,  1.0, 1.0,  // bottom-right
     1.0,  1.0,  1.0, 0.0,  // top-right
    -1.0,  1.0,  0.0, 0.0,  // top-left
];

pub struct GlGameRenderer {
    program: GLuint,
    vao: GLuint,
    vbo: GLuint,
    texture: GLuint,
    tex_w: usize,
    tex_h: usize,
    rgba_buf: Vec<u8>,
}

impl GlGameRenderer {
    pub fn new() -> Self {
        unsafe {
            let vs = compile_shader(gl::VERTEX_SHADER, VS_SRC);
            let fs = compile_shader(gl::FRAGMENT_SHADER, FS_SRC);
            let program = gl::CreateProgram();
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
            gl::LinkProgram(program);
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);

            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (QUAD.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
                QUAD.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let stride = 4 * std::mem::size_of::<f32>() as GLsizei;
            let a_pos = gl::GetAttribLocation(program, c_str("a_pos").as_ptr());
            gl::EnableVertexAttribArray(a_pos as GLuint);
            gl::VertexAttribPointer(a_pos as GLuint, 2, gl::FLOAT, gl::FALSE, stride, ptr::null());

            let a_uv = gl::GetAttribLocation(program, c_str("a_uv").as_ptr());
            gl::EnableVertexAttribArray(a_uv as GLuint);
            gl::VertexAttribPointer(
                a_uv as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );

            gl::BindVertexArray(0);

            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
            gl::BindTexture(gl::TEXTURE_2D, 0);

            Self {
                program,
                vao,
                vbo,
                texture,
                tex_w: 0,
                tex_h: 0,
                rgba_buf: Vec::new(),
            }
        }
    }

    pub fn upload_frame(&mut self, frame: &[u32], w: usize, h: usize) {
        let pixel_count = w * h;
        self.rgba_buf.resize(pixel_count * 4, 0xFF);
        for (i, &pixel) in frame.iter().take(pixel_count).enumerate() {
            let off = i * 4;
            self.rgba_buf[off] = ((pixel >> 16) & 0xFF) as u8;
            self.rgba_buf[off + 1] = ((pixel >> 8) & 0xFF) as u8;
            self.rgba_buf[off + 2] = (pixel & 0xFF) as u8;
            // alpha stays 0xFF from resize
        }

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            if w != self.tex_w || h != self.tex_h {
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RGBA8 as GLint,
                    w as GLsizei,
                    h as GLsizei,
                    0,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    self.rgba_buf.as_ptr() as *const _,
                );
                self.tex_w = w;
                self.tex_h = h;
            } else {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    0,
                    0,
                    w as GLsizei,
                    h as GLsizei,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    self.rgba_buf.as_ptr() as *const _,
                );
            }
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    /// Draw the game quad into the given viewport region, letter/pillar-boxed
    /// to maintain the original aspect ratio (tex_w : tex_h).
    /// `vp_x, vp_y` are in GL coordinates (Y from bottom).
    pub fn draw(&self, vp_x: i32, vp_y: i32, vp_w: i32, vp_h: i32) {
        if vp_w <= 0 || vp_h <= 0 || self.tex_w == 0 || self.tex_h == 0 {
            return;
        }

        // Compute letterbox/pillarbox viewport preserving source aspect ratio
        let src_aspect = self.tex_w as f64 / self.tex_h as f64;
        let dst_aspect = vp_w as f64 / vp_h as f64;

        let (fit_w, fit_h) = if dst_aspect > src_aspect {
            // Destination is wider → pillarbox (black bars on sides)
            let h = vp_h;
            let w = (vp_h as f64 * src_aspect).round() as i32;
            (w, h)
        } else {
            // Destination is taller → letterbox (black bars top/bottom)
            let w = vp_w;
            let h = (vp_w as f64 / src_aspect).round() as i32;
            (w, h)
        };

        let fit_x = vp_x + (vp_w - fit_w) / 2;
        let fit_y = vp_y + (vp_h - fit_h) / 2;

        unsafe {
            gl::Viewport(fit_x, fit_y, fit_w, fit_h);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::SCISSOR_TEST);
            gl::UseProgram(self.program);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_FAN, 0, 4);
            gl::BindVertexArray(0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
        }
    }
}

impl Drop for GlGameRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.texture);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteProgram(self.program);
        }
    }
}

fn c_str(s: &str) -> CString {
    CString::new(s).unwrap()
}

unsafe fn compile_shader(kind: GLenum, src: &str) -> GLuint {
    unsafe {
        let shader = gl::CreateShader(kind);
        let c_src = c_str(src);
        gl::ShaderSource(shader, 1, &c_src.as_ptr(), ptr::null());
        gl::CompileShader(shader);
        shader
    }
}
