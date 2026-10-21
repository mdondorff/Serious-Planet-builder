//! The slope guard of the brush (VIEW-043): stacked raise and lower strokes can make ground steeper than the plan can draw (the tiles of
//! such ground have no morph interval, LOD-008; ADR 0074). A stroke is clamped so that the gradient of the strokes stacked at a point stays
//! under what the LOD parameters allow, less a share kept for the base and the other edits; an item that is already over the limit is
//! flagged. The estimate is conservative where it adds the largest gradient of each stroke that reaches the stroke (the maxima rarely
//! coincide) and where it counts a lower like a raise; it ignores the base, the stamps, the lines, and what a flatten or a smooth does at
//! its edge (VIEW-043 lists the limits).

use std::collections::BTreeSet;

use planet_core::Vec3;
use planet_world_def::item::{Item, ItemStroke};
use planet_world_def::stamp::BrushKind;
use planet_world_def::ItemId;

use crate::brush::{self, BrushStroke, BRUSH_KIND};
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

/// The largest slope one stroke adds, counted for a raise and a lower by magnitude. A flatten and a smooth are not counted: they move the
/// ground towards a target or the mean of its neighbourhood, which mostly takes slope away (a flatten onto a far target can still make a
/// step of its strength at its edge: a limit, VIEW-043).
pub fn stroke_gradient(s: &ItemStroke) -> f64 {
    match s.kind {
        BrushKind::Raise | BrushKind::Lower => STROKE_GRADIENT * s.strength_m / s.radius_m,
        BrushKind::Flatten | BrushKind::Smooth => 0.0,
    }
}

/// A stroke ready for distance queries: its points as places, its radius and its gradient.
struct Poly {
    geo: Vec<GeoPoint>,
    radius_m: f64,
    gradient: f64,
}

impl Poly {
    fn of(s: &ItemStroke) -> Poly {
        Poly { geo: s.points.iter().map(|(la, lo)| GeoPoint::new(*la, *lo)).collect(), radius_m: s.radius_m, gradient: stroke_gradient(s) }
    }

    /// Metres from `dir` to the polyline.
    fn distance(&self, dir: Vec3, planet_radius_m: f64) -> f64 {
        match self.geo.as_slice() {
            [] => f64::INFINITY,
            [only] => polyline::arc_m(dir, only.dir(), planet_radius_m),
            many => polyline::nearest_segment(many, false, dir, planet_radius_m).map_or(f64::INFINITY, |h| h.distance_m),
        }
    }
}

/// The stacked gradient at `dir`: the sum of the gradients of the strokes whose disc reaches within `extra_m` of it (0 for the strokes
/// themselves at their own points, the radius of a stroke about to be painted to find what can overlap it).
pub fn gradient_at(strokes: &[&ItemStroke], dir: Vec3, planet_radius_m: f64, extra_m: f64) -> f64 {
    strokes
        .iter()
        .map(|s| Poly::of(s))
        .filter(|p| p.gradient > 0.0 && p.distance(dir, planet_radius_m) < p.radius_m + extra_m)
        .map(|p| p.gradient)
        .sum()
}

/// The strength `new` may have so that the stack stays under `limit` along its path: its own strength where it fits, less where the strokes it
/// overlaps leave less room, 0 where they leave none. A flatten and a smooth keep theirs.
///
/// The check runs on the pieces the stroke will be filed as (`brush::split`: resampled to a quarter of the radius, cut at 64 points), so a
/// long drag with few raw points is checked along its whole path. Pieces share the point where they are cut and both have full weight
/// there, so a stroke of several pieces counts its own gradient twice.
pub fn allowed_strength(new: &BrushStroke, existing: &[&ItemStroke], planet_radius_m: f64, limit: f64) -> f64 {
    if !matches!(new.kind, BrushKind::Raise | BrushKind::Lower) || new.points.is_empty() {
        return new.strength_m;
    }
    let pieces = brush::split(new, planet_radius_m);
    let places: Vec<GeoPoint> = pieces.iter().flat_map(|p| p.points.iter().copied()).collect();
    let polys: Vec<Poly> = existing.iter().map(|s| Poly::of(s)).filter(|p| p.gradient > 0.0).collect();
    // The strokes it overlaps, found once; the gradient is read at the points of the pieces and at the middle of each segment.
    let near: Vec<&Poly> =
        polys.iter().filter(|p| places.iter().any(|q| p.distance(q.dir(), planet_radius_m) < p.radius_m + new.radius_m)).collect();
    let at = |dir: Vec3| -> f64 {
        near.iter().filter(|p| p.distance(dir, planet_radius_m) < p.radius_m + new.radius_m).map(|p| p.gradient).sum()
    };
    let mut stacked = places.iter().map(|q| at(q.dir())).fold(0.0, f64::max);
    for piece in &pieces {
        for w in piece.points.windows(2) {
            stacked = stacked.max(at((w[0].dir() + w[1].dir()).normalized()));
        }
    }
    let joints = if pieces.len() > 1 { 2.0 } else { 1.0 };
    let room = (limit - stacked).max(0.0);
    let own = joints * STROKE_GRADIENT / new.radius_m;
    (room / own).min(new.strength_m)
}

/// The brush items whose stacked gradient, over the points of their own strokes, passes `limit`.
pub fn steep_items<'a>(items: impl IntoIterator<Item = &'a Item>, limit: f64, planet_radius_m: f64) -> BTreeSet<ItemId> {
    let brushes: Vec<&Item> = items.into_iter().filter(|i| i.kind == BRUSH_KIND).collect();
    // Each stroke with its first point and how far its farthest point is from it, so that a stroke far from a place is skipped at once.
    struct Prepared {
        poly: Poly,
        centre: Vec3,
        reach_m: f64,
    }
    let all: Vec<Prepared> = brushes
        .iter()
        .flat_map(|i| i.strokes.iter())
        .map(Poly::of)
        .filter(|p| p.gradient > 0.0 && !p.geo.is_empty())
        .map(|poly| {
            let centre = poly.geo[0].dir();
            let spread = poly.geo.iter().map(|g| polyline::arc_m(centre, g.dir(), planet_radius_m)).fold(0.0, f64::max);
            let reach_m = spread + poly.radius_m;
            Prepared { poly, centre, reach_m }
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
                    .filter(|p| p.poly.distance(dir, planet_radius_m) < p.poly.radius_m)
                    .map(|p| p.poly.gradient)
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
