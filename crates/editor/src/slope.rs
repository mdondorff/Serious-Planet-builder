//! The slope guard of the brush (VIEW-043): stacked raise and lower strokes can make ground steeper than the plan can draw (the tiles of
//! such ground have no morph interval, LOD-008; ADR 0074). A stroke is clamped so that the gradient of the strokes stacked at a point stays
//! under what the LOD parameters allow, less a share kept for the base and the other edits; an item that is already over the limit is
//! flagged. The estimate is conservative: it adds the largest gradient of each stroke that reaches the stroke, though the maxima rarely
//! coincide, and it ignores the slope of the base.

use std::collections::BTreeSet;

use planet_core::Vec3;
use planet_world_def::item::{Item, ItemStroke};
use planet_world_def::stamp::BrushKind;
use planet_world_def::ItemId;

use crate::brush::{BrushStroke, BRUSH_KIND};
use crate::polyline::{self, GeoPoint};

/// The largest slope of the profile `s (1 - x^2)^2` of a stroke of strength `s` and radius `r` is `STROKE_GRADIENT s / r`: the largest of
/// `4 x (1 - x^2)`, at `x = 1 / sqrt(3)`.
pub const STROKE_GRADIENT: f64 = 1.539_600_717_839_002;

/// The part of the drawable slope (`LodParams::drawable_slope`) left to the ground the brush does not make: the base terrain, the stamps
/// and the other layers. The brush may use the rest. A named constant to tune once the owner has tried it.
pub const BASE_SLOPE_SHARE: f64 = 1.0;

/// What the inspector and the layer list say of an item over the limit.
pub const WARNING: &str = "too steep to draw fully";

/// The largest stacked gradient of the brush that the plan can draw, for a drawable slope of `drawable_slope`.
pub fn gradient_limit(drawable_slope: f64) -> f64 {
    (drawable_slope - BASE_SLOPE_SHARE).max(0.0)
}

/// The largest slope one stroke adds: a raise and a lower by magnitude, a flatten and a smooth none (they add no height of their own).
pub fn stroke_gradient(s: &ItemStroke) -> f64 {
    match s.kind {
        BrushKind::Raise | BrushKind::Lower => STROKE_GRADIENT * s.strength_m / s.radius_m,
        BrushKind::Flatten | BrushKind::Smooth => 0.0,
    }
}

/// Metres from `dir` to the polyline of `points` (latitude and longitude in degrees).
fn distance_to(points: &[(f64, f64)], dir: Vec3, planet_radius_m: f64) -> f64 {
    let geo: Vec<GeoPoint> = points.iter().map(|(la, lo)| GeoPoint::new(*la, *lo)).collect();
    match geo.as_slice() {
        [] => f64::INFINITY,
        [only] => polyline::arc_m(dir, only.dir(), planet_radius_m),
        many => polyline::nearest_segment(many, false, dir, planet_radius_m).map_or(f64::INFINITY, |h| h.distance_m),
    }
}

/// The stacked gradient at `dir`: the sum of the gradients of the strokes whose disc reaches within `extra_m` of it (0 for the strokes
/// themselves at their own points, the radius of a stroke about to be painted to find what can overlap it).
pub fn gradient_at(strokes: &[&ItemStroke], dir: Vec3, planet_radius_m: f64, extra_m: f64) -> f64 {
    strokes
        .iter()
        .filter(|s| stroke_gradient(s) > 0.0)
        .filter(|s| distance_to(&s.points, dir, planet_radius_m) < s.radius_m + extra_m)
        .map(|s| stroke_gradient(s))
        .sum()
}

/// The strength `new` may have so that the stack stays under `limit` along its path: its own strength where it fits, less where the strokes it
/// overlaps leave less room, 0 where they leave none. A flatten and a smooth keep theirs.
pub fn allowed_strength(new: &BrushStroke, existing: &[&ItemStroke], planet_radius_m: f64, limit: f64) -> f64 {
    if !matches!(new.kind, BrushKind::Raise | BrushKind::Lower) || new.points.is_empty() {
        return new.strength_m;
    }
    // The strokes it overlaps, found once; the gradient is read at its points and at the middle of each segment.
    let near: Vec<&ItemStroke> = existing
        .iter()
        .copied()
        .filter(|s| {
            stroke_gradient(s) > 0.0
                && new.points.iter().any(|p| distance_to(&s.points, p.dir(), planet_radius_m) < s.radius_m + new.radius_m)
        })
        .collect();
    let mut stacked = 0.0f64;
    let mut at = |dir: Vec3| stacked = stacked.max(gradient_at(&near, dir, planet_radius_m, new.radius_m));
    for p in &new.points {
        at(p.dir());
    }
    for w in new.points.windows(2) {
        at((w[0].dir() + w[1].dir()).normalized());
    }
    let room = (limit - stacked).max(0.0);
    let own = STROKE_GRADIENT / new.radius_m;
    (room / own).min(new.strength_m)
}

/// The brush items whose stacked gradient, over the points of their own strokes, passes `limit`.
pub fn steep_items<'a>(items: impl IntoIterator<Item = &'a Item>, limit: f64, planet_radius_m: f64) -> BTreeSet<ItemId> {
    let brushes: Vec<&Item> = items.into_iter().filter(|i| i.kind == BRUSH_KIND).collect();
    // Each stroke with its first point and how far its farthest point is from it, so that a stroke far from a place is skipped at once.
    struct Prepared<'s> {
        stroke: &'s ItemStroke,
        centre: Vec3,
        reach_m: f64,
    }
    let all: Vec<Prepared<'_>> = brushes
        .iter()
        .flat_map(|i| i.strokes.iter())
        .filter(|s| stroke_gradient(s) > 0.0 && !s.points.is_empty())
        .map(|s| {
            let centre = GeoPoint::new(s.points[0].0, s.points[0].1).dir();
            let spread =
                s.points.iter().map(|(la, lo)| polyline::arc_m(centre, GeoPoint::new(*la, *lo).dir(), planet_radius_m)).fold(0.0, f64::max);
            Prepared { stroke: s, centre, reach_m: spread + s.radius_m }
        })
        .collect();
    let mut steep = BTreeSet::new();
    for item in &brushes {
        let found = item.strokes.iter().filter(|s| stroke_gradient(s) > 0.0).any(|s| {
            s.points.iter().any(|(la, lo)| {
                let dir = GeoPoint::new(*la, *lo).dir();
                let g: f64 = all
                    .iter()
                    .filter(|p| polyline::arc_m(dir, p.centre, planet_radius_m) < p.reach_m)
                    .filter(|p| distance_to(&p.stroke.points, dir, planet_radius_m) < p.stroke.radius_m)
                    .map(|p| stroke_gradient(p.stroke))
                    .sum();
                g > limit + 1e-9
            })
        });
        if found {
            steep.insert(item.id);
        }
    }
    steep
}
