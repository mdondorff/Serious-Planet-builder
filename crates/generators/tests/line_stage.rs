//! GEN-028: the line stage. The profiles of a ridge and a channel, the distance to a polyline (near the pole and across the antimeridian
//! included), the combination of the lines after the stamps, the bounds over a cap, and the contract of the stage (ADR 0021 point 7).

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::{generate_tile_field, HeightField, Reference};
use planet_generators::stamp::{apply_stamps, channel_delta, polyline_angle, ridge_delta, StampStage};
use planet_world_def::footprint::{angle_between, Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

/// The point `d_m` metres from `anchor` towards `hint` on the sphere.
fn at_distance(anchor: Vec3, hint: Vec3, d_m: f64) -> Vec3 {
    let t = (hint - anchor * anchor.dot(hint)).normalized();
    let a = d_m / R;
    (anchor * libm::cos(a) + t * libm::sin(a)).normalized()
}

/// A line item through `points` (degrees): reach just holds the polyline and its width.
fn line(id: u128, kind: StampKind, amount: f64, width: f64, points: &[(f64, f64)]) -> ItemSpec {
    let dirs: Vec<Vec3> = points.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: dirs[0],
        content_hash: 0,
        reach_m: far + width,
        max_delta_m: amount,
        stamp: None,
    }
    .with_stamp(Stamp::new(kind, [amount, width, 0.0]).with_points(&dirs))
}

fn hill(id: u128, at: Vec3, reach_m: f64, amount: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m,
        max_delta_m: amount.abs(),
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

fn world(items: &[ItemSpec]) -> Arc<WorldDefinition> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i.clone()).unwrap();
    }
    Arc::new(d)
}

fn footprint(def: &WorldDefinition, centre: Vec3, angle: f64) -> Vec<FootprintItem> {
    def.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre, angle }).unwrap()
}

/// The height after the line stage at `p` over a flat base of 0.
fn height(def: &WorldDefinition, p: Vec3) -> f64 {
    apply_stamps(&footprint(def, p, 1e-6), R, p, 0.0)
}

// spec: GEN-028
#[test]
fn the_profiles_equal_their_known_answers_are_monotone_and_zero_at_the_width() {
    let eq = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-12, "{what}: {a} against {b}");
    eq(ridge_delta(10.0, 0.0), 10.0, "ridge crest");
    eq(ridge_delta(10.0, 0.5), 5.625, "ridge half way");
    eq(channel_delta(10.0, 0.0), -10.0, "channel bed");
    eq(channel_delta(10.0, 0.3), -10.0, "channel bed ends at 0.3");
    eq(channel_delta(10.0, 0.65), -5.0, "channel bank at the middle of its slope");
    for f in [ridge_delta(7.0, 1.0), channel_delta(7.0, 1.0), ridge_delta(7.0, 4.0), channel_delta(7.0, 4.0)] {
        assert_eq!(f, 0.0, "exactly zero at and beyond the width");
    }
    // Monotone in x: the ridge never rises, the channel never falls, over a dense sweep, and both are C1 at the width.
    let (mut r_prev, mut c_prev) = (f64::INFINITY, f64::NEG_INFINITY);
    for k in 0..=1200 {
        let x = f64::from(k) / 1000.0;
        let (r, c) = (ridge_delta(7.0, x), channel_delta(7.0, x));
        assert!(r <= r_prev + 1e-15 && (0.0..=7.0).contains(&r), "ridge at {x}: {r}");
        assert!(c >= c_prev - 1e-15 && (-7.0..=0.0).contains(&c), "channel at {x}: {c}");
        (r_prev, c_prev) = (r, c);
    }
    let h = 1e-8;
    for f in [|x| ridge_delta(7.0, x), |x| channel_delta(7.0, x)] as [fn(f64) -> f64; 2] {
        let (v, slope) = (f(1.0 - h), (f(1.0 - h) - f(1.0 - 2.0 * h)) / h);
        assert!(v.abs() <= 1e-9 && slope.abs() <= 1e-4, "value {v} and slope {slope} just inside the width");
    }
}

/// The distance in radians from `p` to the polyline by dense sampling of its arcs: the reference of the analytic distance.
fn sampled_angle(points: &[Vec3], p: Vec3) -> (f64, f64) {
    let (mut best, mut step_max) = (f64::INFINITY, 0.0f64);
    for w in points.windows(2) {
        let len = angle_between(w[0], w[1]);
        let n = 4000;
        step_max = step_max.max(len / f64::from(n));
        for k in 0..=n {
            let t = f64::from(k) / f64::from(n);
            // Spherical interpolation of the arc.
            let q =
                if len < 1e-12 { w[0] } else { (w[0] * libm::sin((1.0 - t) * len) + w[1] * libm::sin(t * len)) * (1.0 / libm::sin(len)) };
            best = best.min(angle_between(p, q));
        }
    }
    (best, step_max)
}

// spec: GEN-028
#[test]
fn the_distance_to_a_polyline_is_the_least_over_its_arcs_near_the_pole_and_across_the_antimeridian() {
    let polylines: [&[(f64, f64)]; 5] = [
        &[(20.0, 30.0), (20.05, 30.0), (20.05, 30.05)],
        &[(0.0, 179.98), (0.0, -179.98), (0.03, -179.95)],
        &[(89.95, 0.0), (89.97, 90.0), (89.9, 200.0 - 360.0)],
        &[(-89.99, 10.0), (-89.9, 120.0)],
        &[(10.0, 10.0), (10.2, 10.0), (10.2, 10.2), (10.0, 10.2), (10.0, 10.01)],
    ];
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for (k, pl) in polylines.iter().enumerate() {
        let pts: Vec<Vec3> = pl.iter().map(|(la, lo)| unit(*la, *lo)).collect();
        let bits: Vec<[u64; 3]> = pts.iter().map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]).collect();
        for _ in 0..25 {
            // Points within a few degrees of the polyline: around a random vertex.
            let v = pts[(rnd() * pts.len() as f64) as usize % pts.len()];
            let hint = Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized();
            let p = at_distance(v, if (hint - v).length() < 1e-3 { Vec3::new(0.0, 0.0, 1.0) } else { hint }, rnd() * 3.0e5);
            let analytic = polyline_angle(&bits, p);
            let (sampled, step) = sampled_angle(&pts, p);
            assert!(
                analytic <= sampled + 1e-12,
                "polyline {k}: the analytic distance {analytic} is the least, not above the sampled {sampled}"
            );
            assert!(
                sampled - analytic <= step + 1e-9,
                "polyline {k}: the sampled {sampled} is within a step {step} of the analytic {analytic}"
            );
        }
        // On a vertex and on the middle of an arc the distance is zero.
        assert!(polyline_angle(&bits, pts[0]) < 1e-12);
        let mid = (pts[0] + pts[1]).normalized();
        assert!(polyline_angle(&bits, mid) < 1e-9, "polyline {k}: the middle of the first arc");
    }
}

// spec: GEN-028
#[test]
fn the_lines_add_the_largest_ridge_and_the_smallest_channel_after_the_stamps_whatever_the_order() {
    let a = [(20.0, 30.0), (20.0, 30.1)];
    let b = [(19.95, 30.05), (20.05, 30.05)]; // crosses a at (20.0, 30.05)
    let ridge_a = line(1, StampKind::Ridge, 40.0, 3_000.0, &a);
    let ridge_b = line(2, StampKind::Ridge, 25.0, 3_000.0, &b);
    let channel_b = line(3, StampKind::Channel, 30.0, 3_000.0, &b);
    let crossing = unit(20.0, 30.05);
    // Two ridges: the larger of the two at the crossing, not their sum.
    let two = world(&[ridge_a.clone(), ridge_b.clone()]);
    assert!((height(&two, crossing) - 40.0).abs() < 1e-3, "two ridges: {}", height(&two, crossing));
    // A ridge crossing a channel adds the ridge's and the channel's deltas.
    let mixed = world(&[ridge_a.clone(), channel_b.clone()]);
    assert!((height(&mixed, crossing) - 10.0).abs() < 1e-3, "40 - 30: {}", height(&mixed, crossing));
    // The same in any order of the items (the ids and priorities differ, the result does not).
    let flipped = world(&[channel_b.clone(), ridge_b.clone(), ridge_a.clone()]);
    let straight = world(&[ridge_a.clone(), ridge_b.clone(), channel_b.clone()]);
    for k in 0..40 {
        let p = at_distance(crossing, unit(20.0, 30.2), f64::from(k) * 400.0 - 8_000.0);
        assert_eq!(height(&flipped, p).to_bits(), height(&straight, p).to_bits(), "point {k}");
    }
    // The two parts of one line that overlap at a joint are not higher than one part's crest.
    let joint = world(&[
        line(4, StampKind::Ridge, 40.0, 3_000.0, &[(20.0, 30.0), (20.0, 30.05)]),
        line(5, StampKind::Ridge, 40.0, 3_000.0, &[(20.0, 30.05), (20.0, 30.1)]),
    ]);
    assert!((height(&joint, crossing) - 40.0).abs() < 1e-3, "the joint: {}", height(&joint, crossing));
    // The lines come after the point stamps: a hill under the ridge is added first, then the ridge's delta.
    let both = world(&[hill(6, crossing, 6_000.0, 20.0), ridge_a]);
    assert!((height(&both, crossing) - 60.0).abs() < 1e-3, "hill 20 + ridge 40: {}", height(&both, crossing));
}

// spec: GEN-028
#[test]
fn a_ridge_across_the_antimeridian_and_one_through_a_pole_have_their_crest_on_the_line() {
    let across = world(&[line(1, StampKind::Ridge, 50.0, 2_000.0, &[(0.0, 179.99), (0.0, -179.99)])]);
    for lon in [179.995, 180.0, -179.995] {
        let h = height(&across, unit(0.0, lon));
        assert!((h - 50.0).abs() < 1e-3, "on the line at {lon}: {h}");
    }
    // 1 km north of the line at the meridian: x = 0.5.
    let off = at_distance(unit(0.0, 180.0), unit(10.0, 180.0), 1_000.0);
    assert!((height(&across, off) - ridge_delta(50.0, 0.5)).abs() < 1e-6);
    let pole = world(&[line(2, StampKind::Channel, 20.0, 1_000.0, &[(89.995, 0.0), (89.995, 180.0)])]);
    // The arc between the two points passes over the pole; the pole is on it.
    assert!((height(&pole, unit(90.0, 0.0)) + 20.0).abs() < 1e-3, "the pole is on the line: {}", height(&pole, unit(90.0, 0.0)));
}

// spec: GEN-028
#[test]
fn the_bounds_over_a_cap_enclose_every_height_the_lines_add_in_it() {
    let items = [
        line(1, StampKind::Ridge, 80.0, 4_000.0, &[(20.0, 30.0), (20.04, 30.02), (20.08, 30.0)]),
        line(2, StampKind::Channel, 60.0, 3_000.0, &[(20.02, 29.98), (20.02, 30.06)]),
    ];
    let def = world(&items);
    let stage = StampStage::new(
        Arc::new(Reference(1)),
        Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>,
        BodyId::PLANET,
        DEFAULT_ITEM_LAYER,
        0,
        1,
    )
    .unwrap();
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for k in 0..60 {
        let centre = at_distance(unit(20.04, 30.02), Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * 12_000.0);
        let angle = (50.0 + rnd() * 6_000.0) / R * if k % 3 == 0 { 0.1 } else { 1.0 };
        let (lo, hi) = stage.height_bounds(centre, angle);
        let (blo, bhi) = Reference(1).height_bounds(centre, angle);
        for s in 0..120 {
            let rim = at_distance(centre, Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * angle * R);
            let h = stage.height(rim);
            assert!(h >= lo && h <= hi, "cap {k} sample {s}: {h} outside [{lo}, {hi}]");
            let base = Reference(1).height(rim);
            assert!(base >= blo && base <= bhi);
        }
    }
}

// spec: GEN-028, GEN-027
#[test]
fn nothing_changes_beyond_the_reach_a_line_stays_within_its_bound_and_a_tile_away_from_it_passes_through() {
    let a = (20.0, 30.0);
    let b = (20.0, 30.06);
    let item = line(1, StampKind::Ridge, 70.0, 2_500.0, &[a, b]);
    let reach = item.reach_m;
    let def = world(std::slice::from_ref(&item));
    let stage = StampStage::new(
        Arc::new(Reference(1)),
        Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>,
        BodyId::PLANET,
        DEFAULT_ITEM_LAYER,
        0,
        1,
    )
    .unwrap();
    let anchor = unit(a.0, a.1);
    for k in 0..200 {
        let angle = f64::from(k) * 0.031;
        let p = at_distance(anchor, unit(20.5 * libm::cos(angle), 30.0 + 3.0 * libm::sin(angle)), 200.0 + f64::from(k) * 150.0);
        let (h, base) = (stage.height(p), Reference(1).height(p));
        assert!((h - base).abs() <= 70.0 + 1e-9, "within the bound at {k}: {}", h - base);
        if angle_between(anchor, p) * R > reach {
            assert_eq!(h.to_bits(), base.to_bits(), "unchanged beyond the reach at {k}");
        }
    }
    // A tile on the other side of the planet passes the stage through and is the base bit for bit.
    let far = TileId::from_direction(&TangentWarp, unit(-20.0, 210.0), 12);
    assert!(stage.passes_through(far, &TangentWarp));
    let (s, base) = (generate_tile_field(&stage, &TangentWarp, far, 16), generate_tile_field(&Reference(1), &TangentWarp, far, 16));
    assert_eq!(s.heights, base.heights, "a tile away from the line is its source");
    // A tile under the line does not pass through.
    let under = TileId::from_direction(&TangentWarp, unit(20.0, 30.03), 12);
    assert!(!stage.passes_through(under, &TangentWarp));
}

fn stage_over(items: &[ItemSpec]) -> StampStage<'static> {
    let def = world(items);
    StampStage::new(Arc::new(Reference(1)), def as Arc<dyn planet_generators::stamp::ItemReader>, BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1)
        .unwrap()
}

// spec: GEN-028
#[test]
fn the_bounds_hold_at_the_joint_of_two_ridges_or_two_channels_and_at_crossings() {
    // Two parts of one line share a point; two ridges (or two channels) overlap there and the stage applies the larger (the smaller), so the
    // bounds may not add their ranges.
    let (a, b) = ([(20.0, 30.0), (20.0, 30.05)], [(20.0, 30.05), (20.0, 30.1)]);
    let cross = [(19.95, 30.05), (20.05, 30.05)];
    let sets: [Vec<ItemSpec>; 4] = [
        vec![line(1, StampKind::Ridge, 40.0, 3_000.0, &a), line(2, StampKind::Ridge, 40.0, 3_000.0, &b)],
        vec![line(1, StampKind::Channel, 40.0, 3_000.0, &a), line(2, StampKind::Channel, 40.0, 3_000.0, &b)],
        vec![line(1, StampKind::Ridge, 40.0, 3_000.0, &a), line(2, StampKind::Ridge, 25.0, 3_000.0, &cross)],
        vec![
            line(1, StampKind::Ridge, 40.0, 3_000.0, &a),
            line(2, StampKind::Channel, 30.0, 3_000.0, &cross),
            line(3, StampKind::Ridge, 40.0, 3_000.0, &b),
        ],
    ];
    for (k, items) in sets.iter().enumerate() {
        let stage = stage_over(items);
        let joint = unit(20.0, 30.05);
        for angle_m in [1.0, 20.0, 100.0, 2_500.0] {
            let (lo, hi) = stage.height_bounds(joint, angle_m / R);
            for s in 0..30 {
                let p = at_distance(joint, unit(20.0 + 0.01 * f64::from(s), 30.05 + 0.007 * f64::from(s)), f64::from(s) / 30.0 * angle_m);
                let h = stage.height(p);
                assert!(h >= lo && h <= hi, "set {k}, cap {angle_m} m, sample {s}: {h} outside [{lo}, {hi}]");
            }
            let h = stage.height(joint);
            assert!(h >= lo && h <= hi, "set {k}, cap {angle_m} m, the joint: {h} outside [{lo}, {hi}]");
        }
    }
}

// spec: GEN-028
#[test]
fn a_short_arc_keeps_its_great_circle_so_the_distance_and_the_bounds_hold_to_a_nanometre() {
    for len_m in [0.01, 0.1, 1.0, 10.0, 40_000.0] {
        let p0 = unit(35.0, 20.0);
        let p1 = at_distance(p0, unit(35.5, 20.2), len_m);
        let bits: Vec<[u64; 3]> = [p0, p1].iter().map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]).collect();
        let mid = (p0 + p1).normalized();
        let d_mid = polyline_angle(&bits, mid) * R;
        assert!(d_mid < 1e-6, "{len_m} m arc: the middle of the arc is {d_mid} m from it");
        // Off the arc by a known amount, abeam of the middle.
        let side = (p0 - p1).cross(mid).normalized();
        let off = (mid * libm::cos(0.5 / R) + side * libm::sin(0.5 / R)).normalized();
        let d_off = polyline_angle(&bits, off) * R;
        // Abeam of the middle of an arc the nearest point is the middle: 0.5 m (within the arc's half length for the shortest arcs, whose
        // ends are as near).
        assert!((d_off - 0.5).abs() <= 1e-3 + len_m / 2.0, "{len_m} m arc: {d_off} against 0.5");
    }
    // The bounds of a ridge on a 1 cm and a 10 cm arc, width 1 m, over caps of 10 cm.
    for len_m in [0.01, 0.1, 1.0] {
        let p0 = unit(35.0, 20.0);
        let p1 = at_distance(p0, unit(35.5, 20.2), len_m);
        let item = {
            let dirs = [p0, p1];
            ItemSpec {
                id: 1,
                layer: DEFAULT_ITEM_LAYER.into(),
                priority: 0,
                dir: p0,
                content_hash: 0,
                reach_m: len_m + 1.5,
                max_delta_m: 100.0,
                stamp: None,
            }
            .with_stamp(Stamp::new(StampKind::Ridge, [100.0, 1.0, 0.0]).with_points(&dirs))
        };
        let stage = stage_over(std::slice::from_ref(&item));
        let mut seed = 99u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for k in 0..80 {
            let c = at_distance(p0, Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * (len_m + 1.0));
            let (lo, hi) = stage.height_bounds(c, 0.1 / R);
            for _ in 0..40 {
                let p = at_distance(c, Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * 0.1);
                let h = stage.height(p);
                assert!(h >= lo && h <= hi, "{len_m} m arc, cap {k}: {h} outside [{lo}, {hi}]");
            }
        }
    }
}

/// A reader that records every item the stage asked for (the recording context of ADR 0021 point 7).
struct Recording {
    inner: Arc<WorldDefinition>,
    seen: std::sync::Mutex<Vec<u128>>,
}

impl planet_generators::stamp::ItemReader for Recording {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        self.inner.radius_m(body)
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        let items = self.inner.items(body, layer, cap);
        self.seen.lock().unwrap().extend(items.iter().map(|i| i.id));
        items
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.inner.max_delta_sum(body, layer)
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<planet_world_def::ReadRecord> {
        self.inner.tile_reads(body, layer, tile, halo)
    }
    fn revision(&self) -> u64 {
        self.inner.revision()
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.inner.footprint_empty(body, layer, tile, halo)
    }
}

// spec: GEN-028, WDEF-007
#[test]
fn every_line_the_stage_reads_over_a_tile_is_in_the_footprint_the_tile_declared_and_its_key_follows_the_line() {
    use planet_generators::stamp::{ItemReader, STAGE_HALO};
    let items = vec![
        line(1, StampKind::Ridge, 60.0, 400.0, &[(20.0, 30.0), (20.004, 30.003), (20.0, 30.006)]),
        line(2, StampKind::Channel, 30.0, 300.0, &[(19.998, 29.999), (19.998, 30.01)]),
        line(3, StampKind::Ridge, 50.0, 500.0, &[(25.0, 40.0), (25.01, 40.0)]), // far away
    ];
    let def = world(&items);
    let at = unit(20.0, 30.003);
    for level in [13u8, 15, 17] {
        let tile = TileId::from_direction(&TangentWarp, at, level);
        let reader = Arc::new(Recording { inner: Arc::clone(&def), seen: std::sync::Mutex::default() });
        let s = StampStage::new(Arc::new(Reference(1)), reader.clone() as Arc<dyn ItemReader>, BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1)
            .unwrap();
        let mut dirs = Vec::new();
        for l in -1..=17i64 {
            for k in -1..=17i64 {
                dirs.push(tile.sample_direction_ext(&TangentWarp, 16, k, l));
            }
        }
        let _ = s.heights(&dirs);
        let declared: Vec<u128> =
            def.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, STAGE_HALO).unwrap().iter().map(|i| i.id).collect();
        let seen = reader.seen.lock().unwrap().clone();
        assert!(!seen.is_empty(), "level {level}: the lines under the tile are read");
        for id in &seen {
            assert!(declared.contains(id), "level {level}: line {id} was read but the tile did not declare it (declared {declared:?})");
        }
        for (k, d) in dirs.iter().enumerate() {
            if s.height(*d) != Reference(1).height(*d) {
                assert!(!declared.is_empty() && !seen.is_empty(), "level {level} sample {k}: the height changed with no line read");
            }
        }
        assert!(!declared.contains(&3), "level {level}: the far line is not declared");
    }
    // The key of a tile follows the line: a moved point re-keys the tile under it, a far tile keeps its key, and a new stage version re-keys.
    let key = |def: &Arc<WorldDefinition>, version: u32, tile: TileId| {
        StampStage::new(Arc::new(Reference(1)), Arc::clone(def) as Arc<dyn ItemReader>, BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, version)
            .unwrap()
            .inputs_hash_for(tile, &TangentWarp)
    };
    let under = TileId::from_direction(&TangentWarp, at, 14);
    let far = TileId::from_direction(&TangentWarp, unit(-30.0, 200.0), 14);
    let mut moved_items = items.clone();
    moved_items[0] = line(1, StampKind::Ridge, 60.0, 400.0, &[(20.0, 30.0), (20.004, 30.004), (20.0, 30.006)]);
    let moved = world(&moved_items);
    assert_ne!(key(&def, 2, under), key(&moved, 2, under), "a moved point re-keys the tile under the line");
    assert_eq!(key(&def, 2, far), key(&moved, 2, far), "a tile away from it keeps its key");
    assert_ne!(key(&def, 1, under), key(&def, 2, under), "a new stage version re-keys");
}

// spec: GEN-028
#[test]
fn a_sharp_bend_has_the_height_of_its_nearest_arc_and_the_bounds_are_not_wider_than_the_declared_deltas_allow() {
    // A polyline that doubles back on itself at 170 degrees: the height is the profile of the distance to the nearer arc.
    let pl = [(20.0, 30.0), (20.0, 30.05), (20.001, 30.0)];
    let pts: Vec<Vec3> = pl.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let def = world(&[line(1, StampKind::Ridge, 40.0, 1_500.0, &pl)]);
    let mut worst = 0.0f64;
    for k in 0..60 {
        let p = at_distance(unit(20.0005, 30.02), unit(20.5, 30.03), f64::from(k) * 40.0 - 600.0);
        let (d, _) = sampled_angle(&pts, p);
        let want = ridge_delta(40.0, d * R / 1_500.0);
        worst = worst.max((height(&def, p) - want).abs());
    }
    assert!(worst < 5e-3, "the height follows the nearest arc at the bend: worst {worst} m");
    // The bounds over a cap are as wide as the base's plus what the declared deltas allow (twice the largest delta of each line item).
    let stage = stage_over(&[
        line(1, StampKind::Ridge, 80.0, 4_000.0, &[(20.0, 30.0), (20.04, 30.02)]),
        line(2, StampKind::Channel, 60.0, 3_000.0, &[(20.02, 29.98), (20.02, 30.06)]),
    ]);
    let centre = unit(20.02, 30.01);
    for angle_m in [10.0, 500.0, 4_000.0] {
        let (lo, hi) = stage.height_bounds(centre, angle_m / R);
        let (blo, bhi) = Reference(1).height_bounds(centre, angle_m / R);
        assert!(
            hi - lo <= (bhi - blo) + 80.0 + 60.0 + 1e-6,
            "cap {angle_m} m: width {} against the base's {} plus 140",
            hi - lo,
            bhi - blo
        );
    }
}
