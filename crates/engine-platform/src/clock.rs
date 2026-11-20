//! Fixed-timestep accumulator. See `app.rs` for the loop it drives.

use std::time::Instant;

/// Default fixed step rate. Change via [`Clock::with_hz`].
pub(crate) const DEFAULT_FIXED_HZ: f32 = 60.0;

/// Maximum frame delta we will integrate. Anything longer (OS suspend,
/// breakpoint, system hiccup) is clamped, preventing a "spiral of
/// death" where the engine tries to catch up indefinitely.
pub(crate) const MAX_FRAME_DT: f32 = 0.25;

pub(crate) struct Clock {
    fixed_dt: f32,
    accumulator: f32,
    last: Instant,
    last_frame_dt: f32,
}

impl Clock {
    pub(crate) fn new(fixed_hz: f32) -> Self {
        Self {
            fixed_dt: 1.0 / fixed_hz,
            accumulator: 0.0,
            last: Instant::now(),
            last_frame_dt: 0.0,
        }
    }

    /// Advance the accumulator by wall-clock time since the last call.
    /// Returns the (possibly clamped) frame delta.
    pub(crate) fn begin_frame(&mut self) -> f32 {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(MAX_FRAME_DT);
        self.last = now;
        self.accumulator += dt;
        self.last_frame_dt = dt;
        dt
    }

    /// Consume one fixed step's worth of accumulated time, if any.
    /// Returns `true` while there is work to do.
    pub(crate) fn consume_fixed(&mut self) -> bool {
        if self.accumulator >= self.fixed_dt {
            self.accumulator -= self.fixed_dt;
            true
        } else {
            false
        }
    }

    /// Fraction of the way through the current fixed step, `[0, 1)`.
    /// Pass to `redraw` for interpolation.
    pub(crate) fn alpha(&self) -> f32 {
        (self.accumulator / self.fixed_dt).clamp(0.0, 1.0)
    }

    pub(crate) fn fixed_dt(&self) -> f32 {
        self.fixed_dt
    }

    /// Frame delta of the most recent `begin_frame`. Not yet consumed
    /// by callers in M3; used by the M8 `update(dt)` hook.
    #[allow(dead_code)]
    pub(crate) fn frame_dt(&self) -> f32 {
        self.last_frame_dt
    }
}