use bytemuck::{Pod, Zeroable};
use engine_core::{Camera2D, SpriteInstance};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 3], // mat3x3 aligned to 3x4
}

pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    capacity: u32,
    num_indices: u32,
}

impl SpritePipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite uniform buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite pipeline layout"),
            bind_group_layouts: &[&uniform_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8, shader_location: 1 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 2 },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let initial_capacity = 1024;
        let vertex_buffer = Self::create_vertex_buffer(device, initial_capacity);
        let index_buffer = Self::create_index_buffer(device, initial_capacity);

        Self {
            pipeline, uniform_buffer, uniform_bind_group, texture_layout,
            vertex_buffer, index_buffer, capacity: initial_capacity, num_indices: 0,
        }
    }

    pub fn create_texture_bind_group(
        &self, device: &wgpu::Device, view: &wgpu::TextureView, filter: engine_core::FilterMode,
    ) -> wgpu::BindGroup {
        let mag_min = match filter {
            engine_core::FilterMode::Nearest => wgpu::FilterMode::Nearest,
            engine_core::FilterMode::Linear => wgpu::FilterMode::Linear,
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: mag_min, min_filter: mag_min, ..Default::default()
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite texture bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        })
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, camera: &Camera2D, width: f32, height: f32, sprites: &[SpriteInstance]) {
        let view_proj = camera.view_proj(width, height);
        let uniforms = Uniforms {
            view_proj: [
                [view_proj.x_axis.x, view_proj.x_axis.y, 0.0, 0.0],
                [view_proj.y_axis.x, view_proj.y_axis.y, 0.0, 0.0],
                [view_proj.z_axis.x, view_proj.z_axis.y, 1.0, 0.0],
            ],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let count = sprites.len() as u32;
        if count == 0 { self.num_indices = 0; return; }

        if count > self.capacity {
            self.capacity = count.next_power_of_two();
            self.vertex_buffer = Self::create_vertex_buffer(device, self.capacity);
            self.index_buffer = Self::create_index_buffer(device, self.capacity);
        }

        let mut vertices = Vec::with_capacity((count * 4) as usize);
        let mut indices = Vec::with_capacity((count * 6) as usize);

        for (i, sprite) in sprites.iter().enumerate() {
            let base = (i * 4) as u16;
            let hw = sprite.size.x * 0.5;
            let hh = sprite.size.y * 0.5;
            
            // Local corners (center is 0,0)
            let tl = sprite.transform.transform_point2(glam::Vec2::new(-hw, -hh));
            let tr = sprite.transform.transform_point2(glam::Vec2::new(hw, -hh));
            let br = sprite.transform.transform_point2(glam::Vec2::new(hw, hh));
            let bl = sprite.transform.transform_point2(glam::vec2(-hw, hh));

            vertices.push(Vertex { pos: [tl.x, tl.y], uv: [sprite.uv[0], sprite.uv[1]], color: sprite.color });
            vertices.push(Vertex { pos: [tr.x, tr.y], uv: [sprite.uv[2], sprite.uv[1]], color: sprite.color });
            vertices.push(Vertex { pos: [br.x, br.y], uv: [sprite.uv[2], sprite.uv[3]], color: sprite.color });
            vertices.push(Vertex { pos: [bl.x, bl.y], uv: [sprite.uv[0], sprite.uv[3]], color: sprite.color });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        self.num_indices = indices.len() as u32;
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, sprites: &[SpriteInstance], texture_bind_groups: &std::collections::HashMap<engine_core::TextureHandle, &'a wgpu::BindGroup>) {
        if self.num_indices == 0 { return; }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let mut current_tex = None;
        let mut chunk_start = 0;

        for (i, sprite) in sprites.iter().enumerate() {
            if Some(sprite.texture) != current_tex {
                if let Some(tex) = current_tex {
                    if let Some(bg) = texture_bind_groups.get(&tex) {
                        let count = (i as u32 - chunk_start) * 6;
                        pass.set_bind_group(1, *bg, &[]);
                        pass.draw_indexed((chunk_start * 6)..((chunk_start * 6) + count), 0, 0..1);
                    }
                }
                current_tex = Some(sprite.texture);
                chunk_start = i as u32;
            }
        }
        if let Some(tex) = current_tex {
            if let Some(bg) = texture_bind_groups.get(&tex) {
                let count = (sprites.len() as u32 - chunk_start) * 6;
                pass.set_bind_group(1, *bg, &[]);
                pass.draw_indexed((chunk_start * 6)..((chunk_start * 6) + count), 0, 0..1);
            }
        }
    }

    fn create_vertex_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite vertex buffer"),
            size: (capacity * 4 * std::mem::size_of::<Vertex>() as u32) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_index_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite index buffer"),
            size: (capacity * 6 * std::mem::size_of::<u16>() as u32) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}