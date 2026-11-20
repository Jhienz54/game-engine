use std::sync::Arc;

use engine_core::InputState;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::clock::{Clock, DEFAULT_FIXED_HZ};
use crate::input::PendingInput;
use crate::PlatformError;

/// The winit-facing app trait. Games implement this to receive fixed
/// updates and per-frame redraws.
///
/// ## Update cadence
///
/// - [`fixed_update`](Self::fixed_update) is called 0 or more times per
///   frame at `1 / fixed_hz` intervals (default 60 Hz). It receives a
///   fresh [`InputState`] snapshot each call. If a frame runs no fixed
///   steps, the game does not update — wall-clock elapsed time is
///   buffered for the next frame.
/// - [`redraw`](Self::redraw) is called exactly once per frame. The
///   `alpha` argument is the fraction of the way through the current
///   fixed step (`[0, 1)`); interpolate your visuals with it.
pub trait PlatformApp {
    /// Called once after the window is created. Initialize GPU resources
    /// here.
    fn resumed(&mut self, window: Arc<Window>);

    /// Raw window events forwarded after the engine handles its own
    /// (close, Escape, redraw). Use for resize, focus, etc.
    fn window_event(&mut self, event: WindowEvent, window: &Window);

    /// Fixed-timestep simulation tick. `dt` is always the fixed step
    /// interval (`1 / 60 s` by default). `input` is a fresh snapshot.
    fn fixed_update(&mut self, dt: f32, input: &InputState);

    /// Variable-rate draw. `alpha ∈ [0, 1)` is the interpolation factor
    /// through the current fixed step.
    fn redraw(&mut self, alpha: f32);
}

struct Wrapper<A: PlatformApp> {
    app: A,
    window: Option<Arc<Window>>,
    clock: Clock,
    pending: PendingInput,
    last_alpha: f32,
}

impl<A: PlatformApp> ApplicationHandler for Wrapper<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("myengine")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0));
        match event_loop.create_window(attrs) {
            Ok(w) => {
                let window = Arc::new(w);
                self.window = Some(window.clone());
                self.app.resumed(window);
            }
            Err(e) => {
                log::error!("window creation failed: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else { return };

        // Engine-consumed events.
        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::KeyboardInput { event: ke, .. } => {
                if ke.state == ElementState::Pressed
                    && matches!(ke.logical_key, Key::Named(NamedKey::Escape))
                {
                    event_loop.exit();
                    return;
                }
                if let PhysicalKey::Code(code) = ke.physical_key {
                    self.pending.key_event(code, ke.state, ke.repeat);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.pending.mouse_button_event(*button, *state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pending.cursor_moved(position.x, position.y);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.pending.scroll(*delta);
            }
            WindowEvent::RedrawRequested => {
                self.app.redraw(self.last_alpha);
                return;
            }
            _ => {}
        }

        // Forward to the game for anything it might care about (resize,
        // focus change, …).
        self.app.window_event(event, &window);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.clock.begin_frame();

        // Run fixed steps. Each step gets a fresh `InputState` snapshot;
        // the first consumes pending edges, subsequent steps see none.
        while self.clock.consume_fixed() {
            let input = self.pending.take_snapshot();
            self.app.fixed_update(self.clock.fixed_dt(), &input);
        }

        self.last_alpha = self.clock.alpha();

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

/// Run the platform loop. Blocks until the loop exits.
pub fn run<A: PlatformApp + 'static>(app: A) -> Result<(), PlatformError> {
    let event_loop = EventLoop::new()?;
    let mut wrapper = Wrapper {
        app,
        window: None,
        clock: Clock::new(DEFAULT_FIXED_HZ),
        pending: PendingInput::default(),
        last_alpha: 0.0,
    };
    event_loop.run_app(&mut wrapper)?;
    Ok(())
}