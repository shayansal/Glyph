use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextError {
    #[error("text run cannot be empty")]
    EmptyRun,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontDescriptor {
    pub family: String,
    pub source: String,
}

impl FontDescriptor {
    pub fn system(family: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            source: "system".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub font_size: f32,
    pub dpi_scale: f32,
    pub clip: Option<ClipRect>,
}

impl TextRun {
    pub fn new(text: impl Into<String>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            font_size,
            dpi_scale: 1.0,
            clip: None,
        }
    }

    pub fn with_dpi_scale(mut self, dpi_scale: f32) -> Self {
        self.dpi_scale = dpi_scale.max(0.1);
        self
    }

    pub fn with_clip(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.clip = Some(ClipRect {
            x,
            y,
            width,
            height,
        });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapedGlyph {
    pub cluster: usize,
    pub ch: char,
    pub advance: f32,
    pub x: f32,
    pub y: f32,
    pub font_family: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapedRun {
    pub cache_key: String,
    pub text: String,
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
    pub dpi_scale: f32,
    pub clip: Option<ClipRect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RasterizedRun {
    pub cache_key: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub alpha_pixels: Vec<u8>,
    pub clipped_bounds: ClipRect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextCacheStats {
    pub hits: usize,
    pub misses: usize,
}

#[derive(Clone, Debug)]
pub struct TextEngine {
    primary: FontDescriptor,
    fallbacks: Vec<FontDescriptor>,
    cached: std::collections::BTreeMap<String, RasterizedRun>,
    stats: TextCacheStats,
}

impl TextEngine {
    pub fn new() -> Self {
        Self {
            primary: FontDescriptor::system("system-ui"),
            fallbacks: Vec::new(),
            cached: std::collections::BTreeMap::new(),
            stats: TextCacheStats::default(),
        }
    }

    pub fn with_font(mut self, font: FontDescriptor) -> Self {
        self.primary = font;
        self
    }

    pub fn with_fallback(mut self, font: FontDescriptor) -> Self {
        self.fallbacks.push(font);
        self
    }

    pub fn shape(&self, run: &TextRun) -> Result<ShapedRun, TextError> {
        if run.text.is_empty() {
            return Err(TextError::EmptyRun);
        }
        let advance = run.font_size * run.dpi_scale * 0.58;
        let line_height = run.font_size * run.dpi_scale * 1.25;
        let mut cursor = 0.0;
        let glyphs = run
            .text
            .chars()
            .enumerate()
            .map(|(cluster, ch)| {
                let font_family = if ch.is_ascii() || self.fallbacks.is_empty() {
                    self.primary.family.clone()
                } else {
                    self.fallbacks[0].family.clone()
                };
                let glyph = ShapedGlyph {
                    cluster,
                    ch,
                    advance,
                    x: cursor,
                    y: 0.0,
                    font_family,
                };
                cursor += advance;
                glyph
            })
            .collect::<Vec<_>>();
        Ok(ShapedRun {
            cache_key: stable_key(&run.text, run.font_size, run.dpi_scale, run.clip),
            text: run.text.clone(),
            width: cursor,
            height: line_height,
            dpi_scale: run.dpi_scale,
            clip: run.clip,
            glyphs,
        })
    }

    pub fn rasterize(&mut self, shaped: &ShapedRun) -> Result<RasterizedRun, TextError> {
        if let Some(cached) = self.cached.get(&shaped.cache_key) {
            self.stats.hits += 1;
            return Ok(cached.clone());
        }
        self.stats.misses += 1;
        let bounds = shaped.clip.unwrap_or(ClipRect {
            x: 0.0,
            y: 0.0,
            width: shaped.width.ceil(),
            height: shaped.height.ceil(),
        });
        let atlas_width = bounds.width.ceil().max(1.0) as u32;
        let atlas_height = bounds.height.ceil().max(1.0) as u32;
        let mut alpha_pixels = vec![0; (atlas_width * atlas_height) as usize];
        for glyph in &shaped.glyphs {
            let start_x = glyph.x.floor().max(0.0) as u32;
            let end_x = ((glyph.x + glyph.advance).ceil() as u32).min(atlas_width);
            let end_y = (shaped.height.ceil() as u32).min(atlas_height);
            for y in 0..end_y {
                for x in start_x..end_x {
                    let index = (y * atlas_width + x) as usize;
                    alpha_pixels[index] = alpha_pixels[index].max(48 + (glyph.cluster % 128) as u8);
                }
            }
        }
        let raster = RasterizedRun {
            cache_key: shaped.cache_key.clone(),
            atlas_width,
            atlas_height,
            alpha_pixels,
            clipped_bounds: bounds,
        };
        self.cached.insert(shaped.cache_key.clone(), raster.clone());
        Ok(raster)
    }

    pub fn cache_stats(&self) -> TextCacheStats {
        self.stats
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn stable_key(text: &str, font_size: f32, dpi_scale: f32, clip: Option<ClipRect>) -> String {
    let clip = clip
        .map(|rect| format!("{}:{}:{}:{}", rect.x, rect.y, rect.width, rect.height))
        .unwrap_or_else(|| "none".to_string());
    format!("{text}:{font_size:.2}:{dpi_scale:.2}:{clip}")
}
