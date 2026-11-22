//! Text rendering via cosmic-text, reusing the sprite pipeline.

use std::collections::HashMap;

use cosmic_text::{
    Align, Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
    fontdb,
};
use engine_core::{FilterMode, SpriteInstance, Text, TextAlign, TextInstance, TextureHandle};
use glam::{Mat3, Vec2};

const ATLAS_WIDTH: u32 = 1024;
const ATLAS_HEIGHT: u32 = 1024;
const ATLAS_PADDING: u32 = 1;

pub struct GlyphAtlas {
    pixels: Vec<u8>,
    next_x: u32,
    next_y: u32,
    row_height: u32,
    dirty: bool,
}

impl GlyphAtlas {
    fn new() -> Self {
        Self { pixels: vec![0; (ATLAS_WIDTH * ATLAS_HEIGHT * 4) as usize], next_x: 0, next_y: 0, row_height: 0, dirty: true }
    }

    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 {
            return Some((0, 0));
        }
        let padded_w = w + ATLAS_PADDING;
        let padded_h = h + ATLAS_PADDING;
        if self.next_x + padded_w > ATLAS_WIDTH {
            self.next_x = 0;
            self.next_y += self.row_height + ATLAS_PADDING;
            self.row_height = 0;
        }
        if self.next_y + padded_h > ATLAS_HEIGHT {
            return None;
        }
        let x = self.next_x;
        let y = self.next_y;
        self.next_x += padded_w;
        self.row_height = self.row_height.max(padded_h);
        Some((x, y))
    }

    fn write_glyph(&mut self, x: u32, y: u32, w: u32, h: u32, data: &[u8], content: SwashContent) {
        for row in 0..h {
            for col in 0..w {
                let dst = ((y + row) * ATLAS_WIDTH + (x + col)) as usize;
                match content {
                    SwashContent::Mask => {
                        let src = (row * w + col) as usize;
                        let alpha = data[src];
                        self.pixels[dst * 4 + 0] = 255;
                        self.pixels[dst * 4 + 1] = 255;
                        self.pixels[dst * 4 + 2] = 255;
                        self.pixels[dst * 4 + 3] = alpha;
                    }
                    SwashContent::Color => {
                        let src = ((row * w + col) * 4) as usize;
                        self.pixels[dst * 4 + 0] = data[src + 0];
                        self.pixels[dst * 4 + 1] = data[src + 1];
                        self.pixels[dst * 4 + 2] = data[src + 2];
                        self.pixels[dst * 4 + 3] = data[src + 3];
                    }
                    SwashContent::SubpixelMask => {
                        let src = ((row * w + col) * 3) as usize;
                        let avg = ((data[src] as u32 + data[src + 1] as u32 + data[src + 2] as u32) / 3) as u8;
                        self.pixels[dst * 4 + 0] = 255;
                        self.pixels[dst * 4 + 1] = 255;
                        self.pixels[dst * 4 + 2] = 255;
                        self.pixels[dst * 4 + 3] = avg;
                    }
                }
            }
        }
        self.dirty = true;
    }

    fn take_dirty(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some((self.pixels.clone(), ATLAS_WIDTH, ATLAS_HEIGHT))
    }
}

#[derive(Debug, Clone, Copy)]
struct GlyphSlot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: GlyphAtlas,
    cache: HashMap<cosmic_text::CacheKey, Option<GlyphSlot>>,
}

impl TextRenderer {
    pub fn new(font_data: Vec<u8>) -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(font_data);
        let font_system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
        let swash_cache = SwashCache::new();
        let atlas = GlyphAtlas::new();
        Self { font_system, swash_cache, atlas, cache: HashMap::new() }
    }

    pub fn measure(&mut self, text: &str, size: f32, max_width: f32) -> (f32, f32) {
        let line_height = size * 1.2;
        let metrics = Metrics::new(size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(Some(max_width), None);
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);
        let mut width = 0.0f32;
        let mut height = 0.0f32;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height += line_height;
        }
        (width.ceil(), height.max(line_height))
    }

    pub fn layout(
        &mut self,
        text_instances: &[TextInstance],
        atlas_texture: TextureHandle,
    ) -> (Vec<SpriteInstance>, Option<(Vec<u8>, u32, u32)>) {
        let mut out = Vec::new();
        for inst in text_instances {
            let text = &inst.text;
            let line_height = text.size * 1.2;
            let metrics = Metrics::new(text.size, line_height);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            // None = no wrapping; alignment then applies to the line's own width.
            buffer.set_size(text.max_width, None);
            let attrs = Attrs::new();
            buffer.set_text(&text.content, &attrs, Shaping::Advanced, None);
            let align = match text.align {
                TextAlign::Left => Align::Left,
                TextAlign::Center => Align::Center,
                TextAlign::Right => Align::Right,
            };
            for line in buffer.lines.iter_mut() {
                line.set_align(Some(align));
            }
            buffer.shape_until_scroll(&mut self.font_system, false);

            for run in buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    // physical() gives the integer pixel position and the cache key.
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let cache_key = physical.cache_key;
                    let slot = if let Some(cached) = self.cache.get(&cache_key) {
                        *cached
                    } else {
                        let slot = self.rasterize_glyph(cache_key);
                        self.cache.insert(cache_key, slot);
                        slot
                    };
                    let Some(slot) = slot else { continue };
                    if slot.w == 0 || slot.h == 0 {
                        continue;
                    }
                    // Top-left of the glyph bitmap, Y-down. `top` is the distance from the
                    // baseline UP to the bitmap's top edge, so we SUBTRACT it from the baseline.
                    let glyph_x = (physical.x + slot.left) as f32;
                    let glyph_y = run.line_y.round() + (physical.y - slot.top) as f32;
                    let center = Vec2::new(glyph_x + slot.w as f32 * 0.5, glyph_y + slot.h as f32 * 0.5);
                    let transform = inst.transform * Mat3::from_translation(center);
                    let u0 = slot.x as f32 / ATLAS_WIDTH as f32;
                    let v0 = slot.y as f32 / ATLAS_HEIGHT as f32;
                    let u1 = (slot.x + slot.w) as f32 / ATLAS_WIDTH as f32;
                    let v1 = (slot.y + slot.h) as f32 / ATLAS_HEIGHT as f32;
                    out.push(SpriteInstance {
                        transform,
                        size: Vec2::new(slot.w as f32, slot.h as f32),
                        uv: [u0, v0, u1, v1],
                        z_index: text.z_index,
                        color: text.color,
                        texture: atlas_texture,
                    });
                }
            }
        }
        let atlas_update = self.atlas.take_dirty();
        (out, atlas_update)
    }

    fn rasterize_glyph(&mut self, cache_key: cosmic_text::CacheKey) -> Option<GlyphSlot> {
        let image = self.swash_cache.get_image(&mut self.font_system, cache_key).as_ref()?;
        let w = image.placement.width;
        let h = image.placement.height;
        if w == 0 || h == 0 {
            return Some(GlyphSlot { x: 0, y: 0, w: 0, h: 0, left: 0, top: 0 });
        }
        let (x, y) = self.atlas.alloc(w, h)?;
        self.atlas.write_glyph(x, y, w, h, &image.data, image.content);
        Some(GlyphSlot { x, y, w, h, left: image.placement.left, top: image.placement.top })
    }

    pub fn atlas_filter(&self) -> FilterMode {
        FilterMode::Nearest
    }
}
