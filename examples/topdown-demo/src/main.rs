//! M2 demo: opens a window, clears to a colour, quits on Escape.
//! Input arrives in M3; sprites in M6; gameplay in M9.

use std::sync::Arc;

use engine::{run, PlatformApp, Renderer, Window, WindowEvent};

struct Demo {
    renderer: Option<Renderer>,
}

impl PlatformApp for Demo {
    fn resumed(&mut self, window: Arc<Window>) {
        match Renderer::new(window) {
            Ok(r) => {
                log::info!("M2: renderer ready");
                self.renderer = Some(r);
            }
            Err(e) => {
                log::error!("M2: renderer init failed: {e}");
                std::process::exit(1);
            }
        }
    }

    fn window_event(&mut self, event: WindowEvent, _window: &Window) {
        if let WindowEvent::Resized(size) = event {
            if let Some(r) = &mut self.renderer {
                r.resize(size.width, size.height);
            }
        }
    }

    fn redraw(&mut self, _window: &Window) {
        let Some(r) = &mut self.renderer else {
            return;
        };
        if let Err(e) = r.render() {
            log::error!("render failed: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    log::info!("topdown-demo: engine v{} — M2", engine::VERSION);

    if let Err(e) = run(Demo { renderer: None }) {
        log::error!("platform error: {e}");
        std::process::exit(1);
    }
}