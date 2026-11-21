use std::sync::Arc;
use engine::{
    run, AssetCache, Camera2D, FilterMode, ImageAsset, InputState, KeyCode, PlatformApp, Renderer,
    Scene, Sprite, Transform2D, Window, WindowEvent, glam::Vec2, core::TextureHandle,
};

struct Demo {
    renderer: Option<Renderer>,
    assets: AssetCache,
    scene: Scene,
    camera: Camera2D,
    exit_requested: bool,
    bg_tex: TextureHandle,
    f_tex: TextureHandle,
}

fn make_f_texture() -> ImageAsset {
    let size = 16;
    let mut pixels = vec![0; size * size * 4];
    let mut set = |x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * size + x) * 4) as usize;
        pixels[i..i+4].copy_from_slice(&c);
    };
    // Dark grey background
    for y in 0..size { for x in 0..size { set(x, y, [40, 40, 40, 255]); } }
    // White "F"
    let w = [255, 255, 255, 255];
    for y in 2..size-2 { set(4, y, w); set(5, y, w); } // Vertical
    for x in 4..12 { set(x, 2, w); set(x, 3, w); } // Top bar
    for x in 4..10 { set(x, 7, w); set(x, 8, w); } // Middle bar
    
    ImageAsset {
        meta: engine::TextureMeta { width: size, height: size, filter: FilterMode::Nearest, label: "test:F".into() },
        pixels,
    }
}

impl Demo {
    fn new() -> Self {
        Self {
            renderer: None,
            assets: AssetCache::new(),
            scene: Scene::new(),
            camera: Camera2D { position: Vec2::new(640.0, 360.0), zoom: 1.0 },
            exit_requested: false,
            bg_tex: TextureHandle::from_raw(0),
            f_tex: TextureHandle::from_raw(0),
        }
    }
}

impl PlatformApp for Demo {
    fn resumed(&mut self, window: Arc<Window>) {
        let mut r = Renderer::new(window).expect("renderer");
        
        // 1. Load assets
        self.bg_tex = self.assets.insert(ImageAsset::solid([100, 30, 40, 255], FilterMode::Linear, "bg"));
        self.f_tex = self.assets.insert(make_f_texture());

        // 2. Upload to GPU
        r.upload_texture(self.bg_tex, self.assets.get(self.bg_tex).unwrap());
        r.upload_texture(self.f_tex, self.assets.get(self.f_tex).unwrap());

        // 3. Spawn Scene Nodes
        let bg_node = self.scene.spawn_with("bg", Transform2D::from_translation(Vec2::new(640.0, 360.0)));
        self.scene.set_sprite(bg_node, Some(Sprite {
            texture: self.bg_tex, size: Vec2::new(400.0, 600.0), color: [1.0, 1.0, 1.0, 1.0], z_index: 0,
        })).unwrap();

        let f_node = self.scene.spawn_with("f", Transform2D::from_translation(Vec2::new(640.0, 360.0)));
        self.scene.set_sprite(f_node, Some(Sprite {
            texture: self.f_tex, size: Vec2::new(200.0, 200.0), color: [1.0, 1.0, 1.0, 1.0], z_index: 1,
        })).unwrap();

        self.renderer = Some(r);
    }

    fn window_event(&mut self, event: WindowEvent, _window: &Window) {
        if let WindowEvent::Resized(size) = event {
            if let Some(r) = &mut self.renderer { r.resize(size.width, size.height); }
        }
    }

    fn fixed_update(&mut self, _dt: f32, input: &InputState) {
        if input.key_pressed(KeyCode::Escape) { self.exit_requested = true; }
    }

    fn redraw(&mut self, alpha: f32) {
        let Some(r) = &mut self.renderer else { return };
        
        // Collect sprites using interpolated transforms
        let sprites = self.scene.collect_sprites(alpha);
        
        if let Err(e) = r.render(&self.camera, &sprites) {
            log::error!("render failed: {e}");
        }
    }

    fn should_exit(&self) -> bool { self.exit_requested }
}

fn main() {
    env_logger::init();
    run(Demo::new()).expect("run");
}