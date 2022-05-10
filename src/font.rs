use std::collections::HashMap;
use lyon::tessellation::VertexBuffers;
use crate::path::GpuVertex;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Font {
    pub name: String,
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
    pub glyph_map: HashMap<u32, Glyph>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Glyph {
    pub advance: f32,
    pub bbox: (f32, f32, f32, f32),
    pub outline: VertexBuffers<GpuVertex, u32>,
}