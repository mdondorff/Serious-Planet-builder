//! The brush tool's geometry (VIEW-035, ADR 0070 point 3): a freehand stroke becomes the points a brush item holds. GPU-free, no store.
//!
//! The pointer gives the places it passed. They are resampled along the great circles between them to a spacing of a quarter of the radius
//! (so a fast drag has no gaps and a slow one no pile of points), and the result is cut into strokes of at most 64 points that each stay
//! within one item's reach, consecutive strokes sharing the point where they meet. [`crate::commands::Editor::add_stroke`] files the
//! strokes into brush items.

use crate::polyline::{self, GeoPoint};
use planet_core::Vec3;
use planet_world_def::cells::CELL_EDGE_M;
use planet_world_def::item::{ItemStroke, MAX_POINTS};
use planet_world_def::stamp::{BrushKind, MAX_BRUSH_POINTS, MAX_STROKES};

/// The layer brush items go into: not one of the named layers, so it applies after all of them (the brush stage stacks it last).
pub const BRUSH_LAYER: &str = "brush";
/// The kind string of a brush item.
pub const BRUSH_KIND: &str = "brush";
/// The brush radius the tool accepts, metres (the definition takes 1 m to a cell edge; the tool stays well inside).
pub const RADIUS_RANGE_M: (f64, f64) = (1.0, 5_000.0);
/// The strength of one stroke the tool accepts, metres: the largest height change at the stroke's centre line.
pub const STRENGTH_RANGE_M: (f64, f64) = (0.01, 100.0);
/// What a new brush starts with, metres.
pub const DEFAULT_RADIUS_M: f64 = 100.0;
pub const DEFAULT_STRENGTH_M: f64 = 5.0;
/// The share of the radius between two points of a stroke (ADR 0070: a quarter).
pub const SPACING_FRACTION: f64 = 0.25;
/// A new brush item reaches at least this far from its anchor, metres, so that the strokes of one area join one item.
pub const MIN_AREA_M: f64 = 1_000.0;
/// A stroke joins an item only when it fits its reach with this much to spare, metres (the index compares with its own arithmetic).
const JOIN_MARGIN_M: f64 = 1e-6;

/// A stroke as the tool records it: the kind, radius and strength in metres, the height a flatten levels towards (a flatten only) and the
/// places of the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushStroke {
    pub kind: BrushKind,
    pub radius_m: f64,
    pub strength_m: f64,
    pub target_m: Option<f64>,
    pub points: Vec<GeoPoint>,
}

/// The points of `points` resampled along the great circles between them every `spacing_m`, the first and last kept; no two consecutive
/// points are one place.
pub fn resample(points: &[GeoPoint], spacing_m: f64, radius_m: f64) -> Vec<GeoPoint> {
    let Some(first) = points.first() else { return Vec::new() };
    let mut out = vec![first.canonical()];
    let mut since = 0.0;
    let mut last = first.dir();
    for p in &points[1..] {
        let (a, b) = (last, p.dir());
        let len = polyline::arc_m(a, b, radius_m);
        if len <= 0.0 {
            continue;
        }
        let mut pos = 0.0;
        while since + (len - pos) >= spacing_m {
            pos += spacing_m - since;
            since = 0.0;
            out.push(GeoPoint::of_dir(polyline::slerp(a, b, pos / len)).canonical());
        }
        since += len - pos;
        last = b;
    }
    // The stroke ends where the pointer ended.
    if since > 1e-3 {
        let end = GeoPoint::of_dir(last).canonical();
        if out.last() != Some(&end) {
            out.push(end);
        }
    }
    out
}

/// The strokes a drag becomes: resampled, then cut at 64 points and wherever a point is further from the piece's first than one item can
/// reach with the radius (a cell edge less the radius), consecutive pieces sharing the point where they meet. A click is one stroke of one
/// point.
pub fn split(stroke: &BrushStroke, planet_radius_m: f64) -> Vec<BrushStroke> {
    let spacing = (stroke.radius_m * SPACING_FRACTION).max(0.1);
    let points = resample(&stroke.points, spacing, planet_radius_m);
    if points.is_empty() {
        return Vec::new();
    }
    let span = CELL_EDGE_M - stroke.radius_m - 2.0 * polyline::REACH_SLACK_M;
    let piece = |points: Vec<GeoPoint>| BrushStroke { points, ..stroke.clone() };
    let mut pieces: Vec<BrushStroke> = Vec::new();
    let mut current = vec![points[0]];
    for q in &points[1..] {
        let breaks = current.len() >= MAX_POINTS || polyline::arc_m(current[0].dir(), q.dir(), planet_radius_m) > span;
        if breaks {
            let shared = *current.last().expect("a piece has a point");
            pieces.push(piece(std::mem::replace(&mut current, vec![shared])));
        }
        current.push(*q);
    }
    pieces.push(piece(current));
    pieces
}

/// The stroke as an item file holds it (the spelling of the file: longitude in (-180, 180], 0 at a pole).
pub fn to_item_stroke(s: &BrushStroke) -> ItemStroke {
    ItemStroke {
        kind: s.kind,
        radius_m: s.radius_m,
        strength_m: s.strength_m,
        target_m: s.target_m,
        points: s.points.iter().map(|p| p.canonical()).map(|p| (p.lat_deg, p.lon_deg)).collect(),
    }
}

/// How far from `anchor` the farthest point of `strokes` is, metres, and the largest radius among them.
pub fn extent(anchor: Vec3, strokes: &[ItemStroke], planet_radius_m: f64) -> (f64, f64) {
    let mut far = 0.0f64;
    let mut radius = 0.0f64;
    for s in strokes {
        radius = radius.max(s.radius_m);
        for (la, lo) in &s.points {
            far = far.max(polyline::arc_m(anchor, GeoPoint::new(*la, *lo).dir(), planet_radius_m));
        }
    }
    (far, radius)
}

/// The sum of the strengths of `strokes`, metres: the bound on the height change of a brush item.
pub fn strength_sum(strokes: &[ItemStroke]) -> f64 {
    strokes.iter().map(|s| s.strength_m).sum()
}

/// The reach of a new brush item whose farthest point is `far_m` from its anchor and whose largest radius is `radius_m`: the area of one
/// item, never under [`MIN_AREA_M`] and never over a cell edge.
pub fn new_reach_m(far_m: f64, radius_m: f64) -> f64 {
    ((far_m + radius_m).max(MIN_AREA_M) + polyline::REACH_SLACK_M).min(CELL_EDGE_M)
}

/// Whether `piece` can be added to a brush item with `strokes`, anchored at `anchor` (latitude and longitude in degrees) and reaching
/// `reach_m`: there is room for the stroke and its points, the bound on the change can hold the strengths, and every point of every stroke
/// with the largest radius stays inside the reach.
pub fn fits(strokes: &[ItemStroke], anchor: (f64, f64), reach_m: f64, piece: &ItemStroke, planet_radius_m: f64) -> bool {
    if strokes.is_empty() {
        return false;
    }
    let points: usize = strokes.iter().map(|s| s.points.len()).sum::<usize>() + piece.points.len();
    if strokes.len() + 1 > MAX_STROKES || points > MAX_BRUSH_POINTS {
        return false;
    }
    if strength_sum(strokes) + piece.strength_m > planet_world_def::item::MAX_DELTA_LIMIT_M {
        return false;
    }
    let mut all = strokes.to_vec();
    all.push(piece.clone());
    let (far, radius) = extent(GeoPoint::new(anchor.0, anchor.1).dir(), &all, planet_radius_m);
    far + radius + JOIN_MARGIN_M <= reach_m
}
