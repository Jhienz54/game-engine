use std::sync::Arc;

use winit::window::Window;

use crate::RendererError;

/// The 2D renderer. Currently does one thing: clear the screen to a
/// fixed colour. Pipeline, batching, and camera arrive in M6.
pub struct Renderer {
    /// Kept alive so a future second surface (e.g. editor preview) can
    /// be created without re-creating the instance.
    #[allow(dead_code)]
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    clear_color: wgpu::Color,
}

impl Renderer {
    /// Initialize wgpu against `window`, log the chosen adapter, and
    /// configure the surface for an SRGB present. Called from
    /// `PlatformApp::resumed`.
    pub fn new(window: Arc<Window>) -> Result<Self, RendererError> {
        // Respect WGPU_BACKEND if set; otherwise enable every backend
        // wgpu knows about. On Linux this yields Vulkan + GL, so a
        // working Vulkan driver wins, and llvmpipe / GLES is the
        // fallback. We log the outcome below.
        // Respect WGPU_BACKEND if set; otherwise enable every backend
        // wgpu knows about. On Linux this yields Vulkan + GL, so a
        // working Vulkan driver wins, and llvmpipe / GLES is the
        // fallback. We log the outcome below.
        //
        // wgpu 24 removed `util::backend_bits_from_env`; `Backends::from_env`
        // is the replacement.
        let backends = wgpu::Backends::from_env().unwrap_or_else(wgpu::Backends::all);
        log::info!("wgpu: instance request backends={backends:?}");

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        // wgpu 24's `request_adapter` returns `Option<Adapter>`, not
        // `Result` (that changed in wgpu 25). `None` here means no
        // backend could match the requested backend bits — usually an
        // environment problem (`WGPU_BACKEND` mis-set, or drivers
        // missing), not a code problem.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| {
            RendererError::Adapter(format!(
                "no adapter matched requested backends {backends:?} — \
                 check WGPU_BACKEND and installed GPU drivers"
            ))
        })?;

        let info = adapter.get_info();
        log::info!(
            "wgpu adapter: name={:?} backend={:?} device_type={:?} driver={:?}",
            info.name,
            info.backend,
            info.device_type,
            info.driver,
        );

        // wgpu 24's request_device takes (desc, trace_path: Option<&Path>).
        // The `trace_path` argument only matters when the `trace` feature
        // is enabled; `None` is correct for our build.
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("engine-render device"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                ..Default::default()
            },
            None,
        ))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or(RendererError::NoSurfaceFormats)?;
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!(
            "wgpu surface configured: {}x{} format={:?} present_mode={:?}",
            config.width,
            config.height,
            format,
            config.present_mode,
        );

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            // Soft blue-grey — easy to distinguish from a black
            // "nothing rendered" frame.
            clear_color: wgpu::Color {
                r: 0.10,
                g: 0.10,
                b: 0.16,
                a: 1.0,
            },
        })
    }

    /// Resize the surface. Ignores zero-sized requests (which happen
    /// when the window is minimized and would panic inside wgpu).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Draw one frame: acquire, clear, present. Handles surface-loss
    /// recovery transparently by reconfiguring and skipping the frame.
    pub fn render(&mut self) -> Result<(), RendererError> {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Surface invalidated (resize, GPU reset). Reconfigure
                // and skip — next frame will succeed.
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(wgpu::SurfaceError::OutOfMemory) => return Err(RendererError::OutOfMemory),
            Err(other) => {
                log::warn!("surface acquire failed: {other:?}");
                return Ok(());
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    /// Set the clear colour for future frames. Components are linear
    /// `[0, 1]`. Used by the M3 demo to show interpolation; the game's
    /// `redraw(alpha)` typically computes this from two states lerped
    /// by `alpha`.
    pub fn set_clear_color(&mut self, rgba: [f32; 4]) {
        self.clear_color = wgpu::Color {
            r: rgba[0] as f64,
            g: rgba[1] as f64,
            b: rgba[2] as f64,
            a: rgba[3] as f64,
        };
    }
}