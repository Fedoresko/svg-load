use lyon::tessellation::VertexBuffers;
use usvg::{Color, LinearGradient};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RenderablePath {
    pub bgcolor: [f32; 4],
    pub gradient_stops: u8,
    pub gradient_pos: Option<Vec<f32>>,
    pub gradient_colors: Option<Vec<[f32; 4]>>,
    pub gradient_start: Option<(f32, f32)>,
    pub gradient_end: Option<(f32, f32)>,
    pub vertices: VertexBuffers<GpuVertex, u32>,
}

impl RenderablePath {
    pub fn fromColor(col: &Color, opacity: f32, mesh: VertexBuffers<GpuVertex, u32>) -> Self {
        RenderablePath {
            bgcolor: [col.red as f32 / 256.0, col.green as f32 / 256.0, col.blue as f32 / 256.0, opacity],
            gradient_stops: 0,
            gradient_colors: None,
            gradient_pos: None,
            gradient_start: None,
            gradient_end: None,
            vertices: mesh,
        }
    }

    pub fn fromGradient(g: &LinearGradient, mesh: VertexBuffers<GpuVertex, u32>) -> Self {
        let n = g.stops.len();
        let t = g.transform;
        let start = t.apply(g.x1, g.y1);
        let end = t.apply(g.x2, g.y2);
        RenderablePath {
            bgcolor: [1.0, 1.0, 1.0, 1.0],
            gradient_stops: n as u8,
            gradient_colors: Some(g.stops.iter().map(|s| [s.color.red as f32 / 256.0, s.color.green as f32 / 256.0, s.color.blue as f32 / 256.0, s.opacity.value() as f32]).collect()),
            gradient_pos: Some(g.stops.iter().map(|s| s.offset.value() as f32).collect()),
            gradient_start: Some((start.0 as f32, start.1 as f32)),
            gradient_end: Some((end.0 as f32, end.1 as f32)),
            vertices: mesh,
        }
    }

    pub fn new(mesh: VertexBuffers<GpuVertex, u32>) -> Self {
        RenderablePath {
            bgcolor: [1.0, 1.0, 1.0, 1.0],
            gradient_stops: 0,
            gradient_colors: None,
            gradient_pos: None,
            gradient_start: None,
            gradient_end: None,
            vertices: mesh,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GpuVertex {
    pub position: [f32; 2],
    pub prim_id: u32,
}
