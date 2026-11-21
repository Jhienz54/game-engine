use std::collections::HashMap;
use std::sync::Arc;

use engine_core::{Camera2D, FilterMode, ImageAsset, SpriteInstance, TextureHandle};
use winit::window::Window;

use crate::RendererError;
use crate::sprite_pipeline::SpritePipeline;

struct GpuTexture {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub struct Renderer {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    clear_color: wgpu::Color,
    
    sprite_pipeline: SpritePipeline,
    textures: HashMap<TextureHandle, GpuTexture>,
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Result<Self, RendererError> {
        let backends = wgpu::Backends::from_env().unwrap_or_else(wgpu::Backends::all);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
        let surface = instance.create_surface(window.clone())?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).ok_or_else(|| RendererError::Adapter("No adapter".into()))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor { label: Some("device"), required_limits: wgpu::Limits::downlevel_webgl2_defaults(), ..Default::default() },
            None,
        ))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).or_else(|| caps.formats.first().copied()).ok_or(RendererError::NoSurfaceFormats)?;
        
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let sprite_pipeline = SpritePipeline::new(&device, format);

        Ok(Self {
            instance, surface, device, queue, config,
            clear_color: wgpu::Color { r: 0.1, g: 0.1, b: 0.16, a: 1.0 },
            sprite_pipeline,
            textures: HashMap::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Uploads pixels to the GPU and registers them under the given handle.
    pub fn upload_texture(&mut self, handle: TextureHandle, asset: &ImageAsset) {
        let size = wgpu::Extent3d { width: asset.meta.width, height: asset.meta.height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&asset.meta.label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            texture.as_image_copy(),
            &asset.pixels,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * asset.meta.width), rows_per_image: None },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.sprite_pipeline.create_texture_bind_group(&self.device, &view, asset.meta.filter);
        
        self.textures.insert(handle, GpuTexture { _texture: texture, bind_group });
    }

    pub fn render(&mut self, camera: &Camera2D, sprites: &[SpriteInstance]) -> Result<(), RendererError> {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(wgpu::SurfaceError::OutOfMemory) => return Err(RendererError::OutOfMemory),
            Err(e) => { log::warn!("Surface error: {:?}", e); return Ok(()); }
        };

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        self.sprite_pipeline.prepare(&self.device, &self.queue, camera, self.config.width as f32, self.config.height as f32, sprites);

        let bind_groups: HashMap<_, _> = self.textures.iter().map(|(k, v)| (*k, &v.bind_group)).collect();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(self.clear_color), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sprite_pipeline.draw(&mut pass, sprites, &bind_groups);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}