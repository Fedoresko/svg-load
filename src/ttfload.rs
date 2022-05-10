use std::collections::HashMap;
use std::path::PathBuf;

use lyon::math::Point;
use lyon::path::PathEvent;
use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, VertexBuffers};
use ttf_parser as ttf;
use ttf_parser::Rect;
use usvg::{Error, Transform};

use crate::font::{Font, Glyph};
use crate::path::GpuVertex;
use crate::svgload::VertexCtor;

const FONT_SIZE: f64 = 1.0;

pub fn load_font(filename: &str, symbols: &str) -> Result<Font, Box<dyn std::error::Error>> {
    let path_buf = PathBuf::from(filename);
    let font_data = std::fs::read(&path_buf)?;

    #[allow(unused_mut)]
        let mut face = ttf::Face::from_slice(&font_data, 0)?;
    // if face.is_variable() {
    //     #[cfg(feature = "variable-fonts")] {
    //         for variation in args.variations {
    //         face.set_variation(variation.axis, variation.value)
    //         .ok_or("failed to create variation coordinates")?;
    //         }
    //     }
    // }

    let units_per_em = face.units_per_em();
    let scale = FONT_SIZE / units_per_em as f64;
    let mut fill_tess = FillTessellator::new();

    let mut g_map = HashMap::new();

    // for encoding in face.tables().cmap.unwrap().subtables {
    //     if !encoding.is_unicode() { continue; }
    //     encoding.codepoints(|cp| {
    //         g_map.insert(cp, encoding.glyph_index(cp).unwrap());
    //     });
    // }


    for ch in symbols.chars() {
        g_map.insert(u32::from(ch), face.glyph_index(ch).unwrap());
    }

    let mut glyphs = HashMap::new();

    for (cp, id) in g_map {
        if let Some(_) = face.glyph_raster_image(id, std::u16::MAX) {
            panic!("Raster fonts not supported!")
        } else if let Some(_) = face.glyph_svg_image(id) {
            panic!("Raster fonts not supported!")
        } else {
            let mut mesh: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();

            let mut builder = Builder::new();
            let ok = builder.build(&face, id).is_ok();

            let mut bbox = (0.0,0.0,0.0,0.0);
            if ok {
                let mut transform = Transform::new_translate(0.0,
                                                             -face.glyph_y_origin(id).unwrap_or(0) as f64);
                transform.scale(scale, scale);
                bbox = (builder.bbbox.x_min as f64, builder.bbbox.y_min as f64, builder.bbbox.x_max as f64,builder.bbbox.y_max as f64);
                fill_tess
                    .tessellate(
                        builder,
                        &FillOptions::tolerance(0.5),
                        &mut BuffersBuilder::new(
                            &mut mesh,
                            VertexCtor {
                                prim_id: id.0 as u32,
                                transform,
                            },
                        ),
                    )
                    .expect("Error during tesselation!");
                transform.apply_to(&mut bbox.0,&mut bbox.1);
                transform.apply_to(&mut bbox.2,&mut bbox.3);
            }

            glyphs.insert(cp, Glyph {
                advance: face.glyph_hor_advance(id).unwrap() as f32 * scale as f32,
                outline: mesh,
                bbox: (bbox.0 as f32, bbox.1 as f32, bbox.2 as f32, bbox.3 as f32),
            });
        }
    }


    let font = Font {
        name: path_buf.file_name().unwrap().to_str().unwrap().into(),
        ascender: face.ascender() as f32 * scale as f32,
        descender: face.descender() as f32 * scale as f32,
        line_gap: face.line_gap() as f32 * scale as f32,
        glyph_map: glyphs,
    };

    Ok(font)
}

struct Builder {
    vec: Vec<PathEvent>,
    needs_end: bool,
    prev: Point,
    first: Point,
    bbbox: Rect,
}

impl ttf::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.needs_end {
            let last = self.prev;
            let first = self.first;
            self.needs_end = false;
            self.prev = point(x, y);
            self.first = self.prev;
            self.vec.push(PathEvent::End {
                last,
                first,
                close: false,
            });
            self.vec.push(PathEvent::Begin { at: point(x, y) });
        } else {
            self.first = point(x, y);
            self.needs_end = true;
            self.vec.push(PathEvent::Begin { at: self.first });
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = point(x, y);
        self.vec.push(PathEvent::Line {
            from,
            to: self.prev,
        });
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = point(x, y);
        self.vec.push(PathEvent::Quadratic {
            from,
            ctrl: point(x1, y1),
            to: self.prev,
        });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = point(x, y);
        self.vec.push(PathEvent::Cubic {
            from,
            ctrl1: point(x1, y1),
            ctrl2: point(x2, y2),
            to: self.prev,
        });
    }

    fn close(&mut self) {
        self.needs_end = false;
        self.prev = self.first;
        self.vec.push(PathEvent::End {
            last: self.prev,
            first: self.first,
            close: true,
        });
    }
}

impl Builder {
    fn new() -> Self {
        Builder {
            vec: Vec::new(),
            first: Point::new(0.0, 0.0),
            prev: Point::new(0.0, 0.0),
            needs_end: false,
            bbbox: Rect{x_min:0,x_max:0,y_min:0,y_max:0},
        }
    }

    fn build(&mut self, face: &ttf::Face,
             glyph_id: ttf::GlyphId) -> Result<&Self, Box<dyn std::error::Error>> {
        self.bbbox = match face.outline_glyph(glyph_id, self) {
            Some(v) => v,
            None => return Err(Box::new(Error::InvalidSize)),
        };

        Ok(self)
    }

}

impl IntoIterator for Builder {
    type Item = PathEvent;
    type IntoIter = std::vec::IntoIter<PathEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

fn point(x: f32, y: f32) -> Point {
    Point::new(x, y)
}