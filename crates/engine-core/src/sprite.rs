use crate::TextureHandle;
use glam::{Mat3, Vec2};
use serde::{Deserialize, Serialize};

/// A 2D sprite component attached to a node.
///
/// The node's `Transform2D` determines the **center** of the sprite.
/// To place a sprite at the exact top-left of the screen, set the node's
/// translation to `(width/2, height/2)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub texture: TextureHandle,
    pub size: Vec2,
    pub color: [f32; 4],
    pub z_index: i32,
}

/// A fully resolved sprite ready for the renderer.
/// Produced by `Scene::collect_sprites`.
#[derive(Debug, Clone)]
pub struct SpriteInstance {
    pub transform: Mat3,
    pub size: Vec2,
    pub z_index: i32,
    pub color: [f32; 4],
    pub texture: TextureHandle,
}