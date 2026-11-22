use std::sync::Arc;
use std::time::Instant;

use engine::glam::Vec2;
use engine::{
    run, AssetCache, Camera2D, FilterMode, ImageAsset, InputState, KeyCode, NodeId, PlatformApp,
    Renderer, Scene, Sprite, Text, TextAlign, TextureHandle, TextureMeta, Transform2D, Window,
    WindowEvent,
};

struct Demo {
    renderer: Option<Renderer>,
    assets: AssetCache,
    scene: Scene,
    camera: Camera2D,
    exit_requested: bool,
    fps_node: Option<NodeId>,
    last_fps_update: Instant,
    frames_since_update: u32,
}

fn make_f_texture() -> ImageAsset {
    let size: u32 = 16;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let mut set = |x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * size + x) * 4) as usize;
        pixels[i..i + 4].copy_from_slice(&c);
    };
    for y in 0..size {
        for x in 0..size {
            set(x, y, [40, 40, 40, 255]);
        }
    }
    let w = [255, 255, 255, 255];
    for y in 2..size - 2 {
        set(4, y, w);
        set(5, y, w);
    }
    for x in 4..12 {
        set(x, 2, w);
        set(x, 3, w);
    }
    for x in 4..10 {
        set(x, 7, w);
        set(x, 8, w);
    }
    ImageAsset {
        meta: TextureMeta { width: size, height: size, filter: FilterMode::Nearest, label: "test:F".into() },
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
            fps_node: None,
            last_fps_update: Instant::now(),
            frames_since_update: 0,
        }
    }

    fn upload(&mut self, r: &mut Renderer, handle: TextureHandle) {
        if let Some(asset) = self.assets.get(handle) {
            r.upload_texture(handle, asset);
        }
    }
}

impl PlatformApp for Demo {
    fn resumed(&mut self, window: Arc<Window>) {
        let mut r = Renderer::new(window).expect("renderer");

        // 1. Assets: a card-colored tile and the "F" orientation test.
        let bg_tex = self.assets.insert(ImageAsset::solid([100, 30, 40, 255], FilterMode::Linear, "bg"));
        let f_tex = self.assets.insert(make_f_texture());
        self.upload(&mut r, bg_tex);
        self.upload(&mut r, f_tex);

        // 2. Scene: the red card (400x600), the F on top of it.
        let card = self.scene.spawn_with("card", Transform2D::from_translation(Vec2::new(640.0, 360.0)));
        let _ = self.scene.set_sprite(
            card,
            Some(Sprite { texture: bg_tex, size: Vec2::new(400.0, 600.0), color: [1.0; 4], z_index: 0 }),
        );
        let f_node = self.scene.spawn_with("f_test", Transform2D::from_translation(Vec2::new(640.0, 360.0)));
        let _ = self.scene.set_sprite(
            f_node,
            Some(Sprite { texture: f_tex, size: Vec2::new(200.0, 200.0), color: [1.0; 4], z_index: 1 }),
        );

        // 3. Text. A text node's position is the TOP-LEFT of its box.
        //    The card spans x 440..840, y 60..660, so a 400px-wide box at x=440 is centered on it.
        let title = self.scene.spawn_with("title", Transform2D::from_translation(Vec2::new(440.0, 80.0)));
        let _ = self.scene.set_text(
            title,
            Some(Text {
                content: "Thalassophobia".into(),
                size: 32.0,
                color: [1.0; 4],
                max_width: Some(400.0),
                align: TextAlign::Center,
                z_index: 10,
            }),
        );

        // 4. FPS overlay (top-left of the window).
        let fps = self.scene.spawn_with("fps", Transform2D::from_translation(Vec2::new(10.0, 10.0)));
        let _ = self.scene.set_text(
            fps,
            Some(Text {
                content: "FPS: --".into(),
                size: 16.0,
                color: [1.0, 1.0, 0.0, 1.0],
                max_width: None,
                align: TextAlign::Left,
                z_index: 1000,
            }),
        );
        self.fps_node = Some(fps);

        self.renderer = Some(r);
    }

    fn window_event(&mut self, event: WindowEvent, _window: &Window) {
        if let WindowEvent::Resized(size) = event {
            if let Some(r) = &mut self.renderer {
                r.resize(size.width, size.height);
            }
        }
    }

    fn fixed_update(&mut self, _dt: f32, input: &InputState) {
        if input.key_pressed(KeyCode::Escape) {
            self.exit_requested = true;
        }
    }

    fn redraw(&mut self, alpha: f32) {
        // FPS: wall-clock time, for DISPLAY only. It never reaches fixed_update or game state.
        self.frames_since_update += 1;
        let elapsed = self.last_fps_update.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            if let Some(id) = self.fps_node {
                let fps = self.frames_since_update as f32 / elapsed;
                if let Ok(Some(text)) = self.scene.text(id) {
                    let mut t = text.clone();
                    t.content = format!("FPS: {fps:.0}");
                    let _ = self.scene.set_text(id, Some(t));
                }
            }
            self.frames_since_update = 0;
            self.last_fps_update = Instant::now();
        }

        let Some(r) = &mut self.renderer else { return };
        let sprites = self.scene.collect_sprites(alpha);
        let texts = self.scene.collect_text_instances(alpha);
        if let Err(e) = r.render(&self.camera, &sprites, &texts) {
            log::error!("render failed: {e}");
        }
    }

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run(Demo::new()) {
        log::error!("platform error: {e}");
        std::process::exit(1);
    }
}
