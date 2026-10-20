//! GEN-030: the brush stage. The weight, each stroke kind, the order of the strokes, the ring of a smooth, the heights as a function of the
//! direction alone (tile seams, threads), the bounds over a cap, and the stage over a real base.

use std::sync::Arc;

use planet_core::Vec3;
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::{apply_stamps, apply_stamps_with, stroke_weight, StampStage};
use planet_world_def::footprint::{angle_between, Cap, FootprintItem};
use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;
const LAYER: &str = "brush";

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

/// A point east of `p` on its tangent (a hint for [`at_distance`]: the move is due east).
fn east_of(p: Vec3) -> Vec3 {
    p + Vec3::new(0.0, 0.0, 1.0).cross(p).normalized()
}

/// Equal to 1e-6 m: a point on an arc is on it to the rounding of the angle.
fn close(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() <= 1e-6, "{what}: {a} against {b}");
}

fn stroke(kind: BrushKind, radius: f64, strength: f64, target: Option<f64>, points: &[(f64, f64)]) -> Stroke {
    Stroke {
        kind,
        radius_m: radius,
        strength_m: strength,
        target_m: target,
        points: points.iter().map(|(la, lo)| unit(*la, *lo)).collect(),
    }
}

/// A brush item with `strokes` (the first point of the first is the anchor); the reach holds every point plus the largest radius, the bound
/// the sum of the strengths.
fn brush(id: u128, strokes: &[Stroke]) -> ItemSpec {
    let anchor = strokes[0].points[0];
    let far = strokes.iter().flat_map(|s| s.points.iter()).map(|p| angle_between(anchor, *p) * R).fold(0.0f64, f64::max);
    let radius = strokes.iter().map(|s| s.radius_m).fold(0.0f64, f64::max);
    ItemSpec {
        id,
        layer: LAYER.into(),
        priority: 0,
        dir: anchor,
        content_hash: 0,
        reach_m: far + radius,
        max_delta_m: strokes.iter().map(|s| s.strength_m).sum(),
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(strokes))
}

fn world(items: &[ItemSpec]) -> Arc<WorldDefinition> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i.clone()).unwrap();
    }
    Arc::new(d)
}

fn footprint(def: &WorldDefinition, centre: Vec3, angle: f64) -> Vec<FootprintItem> {
    def.footprint_items_cap(BodyId::PLANET, LAYER, Cap { centre, angle }).unwrap()
}

/// The height after the brush stage at `p` over a flat base of `base`.
fn height_over(def: &WorldDefinition, p: Vec3, base: f64) -> f64 {
    apply_stamps(&footprint(def, p, 1e-6), R, p, base)
}

// spec: GEN-030
#[test]
fn the_weight_is_the_ridge_profile_one_on_the_line_zero_from_the_radius_and_never_rises() {
    assert_eq!(stroke_weight(0.0), 1.0);
    assert!((stroke_weight(0.5) - 0.5625).abs() < 1e-15);
    assert_eq!(stroke_weight(1.0), 0.0);
    assert_eq!(stroke_weight(3.0), 0.0);
    let mut prev = f64::INFINITY;
    for k in 0..=1200 {
        let w = stroke_weight(f64::from(k) / 1000.0);
        assert!((0.0..=1.0).contains(&w) && w <= prev + 1e-15, "weight at {k}: {w}");
        prev = w;
    }
    // Smooth to the second order at the edge.
    let h = 1e-6;
    assert!(stroke_weight(1.0 - h) < 1e-11, "value just inside the radius");
}

// spec: GEN-030
#[test]
fn raise_and_lower_add_the_strength_on_the_line_and_the_weight_across_it_and_nothing_beyond_the_radius() {
    let line = [(20.0, 30.0), (20.02, 30.0)];
    let def = world(&[brush(1, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line)])]);
    let on = unit(20.01, 30.0);
    let across = |d: f64| at_distance(on, east_of(on), d);
    let eq = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-6, "{what}: {a} against {b}");
    eq(height_over(&def, on, 100.0), 108.0, "on the line");
    eq(height_over(&def, across(100.0), 100.0), 100.0 + 8.0 * 0.5625, "half the radius across");
    close(height_over(&def, across(200.0), 100.0), 100.0, "at the radius");
    close(height_over(&def, across(400.0), 100.0), 100.0, "beyond the radius");
    // The ends are round: a point past the end of the line by half the radius has the half weight.
    eq(height_over(&def, at_distance(unit(20.02, 30.0), unit(20.5, 30.0), 100.0), 100.0), 100.0 + 8.0 * 0.5625, "past the end");
    let lower = world(&[brush(1, &[stroke(BrushKind::Lower, 200.0, 8.0, None, &line)])]);
    eq(height_over(&lower, on, 100.0), 92.0, "lower on the line");
    // A stroke does not add to itself where its parts overlap: a polyline that doubles back adds the strength once.
    let back = world(&[brush(1, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &[(20.0, 30.0), (20.02, 30.0), (20.0, 30.0)])])]);
    eq(height_over(&back, on, 0.0), 8.0, "a doubled-back stroke");
    // A one-point stroke is a dab.
    let dab = world(&[brush(1, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &[(20.0, 30.0)])])]);
    eq(height_over(&dab, unit(20.0, 30.0), 0.0), 8.0, "a dab at its point");
    eq(height_over(&dab, at_distance(unit(20.0, 30.0), unit(20.5, 30.0), 100.0), 0.0), 8.0 * 0.5625, "a dab at half the radius");
    // Two strokes add.
    let two = world(&[brush(1, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line), stroke(BrushKind::Raise, 200.0, 5.0, None, &line)])]);
    eq(height_over(&two, on, 0.0), 13.0, "two raises");
}

// spec: GEN-030
#[test]
fn flatten_moves_towards_its_target_by_at_most_the_strength_and_a_second_pass_goes_further() {
    let line = [(20.0, 30.0), (20.02, 30.0)];
    let on = unit(20.01, 30.0);
    let one = world(&[brush(1, &[stroke(BrushKind::Flatten, 200.0, 4.0, Some(10.0), &line)])]);
    close(height_over(&one, on, 0.0), 4.0, "from 0 towards 10 by the strength");
    close(height_over(&one, on, 12.0), 10.0 + 0.0 * 2.0, "from 12 towards 10 by 2, not by 4: it stops at the target");
    close(height_over(&one, on, 10.0), 10.0, "at the target nothing moves");
    close(height_over(&one, on, 30.0), 26.0, "from above by the strength");
    let three = world(&[brush(
        1,
        &[
            stroke(BrushKind::Flatten, 200.0, 4.0, Some(10.0), &line),
            stroke(BrushKind::Flatten, 200.0, 4.0, Some(10.0), &line),
            stroke(BrushKind::Flatten, 200.0, 4.0, Some(10.0), &line),
        ],
    )]);
    close(height_over(&three, on, 0.0), 10.0, "three passes reach the target");
    // Across the stroke the move is weighted: at half the radius the change from 0 is 4 w.
    let half = at_distance(on, east_of(on), 100.0);
    assert!((height_over(&one, half, 0.0) - 4.0 * 0.5625).abs() < 1e-6);
}

// spec: GEN-030
#[test]
fn the_strokes_of_an_item_apply_in_file_order() {
    let line = [(20.0, 30.0), (20.02, 30.0)];
    let on = unit(20.01, 30.0);
    let raise = || stroke(BrushKind::Raise, 200.0, 10.0, None, &line);
    let level = || stroke(BrushKind::Flatten, 200.0, 10.0, Some(0.0), &line);
    let raise_then_level = world(&[brush(1, &[raise(), level()])]);
    let level_then_raise = world(&[brush(1, &[level(), raise()])]);
    close(height_over(&raise_then_level, on, 0.0), 0.0, "raised, then levelled back");
    close(height_over(&level_then_raise, on, 0.0), 10.0, "levelled (nothing to do), then raised");
}

/// A base with a peak: 100 m at `centre`, falling 1 m per 2 m of ground distance (a cone), as the stage input a smooth reads.
fn cone(centre: Vec3) -> impl Fn(Vec3) -> f64 {
    move |q: Vec3| (100.0 - angle_between(centre, q) * R * 0.5).max(0.0)
}

/// A base sloping 0.2 m per metre towards the east of `origin`'s tangent plane.
fn slope(origin: Vec3) -> impl Fn(Vec3) -> f64 {
    let east = Vec3::new(0.0, 0.0, 1.0).cross(origin).normalized();
    move |q: Vec3| 0.2 * R * q.dot(east)
}

// spec: GEN-030
#[test]
fn smooth_lowers_a_peak_raises_a_pit_leaves_a_plane_and_a_constant_input_alone_and_changes_at_most_the_strength() {
    let at = unit(20.0, 30.0);
    let def = world(&[brush(1, &[stroke(BrushKind::Smooth, 100.0, 3.0, None, &[(20.0, 30.0)])])]);
    let items = footprint(&def, at, 1e-6);
    let peak = cone(at);
    let at_peak = apply_stamps_with(&items, R, at, peak(at), &peak);
    assert!(at_peak < peak(at) && at_peak >= peak(at) - 3.0 - 1e-9, "a peak goes down by at most the strength: {at_peak}");
    let pit = |q: Vec3| 100.0 - peak(q);
    let at_pit = apply_stamps_with(&items, R, at, pit(at), &pit);
    assert!(at_pit > pit(at) && at_pit <= pit(at) + 3.0 + 1e-9, "a pit goes up by at most the strength: {at_pit}");
    let plane = slope(at);
    let on_plane = apply_stamps_with(&items, R, at, plane(at), &plane);
    assert!((on_plane - plane(at)).abs() < 1e-3, "a plane stays: {on_plane} against {}", plane(at));
    assert_eq!(apply_stamps(&items, R, at, 42.0), 42.0, "a constant input stays, bit for bit");
    // The ring is a function of the point and the radius alone: asking again gives the same bits.
    assert_eq!(at_peak.to_bits(), apply_stamps_with(&items, R, at, peak(at), &peak).to_bits());
    // Beyond the radius nothing is read and nothing changes.
    let far = at_distance(at, unit(20.5, 30.0), 150.0);
    assert_eq!(apply_stamps_with(&items, R, far, peak(far), &peak).to_bits(), peak(far).to_bits());
}

// spec: GEN-030
#[test]
fn the_height_is_a_function_of_the_direction_alone_whatever_cap_the_items_came_from_and_the_threads() {
    let line = [(20.0, 30.0), (20.0, 30.03), (20.02, 30.05)];
    let items = [
        brush(1, &[stroke(BrushKind::Raise, 300.0, 9.0, None, &line), stroke(BrushKind::Smooth, 250.0, 4.0, None, &line)]),
        brush(2, &[stroke(BrushKind::Flatten, 200.0, 6.0, Some(20.0), &[(20.01, 30.02), (20.01, 30.04)])]),
    ];
    let def = world(&items);
    let stage = StampStage::new(
        Arc::new(Reference(1)),
        Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>,
        BodyId::PLANET,
        LAYER,
        0,
        1,
    )
    .unwrap();
    let mut dirs = Vec::new();
    for i in 0..40 {
        for j in 0..40 {
            dirs.push(unit(19.995 + 0.0008 * f64::from(i), 29.995 + 0.0013 * f64::from(j)));
        }
    }
    let one = stage.heights_threaded_until(&dirs, &|| false, 1);
    let four = stage.heights_threaded_until(&dirs, &|| false, 4);
    assert_eq!(
        one.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        four.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "the threads change nothing"
    );
    // The same points asked one by one, in reverse, as two halves (two tiles that share a border): the same bits.
    for (k, d) in dirs.iter().enumerate().rev() {
        assert_eq!(stage.height(*d).to_bits(), one[k].to_bits(), "point {k} asked alone");
    }
    let (left, right) = dirs.split_at(dirs.len() / 2);
    let halves: Vec<f64> = stage.heights(left).into_iter().chain(stage.heights(right)).collect();
    assert_eq!(halves.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), one.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    // The stage changed something (the brushes are there) and is the base's where nothing reaches.
    let base: Vec<f64> = Reference(1).heights(&dirs);
    assert!(one.iter().zip(&base).filter(|(a, b)| a != b).count() > 100, "the brushes change the heights");
    let away = unit(-20.0, 210.0);
    assert_eq!(stage.height(away).to_bits(), Reference(1).height(away).to_bits());
}

// spec: GEN-030
#[test]
fn the_bounds_over_a_cap_enclose_every_height_a_brush_adds_in_it_and_stay_within_the_sum_of_the_strengths() {
    let line = [(20.0, 30.0), (20.0, 30.03), (20.02, 30.05)];
    let sets: [Vec<Stroke>; 5] = [
        vec![stroke(BrushKind::Raise, 300.0, 9.0, None, &line)],
        vec![stroke(BrushKind::Lower, 300.0, 9.0, None, &line)],
        vec![stroke(BrushKind::Flatten, 300.0, 9.0, Some(40.0), &line)],
        vec![stroke(BrushKind::Smooth, 300.0, 9.0, None, &line)],
        vec![
            stroke(BrushKind::Raise, 300.0, 9.0, None, &line),
            stroke(BrushKind::Lower, 200.0, 5.0, None, &line[..2]),
            stroke(BrushKind::Flatten, 250.0, 6.0, Some(-30.0), &line[1..]),
        ],
    ];
    let mut seed = 99u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for (n, strokes) in sets.iter().enumerate() {
        let def = world(&[brush(1, strokes)]);
        let stage = StampStage::new(
            Arc::new(Reference(1)),
            Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>,
            BodyId::PLANET,
            LAYER,
            0,
            1,
        )
        .unwrap();
        let sum: f64 = strokes.iter().map(|s| s.strength_m).sum();
        for k in 0..60 {
            let centre = at_distance(unit(20.01, 30.025), unit(20.5, 30.025 + 0.4 * (rnd() - 0.5)), rnd() * 1_500.0);
            let angle_m = [30.0, 150.0, 600.0][k % 3];
            let (lo, hi) = stage.height_bounds(centre, angle_m / R);
            let (blo, bhi) = Reference(1).height_bounds(centre, angle_m / R);
            assert!(
                lo >= blo - sum - 1e-6 && hi <= bhi + sum + 1e-6,
                "set {n}, cap {k}: [{lo}, {hi}] against the base [{blo}, {bhi}] and {sum}"
            );
            for _ in 0..40 {
                let p = at_distance(centre, unit(20.5, 30.0 + rnd()), rnd() * angle_m);
                let h = stage.height(p);
                let b = Reference(1).height(p);
                assert!(h >= lo && h <= hi, "set {n}, cap {k}: {h} outside [{lo}, {hi}]");
                assert!(
                    (h - b).abs() <= sum + 1e-6,
                    "set {n}: the stage moved the height by {} above the sum of the strengths {sum}",
                    (h - b).abs()
                );
            }
        }
    }
}

/// A field of one height everywhere: its bounds are exact, so a bound that is a few ulps short shows.
struct Flat(f64);

impl HeightField for Flat {
    fn height(&self, _dir: Vec3) -> f64 {
        self.0
    }
    fn seed(&self) -> u64 {
        0
    }
    fn implementation_id(&self) -> &'static str {
        "flat"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.abs()
    }
    fn height_bounds(&self, _centre: Vec3, _angular_radius: f64) -> (f64, f64) {
        (self.0, self.0)
    }
    fn inputs_hash(&self) -> u64 {
        1
    }
}

// spec: GEN-030
#[test]
fn the_bounds_hold_over_a_base_with_exact_bounds_for_many_strokes_and_a_bound_exactly_the_sum() {
    for strength in [0.1, 0.37, 1e4 / 64.0] {
        let line = [(20.0, 30.0), (20.02, 30.0)];
        let strokes: Vec<Stroke> = (0..64).map(|_| stroke(BrushKind::Raise, 200.0, strength, None, &line)).collect();
        let def = world(&[brush(1, &strokes)]);
        let stage = StampStage::new(
            Arc::new(Flat(1e4)),
            Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>,
            BodyId::PLANET,
            LAYER,
            0,
            1,
        )
        .unwrap();
        let on = unit(20.01, 30.0);
        for angle in [0.0, 1e-9, 50.0 / R] {
            let (lo, hi) = stage.height_bounds(on, angle);
            let h = stage.height(on);
            assert!(h >= lo && h <= hi, "strength {strength}, cap {angle}: {h} outside [{lo}, {hi}]");
        }
    }
}

// spec: GEN-030
#[test]
fn a_zero_delta_leaves_a_negative_zero_alone() {
    let at = unit(20.0, 30.0);
    let flatten = world(&[brush(1, &[stroke(BrushKind::Flatten, 100.0, 3.0, Some(0.0), &[(20.0, 30.0)])])]);
    let items = footprint(&flatten, at, 1e-6);
    assert_eq!(apply_stamps(&items, R, at, -0.0).to_bits(), (-0.0f64).to_bits(), "a flatten already at its target");
    let smooth = world(&[brush(1, &[stroke(BrushKind::Smooth, 100.0, 3.0, None, &[(20.0, 30.0)])])]);
    let items = footprint(&smooth, at, 1e-6);
    assert_eq!(apply_stamps(&items, R, at, -0.0).to_bits(), (-0.0f64).to_bits(), "a smooth over a constant input");
}

// spec: GEN-030
#[test]
fn repeated_smooth_strokes_converge_on_the_mean_and_never_pass_it() {
    let at = unit(20.0, 30.0);
    let peak = cone(at);
    let mean = {
        let one = world(&[brush(1, &[stroke(BrushKind::Smooth, 100.0, 1e6, None, &[(20.0, 30.0)])])]);
        let items = footprint(&one, at, 1e-6);
        apply_stamps_with(&items, R, at, peak(at), &peak)
    };
    // A strength far above the distance moves the point all the way to the ring mean in one pass.
    assert!(mean < peak(at) && mean > 0.0, "the mean of the ring is below the peak: {mean}");
    for n in 1..=6 {
        let strokes: Vec<Stroke> = (0..n).map(|_| stroke(BrushKind::Smooth, 100.0, 7.0, None, &[(20.0, 30.0)])).collect();
        let def = world(&[brush(1, &strokes)]);
        let h = apply_stamps_with(&footprint(&def, at, 1e-6), R, at, peak(at), &peak);
        assert!(h >= mean - 1e-9 && h <= peak(at), "{n} passes: {h} between the mean {mean} and the peak {}", peak(at));
    }
}

// spec: GEN-030
#[test]
fn the_ring_is_finite_and_the_smooth_bounded_next_to_a_pole() {
    let near = unit(89.9999, 10.0);
    let def = {
        let mut d = WorldDefinition::single_planet(R, 1);
        d.put_item_spec(BodyId::PLANET, brush(1, &[stroke(BrushKind::Smooth, 300.0, 5.0, None, &[(89.9999, 10.0)])])).unwrap();
        d
    };
    let peak = cone(near);
    for p in [near, at_distance(near, unit(89.0, 100.0), 150.0), unit(90.0, 0.0)] {
        let items = footprint(&def, p, 1e-6);
        let h = apply_stamps_with(&items, R, p, peak(p), &peak);
        assert!(h.is_finite() && (h - peak(p)).abs() <= 5.0 + 1e-9, "near the pole: {h} against {}", peak(p));
    }
}

// spec: GEN-030
#[test]
fn a_brush_layer_reads_the_procedural_base_so_a_lower_layers_edit_cannot_leave_its_tile_stale() {
    // A hill in a lower layer, a smooth in the layer above it: the brush's own delta (the stage above minus the stage below) does not depend on
    // the hill, so the smooth's reads off the tile (its ring) never touch a lower layer's items, whose reads the tile's key covers only nearby.
    let hill = |amount: f64| {
        ItemSpec {
            id: 9,
            layer: "stamps".into(),
            priority: 0,
            dir: unit(20.0, 30.0),
            content_hash: 0,
            reach_m: 3_000.0,
            max_delta_m: amount.abs(),
            stamp: None,
        }
        .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
    };
    let smooth = brush(1, &[stroke(BrushKind::Smooth, 2_000.0, 8.0, None, &[(20.0, 30.0), (20.01, 30.0)])]);
    let stages = |amount: f64| {
        let def = world(&[hill(amount), smooth.clone()]);
        let reader = Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>;
        let lower = Arc::new(StampStage::new(Arc::new(Reference(1)), Arc::clone(&reader), BodyId::PLANET, "stamps", 0, 1).unwrap());
        let upper = StampStage::new(Arc::clone(&lower) as Arc<dyn HeightField>, reader, BodyId::PLANET, LAYER, 1, 1).unwrap();
        (lower, upper)
    };
    let (low_a, up_a) = stages(10.0);
    let (low_b, up_b) = stages(300.0);
    let mut moved = 0;
    for i in 0..30 {
        let p = at_distance(unit(20.005, 30.0), east_of(unit(20.005, 30.0)), -1_500.0 + 100.0 * f64::from(i));
        let (da, db) = (up_a.height(p) - low_a.height(p), up_b.height(p) - low_b.height(p));
        assert!((da - db).abs() < 1e-6, "point {i}: the brush adds {da} over one hill and {db} over the other");
        moved += usize::from(da.abs() > 1e-9);
    }
    assert!(moved >= 10, "the smooth acts on the procedural base ({moved} points)");
}

// spec: GEN-030, GEN-029
#[test]
fn a_region_of_mode_2_over_the_point_cuts_a_brush_like_every_item_of_its_layer() {
    let line = [(20.0, 30.0), (20.02, 30.0)];
    let on = unit(20.01, 30.0);
    let square: Vec<Vec3> =
        [(20.005, 29.995), (20.005, 30.005), (20.015, 30.005), (20.015, 29.995)].iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let region = ItemSpec {
        id: 2,
        layer: LAYER.into(),
        priority: 0,
        dir: square[0],
        content_hash: 0,
        reach_m: 3_000.0,
        max_delta_m: 0.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Region, [2.0, 2.0, 0.0]).with_points(&square));
    let raise = brush(1, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line)]);
    let free = world(std::slice::from_ref(&raise));
    let cut = world(&[raise, region]);
    close(height_over(&free, on, 50.0), 58.0, "without the region");
    assert_eq!(height_over(&cut, on, 50.0).to_bits(), 50.0f64.to_bits(), "inside the region the height is the base's");
    // Outside the region the brush still acts.
    let outside = at_distance(unit(20.0, 30.0), unit(19.5, 30.0), 100.0);
    assert!(height_over(&cut, outside, 50.0) > 50.0, "and outside it the raise acts");
}

// spec: GEN-030
#[test]
fn a_brush_over_a_stamp_acts_on_the_stamped_height_and_a_flatten_ends_at_its_target_whatever_the_hill_below() {
    // The stamp stage under the brush stage, as the editor stacks them: a hill in "stamps", a brush in "brush" above it.
    let hill = |amount: f64| {
        ItemSpec {
            id: 9,
            layer: "stamps".into(),
            priority: 0,
            dir: unit(20.01, 30.0),
            content_hash: 0,
            reach_m: 3_000.0,
            max_delta_m: amount.abs(),
            stamp: None,
        }
        .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
    };
    let line = [(20.0, 30.0), (20.02, 30.0)];
    let stack = |hill_m: f64, strokes: &[Stroke]| {
        let def = world(&[hill(hill_m), brush(1, strokes)]);
        let reader = Arc::clone(&def) as Arc<dyn planet_generators::stamp::ItemReader>;
        let lower = Arc::new(StampStage::new(Arc::new(Reference(1)), Arc::clone(&reader), BodyId::PLANET, "stamps", 0, 1).unwrap());
        let upper = StampStage::new(Arc::clone(&lower) as Arc<dyn HeightField>, reader, BodyId::PLANET, LAYER, 1, 1).unwrap();
        (lower, upper)
    };
    let on = unit(20.01, 30.0);
    let beside = at_distance(on, east_of(on), 100.0);
    let far = at_distance(on, east_of(on), 1_500.0);

    // A raise adds its strength on the line and its weight beside it, over the stamped height, whatever the hill.
    for hill_m in [-80.0, 0.0, 40.0, 250.0] {
        let (low, up) = stack(hill_m, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line)]);
        close(up.height(on) - low.height(on), 8.0, "on the line");
        close(up.height(beside) - low.height(beside), 8.0 * 0.5625, "beside the line");
        assert_eq!(up.height(far).to_bits(), low.height(far).to_bits(), "beyond the brush's radius the stack is the stamp's");
    }
    // The stamp is really there under the brush: a hill of 250 m is higher at its middle than one of -80 m by their difference.
    let (low_hi, up_hi) = stack(250.0, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line)]);
    let (low_lo, up_lo) = stack(-80.0, &[stroke(BrushKind::Raise, 200.0, 8.0, None, &line)]);
    close(low_hi.height(on) - low_lo.height(on), 330.0, "the hill's own height difference at its middle");
    close(up_hi.height(on) - up_lo.height(on), 330.0, "kept through the brush, which adds the same to both");

    // A flatten with a strength above the gap lands on its target on the line, from above or from below the hill.
    let target = 1_000.0;
    for hill_m in [-80.0, 250.0] {
        let (low, up) = stack(hill_m, &[stroke(BrushKind::Flatten, 200.0, 10_000.0, Some(target), &line)]);
        close(up.height(on), target, "the flatten reaches its target on the line");
        assert!((low.height(on) - target).abs() > 1.0, "and the stamp stage alone does not ({})", low.height(on));
    }
    // A small strength moves from the stamped height by exactly that much towards the target.
    let (low, up) = stack(250.0, &[stroke(BrushKind::Flatten, 200.0, 3.0, Some(target), &line)]);
    close(up.height(on) - low.height(on), 3.0, "a small flatten steps from the stamped height towards the target");
}

// spec: GEN-030, WDEF-007
#[test]
fn every_height_a_brush_changes_lies_in_a_tile_that_declares_the_brush() {
    // A brush of a long raise stroke, a dab and a smooth stroke far from the first: the footprint of a tile holds the item only where a
    // stroke reaches the tile and its halo, so a sample the stage changes must lie in a tile whose footprint holds the item.
    use planet_core::{TangentWarp, TileId};
    use planet_generators::stamp::{ItemReader, STAGE_HALO};
    let long: Vec<(f64, f64)> = (0..12).map(|k| (20.0, 30.0 + 0.0008 * f64::from(k))).collect();
    let items = [brush(
        1,
        &[
            stroke(BrushKind::Raise, 40.0, 7.0, None, &long),
            stroke(BrushKind::Lower, 60.0, 5.0, None, &[(20.002, 30.004)]),
            stroke(BrushKind::Smooth, 30.0, 3.0, None, &[(19.9985, 30.012), (19.9985, 30.0135)]),
        ],
    )];
    let def = world(&items);
    let stage = StampStage::new(Arc::new(Reference(1)), Arc::clone(&def) as Arc<dyn ItemReader>, BodyId::PLANET, LAYER, 0, 1).unwrap();
    let mut changed = 0;
    for level in [15u8, 17] {
        let mut tiles = std::collections::BTreeSet::new();
        for i in 0..60 {
            for j in 0..60 {
                let p = unit(19.9975 + 0.00009 * f64::from(i), 29.9985 + 0.00032 * f64::from(j));
                tiles.insert(TileId::from_direction(&TangentWarp, p, level));
            }
        }
        for tile in tiles {
            let declared = !def.footprint_items(BodyId::PLANET, LAYER, tile, STAGE_HALO).unwrap().is_empty();
            for l in 0..=8i64 {
                for k in 0..=8i64 {
                    let d = tile.sample_direction_ext(&TangentWarp, 8, k, l);
                    if stage.height(d) != Reference(1).height(d) {
                        changed += 1;
                        assert!(declared, "level {level}: {tile:?} has a changed sample but does not declare the brush");
                    }
                }
            }
        }
    }
    assert!(changed > 100, "the setup must change heights, got {changed}");
}
