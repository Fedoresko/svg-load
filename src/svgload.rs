use std::collections::HashMap;
use std::convert::TryInto;

use lyon::math::Point;
use lyon::path::PathEvent;
use lyon::tessellation::*;
use usvg::{LinearGradient, NodeKind, Paint, Transform, Tree};
use crate::path::{RenderablePath, GpuVertex};

pub fn load_svg(filename: &str) -> Vec<RenderablePath> {
    let opt = usvg::Options::default();
    let file_data = std::fs::read(filename).unwrap();
    let rtree = Tree::from_data(&file_data, &opt.to_ref()).unwrap();

    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    let mut gradients: HashMap<String, LinearGradient> = HashMap::new();
    let mut primitives : Vec<RenderablePath> = Vec::new();

    for node in rtree.root().descendants() {
        let data = &*node.borrow();
        match data {
            NodeKind::Svg(_) => {}
            NodeKind::Defs => {}
            NodeKind::LinearGradient(gradient) => {
                gradients.insert(gradient.id.clone(), gradient.clone());
            }
            NodeKind::RadialGradient(_) => {}
            NodeKind::ClipPath(_) => {}
            NodeKind::Mask(_) => {}
            NodeKind::Pattern(_) => {}
            NodeKind::Filter(_) => {}
            NodeKind::Path(path) => {
                let t = &data.transform();
                let paint = &path.fill.as_ref().unwrap().paint;
                let mut mesh: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();

                fill_tess
                    .tessellate(
                        convert_path(&path),
                        &FillOptions::tolerance(0.01),
                        &mut BuffersBuilder::new(
                            &mut mesh,
                            VertexCtor {
                                prim_id: primitives.len() as u32 - 1,
                                transform: t.clone(),
                            },
                        ),
                    )
                    .expect("Error during tesselation!");


                if (path.stroke.is_some()) {
                    let mut mesh_s: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
                    let stroke = path.stroke.as_ref().unwrap();
                    let opts = convert_stroke(stroke);
                    stroke_tess.tessellate(
                        convert_path(path),
                        &opts.with_tolerance(0.01),
                        &mut BuffersBuilder::new(
                            &mut mesh_s,
                            VertexCtor {
                                prim_id: primitives.len() as u32 - 1,
                                transform: t.clone(),
                            },
                        ),
                    );

                    let stoke_p = primitive_from_paint(&mut gradients, stroke.opacity.value() as f32, mesh_s, &stroke.paint);
                    primitives.push(stoke_p);
                }

                let path = primitive_from_paint(&mut gradients,  path.fill.as_ref().unwrap().opacity.value() as f32,mesh, paint);
                primitives.push(path);
            }
            NodeKind::Image(_) => {}
            NodeKind::Group(_) => {}
        }
    }
    primitives
}

fn primitive_from_paint(gradients: &mut HashMap<String, LinearGradient>, opacity: f32, mut mesh_s: VertexBuffers<GpuVertex, u32>, paint: &Paint) -> RenderablePath {
    match paint {
        Paint::Color(col) => {
            RenderablePath::fromColor(col, opacity,mesh_s)
        }
        Paint::Link(link) => {
            let grad = gradients.get(link);
            if grad.is_some() {
                RenderablePath::fromGradient(grad.unwrap(), mesh_s)
            } else {
                RenderablePath::new(mesh_s)
            }
        }
    }
}

pub struct VertexCtor {
    pub prim_id: u32,
    pub transform: Transform,
}

impl FillVertexConstructor<GpuVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> GpuVertex {
        let position = vertex.position().to_array();
        let vec = self.transform.apply(position[0] as f64, position[1] as f64);
        GpuVertex {
            position: [vec.0 as f32, vec.1 as f32],
            prim_id: self.prim_id,
        }
    }
}

impl StrokeVertexConstructor<GpuVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> GpuVertex {
        GpuVertex {
            position: vertex.position().to_array(),
            prim_id: self.prim_id,
        }
    }
}

fn point(x: &f64, y: &f64) -> Point {
    Point::new((*x) as f32, (*y) as f32)
}

pub struct PathConvIter<'a> {
    iter: std::slice::Iter<'a, usvg::PathSegment>,
    prev: Point,
    first: Point,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl<'l> Iterator for PathConvIter<'l> {
    type Item = PathEvent;
    fn next(&mut self) -> Option<PathEvent> {
        if self.deferred.is_some() {
            return self.deferred.take();
        }

        let next = self.iter.next();
        match next {
            Some(usvg::PathSegment::MoveTo { x, y }) => {
                if self.needs_end {
                    let last = self.prev;
                    let first = self.first;
                    self.needs_end = false;
                    self.prev = point(x, y);
                    self.deferred = Some(PathEvent::Begin { at: self.prev });
                    self.first = self.prev;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    self.first = point(x, y);
                    self.needs_end = true;
                    Some(PathEvent::Begin { at: self.first })
                }
            }
            Some(usvg::PathSegment::LineTo { x, y }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Line {
                    from,
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::CurveTo {
                     x1,
                     y1,
                     x2,
                     y2,
                     x,
                     y,
                 }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Cubic {
                    from,
                    ctrl1: point(x1, y1),
                    ctrl2: point(x2, y2),
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::ClosePath) => {
                self.needs_end = false;
                self.prev = self.first;
                Some(PathEvent::End {
                    last: self.prev,
                    first: self.first,
                    close: true,
                })
            }
            None => {
                if self.needs_end {
                    self.needs_end = false;
                    let last = self.prev;
                    let first = self.first;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    None
                }
            }
        }
    }
}

pub fn convert_path(p: &usvg::Path) -> PathConvIter {
    PathConvIter {
        iter: p.data.iter(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
        deferred: None,
        needs_end: false,
    }
}

pub fn convert_stroke(s: &usvg::Stroke) -> StrokeOptions {
    let linecap = match s.linecap {
        usvg::LineCap::Butt => LineCap::Butt,
        usvg::LineCap::Square => LineCap::Square,
        usvg::LineCap::Round => LineCap::Round,
    };
    let linejoin = match s.linejoin {
        usvg::LineJoin::Miter => LineJoin::Miter,
        usvg::LineJoin::Bevel => LineJoin::Bevel,
        usvg::LineJoin::Round => LineJoin::Round,
    };

    StrokeOptions::tolerance(0.01)
        .with_line_width(s.width.value() as f32)
        .with_line_cap(linecap)
        .with_line_join(linejoin)
}