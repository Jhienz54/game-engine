/// Errors returned by the platform layer.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// Creating the event loop failed.
    #[error("event loop error: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
}