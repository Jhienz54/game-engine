/// Errors returned by the renderer.
#[derive(Debug, thiserror::Error)]
/// Errors returned by the renderer.
pub enum RendererError {
    /// Failed to create the wgpu surface from the window.
    #[error("surface creation failed: {0}")]
    SurfaceCreation(#[from] wgpu::CreateSurfaceError),
    /// No adapter compatible with the surface was found.
    ///
    /// wgpu 24 does not re-export `RequestAdapterError` from its top
    /// level; we stringify it here. It is diagnostic only — if you see
    /// this, the fix is environment (`WGPU_BACKEND`, drivers), not code.
    #[error("no compatible GPU adapter found: {0}")]
    Adapter(String),
    /// Failed to create the logical device.
    #[error("device creation failed: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    /// The surface reported no supported texture formats.
    #[error("surface has no supported texture formats")]
    NoSurfaceFormats,
    /// Out of GPU memory during present — unrecoverable.
    #[error("out of GPU memory")]
    OutOfMemory,
}