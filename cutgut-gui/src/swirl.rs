use std::sync::Arc;

use eframe::{
    egui,
    glow::{self, HasContext},
};

pub struct Swirl {
    gl: Arc<glow::Context>,
    program: glow::Program,
    vao: glow::VertexArray,
}

impl Swirl {
    pub fn new(gl: Arc<glow::Context>) -> Option<Arc<Self>> {
        unsafe {
            let program = gl.create_program().ok()?;
            let mut shaders = Vec::new();
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = gl.create_shader(kind).ok()?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    gl.delete_shader(shader);
                    for shader in shaders {
                        gl.delete_shader(shader);
                    }
                    gl.delete_program(program);
                    return None;
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            if !gl.get_program_link_status(program) {
                gl.delete_program(program);
                return None;
            }
            let vao = match gl.create_vertex_array() {
                Ok(vao) => vao,
                Err(_) => {
                    gl.delete_program(program);
                    return None;
                }
            };
            Some(Arc::new(Self { gl, program, vao }))
        }
    }

    pub fn paint(
        self: &Arc<Self>,
        painter: &egui::Painter,
        rect: egui::Rect,
        time: f32,
        opacity: f32,
    ) {
        let shader = self.clone();
        painter.add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _| unsafe {
                let gl = &shader.gl;
                gl.use_program(Some(shader.program));
                gl.bind_vertex_array(Some(shader.vao));
                gl.uniform_1_f32(
                    gl.get_uniform_location(shader.program, "phase").as_ref(),
                    time,
                );
                gl.uniform_1_f32(
                    gl.get_uniform_location(shader.program, "opacity").as_ref(),
                    opacity,
                );
                gl.uniform_1_f32(
                    gl.get_uniform_location(shader.program, "aspect").as_ref(),
                    info.viewport.width() / info.viewport.height().max(1.0),
                );
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                gl.bind_vertex_array(None);
                gl.use_program(None);
            })),
        });
    }
}

impl Drop for Swirl {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_program(self.program);
        }
    }
}

const VERTEX: &str = r#"#version 330
out vec2 uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

const FRAGMENT: &str = r#"#version 330
in vec2 uv;
out vec4 color;
uniform float phase;
uniform float opacity;
uniform float aspect;
void main() {
    vec2 p = uv - 0.5;
    p.x *= aspect;
    float radius = length(p);
    float angle = atan(p.y, p.x);
    float wave = angle * 5.0 + 7.5 * log(max(radius, 0.004)) - phase;
    float v = 0.5 + 0.5 * sin(wave);
    float aa = max(fwidth(v) * 0.8, 0.004);
    vec3 a = vec3(0.15, 0.018, 0.035);
    vec3 b = vec3(0.72, 0.31, 0.40);
    vec3 c = vec3(0.96, 0.77, 0.77);
    vec3 rgb = mix(a, b, smoothstep(0.32-aa, 0.32+aa, v));
    rgb = mix(rgb, c, smoothstep(0.69-aa, 0.69+aa, v));
    rgb = mix(vec3(0.20, 0.04, 0.065), rgb, smoothstep(0.002, 0.014, radius));
    color = vec4(rgb * opacity, opacity);
}
"#;
