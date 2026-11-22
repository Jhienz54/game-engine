use serde::{Deserialize, Serialize};

/// Horizontal alignment of text within its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlign {
    /// Align to the left edge.
    #[default]
    Left,
    /// Center horizontally.
    Center,
    /// Align to the right edge.
    Right,
}

/// A text component attached to a node. The node position is the top-left of the text block (Y-down).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    /// The string to render.
    pub content: String,
    /// Font size in pixels.
    pub size: f32,
    /// RGBA color tint.
    pub color: [f32; 4],
    /// Maximum width for wrapping. `None` means no wrapping.
    pub max_width: Option<f32>,
    /// Horizontal alignment within `max_width`.
    pub align: TextAlign,
    /// Higher values draw on top.
    pub z_index: i32,
}

impl Text {
    /// Create a text component with defaults.
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into(), size: 16.0, color: [1.0; 4], max_width: None, align: TextAlign::Left, z_index: 10 }
    }
}

/// A resolved text instance ready for the renderer.
#[derive(Debug, Clone)]
pub struct TextInstance {
    /// Interpolated world transform (origin of the text block).
    pub transform: glam::Mat3,
    /// The text data to render.
    pub text: Text,
}
