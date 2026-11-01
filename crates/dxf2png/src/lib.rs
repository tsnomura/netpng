use std::collections::HashSet;
use std::f64::consts::PI;

use anyhow::{bail, Context};
use clap::Parser;
use dxf::entities::{Entity, EntityType};
use dxf::{Drawing, Point as DxfPoint, Vector};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform,
};

/// Fixed stroke width in pixels.
const LINE_WIDTH: f32 = 1.0;
/// Upper bound on the rendered canvas in pixels.
const MAX_PIXELS: u64 = 160_000_000;
/// Longer canvas side when no size option is given.
const DEFAULT_LONG_SIDE: u32 = 1024;

/// Rasterize a DXF drawing to PNG (parsed with the `dxf` crate, rendered
/// with `tiny-skia`).
///
/// Supported entities: LINE, CIRCLE, ARC, ELLIPSE, LWPOLYLINE, POLYLINE
/// (including bulges), SPLINE (drawn as a polyline through its fit or
/// control points), POINT, and INSERT (block references, resolved
/// recursively). Only model space is drawn; text and all other entity
/// types are ignored. Without a size option the longer side is 1024 px.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Input path (DXF), "-" or omitted for stdin.
    #[arg(value_name = "INPUT", conflicts_with = "input_flag")]
    input: Option<String>,
    /// Same as INPUT, as a flag (give one or the other, not both).
    #[arg(short = 'i', long = "input", value_name = "INPUT")]
    input_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Exact canvas size in pixels, "W H"; the drawing is stretched to
    /// fill it (overrides --width/--height).
    #[arg(long, num_args = 2, value_names = ["W", "H"],
          value_parser = clap::value_parser!(u32).range(1..),
          conflicts_with_all = ["width", "height"])]
    xysize: Vec<u32>,
    /// Canvas width in pixels; the height is derived from the drawing's
    /// aspect ratio.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..),
          conflicts_with_all = ["xysize", "height"])]
    width: Option<u32>,
    /// Canvas height in pixels; the width is derived from the drawing's
    /// aspect ratio.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..),
          conflicts_with_all = ["xysize", "width"])]
    height: Option<u32>,
    /// Background color, "R,G,B" or "R,G,B,A".
    #[arg(long, default_value = "255,255,255", conflicts_with = "transparent")]
    background: String,
    /// Stroke color, "R,G,B" or "R,G,B,A".
    #[arg(long, default_value = "0,0,0")]
    foreground: String,
    /// Transparent background (no fill).
    #[arg(long)]
    transparent: bool,
    #[arg(long, value_enum, default_value = "fast")]
    compression: netpng_core::CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let input = args
        .input_flag
        .as_deref()
        .or(args.input.as_deref())
        .unwrap_or("-");
    let mut reader = netpng_core::open_input(input)?;
    let drawing = Drawing::load(&mut reader)
        .with_context(|| format!("failed to parse DXF input {input:?}"))?;

    let geoms = collect_drawing(&drawing);
    if geoms.is_empty() {
        bail!(
            "DXF drawing contains no renderable model-space geometry (LINE, CIRCLE, ARC, \
             ELLIPSE, LWPOLYLINE, POLYLINE, SPLINE, POINT, INSERT)"
        );
    }

    let bbox = bounds(&geoms);
    let xysize = (args.xysize.len() == 2).then(|| (args.xysize[0], args.xysize[1]));
    let (w, h) = canvas_size(xysize, args.width, args.height, bbox.2 - bbox.0, bbox.3 - bbox.1);
    if (w as u64) * (h as u64) > MAX_PIXELS {
        bail!(
            "canvas is {w}x{h} pixels; the limit is {MAX_PIXELS} pixels \
             (use --xysize/--width/--height to shrink it)"
        );
    }

    let mut pixmap: Pixmap = Pixmap::new(w, h).context("failed to allocate canvas")?;
    if !args.transparent {
        let [r, g, b, a] = netpng_core::parse_color(&args.background)?;
        pixmap.fill(Color::from_rgba8(r, g, b, a));
    }
    let [fr, fg, fb, fa] = netpng_core::parse_color(&args.foreground)?;

    let view = make_view(bbox, w, h, xysize.is_some());

    let mut paint = Paint::default();
    paint.set_color_rgba8(fr, fg, fb, fa);
    let stroke = Stroke {
        width: LINE_WIDTH,
        line_cap: LineCap::Round,
        line_join: LineJoin::Miter,
        ..Default::default()
    };
    let id = Transform::identity();

    for g in &geoms {
        if let Some((path, fill)) = build_path(g, &view) {
            if fill {
                pixmap.fill_path(&path, &paint, FillRule::Winding, id, None);
            } else {
                pixmap.stroke_path(&path, &paint, &stroke, id, None);
            }
        }
    }

    let pixels = pixmap.take_demultiplied();
    let img = netpng_core::Image {
        width: w,
        height: h,
        pixels,
    };
    netpng_core::write_image(&args.output, &img, args.compression)
}

/// Canvas size in pixels: `xysize` as given; `width` or `height` with the
/// other side from the drawing's aspect ratio; otherwise the longer side is
/// DEFAULT_LONG_SIDE. A side with no extent in the drawing is 1 pixel, and
/// a drawing with no extent at all (a single point) gets a square canvas.
fn canvas_size(
    xysize: Option<(u32, u32)>,
    width: Option<u32>,
    height: Option<u32>,
    bw: f64,
    bh: f64,
) -> (u32, u32) {
    // `ratio` = derived side / known side.
    let derive = |known: u32, ratio: f64| -> u32 {
        if !ratio.is_finite() {
            known
        } else if ratio > 0.0 {
            ((known as f64) * ratio).round().max(1.0) as u32
        } else {
            1
        }
    };
    if let Some(wh) = xysize {
        return wh;
    }
    match (width, height) {
        (Some(w), _) => (w, derive(w, bh / bw)),
        (None, Some(h)) => (derive(h, bw / bh), h),
        (None, None) if bw >= bh => (DEFAULT_LONG_SIDE, derive(DEFAULT_LONG_SIDE, bh / bw)),
        (None, None) => (derive(DEFAULT_LONG_SIDE, bw / bh), DEFAULT_LONG_SIDE),
    }
}

/// Fits the bounding box `(minx, miny, maxx, maxy)` into a `w`x`h` canvas,
/// inset by half a line width so strokes are not clipped at the edges.
/// With `stretch` each axis fills the canvas independently (--xysize);
/// otherwise one uniform scale is used and the drawing is centered, so
/// circles stay round even when rounding makes the canvas aspect differ
/// slightly from the drawing's.
fn make_view(bbox: (f64, f64, f64, f64), w: u32, h: u32, stretch: bool) -> View {
    let (minx, miny, maxx, maxy) = bbox;
    let (bw, bh) = (maxx - minx, maxy - miny);
    let lw = LINE_WIDTH as f64;
    let fx = (bw > 0.0).then(|| (w as f64 - lw).max(0.0) / bw);
    let fy = (bh > 0.0).then(|| (h as f64 - lw).max(0.0) / bh);
    let (sx, sy) = if stretch {
        (fx.unwrap_or(0.0), fy.unwrap_or(0.0))
    } else {
        let s = match (fx, fy) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => 0.0,
        };
        (s, s)
    };
    View {
        minx,
        maxy,
        sx,
        sy,
        ox: (w as f64 - bw * sx) / 2.0,
        oy: (h as f64 - bh * sy) / 2.0,
    }
}

type P = (f64, f64);
type V3 = (f64, f64, f64);

/// Model-space geometry collected from a DXF drawing.
#[derive(Debug)]
enum Geom {
    /// Straight segment.
    Line(P, P),
    /// Polyline; `bulges[i]` is the arc factor on the segment from vertex i
    /// to i+1 (0 for a straight segment).
    Poly {
        pts: Vec<P>,
        bulges: Vec<f64>,
        closed: bool,
    },
    /// Elliptical arc (a circle is the special case with equal axes):
    /// point(t) = center + major·cos(t) + minor·sin(t), drawn from t0 to t1
    /// (the sweep t1 - t0 may be negative).
    Arc {
        center: P,
        major: P,
        minor: P,
        t0: f64,
        t1: f64,
    },
    /// Point marker (drawn as a small filled dot).
    Dot(P),
}

/// Model→screen map: x right, y down.
struct View {
    minx: f64,
    maxy: f64,
    sx: f64,
    sy: f64,
    ox: f64,
    oy: f64,
}

impl View {
    fn map(&self, x: f64, y: f64) -> (f32, f32) {
        (
            ((x - self.minx) * self.sx + self.ox) as f32,
            ((self.maxy - y) * self.sy + self.oy) as f32,
        )
    }

    /// Maps a model-space vector (no translation).
    fn vec(&self, x: f64, y: f64) -> (f32, f32) {
        ((x * self.sx) as f32, (-y * self.sy) as f32)
    }
}

fn cross(u: V3, v: V3) -> V3 {
    (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0)
}

/// Unit vector along `v`, or +Z if `v` is zero (DXF's default normal).
fn unit(v: &Vector) -> V3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len > 0.0 {
        (v.x / len, v.y / len, v.z / len)
    } else {
        (0.0, 0.0, 1.0)
    }
}

/// 2D affine map (block-local/OCS to model space).
#[derive(Clone, Copy)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

impl Affine {
    fn identity() -> Self {
        Affine::translate(0.0, 0.0)
    }

    fn translate(tx: f64, ty: f64) -> Self {
        Affine {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        }
    }

    fn map(self, x: f64, y: f64) -> P {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        )
    }

    /// Maps a vector (no translation).
    fn vec(self, x: f64, y: f64) -> P {
        (self.a * x + self.b * y, self.c * x + self.d * y)
    }

    /// self ∘ inner (apply `inner` first, then self).
    fn then(self, inner: &Affine) -> Affine {
        let (tx, ty) = self.map(inner.tx, inner.ty);
        Affine {
            a: self.a * inner.a + self.b * inner.c,
            b: self.a * inner.b + self.b * inner.d,
            c: self.c * inner.a + self.d * inner.c,
            d: self.c * inner.b + self.d * inner.d,
            tx,
            ty,
        }
    }

    /// Block INSERT transform: p' = anchor + R(rad)·S(sx, sy)·p, where p is
    /// relative to the block's base point and `anchor` is the insertion
    /// point in the parent's (OCS) space.
    fn insert_transform(anchor: P, rad: f64, sx: f64, sy: f64) -> Self {
        let (cr, sr) = (rad.cos(), rad.sin());
        Affine {
            a: cr * sx,
            b: -sr * sy,
            c: sr * sx,
            d: cr * sy,
            tx: anchor.0,
            ty: anchor.1,
        }
    }

    /// OCS → WCS for extrusion direction `n` (DXF's "arbitrary axis
    /// algorithm"), projected onto the XY plane; elevation along `n` is
    /// dropped. For the usual n = +Z this is the identity; n = -Z (what
    /// AutoCAD's MIRROR produces for circles, arcs and polylines) flips x.
    fn ocs(n: &Vector) -> Self {
        let n = unit(n);
        let ax = if n.0.abs() < 1.0 / 64.0 && n.1.abs() < 1.0 / 64.0 {
            cross((0.0, 1.0, 0.0), n)
        } else {
            cross((0.0, 0.0, 1.0), n)
        };
        let len = (ax.0 * ax.0 + ax.1 * ax.1 + ax.2 * ax.2).sqrt();
        let ax = (ax.0 / len, ax.1 / len, ax.2 / len);
        let ay = cross(n, ax);
        Affine {
            a: ax.0,
            b: ay.0,
            c: ax.1,
            d: ay.1,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

/// Model-space geometry of the whole drawing, with block references
/// expanded. Paper-space entities (layouts, title blocks) are skipped.
fn collect_drawing(drawing: &Drawing) -> Vec<Geom> {
    let mut geoms = Vec::new();
    let mut visited = HashSet::new();
    for e in drawing.entities().filter(|e| !e.common.is_in_paper_space) {
        collect_entity(drawing, e, &mut geoms, &Affine::identity(), &mut visited);
    }
    geoms
}

fn collect_entity(
    drawing: &Drawing,
    e: &Entity,
    geoms: &mut Vec<Geom>,
    xf: &Affine,
    visited: &mut HashSet<String>,
) {
    match &e.specific {
        EntityType::Line(l) => {
            geoms.push(Geom::Line(xf.map(l.p1.x, l.p1.y), xf.map(l.p2.x, l.p2.y)));
        }
        EntityType::Circle(c) => {
            let xf = xf.then(&Affine::ocs(&c.normal));
            geoms.push(Geom::Arc {
                center: xf.map(c.center.x, c.center.y),
                major: xf.vec(c.radius, 0.0),
                minor: xf.vec(0.0, c.radius),
                t0: 0.0,
                t1: 2.0 * PI,
            });
        }
        EntityType::Arc(a) => {
            let xf = xf.then(&Affine::ocs(&a.normal));
            let t0 = a.start_angle.to_radians();
            let mut t1 = a.end_angle.to_radians();
            if t1 <= t0 {
                t1 += 2.0 * PI;
            }
            geoms.push(Geom::Arc {
                center: xf.map(a.center.x, a.center.y),
                major: xf.vec(a.radius, 0.0),
                minor: xf.vec(0.0, a.radius),
                t0,
                t1,
            });
        }
        EntityType::Ellipse(el) => {
            // ELLIPSE is in WCS; its parameter runs counterclockwise about
            // its own normal, so minor = ratio · (normal × major).
            let major = (el.major_axis.x, el.major_axis.y, el.major_axis.z);
            let k = el.minor_axis_ratio;
            let mn = cross(unit(&el.normal), major);
            let t0 = el.start_parameter;
            let mut t1 = el.end_parameter;
            if t1 <= t0 {
                t1 += 2.0 * PI;
            }
            geoms.push(Geom::Arc {
                center: xf.map(el.center.x, el.center.y),
                major: xf.vec(major.0, major.1),
                minor: xf.vec(mn.0 * k, mn.1 * k),
                t0,
                t1,
            });
        }
        EntityType::LwPolyline(p) => {
            if p.vertices.len() >= 2 {
                let xf = xf.then(&Affine::ocs(&p.extrusion_direction));
                geoms.push(Geom::Poly {
                    pts: p.vertices.iter().map(|v| xf.map(v.x, v.y)).collect(),
                    bulges: p.vertices.iter().map(|v| v.bulge).collect(),
                    closed: (p.flags & 1) != 0,
                });
            }
        }
        EntityType::Polyline(p) => {
            // 16 = 3D polygon mesh, 64 = polyface mesh: their vertices are
            // not a path.
            if (p.flags & (16 | 64)) != 0 {
                return;
            }
            // 8 = 3D polyline: vertices are already in WCS.
            let xf = if (p.flags & 8) != 0 {
                *xf
            } else {
                xf.then(&Affine::ocs(&p.normal))
            };
            // Vertex flag 16 = spline frame control point, not on the curve.
            let verts: Vec<_> = p.vertices().filter(|v| (v.flags & 16) == 0).collect();
            if verts.len() >= 2 {
                geoms.push(Geom::Poly {
                    pts: verts.iter().map(|v| xf.map(v.location.x, v.location.y)).collect(),
                    bulges: verts.iter().map(|v| v.bulge).collect(),
                    closed: (p.flags & 1) != 0,
                });
            }
        }
        EntityType::Spline(s) => {
            let closed = (s.flags & 1) != 0;
            let src: &[DxfPoint] = if s.fit_points.is_empty() {
                &s.control_points
            } else {
                &s.fit_points
            };
            if src.len() >= 2 {
                geoms.push(Geom::Poly {
                    pts: src.iter().map(|v| xf.map(v.x, v.y)).collect(),
                    bulges: vec![0.0; src.len()],
                    closed,
                });
            }
        }
        EntityType::ModelPoint(p) => {
            geoms.push(Geom::Dot(xf.map(p.location.x, p.location.y)));
        }
        EntityType::Insert(ins) => {
            let name = ins.name.to_ascii_lowercase();
            // *Model_Space/*Paper_Space are drawn via entities(), never
            // via INSERT. Other anonymous blocks (*U dynamic blocks, ...)
            // are ordinary block references.
            if name == "*model_space" || name.starts_with("*paper_space") {
                return;
            }
            if visited.contains(&name) {
                return; // block reference cycle
            }
            let Some(block) = drawing
                .blocks()
                .find(|b| b.name.to_ascii_lowercase() == name)
            else {
                return; // xref or missing block
            };
            visited.insert(name.clone());
            // Block-local p → (p - base point) → rotate/scale/move to the
            // insertion point in the INSERT's OCS → WCS → the parent's xf.
            let child = xf
                .then(&Affine::ocs(&ins.extrusion_direction))
                .then(&Affine::insert_transform(
                    (ins.location.x, ins.location.y),
                    ins.rotation.to_radians(),
                    ins.x_scale_factor,
                    ins.y_scale_factor,
                ))
                .then(&Affine::translate(-block.base_point.x, -block.base_point.y));
            for be in &block.entities {
                collect_entity(drawing, be, geoms, &child, visited);
            }
            visited.remove(&name);
        }
        _ => {}
    }
}

/// (minx, miny, maxx, maxy) over all geometry; curves are sampled.
fn bounds(geoms: &[Geom]) -> (f64, f64, f64, f64) {
    let mut minx = f64::INFINITY;
    let mut miny = f64::INFINITY;
    let mut maxx = f64::NEG_INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    let mut inc = |x: f64, y: f64| {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    };
    for g in geoms {
        match g {
            Geom::Line(p1, p2) => {
                inc(p1.0, p1.1);
                inc(p2.0, p2.1);
            }
            Geom::Poly {
                pts,
                bulges,
                closed,
            } => {
                for p in pts {
                    inc(p.0, p.1);
                }
                // Bulge arcs can extend well beyond their vertices.
                let n = pts.len();
                let segs = if *closed { n } else { n - 1 };
                for i in 0..segs {
                    let j = (i + 1) % n;
                    let bg = bulges.get(i).copied().unwrap_or(0.0);
                    let d2 = (pts[i].0 - pts[j].0).powi(2) + (pts[i].1 - pts[j].1).powi(2);
                    if bg.abs() > 1e-9 && d2 > 1e-18 {
                        let (c, mj, mn, t1) = bulge_arc(pts[i], pts[j], bg);
                        for k in 0..=32 {
                            let t = t1 * (k as f64) / 32.0;
                            let (ct, st) = (t.cos(), t.sin());
                            inc(
                                c.0 + mj.0 * ct + mn.0 * st,
                                c.1 + mj.1 * ct + mn.1 * st,
                            );
                        }
                    }
                }
            }
            Geom::Arc {
                center,
                major,
                minor,
                t0,
                t1,
            } => {
                let n = 256;
                for i in 0..=n {
                    let t = t0 + (t1 - t0) * (i as f64) / (n as f64);
                    let (ct, st) = (t.cos(), t.sin());
                    inc(
                        center.0 + major.0 * ct + minor.0 * st,
                        center.1 + major.1 * ct + minor.1 * st,
                    );
                }
            }
            Geom::Dot(p) => inc(p.0, p.1),
        }
    }
    (minx, miny, maxx, maxy)
}

/// Builds the screen-space path for one geometry. The boolean is true when
/// the path must be filled (POINT markers) and stroked otherwise.
fn build_path(g: &Geom, v: &View) -> Option<(Path, bool)> {
    let mut b = PathBuilder::new();
    match g {
        Geom::Line(p1, p2) => {
            let (x, y) = v.map(p1.0, p1.1);
            b.move_to(x, y);
            let (x, y) = v.map(p2.0, p2.1);
            b.line_to(x, y);
        }
        Geom::Poly {
            pts,
            bulges,
            closed,
        } => {
            let (x, y) = v.map(pts[0].0, pts[0].1);
            b.move_to(x, y);
            let n = pts.len();
            let segs = if *closed { n } else { n - 1 };
            for i in 0..segs {
                let j = (i + 1) % n;
                let bg = bulges.get(i).copied().unwrap_or(0.0);
                let d2 = (pts[i].0 - pts[j].0).powi(2) + (pts[i].1 - pts[j].1).powi(2);
                if bg.abs() > 1e-9 && d2 > 1e-18 {
                    let (c, mj, mn, t1) = bulge_arc(pts[i], pts[j], bg);
                    add_arc(&mut b, v, (c, mj, mn), 0.0, t1, false);
                } else {
                    let (x, y) = v.map(pts[j].0, pts[j].1);
                    b.line_to(x, y);
                }
            }
            if *closed && n > 2 {
                b.close();
            }
        }
        Geom::Arc {
            center,
            major,
            minor,
            t0,
            t1,
        } => {
            add_arc(&mut b, v, (*center, *major, *minor), *t0, *t1, true);
        }
        Geom::Dot(p) => {
            let (x, y) = v.map(p.0, p.1);
            b.push_circle(x, y, 1.5);
            return b.finish().map(|p| (p, true));
        }
    }
    b.finish().map(|p| (p, false))
}

/// Arc/ellipse segment from vertex `p1` to vertex `p2` with DXF bulge `b`
/// (b = tan(theta/4), signed, positive = counterclockwise). Returns the arc
/// in the Arc geometry's parameterization (t0 = 0, t1 = 4·atan(b)):
/// point(t) = center + major·cos(t) + minor·sin(t).
fn bulge_arc(p1: P, p2: P, b: f64) -> (P, P, P, f64) {
    let (dx, dy) = (p2.0 - p1.0, p2.1 - p1.1);
    let c = (dx * dx + dy * dy).sqrt();
    let rb = b.abs();
    let mx = (p1.0 + p2.0) / 2.0;
    let my = (p1.1 + p2.1) / 2.0;
    let d = c * (1.0 - b * b) / (4.0 * rb);
    let (ux, uy) = (dx / c, dy / c);
    let (cx, cy) = if b > 0.0 {
        (mx - d * uy, my + d * ux)
    } else {
        (mx + d * uy, my - d * ux)
    };
    let major = (p1.0 - cx, p1.1 - cy);
    let minor = (-major.1, major.0);
    ((cx, cy), major, minor, 4.0 * b.atan())
}

/// Adds an arc of the ellipse `(center, major, minor)` in the
/// parameterization point(t) = center + major·cos(t) + minor·sin(t), drawn
/// from t0 to t1 (either sweep direction), approximated by cubic Béziers
/// with at most 90° per segment. Since Béziers are exact under affine maps,
/// this stays accurate through the model→screen mapping.
fn add_arc(
    b: &mut PathBuilder,
    v: &View,
    (center, major, minor): (P, P, P),
    t0: f64,
    t1: f64,
    move_first: bool,
) {
    let span = t1 - t0;
    if span.abs() < 1e-9 {
        return;
    }
    let n = (span.abs() / (PI / 2.0)).ceil().max(1.0) as usize;
    let (cx, cy) = v.map(center.0, center.1);
    let (mjx, mjy) = v.vec(major.0, major.1);
    let (mnx, mny) = v.vec(minor.0, minor.1);
    let pt = |t: f64| -> (f32, f32) {
        let (ct, st) = (t.cos() as f32, t.sin() as f32);
        (cx + mjx * ct + mnx * st, cy + mjy * ct + mny * st)
    };
    // d(point)/dt. The handle length `k` below carries the sweep's sign,
    // so this must not be flipped for clockwise sweeps.
    let tang = |t: f64| -> (f32, f32) {
        let (s, c2) = (t.sin() as f32, t.cos() as f32);
        (-s * mjx + c2 * mnx, -s * mjy + c2 * mny)
    };
    if move_first {
        let (x, y) = pt(t0);
        b.move_to(x, y);
    }
    let k = ((4.0 / 3.0) * ((span / (n as f64)) / 4.0).tan()) as f32;
    for i in 0..n {
        let a = t0 + span * (i as f64) / (n as f64);
        let e = t0 + span * ((i + 1) as f64) / (n as f64);
        let (px, py) = pt(a);
        let (tx, ty) = tang(a);
        let (ex, ey) = pt(e);
        let (ux, uy) = tang(e);
        b.cubic_to(px + k * tx, py + k * ty, ex - k * ux, ey - k * uy, ex, ey);
    }
    if span.abs() >= 2.0 * PI - 1e-6 {
        b.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> P {
        (x, y)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Parses a minimal DXF (R2000 header, optional BLOCKS, ENTITIES)
    /// given as `code\nvalue\n...` pair text.
    fn load(entities: &str, blocks: &str) -> Drawing {
        let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n");
        if !blocks.is_empty() {
            s += "0\nSECTION\n2\nBLOCKS\n";
            s += blocks;
            s += "0\nENDSEC\n";
        }
        s += "0\nSECTION\n2\nENTITIES\n";
        s += entities;
        s += "0\nENDSEC\n0\nEOF\n";
        Drawing::load(&mut s.as_bytes()).expect("test DXF parses")
    }

    fn only_arc(geoms: &[Geom]) -> (P, P, P) {
        let arcs: Vec<_> = geoms
            .iter()
            .filter_map(|g| match g {
                Geom::Arc {
                    center,
                    major,
                    minor,
                    ..
                } => Some((*center, *major, *minor)),
                _ => None,
            })
            .collect();
        assert_eq!(arcs.len(), 1, "expected exactly one arc in {geoms:?}");
        arcs[0]
    }

    #[test]
    fn bulge_semicircle() {
        // (0,0) -> (1,0), b = 1: semicircle, r = 0.5, center (0.5, 0),
        // passing through (0.5, -0.5) (positive bulge = CCW).
        let (c, mj, mn, t1) = bulge_arc(pt(0.0, 0.0), pt(1.0, 0.0), 1.0);
        assert!((c.0 - 0.5).abs() < 1e-12);
        assert!(c.1.abs() < 1e-12);
        assert!((t1 - PI).abs() < 1e-12);
        let r = mj.0.hypot(mj.1);
        assert!((r - 0.5).abs() < 1e-12);
        // point(t) = c + mj·cos t + mn·sin t
        let q = |t: f64| {
            (
                c.0 + mj.0 * t.cos() + mn.0 * t.sin(),
                c.1 + mj.1 * t.cos() + mn.1 * t.sin(),
            )
        };
        assert!((q(0.0).0 - 0.0).abs() < 1e-12 && q(0.0).1.abs() < 1e-12);
        assert!((q(PI).0 - 1.0).abs() < 1e-12 && q(PI).1.abs() < 1e-12);
        let mid = q(PI / 2.0);
        assert!((mid.0 - 0.5).abs() < 1e-12);
        assert!((mid.1 + 0.5).abs() < 1e-12);
    }

    #[test]
    fn bulge_quarter_arc() {
        // (0,0) -> (1,1), b = tan(pi/16): 45° CCW arc.
        let b = (PI / 16.0).tan();
        let (c, mj, mn, t1) = bulge_arc(pt(0.0, 0.0), pt(1.0, 1.0), b);
        assert!((t1 - PI / 4.0).abs() < 1e-12);
        let q = |t: f64| {
            (
                c.0 + mj.0 * t.cos() + mn.0 * t.sin(),
                c.1 + mj.1 * t.cos() + mn.1 * t.sin(),
            )
        };
        let end = q(t1);
        assert!((end.0 - 1.0).abs() < 1e-12);
        assert!((end.1 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn bezier_bulge_arcs_stay_on_the_circle_in_both_directions() {
        // (0,0) -> (2,0) with bulge ±1 is a semicircle of radius 1 about
        // (1,0). Screen y = -model y here. Every Bézier's midpoint must lie
        // on that circle and on the correct side: b = +1 (CCW) dips below
        // (model y < 0), b = -1 (CW) rises above (model y > 0).
        let v = View {
            minx: 0.0,
            maxy: 0.0,
            sx: 1.0,
            sy: 1.0,
            ox: 0.0,
            oy: 0.0,
        };
        for (bulge, model_side) in [(1.0, -1.0f32), (-1.0, 1.0f32)] {
            let g = Geom::Poly {
                pts: vec![pt(0.0, 0.0), pt(2.0, 0.0)],
                bulges: vec![bulge, 0.0],
                closed: false,
            };
            let (path, _) = build_path(&g, &v).expect("path");
            let mut prev = (0.0f32, 0.0f32);
            let mut cubics = 0;
            for seg in path.segments() {
                match seg {
                    tiny_skia::PathSegment::MoveTo(p) => prev = (p.x, p.y),
                    tiny_skia::PathSegment::CubicTo(c1, c2, p) => {
                        let mx = (prev.0 + 3.0 * c1.x + 3.0 * c2.x + p.x) / 8.0;
                        let my = (prev.1 + 3.0 * c1.y + 3.0 * c2.y + p.y) / 8.0;
                        let r = ((mx - 1.0).powi(2) + my.powi(2)).sqrt();
                        assert!((r - 1.0).abs() < 1e-3, "bulge {bulge}: midpoint r = {r}");
                        assert!(-my * model_side > 0.0, "bulge {bulge}: arc on the wrong side");
                        prev = (p.x, p.y);
                        cubics += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(cubics, 2);
        }
    }

    #[test]
    fn nested_insert_composition() {
        // Parent insert: scale 2 at (10,0). Child insert (in the parent's
        // local space): 90° CCW at (1,1).
        let parent = Affine::insert_transform(pt(10.0, 0.0), 0.0, 2.0, 2.0);
        let child_local = Affine::insert_transform(pt(1.0, 1.0), PI / 2.0, 1.0, 1.0);
        let child = parent.then(&child_local);
        // child-local origin -> parent-local (1,1) -> model (12,2)
        let p0 = child.map(0.0, 0.0);
        assert!((p0.0 - 12.0).abs() < 1e-12);
        assert!((p0.1 - 2.0).abs() < 1e-12);
        // child-local (1,0): rotate -> (0,1) + (1,1) = (1,2) -> model (12,4)
        let p = child.map(1.0, 0.0);
        assert!((p.0 - 12.0).abs() < 1e-12);
        assert!((p.1 - 4.0).abs() < 1e-12);
    }

    #[test]
    fn affine_then_composes() {
        let outer = Affine::insert_transform(pt(10.0, 20.0), PI / 2.0, 2.0, 1.0);
        let inner = Affine::insert_transform(pt(1.0, 2.0), 0.0, 3.0, 4.0);
        let composed = outer.then(&inner);
        for &(x, y) in &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (5.0, -3.0)] {
            let ip = inner.map(x, y);
            let direct = outer.map(ip.0, ip.1);
            let via = composed.map(x, y);
            assert!((direct.0 - via.0).abs() < 1e-12);
            assert!((direct.1 - via.1).abs() < 1e-12);
        }
    }

    #[test]
    fn affine_insert_transform_points() {
        // 90° CCW, scale 2, anchor at origin: local (1,0) -> (0,2)
        let xf = Affine::insert_transform(pt(0.0, 0.0), PI / 2.0, 2.0, 2.0);
        let p = xf.map(1.0, 0.0);
        assert!((p.0 - 0.0).abs() < 1e-12);
        assert!((p.1 - 2.0).abs() < 1e-12);
        // anchor (10,10), 90° CCW, scale 1: the local base point (0,0) maps
        // to the anchor, local (1,0) maps to the anchor rotated by 90°.
        let xf = Affine::insert_transform(pt(10.0, 10.0), PI / 2.0, 1.0, 1.0);
        let p0 = xf.map(0.0, 0.0);
        assert!((p0.0 - 10.0).abs() < 1e-12);
        assert!((p0.1 - 10.0).abs() < 1e-12);
        let p = xf.map(1.0, 0.0);
        assert!((p.0 - 10.0).abs() < 1e-12);
        assert!((p.1 - 11.0).abs() < 1e-12);
    }

    #[test]
    fn ocs_of_plus_and_minus_z() {
        let id = Affine::ocs(&Vector::new(0.0, 0.0, 1.0));
        assert_eq!(id.map(3.0, 4.0), (3.0, 4.0));
        let mirrored = Affine::ocs(&Vector::new(0.0, 0.0, -1.0));
        let p = mirrored.map(3.0, 4.0);
        assert!(close(p.0, -3.0) && close(p.1, 4.0), "{p:?}");
    }

    #[test]
    fn insert_maps_block_base_point_to_insertion_point() {
        // Block base point (5,5); a circle drawn at the base point must land
        // on the insertion point (0,0).
        let d = load(
            "0\nINSERT\n8\n0\n2\nB1\n10\n0\n20\n0\n30\n0\n",
            "0\nBLOCK\n8\n0\n2\nB1\n70\n0\n10\n5\n20\n5\n30\n0\n3\nB1\n\
             0\nCIRCLE\n8\n0\n10\n5\n20\n5\n40\n3\n0\nENDBLK\n8\n0\n",
        );
        let (c, mj, _) = only_arc(&collect_drawing(&d));
        assert!(close(c.0, 0.0) && close(c.1, 0.0), "center {c:?}");
        assert!(close(mj.0.hypot(mj.1), 3.0));
    }

    #[test]
    fn extrusion_minus_z_mirrors_ocs_entities() {
        // CIRCLE at OCS (5,0) with normal -Z is at WCS (-5,0).
        let d = load(
            "0\nCIRCLE\n8\n0\n10\n5\n20\n0\n30\n0\n40\n2\n210\n0\n220\n0\n230\n-1\n",
            "",
        );
        let (c, _, _) = only_arc(&collect_drawing(&d));
        assert!(close(c.0, -5.0) && close(c.1, 0.0), "center {c:?}");

        // LWPOLYLINE vertex at OCS (5,1) with normal -Z is at WCS (-5,1).
        let d = load(
            "0\nLWPOLYLINE\n8\n0\n90\n2\n70\n0\n10\n0\n20\n0\n10\n5\n20\n1\n\
             210\n0\n220\n0\n230\n-1\n",
            "",
        );
        match &collect_drawing(&d)[..] {
            [Geom::Poly { pts, .. }] => assert!(close(pts[1].0, -5.0) && close(pts[1].1, 1.0)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn ellipse_minor_axis_follows_its_normal() {
        // major (2,0), ratio 0.5: minor is +Y for normal +Z, -Y for -Z.
        for (nz, minor_y) in [(1.0, 1.0), (-1.0, -1.0)] {
            let d = load(
                &format!(
                    "0\nELLIPSE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n2\n21\n0\n31\n0\n\
                     210\n0\n220\n0\n230\n{nz}\n40\n0.5\n41\n0\n42\n3.14159\n"
                ),
                "",
            );
            let (_, _, mn) = only_arc(&collect_drawing(&d));
            assert!(close(mn.0, 0.0) && close(mn.1, minor_y), "nz {nz}: minor {mn:?}");
        }
    }

    #[test]
    fn paper_space_entities_are_skipped() {
        let d = load(
            "0\nCIRCLE\n8\n0\n10\n0\n20\n0\n40\n5\n\
             0\nLINE\n8\n0\n67\n1\n10\n100\n20\n100\n11\n200\n21\n100\n",
            "",
        );
        let geoms = collect_drawing(&d);
        assert_eq!(geoms.len(), 1, "{geoms:?}");
        assert!(matches!(geoms[0], Geom::Arc { .. }));
    }

    #[test]
    fn anonymous_u_block_is_expanded() {
        // AutoCAD writes dynamic block references as INSERTs of "*U" blocks.
        let d = load(
            "0\nINSERT\n8\n0\n2\n*U1\n10\n0\n20\n0\n30\n0\n",
            "0\nBLOCK\n8\n0\n2\n*U1\n70\n1\n10\n0\n20\n0\n30\n0\n3\n*U1\n\
             0\nCIRCLE\n8\n0\n10\n0\n20\n0\n40\n5\n0\nENDBLK\n8\n0\n",
        );
        only_arc(&collect_drawing(&d));
    }

    #[test]
    fn old_style_polyline_closed_flag_is_bit_1() {
        let d = load(
            "0\nPOLYLINE\n8\n0\n66\n1\n70\n1\n10\n0\n20\n0\n30\n0\n\
             0\nVERTEX\n8\n0\n10\n0\n20\n0\n0\nVERTEX\n8\n0\n10\n20\n20\n0\n\
             0\nVERTEX\n8\n0\n10\n10\n20\n15\n0\nSEQEND\n8\n0\n",
            "",
        );
        match &collect_drawing(&d)[..] {
            [Geom::Poly { pts, closed, .. }] => {
                assert_eq!(pts.len(), 3);
                assert!(*closed);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn bounds_covers_sampled_arc() {
        let g = vec![Geom::Arc {
            center: pt(0.0, 0.0),
            major: pt(5.0, 0.0),
            minor: pt(0.0, 5.0),
            t0: 0.0,
            t1: PI / 2.0,
        }];
        let (minx, miny, maxx, maxy) = bounds(&g);
        assert!((minx - 0.0).abs() < 1e-9);
        assert!(miny.abs() < 1e-9);
        assert!((maxx - 5.0).abs() < 1e-9);
        assert!((maxy - 5.0).abs() < 1e-9);
    }

    #[test]
    fn bezier_arc_hits_endpoints() {
        // Full circle r=3 at the origin: 4 cubic segments; the path must
        // pass through the 4 cardinal points.
        let v = View {
            minx: 0.0,
            maxy: 0.0,
            sx: 1.0,
            sy: 1.0,
            ox: 0.0,
            oy: 0.0,
        };
        let g = Geom::Arc {
            center: pt(0.0, 0.0),
            major: pt(3.0, 0.0),
            minor: pt(0.0, 3.0),
            t0: 0.0,
            t1: 2.0 * PI,
        };
        let (path, fill) = build_path(&g, &v).expect("path");
        assert!(!fill);
        let mut saw: [bool; 4] = [false; 4];
        for seg in path.segments() {
            let (x, y) = match seg {
                tiny_skia::PathSegment::MoveTo(p)
                | tiny_skia::PathSegment::LineTo(p)
                | tiny_skia::PathSegment::QuadTo(_, p)
                | tiny_skia::PathSegment::CubicTo(_, _, p) => (p.x, p.y),
                _ => continue,
            };
            for i in 0..4 {
                let (ex, ey) = [(3.0, 0.0), (0.0, -3.0), (-3.0, 0.0), (0.0, 3.0)][i];
                if (x - ex).abs() < 1e-3 && (y - ey).abs() < 1e-3 {
                    saw[i] = true;
                }
            }
        }
        assert!(saw.iter().all(|s| *s), "missing cardinal point: {saw:?}");
    }

    #[test]
    fn canvas_size_rules() {
        // Explicit sizes.
        assert_eq!(canvas_size(Some((640, 480)), None, None, 10.0, 5.0), (640, 480));
        assert_eq!(canvas_size(None, Some(200), None, 4.0, 2.0), (200, 100));
        assert_eq!(canvas_size(None, None, Some(50), 4.0, 2.0), (100, 50));
        // Default: longer side 1024, regardless of drawing units.
        assert_eq!(canvas_size(None, None, None, 4.0, 2.0), (1024, 512));
        assert_eq!(canvas_size(None, None, None, 0.3, 0.6), (512, 1024));
        assert_eq!(canvas_size(None, None, None, 60000.0, 30000.0), (1024, 512));
        // Degenerate drawings.
        assert_eq!(canvas_size(None, None, None, 10.0, 0.0), (1024, 1));
        assert_eq!(canvas_size(None, None, None, 0.0, 10.0), (1, 1024));
        assert_eq!(canvas_size(None, None, None, 0.0, 0.0), (1024, 1024));
        assert_eq!(canvas_size(None, Some(100), None, 0.0, 5.0), (100, 100));
    }

    #[test]
    fn view_keeps_aspect_and_centers() {
        // A 4x2 drawing in an 8x4 canvas: one scale for both axes, and equal
        // margins on both sides of each axis.
        let v = make_view((0.0, 0.0, 4.0, 2.0), 8, 4, false);
        assert!(close(v.sx, v.sy), "sx {} sy {}", v.sx, v.sy);
        let (l, t) = v.map(0.0, 2.0);
        let (r, b) = v.map(4.0, 0.0);
        assert!(((l - 0.0) - (8.0 - r)).abs() < 1e-5);
        assert!(((t - 0.0) - (4.0 - b)).abs() < 1e-5);
        // --xysize stretches each axis independently.
        let v = make_view((0.0, 0.0, 4.0, 2.0), 9, 9, true);
        assert!(close(v.sx, 2.0) && close(v.sy, 4.0));
        // A single point lands in the middle of the canvas.
        let v = make_view((3.0, 3.0, 3.0, 3.0), 10, 10, false);
        assert_eq!(v.map(3.0, 3.0), (5.0, 5.0));
    }
}
