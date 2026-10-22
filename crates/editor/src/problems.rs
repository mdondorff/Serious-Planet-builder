//! Edit health (VIEW-047, M7 A5): the edits the engine cannot draw fully or that cost more than they should, found as data so the problems tab can
//! list them and offer a fix. Three kinds: a brush whose strokes stack too steep (VIEW-043), a brush with very many strokes stacked at one place, and
//! an edit near tiles the plan holds back (ADR 0074, VIEW-044). The fixes are pure functions over the strokes: reduce the strengths until the brush
//! is no longer too steep, and merge a stack of strokes into one.

use std::collections::{BTreeMap, BTreeSet};

use planet_core::Vec3;
use planet_world_def::item::{Item, ItemStroke};
use planet_world_def::stamp::BrushKind;
use planet_world_def::ItemId;

use crate::brush::BRUSH_KIND;
use crate::lists::Row;
use crate::polyline::{self, GeoPoint};
use crate::slope::{self, Poly};

/// The strokes at one point from which a brush is listed as stacked: the discs of this many strokes (of any kind) cover it.
pub const STACK_WARN: usize = 8;
/// The strokes in one brush item from which it is listed (the file holds at most 64, WDEF-013).
pub const STROKES_WARN: usize = 48;
/// The factor the strengths of a brush that is stacked but not too steep are cut by.
pub const REDUCE_STACKED: f64 = 0.5;

/// What is wrong with an edit.
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The stacked gradient of the brush is over what the plan can draw (VIEW-043).
    TooSteep,
    /// `depth` strokes cover one point of the brush, or the item holds `strokes` of at most 64.
    Stacked { depth: usize, strokes: usize },
    /// The plan holds a tile within the edit's footprint back (ADR 0074): the ground there is drawn coarser than asked.
    TilesCapped,
}

impl Issue {
    /// The line the list shows.
    pub fn text(&self) -> String {
        match self {
            Issue::TooSteep => slope::WARNING.to_string(),
            Issue::Stacked { depth, strokes } => {
                if *depth >= STACK_WARN {
                    format!("{depth} strokes stacked at one place ({strokes} of 64 strokes)")
                } else {
                    format!("{strokes} of 64 strokes")
                }
            }
            Issue::TilesCapped => "tiles near it are held back (too steep to refine)".to_string(),
        }
    }
}

/// One edit in the problems list: its row and what is wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub row: Row,
    pub issues: Vec<Issue>,
    /// Whether the fixes apply: a brush with a raise or lower stroke can be reduced; a brush with two overlapping strokes of one kind can be merged.
    pub can_reduce: bool,
    pub can_merge: bool,
}

/// A stroke ready for the stack count: its path, and the part of the planet it stays within.
struct Prepared {
    poly: Poly,
    centre: Vec3,
    reach_m: f64,
}

fn prepare(strokes: impl IntoIterator<Item = ItemStroke>, planet_radius_m: f64) -> Vec<Prepared> {
    strokes
        .into_iter()
        .map(|s| Poly::of(&s))
        .filter(|p| !p.geo.is_empty())
        .map(|poly| {
            let centre = poly.geo[0].dir();
            let spread = poly.geo.iter().map(|g| polyline::arc_m(centre, g.dir(), planet_radius_m)).fold(0.0, f64::max);
            let reach_m = spread + poly.radius_m;
            Prepared { poly, centre, reach_m }
        })
        .collect()
}

/// For each brush item, the most strokes (of any item and any kind) whose discs cover one of the points of its own strokes: the depth of its
/// stack. An item with no stroke point is not in the map.
pub fn stack_depths<'a>(items: impl IntoIterator<Item = &'a Item>, planet_radius_m: f64) -> BTreeMap<ItemId, usize> {
    let brushes: Vec<&Item> = items.into_iter().filter(|i| i.kind == BRUSH_KIND).collect();
    let all = prepare(brushes.iter().flat_map(|i| i.strokes.iter().cloned()), planet_radius_m);
    let mut out = BTreeMap::new();
    for item in brushes {
        let depth = item
            .strokes
            .iter()
            .flat_map(|s| s.points.iter())
            .map(|(la, lo)| {
                let dir = GeoPoint::new(*la, *lo).dir();
                all.iter()
                    .filter(|p| polyline::arc_m(dir, p.centre, planet_radius_m) < p.reach_m)
                    .filter(|p| p.poly.distance(dir, planet_radius_m) < p.poly.radius_m)
                    .count()
            })
            .max();
        if let Some(d) = depth {
            out.insert(item.id, d);
        }
    }
    out
}

/// The brush items that are listed as stacked, with the depth of their stack.
pub fn stacked_items<'a>(items: impl IntoIterator<Item = &'a Item>, planet_radius_m: f64) -> BTreeMap<ItemId, usize> {
    let items: Vec<&Item> = items.into_iter().collect();
    let depths = stack_depths(items.iter().copied(), planet_radius_m);
    items
        .into_iter()
        .filter(|i| i.kind == BRUSH_KIND)
        .filter_map(|i| {
            let d = depths.get(&i.id).copied().unwrap_or(0);
            (d >= STACK_WARN || i.strokes.len() >= STROKES_WARN).then_some((i.id, d))
        })
        .collect()
}

/// Whether two strokes of one kind overlap: a point of one lies within the disc of the other.
fn overlaps(a: &Poly, b: &Poly, planet_radius_m: f64) -> bool {
    a.geo.iter().any(|g| b.distance(g.dir(), planet_radius_m) < b.radius_m)
        || b.geo.iter().any(|g| a.distance(g.dir(), planet_radius_m) < a.radius_m)
}

/// The strokes with the deepest stack of one kind (a raise or a lower) merged into one: the first stroke of the stack keeps its place and its
/// path, takes the largest radius and the sum of the strengths, and the others are left out. `None` when no two strokes of one kind overlap.
/// The merged stroke is not the same ground as the stack (the others' paths are dropped; the sum along one path is steeper): the list
/// offers it as a simplification and the result is a command that one undo takes back. Returns the new list and how many strokes were merged.
pub fn merge_stack(strokes: &[ItemStroke], planet_radius_m: f64) -> Option<(Vec<ItemStroke>, usize)> {
    let polys: Vec<Poly> = strokes.iter().map(Poly::of).collect();
    let mut best: Option<(usize, Vec<usize>)> = None;
    for i in 0..strokes.len() {
        if !matches!(strokes[i].kind, BrushKind::Raise | BrushKind::Lower) || polys[i].geo.is_empty() {
            continue;
        }
        let partners: Vec<usize> = (0..strokes.len())
            .filter(|&j| {
                j != i && strokes[j].kind == strokes[i].kind && !polys[j].geo.is_empty() && overlaps(&polys[i], &polys[j], planet_radius_m)
            })
            .collect();
        if !partners.is_empty() && best.as_ref().is_none_or(|(_, p)| partners.len() > p.len()) {
            best = Some((i, partners));
        }
    }
    let (seed, partners) = best?;
    let mut members: BTreeSet<usize> = partners.into_iter().collect();
    members.insert(seed);
    // The first of the stack keeps its place (and so the first stroke of the item keeps the anchor's point).
    let first = *members.iter().next().expect("two members");
    let mut merged = strokes[first].clone();
    merged.radius_m = members.iter().map(|&i| strokes[i].radius_m).fold(0.0, f64::max);
    merged.strength_m = members.iter().map(|&i| strokes[i].strength_m).sum();
    let out: Vec<ItemStroke> = strokes
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if i == first {
                Some(merged.clone())
            } else if members.contains(&i) {
                None
            } else {
                Some(s.clone())
            }
        })
        .collect();
    Some((out, members.len()))
}

/// The strokes of `item` with the strengths of its raises and lowers multiplied by `factor` (a flatten and a smooth are not counted by the guard).
pub fn scaled(item: &Item, factor: f64) -> Vec<ItemStroke> {
    item.strokes
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if matches!(s.kind, BrushKind::Raise | BrushKind::Lower) {
                s.strength_m *= factor;
            }
            s
        })
        .collect()
}

/// The largest factor in `(0, 1]` (to about 1 %) by which the strengths of the raises and lowers of item `id` can be multiplied so that it is no
/// longer over `limit` (the guard of VIEW-043, `slope::gradient_limit`), when the other items stay as they are; `Some(1.0)` when it is not over
/// it; `None` when it is over it even at a hundredth of its strengths (the other edits make it so).
pub fn fit_factor(items: &[Item], id: ItemId, limit: f64, planet_radius_m: f64) -> Option<f64> {
    let steep_at = |factor: f64| -> bool {
        let trial: Vec<Item> =
            items.iter().map(|i| if i.id == id { Item { strokes: scaled(i, factor), ..i.clone() } } else { i.clone() }).collect();
        slope::steep_items(&trial, limit, planet_radius_m).contains(&id)
    };
    if !steep_at(1.0) {
        return Some(1.0);
    }
    if steep_at(0.01) {
        return None;
    }
    let (mut ok, mut bad) = (0.01, 1.0);
    while bad - ok > 0.01 {
        let mid = 0.5 * (ok + bad);
        if steep_at(mid) {
            bad = mid;
        } else {
            ok = mid;
        }
    }
    Some(ok)
}
