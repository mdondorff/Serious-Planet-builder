//! The edit preview (REND-033): the stamps whose delta the vertex stage adds to the stale tiles until the exact ones arrive.
//!
//! A preview stamp is a stamp (GEN-026) with a sign: an item added by an edit is +1, an item removed is -1, a changed item is both its
//! old state (-1) and its new one (+1). The list is computed from the footprint items before and after an edit, GPU-free, and handed
//! to the plan pipeline; the renderer only evaluates it.

use planet_core::Vec3;
use planet_world_def::footprint::FootprintItem;
use planet_world_def::item_id::ItemId;
use planet_world_def::stamp::{BrushKind, StampKind, Stroke};

/// The most preview stamps a plan carries (the globals table of the renderer has this many entries; the vertex stage loops over the
/// active count, so a smaller list costs what it holds).
pub const MAX_PREVIEW_STAMPS: usize = 32;

/// The most stamps the delta of an edit in flight takes, so that a transient preview (a brush stroke under the pointer, at most
/// [`BRUSH_CHAIN_STAMPS`]) always fits beside it in [`MAX_PREVIEW_STAMPS`] whole.
pub const MAX_DELTA_STAMPS: usize = MAX_PREVIEW_STAMPS - BRUSH_CHAIN_STAMPS;

/// The most stamps one brush stroke is spread over (VIEW-036), its two end caps included: a longer stroke is previewed with a wider spacing.
pub const BRUSH_CHAIN_STAMPS: usize = 16;
/// The spacing of the stamps of a brush chain, as a share of the radius.
pub const BRUSH_CHAIN_SPACING: f64 = 0.5;
/// The reach of the stamps of a chain, as a share of the stroke's radius. A line of `(1 - x^2)^2` bumps has the cross profile `(1 - x^2)^(5/2)`
/// where the stage's is `(1 - x^2)^2` on the distance to the line; both fall the same way near the line when the bumps reach `sqrt(5/4)`
/// times further, and the chain then stays within a few percent of the stage's profile across the line.
pub const BRUSH_CHAIN_REACH: f64 = 1.118;
/// The reach of the plateaus of a flatten chain, as a share of the radius.
pub const BRUSH_PLATEAU_REACH: f64 = 0.8;

/// One signed stamp of the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewStamp {
    /// +1 adds the stamp's delta, -1 takes it away.
    pub sign: i8,
    pub kind: StampKind,
    /// The stamp's first parameter: the hill's height, the crater's depth, the plateau's target (absolute).
    pub amount_m: f64,
    /// The unit direction of the anchor.
    pub dir: Vec3,
    pub reach_m: f64,
    /// The item's bound `M` (the plateau's clamp).
    pub max_delta_m: f64,
}

impl PreviewStamp {
    /// The kind as the shader reads it: 1 hill, 2 crater, 3 plateau.
    pub fn kind_code(&self) -> f32 {
        f32::from(self.kind as u8)
    }
}

/// The list of a delta and how many entries it had to leave out.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PreviewList {
    /// In application order of the items they come from (priority, then id), a changed item's removal before its addition.
    pub stamps: Vec<PreviewStamp>,
    /// Entries beyond [`MAX_PREVIEW_STAMPS`] that were dropped (the farthest from the camera go first).
    pub dropped: usize,
}

/// The unit direction `t` (0 to 1) of the way from `a` to `b` along the great circle.
fn slerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    let omega = a.angle_to(b);
    if omega < 1e-12 {
        return a;
    }
    let s = libm::sin(omega);
    (a * (libm::sin((1.0 - t) * omega) / s) + b * (libm::sin(t * omega) / s)).normalized()
}

/// A brush stroke as a chain of preview stamps along its line (VIEW-036). A raise is hills of the stroke's radius and a lower the same
/// with the sign turned, a flatten is plateaus at the stroke's target with the strength as their bound; a smooth is not previewed (it
/// moves the ground towards the mean of what is around it, which the stale tile cannot give: it shows when the exact tile arrives).
/// `sign` is +1 for a stroke the edit added and -1 for one it took away (a removed flatten is not previewed, as a removed plateau is not).
///
/// The stamps of a raise or a lower sit every [`BRUSH_CHAIN_SPACING`] of the radius along the line, both ends included (wider when the
/// stroke would need more than [`BRUSH_CHAIN_STAMPS`] with the two caps), reach [`BRUSH_CHAIN_REACH`] times the radius and the amplitude
/// that makes their sum the strength on the line; a dab at each end, of what the chain falls short of the strength there, gives the stroke its
/// rounded ends (the stage's distance to the polyline is to the end point there). A flatten is plateaus of the radius at the same places. A stroke of one point is
/// one stamp of the whole strength. The sum is approximate (the bends differ from the stage's distance to the polyline), and the exact tiles
/// replace it.
pub fn brush_chain(stroke: &Stroke, sign: i8, planet_radius_m: f64) -> Vec<PreviewStamp> {
    let (kind, flip) = match stroke.kind {
        BrushKind::Raise => (StampKind::Hill, 1i8),
        BrushKind::Lower => (StampKind::Hill, -1),
        BrushKind::Flatten if sign > 0 => (StampKind::Plateau, 1),
        BrushKind::Flatten | BrushKind::Smooth => return Vec::new(),
    };
    let points = &stroke.points;
    let Some(first) = points.first() else { return Vec::new() };
    let (radius, strength) = (stroke.radius_m, stroke.strength_m);
    let lengths: Vec<f64> = points.windows(2).map(|w| planet_radius_m * w[0].angle_to(w[1])).collect();
    let total: f64 = lengths.iter().sum();
    let at = |d: f64| -> Vec3 {
        let mut left = d;
        for (i, (w, len)) in points.windows(2).zip(&lengths).enumerate() {
            if left <= *len || i == lengths.len() - 1 {
                let t = if *len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 };
                return slerp(w[0], w[1], t);
            }
            left -= len;
        }
        *first
    };
    let stamp = |dir: Vec3, amount_m: f64, reach_m: f64, bound_m: f64| PreviewStamp {
        sign: sign * flip,
        kind,
        amount_m: if kind == StampKind::Plateau { stroke.target_m.unwrap_or(0.0) } else { amount_m },
        dir,
        reach_m,
        max_delta_m: bound_m,
    };
    // A plateau is flat out to 0.7 of its reach and each one moves the ground towards the target by at most its bound, so a row of them
    // would move it by the bound times the number that cover a point; the reach is `BRUSH_PLATEAU_REACH` of the radius (the stage's weight
    // `(1 - x^2)^2` is about a half there) and the bound is the strength shared over the plateaus covering the line.
    let plateau_reach = BRUSH_PLATEAU_REACH * radius;
    // A click, or a stroke too short to have a line: one stamp, the whole strength.
    if total < 0.05 * radius {
        let reach = if kind == StampKind::Plateau { plateau_reach } else { radius };
        return vec![stamp(*first, strength, reach, strength)];
    }
    let caps = usize::from(kind == StampKind::Hill) * 2;
    let most = BRUSH_CHAIN_STAMPS - caps;
    let n = ((total / (BRUSH_CHAIN_SPACING * radius) - 1e-6).ceil() as usize + 1).clamp(2, most);
    let spacing = total / (n - 1) as f64;
    let mut out = Vec::with_capacity(n + caps);
    if kind == StampKind::Hill {
        let reach = BRUSH_CHAIN_REACH * radius;
        // The mean of the chain along its line is `amount * 16 / 15 * reach / spacing`, which this cancels.
        let amount = strength * (0.9375 * spacing / reach).min(1.0);
        out.extend((0..n).map(|k| stamp(at(k as f64 * spacing), amount, reach, strength)));
        // The ends: the stage's weight at the end point is the whole strength; what the chain adds there falls short of it by a share that
        // depends on the spacing (about a half when the stamps are close, nothing when they are as far apart as their reach), and a dab of
        // the radius makes it up.
        let sum_at = |p: Vec3| -> f64 {
            out.iter()
                .map(|s| {
                    let x = planet_radius_m * s.dir.angle_to(p) / s.reach_m;
                    if x >= 1.0 {
                        0.0
                    } else {
                        s.amount_m * (1.0 - x * x) * (1.0 - x * x)
                    }
                })
                .sum()
        };
        let (start, end) = (*first, at(total));
        let caps = [(start, (strength - sum_at(start)).clamp(0.0, strength)), (end, (strength - sum_at(end)).clamp(0.0, strength))];
        out.extend(caps.map(|(dir, amount)| stamp(dir, amount, radius, strength)));
    } else {
        let covering = 1.0 + 2.0 * (0.7 * plateau_reach / spacing).floor();
        out.extend((0..n).map(|k| stamp(at(k as f64 * spacing), 0.0, plateau_reach, strength / covering)));
    }
    out
}

/// One entry of a delta: the item it comes from (application order: priority, then id), the unit the cap keeps or drops whole (a plain stamp
/// is its own unit 0; a stroke of a brush item is unit `index + 1`, a removal and an addition of the same stroke index being one unit), the
/// phase (a removal before an addition) and the stamp.
struct Entry {
    priority: i32,
    id: ItemId,
    unit: u32,
    addition: bool,
    stamp: PreviewStamp,
}

/// The entries of a brush item's strokes from the `from`-th on (in file order), added (`sign` +1) or taken away (-1).
fn brush_entries(item: &FootprintItem, sign: i8, from: usize, radius_m: f64) -> Vec<Entry> {
    let Some(stamp) = item.stamp.as_ref().filter(|s| s.kind.is_brush()) else { return Vec::new() };
    stamp
        .strokes()
        .iter()
        .enumerate()
        .skip(from)
        .flat_map(|(k, s)| {
            brush_chain(&s.to_stroke(), sign, radius_m).into_iter().map(move |stamp| Entry {
                priority: item.priority,
                id: item.id,
                unit: k as u32 + 1,
                addition: sign > 0,
                stamp,
            })
        })
        .collect()
}

fn entry(item: &FootprintItem, sign: i8) -> Option<Entry> {
    let stamp = item.stamp.as_ref()?;
    // The preview shader draws the radial stamps; a line and a brush show when their exact tiles arrive (GEN-028, GEN-030, M6
    // plan E3).
    if stamp.kind.is_line() || stamp.kind.is_brush() {
        return None;
    }
    let dir = Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]));
    Some(Entry {
        priority: item.priority,
        id: item.id,
        unit: 0,
        addition: sign > 0,
        stamp: PreviewStamp {
            sign,
            kind: stamp.kind,
            amount_m: stamp.params()[0],
            dir,
            reach_m: f64::from_bits(item.reach_bits),
            max_delta_m: f64::from_bits(item.max_delta_bits),
        },
    })
}

/// The preview of an edit that took the footprint items `before` to `after` (any items: only stamps count): the removals and additions
/// that differ, in application order. `before` must be the items the stale tiles were built from, the oldest state any resident tile
/// still shows (the caller keeps it until every tile has been refreshed), and `after` the current items: the list then takes any
/// stale tile to the current state however many edits came between (a drag in many steps is one delta from the first state).
///
/// A removed plateau is not previewed: its effect on a stale tile depends on the ground the plateau flattened, which the tile no longer
/// holds, so the exact tile shows it (a changed plateau previews as its new state only). With more than [`MAX_PREVIEW_STAMPS`] entries
/// the items nearest to `camera` (a planet-fixed position) are kept, an item's two entries together or not at all.
pub fn preview_delta(before: &[FootprintItem], after: &[FootprintItem], camera: Vec3) -> PreviewList {
    delta(before, after, camera, None)
}

/// [`preview_delta`] with the brushes (VIEW-036), on a body of `planet_radius_m`: a brush item added or removed is the chains of its strokes
/// ([`brush_chain`]), and a brush item that only gained strokes at the end (the editor appends: a stroke joins the item of its area) is
/// the chains of the new strokes alone, so that a stroke added to a large item costs its own stamps, and one that lost strokes from the end
/// (the undo of that) the new strokes taken away. Any other change of a brush item is its old state taken away and its new one added.
pub fn preview_delta_on(before: &[FootprintItem], after: &[FootprintItem], camera: Vec3, planet_radius_m: f64) -> PreviewList {
    delta(before, after, camera, Some(planet_radius_m))
}

fn delta(before: &[FootprintItem], after: &[FootprintItem], camera: Vec3, brush_radius_m: Option<f64>) -> PreviewList {
    let mut entries: Vec<Entry> = Vec::new();
    // Brush items that only gained or lost strokes at the end: only those strokes are added or taken away.
    let mut appended: Vec<ItemId> = Vec::new();
    if let Some(radius) = brush_radius_m {
        for new in after {
            let strokes = |i: &FootprintItem| i.stamp.as_ref().filter(|s| s.kind.is_brush()).map(|s| s.strokes().to_vec());
            let Some(old) = before.iter().find(|o| o.id == new.id && *o != new) else { continue };
            if let (Some(os), Some(ns)) = (strokes(old), strokes(new)) {
                let same_place = old.dir == new.dir && old.priority == new.priority;
                if same_place && ns.len() > os.len() && ns[..os.len()] == os[..] {
                    entries.extend(brush_entries(new, 1, os.len(), radius));
                    appended.push(new.id);
                } else if same_place && os.len() > ns.len() && os[..ns.len()] == ns[..] {
                    entries.extend(brush_entries(old, -1, ns.len(), radius));
                    appended.push(new.id);
                }
            }
        }
    }
    let one = |item: &FootprintItem, sign: i8| -> Vec<Entry> {
        match (brush_radius_m, item.stamp.as_ref().is_some_and(|s| s.kind.is_brush())) {
            (Some(radius), true) => brush_entries(item, sign, 0, radius),
            _ => entry(item, sign).into_iter().collect(),
        }
    };
    for old in before {
        if !appended.contains(&old.id) && !after.iter().any(|new| new == old) {
            entries.extend(one(old, -1).into_iter().filter(|e| e.stamp.kind != StampKind::Plateau));
        }
    }
    for new in after {
        if !appended.contains(&new.id) && !before.iter().any(|old| old == new) {
            entries.extend(one(new, 1));
        }
    }
    let total = entries.len();
    if total > MAX_DELTA_STAMPS {
        // Whole units, nearest first: the angle between the unit's nearest entry and the camera direction (larger cosine = nearer), ties by
        // id and unit. A unit is an item's stamp, or one stroke of a brush item, so that one large brush item is not dropped whole.
        let cam = camera.normalized();
        let mut units: Vec<(ItemId, u32)> = entries.iter().map(|e| (e.id, e.unit)).collect();
        units.sort_unstable();
        units.dedup();
        let in_unit = |e: &Entry, u: &(ItemId, u32)| e.id == u.0 && e.unit == u.1;
        let near =
            |u: &(ItemId, u32)| entries.iter().filter(|e| in_unit(e, u)).map(|e| e.stamp.dir.dot(cam)).fold(f64::NEG_INFINITY, f64::max);
        units.sort_by(|a, b| near(b).total_cmp(&near(a)).then(a.cmp(b)));
        let mut kept: Vec<(ItemId, u32)> = Vec::new();
        let mut count = 0;
        for u in units {
            let n = entries.iter().filter(|e| in_unit(e, &u)).count();
            if count + n > MAX_DELTA_STAMPS {
                break;
            }
            count += n;
            kept.push(u);
        }
        entries.retain(|e| kept.iter().any(|u| in_unit(e, u)));
    }
    let dropped = total - entries.len();
    // Application order: by item; a changed item's removals before its additions; a brush item's strokes in file order.
    entries.sort_by_key(|e| (e.priority, e.id, e.addition, e.unit));
    PreviewList { stamps: entries.into_iter().map(|e| e.stamp).collect(), dropped }
}
