use glam::{Mat3, Vec2};
use serde::{Deserialize, Serialize};

/// A 2D orthographic camera.
///
/// ## Coordinate System
/// We use a **Y-down** coordinate system (standard for UI and 2D games).
/// - World origin `(0,0)` is the top-left.
/// - Positive X goes right, positive Y goes down.
/// - The camera's `position` is the center of the view in world space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera2D {
    /// Center of the view in world coordinates.
    pub position: Vec2,
    /// Zoom level (1.0 = 1 pixel per pixel).
    pub zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self { position: Vec2::ZERO, zoom: 1.0 }
    }
}

impl Camera2D {
    /// Creates the projection matrix for a given window size.
    /// Maps world coordinates (Y-down) to Normalized Device Coordinates
    /// (NDC, where Y is up: -1 is bottom, 1 is top).
    pub fn view_proj(&self, width: f32, height: f32) -> Mat3 {
        // Map world X to NDC X (-1 to 1): scale = 2.0 / width
        // Map world Y to NDC Y (1 to -1): scale = -2.0 / height
        let scale = Mat3::from_scale(Vec2::new(
            2.0 * self.zoom / width,
            -2.0 * self.zoom / height,
        ));
        let translate = Mat3::from_translation(-self.position);
        scale * translate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_maps_to_origin() {
        let cam = Camera2D { position: Vec2::new(100.0, 100.0), zoom: 1.0 };
        let vp = cam.view_proj(200.0, 200.0);
        let p = vp.transform_point2(Vec2::new(100.0, 100.0));
        assert!(p.abs_diff_eq(Vec2::ZERO, 1e-5));
    }

    #[test]
    fn corners_map_to_ndc_and_are_y_down() {
        let cam = Camera2D { position: Vec2::new(100.0, 100.0), zoom: 1.0 };
        let vp = cam.view_proj(200.0, 200.0);
        
        // World Top-Left (0,0) maps to NDC (-1, 1)
        let tl = vp.transform_point2(Vec2::new(0.0, 0.0));
        assert!(tl.abs_diff_eq(Vec2::new(-1.0, 1.0), 1e-5), "TL: {:?}", tl);
        
        // World Bottom-Right (200,200) maps to NDC (1, -1)
        let br = vp.transform_point2(Vec2::new(200.0, 200.0));
        assert!(br.abs_diff_eq(Vec2::new(1.0, -1.0), 1e-5), "BR: {:?}", br);
    }

    #[test]
    fn zoom_scales_around_center() {
        let cam = Camera2D { position: Vec2::ZERO, zoom: 2.0 };
        let vp = cam.view_proj(100.0, 100.0);
        // With zoom 2.0, a point at (25, 25) reaches the top-right corner of NDC.
        let p = vp.transform_point2(Vec2::new(25.0, 25.0));
        assert!(p.abs_diff_eq(Vec2::new(1.0, -1.0), 1e-5), "P: {:?}", p);
    }
}