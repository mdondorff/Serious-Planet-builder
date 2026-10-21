//! The stamp stage (GEN-026): the authored items of one layer added to a base height field.
//!
//! A stamp is a delta of the height around its anchor (a hill, a crater, a plateau); the stage adds the stamps whose reach contains a
//! point to the base height there, in application order, and then the lines (a ridge and a channel, GEN-028: a delta of the distance to
//! a polyline, combined by the largest ridge and the smallest channel). It reads items once per batch through an [`ItemReader`] (so a
//! contract test can record what it read), reads the base only as a height field at points, and chains its tile keys to the base's
//! (STRM-027).

use std::sync::{Arc, RwLock};

use planet_core::{FaceMapping, TileId, Vec3};
pub use planet_world_def::footprint::polyline_angle;
use planet_world_def::footprint::{angle_between, point_of, stroke_angle, Cap, FootprintItem};
use planet_world_def::stamp::{BrushKind, Stamp, StampKind};
use planet_world_def::{BodyId, GenContext, ReadRecord, WorldDefinition};

use crate::height::HeightField;

/// The halo, as a fraction of the tile edge, with which the stage declares its items for a tile (GEN-026): the sample stencil and the
/// skirt need a small part of it; a larger halo only adds false positives. It is part of the key through the kind 3 and 4 entries, and a
/// change bumps the stage version.
pub const STAGE_HALO: f64 = 0.25;

/// `s(t) = t^2 (3 - 2t)`.
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// The delta of a hill of height `amount` (signed) at `x = distance / reach`.
pub fn hill_delta(amount: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let q = 1.0 - x * x;
    amount * q * q
}

/// The `x = distance / reach` of a crater's rim crest, where its delta is greatest.
const CRATER_CREST_X: f64 = 0.85;

/// The delta of a crater of depth `depth` (at least 0) at `x = distance / reach`: a bowl with a low rim.
pub fn crater_delta(depth: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let bowl = 1.0 - smooth((x / 0.8).min(1.0));
    let rim = if (0.7..=1.0).contains(&x) {
        let u = (x - 0.85) / 0.15;
        let v = 1.0 - u * u;
        v * v
    } else {
        0.0
    };
    depth * (-bowl + 0.2 * rim)
}

/// The weight of a plateau at `x = distance / reach`: 1 up to 0.7, falling to 0 at 1.
pub fn plateau_weight(x: f64) -> f64 {
    1.0 - smooth(((x - 0.7) / 0.3).clamp(0.0, 1.0))
}

/// The delta of a plateau toward the absolute height `target` at `x`, for the running height `h` and the bound `m`.
pub fn plateau_delta(target: f64, h: f64, m: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    plateau_weight(x) * (target - h).clamp(-m, m)
}

/// The delta of a ridge of height `amount` (at least 0) at `x = distance / width` from its polyline (GEN-028): the profile of a hill.
pub fn ridge_delta(amount: f64, x: f64) -> f64 {
    hill_delta(amount, x)
}

/// The delta of a channel of depth `amount` (at least 0) at `x = distance / width` from its polyline (GEN-028): a flat bed out to 0.3 of the
/// width, then a smooth rise to the ground at the width; at most 0, non-decreasing in `x`.
pub fn channel_delta(amount: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let bed = if x <= 0.3 { 1.0 } else { 1.0 - smooth((x - 0.3) / 0.7) };
    -amount * bed
}

/// The weight of a brush stroke at `x = distance / radius` from its polyline (GEN-030): the profile of a ridge, 1 on the line, 0 from `x = 1`,
/// smooth to the second order at the edge, non-increasing in `x`.
pub fn stroke_weight(x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let u = 1.0 - x * x;
    u * u
}

/// The eight directions at the angle `angle` from the unit vector `dir`, on the bearings 0, 45, ... 315 degrees from the local east axis
/// (GEN-030). East is `z x dir` (the y axis near a pole, where it is undefined); `libm` sine and cosine, f64 (CON-14).
fn ring(dir: Vec3, angle: f64) -> [Vec3; 8] {
    let east = Vec3::new(0.0, 0.0, 1.0).cross(dir);
    let east = if east.length() > 1e-9 { east.normalized() } else { Vec3::new(0.0, 1.0, 0.0) };
    let north = dir.cross(east);
    let (sa, ca) = (libm::sin(angle), libm::cos(angle));
    let mut out = [dir; 8];
    for (k, q) in out.iter_mut().enumerate() {
        let bearing = k as f64 * core::f64::consts::FRAC_PI_4;
        let (sb, cb) = (libm::sin(bearing), libm::cos(bearing));
        *q = (dir * ca + (east * cb + north * sb) * sa).normalized();
    }
    out
}

/// What the smooth strokes of a layer have done at one point, over all its brush items (GEN-030): the sum of their deltas, the means of the
/// rings already read by radius (strokes of one radius share their ring), and the base at the point. One drag of the editor is one item, so
/// the count runs over the items: each pulls towards the mean of the procedural base, and what an earlier item moved is not moved again.
#[derive(Default)]
struct SmoothState {
    smoothed: f64,
    rings: Vec<(u64, f64)>,
    b: Option<f64>,
}

/// The running height after the strokes of a brush `stamp` (GEN-030) at `dir`, from `h`; `base_at` reads the procedural base (the field below
/// every authored layer: its key is field-wide, ADR 0066 point 4) at the point and on a smooth's ring. A stroke the point is beyond changes
/// nothing, bits included.
fn apply_brush(stamp: &Stamp, radius_m: f64, dir: Vec3, mut h: f64, base_at: &dyn Fn(Vec3) -> f64, smooth: &mut SmoothState) -> f64 {
    let SmoothState { smoothed, rings, b } = smooth;
    for s in stamp.strokes() {
        let (radius, strength) = (s.radius_m(), s.strength_m());
        // The stroke's disc first: a point beyond its first point's spread plus its radius is beyond the stroke (one angle, not one per arc). An arc
        // between two points within `spread` of the first stays within it (caps under 90 degrees; an item is one cell edge across, WDEF-007).
        let reach = s.spread_angle() + radius / radius_m;
        if angle_between(point_of(&s.points_bits()[0]), dir) > reach * (1.0 + 1e-9) + 1e-12 {
            continue;
        }
        let x = radius_m * stroke_angle(s.points_bits(), dir) / radius;
        if x >= 1.0 {
            continue;
        }
        let w = stroke_weight(x);
        // A delta of 0 leaves the bits of the running height alone (a height of -0.0 stays -0.0).
        let delta = match s.kind {
            BrushKind::Raise => strength * w,
            BrushKind::Lower => -(strength * w),
            BrushKind::Flatten => w * (s.target_m() - h).clamp(-strength, strength),
            BrushKind::Smooth => {
                let bits = s.radius_m().to_bits();
                let mean = match rings.iter().find(|r| r.0 == bits) {
                    Some(r) => r.1,
                    None => {
                        let m = ring(dir, radius / (2.0 * radius_m)).iter().map(|q| base_at(*q)).sum::<f64>() / 8.0;
                        rings.push((bits, m));
                        m
                    }
                };
                let here = *b.get_or_insert_with(|| base_at(dir));
                // Towards the mean of the base round the point by at most the strength, and not past it: what the item's earlier smooth strokes
                // moved is taken off the distance, so repeated passes converge on the mean instead of piling up.
                let d = w * ((mean - here) - *smoothed).clamp(-strength, strength);
                *smoothed += d;
                d
            }
        };
        if delta != 0.0 {
            h += delta;
        }
    }
    h
}

/// The weight of a stroke at its nearest and at its farthest point of `cap` (the weight is non-increasing in the distance, so these bound it).
fn stroke_weights(s: &planet_world_def::stamp::StrokeBits, cap: Cap, radius_m: f64) -> (f64, f64) {
    let slack = 16.0 * f64::EPSILON;
    let d = stroke_angle(s.points_bits(), cap.centre);
    let radius = s.radius_m();
    let x_near = ((d - cap.angle - slack).max(0.0) * radius_m / radius * (1.0 - 1e-9)).max(0.0);
    let x_far = (d + cap.angle + slack) * radius_m / radius * (1.0 + 1e-9);
    (stroke_weight(x_near), stroke_weight(x_far))
}

/// The delta of `stamp` at `x` for the running height `h` and the item's bound `m`. The lines are not applied here: their distance is
/// to a polyline, not an anchor ([`apply_stamps`]).
pub fn stamp_delta(stamp: &Stamp, m: f64, h: f64, x: f64) -> f64 {
    let p = stamp.params();
    match stamp.kind {
        StampKind::Hill => hill_delta(p[0], x),
        StampKind::Crater => crater_delta(p[0], x),
        StampKind::Plateau => plateau_delta(p[0], h, m, x),
        // The lines are applied over their polyline and the regions only cut (apply_stamps), not by this radial profile.
        StampKind::Ridge | StampKind::Channel | StampKind::Region | StampKind::Brush => 0.0,
    }
}

/// The nearest and the farthest `x = distance / reach` of `cap` from the anchor of a radial `item`: the angles widen outward by a few ulps of
/// a radian (an angle from `atan2` is wrong by about that much however short the reach), then the distances by a relative 1e-9: the profile
/// argument is rounded.
fn radial_x(item: &FootprintItem, cap: Cap, radius_m: f64) -> (f64, f64) {
    let reach = f64::from_bits(item.reach_bits);
    let d = angle_between(anchor_of(item), cap.centre);
    let to_x = |angle: f64| angle * radius_m / reach;
    let slack = 16.0 * f64::EPSILON;
    ((to_x((d - cap.angle - slack).max(0.0)) * (1.0 - 1e-9)).max(0.0), (to_x(d + cap.angle + slack) * (1.0 + 1e-9)).min(1.0))
}

/// The range `(lo, hi)` of the delta a hill, a crater or a line `item` can add at any point of `cap` (GEN-026), widened outward by a few
/// ulps. The profiles are monotone in `x` (the hill, the bowl, the line's profile), so the range comes from the nearest and the farthest
/// distance of the cap from the anchor or the polyline; a crater whose cap reaches its rim takes the rim's maximum. An item that is no stamp
/// adds nothing; a plateau and a brush map the running height instead and are not ranged here (`HeightRange`).
fn delta_range(item: &FootprintItem, cap: Cap, radius_m: f64) -> (f64, f64) {
    let Some(stamp) = &item.stamp else { return (0.0, 0.0) };
    if stamp.kind.is_region() {
        // A region changes no height itself: it only cuts the deltas of the other items (GEN-029), and the cut stays inside their range.
        return (0.0, 0.0);
    }
    let m = f64::from_bits(item.max_delta_bits);
    let grow = |x: f64| x.abs() * 8.0 * f64::EPSILON;
    if stamp.kind.is_line() {
        // The profile is monotone in the distance to the polyline: the nearest and the farthest distance of the cap bound the range
        // (GEN-028), widened as the radial stamps' are.
        let [amount, width, _] = stamp.params();
        let slack = 16.0 * f64::EPSILON;
        let d = polyline_angle(stamp.points_bits(), cap.centre);
        let x_near = ((d - cap.angle - slack).max(0.0) * radius_m / width * (1.0 - 1e-9)).max(0.0);
        let x_far = ((d + cap.angle + slack) * radius_m / width * (1.0 + 1e-9)).min(1.0);
        let (lo, hi) = if stamp.kind == StampKind::Ridge {
            (ridge_delta(amount, x_far), ridge_delta(amount, x_near))
        } else {
            (channel_delta(amount, x_near), channel_delta(amount, x_far))
        };
        return ((lo - grow(lo)).max(-m - grow(m)), (hi + grow(hi)).min(m + grow(m)));
    }
    let (x_min, x_max) = radial_x(item, cap, radius_m);
    let a = stamp.params()[0];
    let (lo, hi) = match stamp.kind {
        StampKind::Hill => {
            let (u, v) = (hill_delta(a, x_min), hill_delta(a, x_max));
            (u.min(v), u.max(v))
        }
        StampKind::Crater => {
            // The profile rises to the rim's crest at `x = 0.85` (the bowl is non-decreasing to 0.8 and the rim's bump up to its crest) and
            // falls after it: its range over `[x_min, x_max]` is that of the end values and, where the crest is inside, the crest's.
            let (u, v) = (crater_delta(a, x_min), crater_delta(a, x_max));
            let (mut lo, mut hi) = (u.min(v), u.max(v));
            if x_min <= CRATER_CREST_X && CRATER_CREST_X <= x_max {
                let crest = crater_delta(a, CRATER_CREST_X);
                (lo, hi) = (lo.min(crest), hi.max(crest));
            }
            (lo, hi)
        }
        StampKind::Plateau | StampKind::Ridge | StampKind::Channel | StampKind::Region | StampKind::Brush => {
            unreachable!("the plateaus and the brushes map the running height (`HeightRange`), the lines and the regions returned above")
        }
    };
    ((lo - grow(lo)).max(-m - grow(m)), (hi + grow(hi)).min(m + grow(m)))
}

/// The unit direction of an item's anchor.
fn anchor_of(item: &FootprintItem) -> Vec3 {
    Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]))
}

/// The running height at `dir` after the stamps of `items` (application order, the caller's) were added to `base`, on a body of `radius_m`.
pub fn apply_stamps(items: &[FootprintItem], radius_m: f64, dir: Vec3, base: f64) -> f64 {
    apply_stamps_with(items, radius_m, dir, base, &|_| base)
}

/// [`apply_stamps`] with `base_at`, the stage input (the layers below) at any direction: the brush strokes of a smooth read it on a ring round the
/// point (GEN-030). `base` is `base_at(dir)`. [`apply_stamps`] gives a constant input, on which a smooth changes nothing.
pub fn apply_stamps_with(items: &[FootprintItem], radius_m: f64, dir: Vec3, base: f64, base_at: &dyn Fn(Vec3) -> f64) -> f64 {
    // A region of mode 2 (clip inside) that contains the point cuts every delta of the layer, whatever the order of the items (GEN-029):
    // the height there is the base's. The polygon lies within the item's reach of its anchor (WDEF-012), so a point beyond it is outside.
    for item in items {
        let Some(stamp) = &item.stamp else { continue };
        if stamp.kind.is_region()
            && stamp.params()[1] == 2.0
            && radius_m * angle_between(anchor_of(item), dir) <= f64::from_bits(item.reach_bits)
            && planet_world_def::region::polygon_contains_bits(stamp.points_bits(), dir)
        {
            return base;
        }
    }
    let mut h = base;
    let mut smooth = SmoothState::default();
    // The lines are combined after the point stamps: the largest ridge and the smallest channel at the point (GEN-028), so the result does
    // not depend on the order of the lines and overlapping parts of a line do not add up.
    let (mut ridge, mut channel) = (0.0f64, 0.0f64);
    for item in items {
        let Some(stamp) = &item.stamp else { continue };
        let reach = f64::from_bits(item.reach_bits);
        let x = radius_m * angle_between(anchor_of(item), dir) / reach;
        if x >= 1.0 || stamp.kind.is_region() {
            // Out of the item's reach: the reach holds the polyline and its width (WDEF-011), so the item adds nothing here. A region
            // adds no height (it only cuts, above): it must not touch the bits of the running height either (a base of -0.0 stays).
            continue;
        }
        if stamp.kind.is_line() {
            let [amount, width, _] = stamp.params();
            let xl = radius_m * polyline_angle(stamp.points_bits(), dir) / width;
            if stamp.kind == StampKind::Ridge {
                ridge = ridge.max(ridge_delta(amount, xl));
            } else {
                channel = channel.min(channel_delta(amount, xl));
            }
        } else if stamp.kind.is_brush() {
            h = apply_brush(stamp, radius_m, dir, h, base_at, &mut smooth);
        } else {
            h += stamp_delta(stamp, f64::from_bits(item.max_delta_bits), h, x);
        }
    }
    // Only a line that is there changes the bits of the running height (a base of -0.0 stays -0.0 without one).
    if ridge != 0.0 {
        h += ridge;
    }
    if channel != 0.0 {
        h += channel;
    }
    h
}

/// Where a stage reads its items: the footprint of a cap, the sum of the bounds of a layer, and the reads a tile declares. The world
/// definition implements it; the contract test (ADR 0021 point 7) swaps in a reader that records what the stage asked for.
pub trait ItemReader: Send + Sync {
    /// The radius of the body, if it has a usable one.
    fn radius_m(&self, body: BodyId) -> Option<f64>;
    /// The items of `layer` whose reach touches `cap`, in application order (WDEF-007).
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem>;
    /// The sum of `max_delta_m` over every item of the layer (a bound for field-wide uses).
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64;
    /// The reads a tile declares for its items, with the stage's halo (WDEF-007, WDEF-008): the footprint or summary entries.
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord>;
    /// The revision of the items (never 0): differs after every change and between definitions (STRM-027).
    fn revision(&self) -> u64;
    /// Whether no item of `layer` touches `tile` with `halo` (WDEF-007): the footprint the tile declares is empty.
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool;
}

/// The range of the running height of a stage at the points of a cap, carried through the items in the order the stage applies them
/// (GEN-026): an item that adds a delta independent of the height widens it by the delta's range; an item that pulls the height towards a
/// target (a plateau, a flatten stroke) maps it, and its map is monotone in the height, so the images of the ends are the ends of the image.
/// `mag` is the largest magnitude met on the way (the rounding of the stage's additions is a few ulps of it).
struct HeightRange {
    lo: f64,
    hi: f64,
    mag: f64,
    ops: usize,
}

impl HeightRange {
    fn new(lo: f64, hi: f64) -> HeightRange {
        HeightRange { lo, hi, mag: lo.abs().max(hi.abs()), ops: 0 }
    }

    /// The height plus a delta in `[a, b]`.
    fn add(&mut self, a: f64, b: f64) {
        // A delta of 0 leaves the bits alone: with no item in reach the range is the base's, exactly.
        if a == 0.0 && b == 0.0 {
            return;
        }
        self.lo += a;
        self.hi += b;
        self.touch(a.abs().max(b.abs()));
    }

    /// The height after `h + w * clamp(target - h, -m, m)` for a weight `w` in `[w_lo, w_hi]`: for a fixed `w` the map is non-decreasing in
    /// `h` (its slope is `1 - w` or 1), and for a fixed `h` it is linear in `w`, so the range comes from the ends of both.
    fn seek(&mut self, target: f64, m: f64, w_lo: f64, w_hi: f64) {
        let f = |w: f64, h: f64| h + w * (target - h).clamp(-m, m);
        let (lo, hi) = (f(w_lo, self.lo).min(f(w_hi, self.lo)), f(w_lo, self.hi).max(f(w_hi, self.hi)));
        self.touch(target.abs().max((target - self.lo).abs()).max((target - self.hi).abs()));
        self.lo = lo;
        self.hi = hi;
    }

    fn touch(&mut self, m: f64) {
        self.ops += 1;
        self.mag = self.mag.max(self.lo.abs()).max(self.hi.abs()).max(m);
    }
}

impl ItemReader for WorldDefinition {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        match self.get(&planet_world_def::body_radius_path(body)) {
            Some(planet_world_def::NodeValue::F64(r)) if r.is_finite() && r > 0.0 => Some(r),
            _ => None,
        }
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        self.footprint_items_cap(body, layer, cap).expect("a stage reads the items of a body that has a radius")
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.item_ids(body)
            .into_iter()
            .filter(|id| self.item_layer(body, *id).as_deref() == Some(layer))
            .filter_map(|id| self.item_max_delta(body, id))
            .sum()
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        let ctx = GenContext::new(self);
        ctx.query_footprint(body, layer, tile, halo).expect("a stage declares the items of a body that has a radius");
        ctx.read_records()
    }
    fn revision(&self) -> u64 {
        WorldDefinition::revision(self)
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.footprint_items(body, layer, tile, halo).is_ok_and(|v| v.is_empty())
    }
}

/// A definition shared between the editor and the stage: the stage reads whatever it holds at the moment of a batch.
impl ItemReader for RwLock<WorldDefinition> {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        self.read().expect("definition lock").radius_m(body)
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        self.read().expect("definition lock").items(body, layer, cap)
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.read().expect("definition lock").max_delta_sum(body, layer)
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        self.read().expect("definition lock").tile_reads(body, layer, tile, halo)
    }
    fn revision(&self) -> u64 {
        self.read().expect("definition lock").revision()
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.read().expect("definition lock").footprint_empty(body, layer, tile, halo)
    }
}

/// The stamps of one layer added to a base field (GEN-026).
pub struct StampStage<'a> {
    base: Arc<dyn HeightField + 'a>,
    items: Arc<dyn ItemReader>,
    body: BodyId,
    layer: String,
    stack_index: u32,
    stage_version: u32,
    inputs: u64,
}

/// The version of the stamp stage (bumped whenever its output changes, ADR 0006): part of the key of every stamped tile.
pub const STAMP_STAGE_VERSION: u32 = 4;

/// The implementation id of the stamp stage in height keys.
pub const STAMP_STAGE_ID: &str = "stamp-stage";

impl<'a> StampStage<'a> {
    /// A stage over `base` for `layer` of `body`, at `stack_index` in the stack of ADR 0007, version `stage_version`. `None` when the
    /// reader knows no usable radius for the body.
    pub fn new(
        base: Arc<dyn HeightField + 'a>,
        items: Arc<dyn ItemReader>,
        body: BodyId,
        layer: &str,
        stack_index: u32,
        stage_version: u32,
    ) -> Option<StampStage<'a>> {
        items.radius_m(body)?;
        let text: Vec<u64> = layer.bytes().map(u64::from).collect();
        let inputs = planet_core::hash::hash_u64s(
            0x0073_7467,
            &[base.inputs_hash(), planet_core::hash::hash_u64s(3, &text), u64::from(stack_index), u64::from(stage_version)],
        );
        Some(StampStage { base, items, body, layer: layer.to_string(), stack_index, stage_version, inputs })
    }

    /// The items a batch of directions needs: those whose reach touches the smallest cap this stage builds around them.
    fn items_for(&self, dirs: &[Vec3]) -> Vec<FootprintItem> {
        let Some(&first) = dirs.first() else { return Vec::new() };
        let sum = dirs.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, d| a + *d);
        let centre = if sum.length() > 1e-9 { sum.normalized() } else { first };
        let angle = dirs.iter().fold(0.0f64, |a, d| a.max(angle_between(centre, *d)));
        self.items.items(self.body, &self.layer, Cap { centre, angle })
    }
}

impl HeightField for StampStage<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.heights(&[dir])[0]
    }

    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let base = self.base.heights_until(dirs, stop);
        self.add_stamps(dirs, base, stop)
    }

    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        let base = self.base.heights_threaded_until(dirs, stop, threads);
        self.add_stamps(dirs, base, stop)
    }

    fn seed(&self) -> u64 {
        self.base.seed()
    }

    fn implementation_id(&self) -> &'static str {
        STAMP_STAGE_ID
    }

    fn version(&self) -> u32 {
        self.stage_version
    }

    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m() + self.items.max_delta_sum(self.body, &self.layer)
    }

    fn inputs_hash(&self) -> u64 {
        self.inputs
    }

    fn read_records(&self) -> Vec<ReadRecord> {
        self.base.read_records()
    }

    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let mut reads = self.base.read_records_for(tile, map);
        reads.extend(self.items.tile_reads(self.body, &self.layer, tile, STAGE_HALO));
        reads
    }

    fn reads_cells(&self) -> bool {
        true
    }

    /// The base's hash for the tile, its version, implementation and bake key, the stage's own inputs and the tile's item reads (CON-07):
    /// the 64-bit in-memory keys chain the stage below as the 128-bit ones do.
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        let source = self
            .base
            .source_key()
            .map_or([0u64; 2], |k| [u64::from_le_bytes(k[..8].try_into().unwrap()), u64::from_le_bytes(k[8..].try_into().unwrap())]);
        let id: Vec<u64> = self.base.implementation_id().bytes().map(u64::from).collect();
        let reads = planet_world_def::records_hash64(&self.items.tile_reads(self.body, &self.layer, tile, STAGE_HALO));
        planet_core::hash::hash_u64s(
            0x0073_7468,
            &[
                self.base.inputs_hash_for(tile, map),
                u64::from(self.base.version()),
                planet_core::hash::hash_u64s(4, &id),
                source[0],
                source[1],
                self.inputs,
                reads,
            ],
        )
    }

    fn body(&self) -> BodyId {
        self.body
    }

    fn surface_id(&self) -> &'static str {
        self.base.surface_id()
    }

    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> crate::climate::ClimateTexture {
        self.base.climate(map, id)
    }

    fn climate_version_word(&self) -> u64 {
        self.base.climate_version_word()
    }

    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        let (base, climate) = self.base.heights_and_climate(map, id, dirs, stop);
        (self.add_stamps(dirs, base, stop), climate)
    }

    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.base.height_bounds(centre, angular_radius);
        let cap = Cap { centre, angle: angular_radius };
        let radius_m = self.items.radius_m(self.body).expect("a stage reads a body that has a radius");
        let items = self.items.items(self.body, &self.layer, cap);
        // The range of the running height through the items in their order, not the sum of what each can add and not their declared maxima:
        // a plateau adds the gap between its target and the ground, which is nearly one number over a fine tile whatever its bound (a cap on the
        // flat top of a hill moves up as a whole and keeps its extent, which is what the morph interval of a fine tile needs, GEN-026).
        let mut run = HeightRange::new(lo, hi);
        // The lines are not summed: the stage adds the largest ridge and the smallest channel (GEN-028), so the ridges' range is the
        // largest of their lows and highs (floored at 0) and the channels' the smallest (capped at 0).
        let (mut ridge_lo, mut ridge_hi, mut channel_lo, mut channel_hi) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        // The smooth strokes of a run (no pulling item in between) add at most what their strengths and weights allow and at most the width of
        // the range their count can take, `smooth_width` (below): what the strokes of the layer moved never passes the mean of the base.
        let smooth_width = {
            let widest = items
                .iter()
                .filter_map(|i| i.stamp.as_ref())
                .flat_map(|s| s.strokes().iter())
                .filter(|s| s.kind == planet_world_def::stamp::BrushKind::Smooth)
                .map(|s| s.radius_m())
                .fold(0.0f64, f64::max);
            (widest > 0.0).then(|| self.smooth_width(cap, radius_m, widest))
        };
        let (mut run_lo, mut run_hi) = (0.0f64, 0.0f64);
        let flush = |run: &mut HeightRange, run_lo: &mut f64, run_hi: &mut f64| {
            if let Some(w) = smooth_width {
                run.add((*run_lo).max(-w), (*run_hi).min(w));
            }
            (*run_lo, *run_hi) = (0.0, 0.0);
        };
        for item in &items {
            let Some(stamp) = item.stamp.as_ref() else { continue };
            match stamp.kind {
                StampKind::Ridge | StampKind::Channel => {
                    let (a, b) = delta_range(item, cap, radius_m);
                    if stamp.kind == StampKind::Ridge {
                        ridge_lo = ridge_lo.max(a);
                        ridge_hi = ridge_hi.max(b);
                    } else {
                        channel_lo = channel_lo.min(a);
                        channel_hi = channel_hi.min(b);
                    }
                }
                StampKind::Region => {}
                StampKind::Hill | StampKind::Crater => {
                    let (a, b) = delta_range(item, cap, radius_m);
                    run.add(a, b);
                }
                StampKind::Plateau => {
                    flush(&mut run, &mut run_lo, &mut run_hi);
                    let (x_min, x_max) = radial_x(item, cap, radius_m);
                    let m = f64::from_bits(item.max_delta_bits);
                    run.seek(stamp.params()[0], m, plateau_weight(x_max), plateau_weight(x_min));
                }
                StampKind::Brush => {
                    use planet_world_def::stamp::BrushKind;
                    for st in stamp.strokes() {
                        let (w_near, w_far) = stroke_weights(st, cap, radius_m);
                        let strength = st.strength_m();
                        match st.kind {
                            BrushKind::Raise => run.add(strength * w_far, strength * w_near),
                            BrushKind::Lower => run.add(-strength * w_near, -strength * w_far),
                            BrushKind::Smooth => {
                                // Each stroke of the run rounds once when the stage adds it, whatever the run is added as here.
                                run.ops += 1;
                                run_lo -= strength * w_near;
                                run_hi += strength * w_near;
                            }
                            BrushKind::Flatten => {
                                flush(&mut run, &mut run_lo, &mut run_hi);
                                run.seek(st.target_m(), strength, w_far, w_near);
                            }
                        }
                    }
                }
            }
        }
        flush(&mut run, &mut run_lo, &mut run_hi);
        // The ridge and the channel are two additions in the stage.
        if ridge_lo + channel_lo != 0.0 || ridge_hi + channel_hi != 0.0 {
            run.ops += 1;
        }
        run.add(ridge_lo + channel_lo, ridge_hi + channel_hi);
        let (mut out_lo, mut out_hi) = (run.lo, run.hi);
        // A region of mode 2 among the items may cut every delta of the layer at a point of the cap (GEN-029): the height there is the base's,
        // so the range holds the base's as well.
        if items.iter().any(|i| i.stamp.as_ref().is_some_and(|s| s.kind.is_region() && s.params()[1] == 2.0)) {
            out_lo = out_lo.min(lo);
            out_hi = out_hi.max(hi);
        }
        // Each step of the running height rounds by at most a few ulps of the largest magnitude involved.
        let slack = run.ops as f64 * run.mag * f64::EPSILON * 4.0 + ((out_lo - lo).abs() + (out_hi - hi).abs()) * 8.0 * f64::EPSILON;
        (out_lo - slack, out_hi + slack)
    }

    fn water_cull_level(&self) -> Option<f64> {
        self.base.water_cull_level()
    }

    fn source_key(&self) -> Option<[u8; 16]> {
        None
    }

    fn upstream(&self) -> Option<&dyn HeightField> {
        Some(self.base.as_ref())
    }

    fn stage_params(&self) -> Option<(&str, u32)> {
        Some((&self.layer, self.stack_index))
    }
    fn item_revision(&self) -> u64 {
        planet_core::hash::hash_u64s(0x0072_6576, &[self.base.item_revision(), self.items.revision()]) | 1
    }

    /// No item of the layer touches the tile (the footprint it declares is empty): the stage adds nothing there, so its heights, climate
    /// and bounds are the base's bit for bit and its keys are the base's (STRM-029, GEN-027).
    ///
    /// Only for tile ids made on the cell grid's mapping (the footprint is made there, `CELL_MAPPING_ID`): on another mapping the same id
    /// is another place, and the stage does not claim it. Never for a root tile (level 0): the normal stencil of a coarse mesh (4 cells)
    /// reaches past the declared cap there, and no root is stored or drawn (the render floor is 3).
    fn passes_through(&self, tile: TileId, map: &dyn FaceMapping) -> bool {
        tile.level() > 0
            && map.id() == planet_world_def::cells::CELL_MAPPING_ID
            && self.items.footprint_empty(self.body, &self.layer, tile, STAGE_HALO)
    }
}

impl StampStage<'_> {
    /// The width of the range that the deltas of a run of smooth strokes can sum to over `cap` (GEN-030): a smooth moves the point towards the
    /// mean of the procedural base on a ring (`widest_m` the largest radius) and never past it, and what the strokes of the layer moved together
    /// stays between 0 and the distance `d` to that mean, so a run adds at most `max(0, d_hi) - min(0, d_lo)`, with `d` from the bounds of the
    /// base over the cap and over the cap widened by the ring.
    fn smooth_width(&self, cap: Cap, radius_m: f64, widest_m: f64) -> f64 {
        let mut bottom: &dyn HeightField = self.base.as_ref();
        while let Some(up) = bottom.upstream() {
            bottom = up;
        }
        let (here_lo, here_hi) = bottom.height_bounds(cap.centre, cap.angle);
        let (ring_lo, ring_hi) =
            bottom.height_bounds(cap.centre, cap.angle + widest_m / (2.0 * radius_m) * (1.0 + 1e-9) + 16.0 * f64::EPSILON);
        let (d_lo, d_hi) = (ring_lo - here_hi, ring_hi - here_lo);
        let grow = (here_lo.abs() + here_hi.abs() + ring_lo.abs() + ring_hi.abs()) * 16.0 * f64::EPSILON;
        d_hi.max(0.0) - d_lo.min(0.0) + grow
    }

    /// The base heights of `dirs` with the stamps added; elements after `stop` turned true are 0.0 (GEN-020).
    fn add_stamps(&self, dirs: &[Vec3], base: Vec<f64>, stop: &dyn Fn() -> bool) -> Vec<f64> {
        let items = self.items_for(dirs);
        let radius_m = self.items.radius_m(self.body).expect("a stage reads a body that has a radius");
        // A smooth reads the procedural base, the field below every authored layer: its key is field-wide, so reading it off the tile's footprint
        // cannot leave a persisted tile stale (ADR 0066 point 4, GEN-030).
        let mut bottom: &dyn HeightField = self.base.as_ref();
        while let Some(up) = bottom.upstream() {
            bottom = up;
        }
        let below = |q: Vec3| bottom.height(q);
        dirs.iter().zip(base).map(|(&d, b)| if stop() { 0.0 } else { apply_stamps_with(&items, radius_m, d, b, &below) }).collect()
    }
}
