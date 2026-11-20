use glam::{Mat3, Vec2};
use serde::{Deserialize, Serialize};

/// Local or world 2D affine transform.
///
/// Composition order matches the classic convention:
/// `parent_world = parent_chain * self.to_matrix()`. `Scene` handles
/// hierarchy composition; this type only knows about a single node's
/// own placement in its parent's space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    /// Position in parent space.
    pub translation: Vec2,
    /// Rotation in radians, counter-clockwise.
    pub rotation: f32,
    /// Non-uniform scale.
    pub scale: Vec2,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    /// Identity: no translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec2::ZERO,
        rotation: 0.0,
        scale: Vec2::ONE,
    };

    /// Translation-only transform.
    pub const fn from_translation(translation: Vec2) -> Self {
        Self {
            translation,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }

    /// Rotation-only transform.
    pub const fn from_rotation(rotation: f32) -> Self {
        Self {
            translation: Vec2::ZERO,
            rotation,
            scale: Vec2::ONE,
        }
    }

    /// Scale-only transform.
    pub const fn from_scale(scale: Vec2) -> Self {
        Self {
            translation: Vec2::ZERO,
            rotation: 0.0,
            scale,
        }
    }

    /// Composed 3×3 affine matrix: `Translation * Rotation * Scale`.
    pub fn to_matrix(self) -> Mat3 {
        Mat3::from_scale_angle_translation(self.scale, self.rotation, self.translation)
    }

    /// Interpolate between two transforms for render-time smoothing.
    ///
    /// - Translation and scale are linearly interpolated.
    /// - Rotation is interpolated via the shortest arc (wrapped to
    ///   `[-π, π]` first), so going from `+0.9π` to `-0.9π` rotates the
    ///   *short* way around rather than nearly a full turn.
    ///
    /// Designed for `alpha ∈ [0, 1]` between two adjacent fixed steps.
    /// Do not use for animation timing — separate concern.
    pub fn lerp(a: Self, b: Self, alpha: f32) -> Self {
        use std::f32::consts::PI;

        let mut dr = b.rotation - a.rotation;
        // Wrap the delta into [-PI, PI].
        while dr > PI {
            dr -= 2.0 * PI;
        }
        while dr < -PI {
            dr += 2.0 * PI;
        }

        Self {
            translation: a.translation.lerp(b.translation, alpha),
            rotation: a.rotation + dr * alpha,
            scale: a.scale.lerp(b.scale, alpha),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn identity_matrix() {
        let m = Transform2D::IDENTITY.to_matrix();
        let p = m.transform_point2(Vec2::new(3.0, 4.0));
        approx(p.x, 3.0);
        approx(p.y, 4.0);
    }

    #[test]
    fn rotation_quarter_turn() {
        let t = Transform2D::from_rotation(PI / 2.0);
        let p = t.to_matrix().transform_point2(Vec2::new(1.0, 0.0));
        approx(p.x, 0.0);
        approx(p.y, 1.0);
    }

    #[test]
    fn lerp_endpoints() {
        let a = Transform2D::from_translation(Vec2::new(0.0, 0.0));
        let b = Transform2D::from_translation(Vec2::new(10.0, 20.0));
        approx(Transform2D::lerp(a, b, 0.0).translation.x, 0.0);
        approx(Transform2D::lerp(a, b, 1.0).translation.x, 10.0);
        approx(Transform2D::lerp(a, b, 0.5).translation.y, 10.0);
    }

    #[test]
    fn lerp_rotation_takes_short_arc() {
        // From +0.9 PI to -0.9 PI: short arc is -0.2 PI (through PI).
        let a = Transform2D::from_rotation(0.9 * PI);
        let b = Transform2D::from_rotation(-0.9 * PI);
        let mid = Transform2D::lerp(a, b, 0.5);
        // Midway between 0.9 PI and 1.1 PI (i.e. -0.9 PI wrapped).
        approx(mid.rotation, PI);
    }
}