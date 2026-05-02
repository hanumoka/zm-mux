//! Solid-color rect drawing for GpuBackend (cursor outline + pane borders).
//!
//! Phase 1.3.9-B-3.  This module owns a tiny wgpu pipeline whose vertex
//! buffer contains pre-tessellated rectangles in NDC space with per-vertex
//! color.  Each rect is two triangles (six vertices, no index buffer) —
//! cheap enough at terminal scales (~tens of rects per frame) that we
//! re-create the vertex buffer per frame instead of reusing one.

use wgpu::util::DeviceExt;

const RECT_SHADER: &str = r#"
struct VertexInput {
    @location(0) pos_ndc: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.pos_ndc, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct RectVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

pub struct RectPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl RectPipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("zm-mux rect shader"),
            source: wgpu::ShaderSource::Wgsl(RECT_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("zm-mux rect pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("zm-mux rect pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }

    /// Build a vertex buffer for the given rects and bind it on the pass
    /// for a single draw call.  Returns the buffer so the caller keeps
    /// it alive for the duration of the render pass.
    pub fn build_buffer(
        &self,
        device: &wgpu::Device,
        vertices: &[RectVertex],
    ) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zm-mux rect vertices"),
            contents: vertex_bytes(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        buffer: &'a wgpu::Buffer,
        vertex_count: u32,
    ) {
        if vertex_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..vertex_count, 0..1);
    }
}

fn vertex_bytes(verts: &[RectVertex]) -> &[u8] {
    let len = std::mem::size_of_val(verts);
    // Safety: RectVertex is repr(C), Copy, contains only f32 — no padding,
    // no interior pointers.
    unsafe { std::slice::from_raw_parts(verts.as_ptr() as *const u8, len) }
}

/// Append a filled rectangle in window pixel space.  Color must already
/// be in linear RGB (call srgb_to_linear on sRGB input).  width/height
/// are the surface size in pixels.
pub fn push_rect(
    out: &mut Vec<RectVertex>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: [f32; 4],
    surface_w: u32,
    surface_h: u32,
) {
    if w <= 0 || h <= 0 || surface_w == 0 || surface_h == 0 {
        return;
    }
    let sw = surface_w as f32;
    let sh = surface_h as f32;
    let to_ndc = |px: i32, py: i32| -> [f32; 2] {
        let nx = (px as f32 / sw) * 2.0 - 1.0;
        let ny = -((py as f32 / sh) * 2.0 - 1.0);
        [nx, ny]
    };
    let p00 = to_ndc(x, y);
    let p10 = to_ndc(x + w, y);
    let p01 = to_ndc(x, y + h);
    let p11 = to_ndc(x + w, y + h);
    let v = |pos: [f32; 2]| RectVertex { pos, color };
    // Two triangles: (00, 10, 01) and (10, 11, 01)
    out.push(v(p00));
    out.push(v(p10));
    out.push(v(p01));
    out.push(v(p10));
    out.push(v(p11));
    out.push(v(p01));
}
