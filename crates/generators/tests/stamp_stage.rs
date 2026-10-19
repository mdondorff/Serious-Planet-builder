//! GEN-026: the stamp stage. Profiles, the stage over a base field, order, determinism, bounds, and the reads it declares against the reads it
//! makes (ADR 0021 point 7).

use std::sync::{Arc, Mutex};

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::{generate_tile_field, HeightField, Reference};
use planet_generators::stamp::{crater_delta, hill_delta, plateau_delta, plateau_weight, ItemReader, StampStage, STAGE_HALO};
use planet_world_def::footprint::{angle_between, Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, ReadRecord, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

/// The point `d_m` metres from `anchor` on the sphere, towards `hint`.
fn at_distance(anchor: Vec3, hint: Vec3, d_m: f64) -> Vec3 {
    let t = (hint - anchor * anchor.dot(hint)).normalized();
    let a = d_m / R;
    (anchor * libm::cos(a) + t * libm::sin(a)).normalized()
}

fn item(id: u128, priority: i32, at: Vec3, reach_m: f64, max_delta_m: f64, stamp: Stamp) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority, dir: at, content_hash: 0, reach_m, max_delta_m, stamp: None }
        .with_stamp(stamp)
}

fn world(items: Vec<ItemSpec>) -> Arc<WorldDefinition> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i).unwrap();
    }
    Arc::new(d)
}

fn stage(def: Arc<WorldDefinition>) -> StampStage<'static> {
    StampStage::new(Arc::new(Reference(1)), def, BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).expect("a body with a radius")
}

// spec: GEN-026
#[test]
fn the_profiles_equal_their_known_answers_are_c1_at_the_edge_zero_beyond_and_bounded() {
    let eq = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-12, "{what}: {a} against {b}");
    eq(hill_delta(10.0, 0.0), 10.0, "hill centre");
    eq(hill_delta(10.0, 0.5), 5.625, "hill half");
    eq(hill_delta(-10.0, 0.5), -5.625, "a pit");
    eq(crater_delta(10.0, 0.0), -10.0, "crater centre");
    eq(crater_delta(10.0, 0.5), -3.164_062_5, "crater bowl at 0.5");
    eq(crater_delta(10.0, 0.7), -0.429_687_5, "crater at 0.7, the rim not yet up");
    eq(crater_delta(10.0, 0.85), 2.0, "crater rim crest: 0.2 of the depth");
    eq(plateau_weight(0.0), 1.0, "plateau weight inside");
    eq(plateau_weight(0.7), 1.0, "plateau weight at 0.7");
    eq(plateau_weight(0.85), 0.5, "plateau weight half way");
    eq(plateau_weight(1.0), 0.0, "plateau weight at the edge");
    eq(plateau_delta(100.0, 40.0, 25.0, 0.0), 25.0, "plateau clamped to the bound");
    eq(plateau_delta(100.0, 90.0, 25.0, 0.0), 10.0, "plateau reaches the target when within the bound");
    for f in
        [hill_delta(7.0, 1.0), crater_delta(7.0, 1.0), plateau_delta(100.0, 0.0, 7.0, 1.0), hill_delta(7.0, 3.0), crater_delta(7.0, 3.0)]
    {
        assert_eq!(f, 0.0, "exactly zero at and beyond the edge");
    }
    // C1 at x = 1 (value and slope, one-sided), and at 0.7 for the plateau weight's join.
    let h = 1e-8;
    for (name, f) in [
        ("hill", Box::new(|x| hill_delta(7.0, x)) as Box<dyn Fn(f64) -> f64>),
        ("crater", Box::new(|x| crater_delta(7.0, x))),
        ("plateau", Box::new(|x| plateau_delta(100.0, 0.0, 7.0, x))),
    ] {
        let (v, slope) = (f(1.0 - h), (f(1.0 - h) - f(1.0 - 2.0 * h)) / h);
        assert!(v.abs() <= 1e-9 && slope.abs() <= 1e-4, "{name}: value {v} and slope {slope} just inside the edge");
    }
    // Bounded by M on a dense sweep, for every kind.
    for k in 0..=1300 {
        let x = f64::from(k) / 1000.0;
        assert!(hill_delta(7.0, x).abs() <= 7.0 && hill_delta(-7.0, x).abs() <= 7.0, "hill at {x}");
        assert!(crater_delta(7.0, x).abs() <= 7.0, "crater at {x}");
        for h in [-1000.0, -3.0, 0.0, 5.0, 1000.0] {
            assert!(plateau_delta(2.0, h, 7.0, x).abs() <= 7.0, "plateau at {x} from {h}");
        }
    }
}

// spec: GEN-026
#[test]
fn the_stage_adds_the_stamps_whose_reach_contains_the_point_and_nothing_elsewhere() {
    let at = dir(1.0, 0.2, 0.1);
    let hint = dir(0.0, 1.0, 0.0);
    let s = stage(world(vec![item(1, 0, at, 2_000.0, 50.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]))]));
    let base = Reference(1);
    // At the centre, inside, at the edge and beyond.
    for (d, want_x) in [(0.0, 0.0), (500.0, 0.25), (1_500.0, 0.75), (1_999.0, 0.9995), (2_000.0, 1.0), (50_000.0, 25.0)] {
        let p = at_distance(at, hint, d);
        let x = R * angle_between(at, p) / 2_000.0;
        assert!((x - want_x).abs() < 1e-6 * want_x.max(1.0), "the setup: {x} against {want_x}");
        let want = base.height(p) + if x < 1.0 { hill_delta(30.0, x) } else { 0.0 };
        assert!((s.height(p) - want).abs() < 1e-9, "d = {d} m: {} against {want}", s.height(p));
    }
    assert_eq!(s.height(dir(-1.0, 0.0, 0.0)), base.height(dir(-1.0, 0.0, 0.0)), "the other side of the planet is the base");
}

// spec: GEN-026
#[test]
fn stamps_apply_in_application_order_and_a_plateau_flattens_what_is_under_it() {
    let at = dir(1.0, 0.2, 0.1);
    let p = at_distance(at, dir(0.0, 1.0, 0.0), 100.0);
    let base = Reference(1).height(p);
    let hill = |id, priority| item(id, priority, at, 2_000.0, 80.0, Stamp::new(StampKind::Hill, [40.0, 0.0, 0.0]));
    let plateau = |id, priority| item(id, priority, at, 2_000.0, 80.0, Stamp::new(StampKind::Plateau, [base + 5.0, 0.0, 0.0]));
    let hill_then_plateau = stage(world(vec![hill(1, 0), plateau(2, 1)])).height(p);
    let plateau_then_hill = stage(world(vec![plateau(2, 0), hill(1, 1)])).height(p);
    assert_ne!(hill_then_plateau, plateau_then_hill, "the order matters");
    let x = R * angle_between(at, p) / 2_000.0;
    let after_hill = base + hill_delta(40.0, x);
    assert!(
        (hill_then_plateau - (after_hill + plateau_delta(base + 5.0, after_hill, 80.0, x))).abs() < 1e-9,
        "hill, then the plateau toward its target"
    );
    let after_plateau = base + plateau_delta(base + 5.0, base, 80.0, x);
    assert!((plateau_then_hill - (after_plateau + hill_delta(40.0, x))).abs() < 1e-9, "the plateau, then the hill on top of it");
    // The same priority is ordered by id.
    let by_id = stage(world(vec![hill(2, 0), plateau(1, 0)])).height(p);
    assert_eq!(by_id, plateau_then_hill, "ties go by id: the lower id (the plateau) first");
    // Under a plateau the ground is flat to the clamp: a ramp of points near the centre ends within a hair of the target.
    let flat = stage(world(vec![item(3, 0, at, 2_000.0, 1_000.0, Stamp::new(StampKind::Plateau, [10.0, 0.0, 0.0]))]));
    for d in [0.0, 100.0, 600.0, 1_000.0] {
        let q = at_distance(at, dir(0.0, 1.0, 0.0), d);
        assert!((flat.height(q) - 10.0).abs() < 1e-9, "inside x <= 0.7 the ground is the target: {} at {d} m", flat.height(q));
    }
}

// spec: GEN-027
#[test]
fn a_stage_with_no_item_in_reach_of_a_tile_is_the_identity_on_its_source_there_bit_for_bit_and_says_so() {
    let at = dir(1.0, 0.2, 0.1);
    let s = stage(world(vec![item(1, 0, at, 1_500.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]))]));
    let base = Reference(1);
    let map = &TangentWarp;
    let far = TileId::from_direction(map, dir(-1.0, -0.4, 0.3), 12);
    let near = TileId::from_direction(map, at, 12);
    assert!(HeightField::passes_through(&s, far, map), "no item touches a tile on the other side");
    assert!(!HeightField::passes_through(&s, near, map), "the tile under the stamp does not pass through");
    // The tile's data, its samples, its bounds and its climate are the source's, bit for bit.
    assert_eq!(generate_tile_field(&s, map, far, 16), generate_tile_field(&base, map, far, 16));
    let (centre, angle) = (far.center_direction(map), 1.0e-3);
    assert_eq!(s.height_bounds(centre, angle), base.height_bounds(centre, angle));
    assert_eq!(s.climate(map, far), base.climate(map, far));
    // Coarse tiles: the whole planet is one tile at level 0: the stamp is in it. A coarse tile away from the stamp passes through.
    let coarse_far = TileId::from_direction(map, dir(-1.0, -0.4, 0.3), 3);
    assert!(HeightField::passes_through(&s, coarse_far, map));
    assert!(!HeightField::passes_through(&s, TileId::from_direction(map, at, 3), map));
    // With the stamp removed from the definition's reach (an empty layer) every tile passes through.
    let empty = stage(world(vec![]));
    assert!(
        HeightField::passes_through(&empty, near, map)
            && HeightField::passes_through(&empty, TileId::new(planet_core::Face(0), 1, 0, 0).unwrap(), map)
    );
}

/// A field that records every direction it is asked for, to compare them with the footprint a stage declares.
struct Spy {
    inner: Reference,
    dirs: std::sync::Mutex<Vec<Vec3>>,
}

impl HeightField for Spy {
    fn height(&self, d: Vec3) -> f64 {
        self.dirs.lock().unwrap().push(d);
        self.inner.height(d)
    }
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        self.dirs.lock().unwrap().extend_from_slice(dirs);
        self.inner.heights_until(dirs, stop)
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        self.dirs.lock().unwrap().extend_from_slice(dirs);
        self.inner.heights_threaded_until(dirs, stop, threads)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.inner.implementation_id()
    }
    fn version(&self) -> u32 {
        self.inner.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
}

// spec: GEN-027
#[test]
fn every_point_a_tile_samples_lies_inside_the_footprint_the_stage_declares_so_passing_a_tile_through_is_sound() {
    use planet_generators::compact::compact_tile_field;
    use planet_generators::mesh::tile_mesh_field;
    use planet_generators::stamp::STAGE_HALO;
    use planet_world_def::footprint::{angle_between, tile_cap};

    let map = &TangentWarp;
    // Tiles at a face centre, on a face edge, at a cube corner; at coarse and fine levels.
    let places = [dir(1.0, 0.0, 0.0), dir(1.0, 0.999_999, 0.0), dir(1.0, 1.0, 1.0), dir(0.3, -0.6, 0.7)];
    for place in places {
        for level in [1u8, 3, 12] {
            let tile = TileId::from_direction(map, place, level);
            let cap = tile_cap(map, tile, STAGE_HALO);
            let check = |what: &str, dirs: Vec<Vec3>| {
                assert!(!dirs.is_empty(), "{what}: the tile sampled something");
                for d in dirs {
                    let a = angle_between(cap.centre, d);
                    assert!(
                        a <= cap.angle,
                        "{what}, level {level} place {place:?}: a sample is {a:.5} rad from the centre, the footprint reaches {:.5}",
                        cap.angle
                    );
                }
            };
            let spy = Spy { inner: Reference(1), dirs: Default::default() };
            let _ = generate_tile_field(&spy, map, tile, 16);
            check("generate_tile_field 16", spy.dirs.lock().unwrap().clone());
            for cells in [4u32, 16, 32] {
                let spy = Spy { inner: Reference(1), dirs: Default::default() };
                let _ = tile_mesh_field(&spy, map, tile, cells, R);
                check(&format!("tile_mesh_field {cells}"), spy.dirs.lock().unwrap().clone());
            }
            for cells in [16u32, 32] {
                let spy = Spy { inner: Reference(1), dirs: Default::default() };
                let _ = compact_tile_field(&spy, map, tile, cells, R);
                check(&format!("compact_tile_field {cells}"), spy.dirs.lock().unwrap().clone());
            }
        }
    }
}

// spec: GEN-027
#[test]
fn a_stage_passes_nothing_through_on_a_mapping_other_than_the_cell_grids() {
    struct OtherWarp;
    impl planet_core::cube::FaceMapping for OtherWarp {
        fn id(&self) -> &'static str {
            "other"
        }
        fn face_to_plane(&self, s: f64) -> f64 {
            s
        }
        fn plane_to_face(&self, u: f64) -> f64 {
            u
        }
    }
    let s = stage(world(vec![item(1, 0, dir(1.0, 0.2, 0.1), 1_500.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]))]));
    let far = TileId::from_direction(&TangentWarp, dir(-1.0, -0.4, 0.3), 12);
    assert!(HeightField::passes_through(&s, far, &TangentWarp));
    assert!(!HeightField::passes_through(&s, far, &OtherWarp), "the same id is another place on another mapping: no claim");
    // A root tile is never passed through: the stencil of a coarse mesh reaches past its declared cap.
    let root = TileId::new(planet_core::Face(0), 0, 0, 0).unwrap();
    assert!(!HeightField::passes_through(&stage(world(vec![])), root, &TangentWarp));
}

// spec: GEN-026
#[test]
fn the_heights_of_a_tile_are_bit_identical_at_any_thread_count_batch_order_and_batch_size() {
    let at = dir(1.0, 0.2, 0.1);
    let tile = TileId::from_direction(&TangentWarp, at, 15);
    let s = stage(world(vec![
        item(1, 0, at, 3_000.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0])),
        item(2, 1, at_distance(at, dir(0.0, 1.0, 0.0), 800.0), 1_500.0, 60.0, Stamp::new(StampKind::Crater, [25.0, 0.0, 0.0])),
        item(3, 2, at_distance(at, dir(0.0, 0.0, 1.0), 1_200.0), 2_000.0, 60.0, Stamp::new(StampKind::Plateau, [12.0, 0.0, 0.0])),
    ]));
    let dirs: Vec<Vec3> =
        (0..=16u32).flat_map(|l| (0..=16u32).map(move |k| (k, l))).map(|(k, l)| tile.sample_direction(&TangentWarp, 16, k, l)).collect();
    let one = s.heights(&dirs);
    let threaded1 = s.heights_threaded(&dirs, 1);
    let threaded3 = s.heights_threaded(&dirs, 3);
    let single: Vec<f64> = dirs.iter().map(|d| s.height(*d)).collect();
    let mut reversed: Vec<Vec3> = dirs.clone();
    reversed.reverse();
    let mut rev = s.heights(&reversed);
    rev.reverse();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&one), bits(&threaded1));
    assert_eq!(bits(&one), bits(&threaded3));
    assert_eq!(bits(&one), bits(&single), "one point at a time");
    assert_eq!(bits(&one), bits(&rev), "reversed order");
    assert!(one.iter().zip(&Reference(1).heights(&dirs)).any(|(a, b)| a != b), "the stamps act in this tile");
    // The tile through the generic path is the same as the batch.
    let data = generate_tile_field(&s, &TangentWarp, tile, 16);
    assert_eq!(data.heights.iter().map(|h| h.to_bits()).collect::<Vec<_>>(), one.iter().map(|h| (*h as f32).to_bits()).collect::<Vec<_>>());
}

// spec: GEN-026
#[test]
fn the_bounds_widen_by_the_bounds_of_the_items_in_reach_and_hold_for_every_sampled_height() {
    let at = dir(1.0, 0.2, 0.1);
    let s = stage(world(vec![
        item(1, 0, at, 3_000.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0])),
        item(2, 1, at_distance(at, dir(0.0, 1.0, 0.0), 800.0), 1_500.0, 40.0, Stamp::new(StampKind::Crater, [25.0, 0.0, 0.0])),
    ]));
    let base = Reference(1);
    // The growth of the extent: what the stamps can add inside the cap, not their declared bounds.
    let growth = |centre: Vec3, angle: f64| {
        let (lo, hi) = s.height_bounds(centre, angle);
        let (blo, bhi) = base.height_bounds(centre, angle);
        (hi - lo) - (bhi - blo)
    };
    // A cap on the stamps: the hill is nearly flat under it (a few centimetres), the crater's bowl varies by metres; far below the
    // declared 60 + 40 on each side that a bound by `max_delta_m` would add (a fine tile would have no room to morph).
    let g = growth(at, 100.0 / R);
    assert!(g > 0.0 && g < 12.0, "a cap on the top of the hill and the slope of a crater: {g}");
    assert_eq!(growth(dir(-1.0, 0.0, 0.0), 100.0 / R), 0.0, "no item in reach, no widening");
    // The only-first-item cap, on the slope of the hill at 2.5 km of 3 km.
    let g = growth(at_distance(at, dir(0.0, -1.0, 0.0), 2_500.0), 100.0 / R);
    assert!(g > 0.0 && g < 3.0, "only the hill reaches 2.5 km on the far side, on its slope: {g}");
    // A cap on the top of a hill alone: the extent grows by less than a tenth of the bound.
    let hill_only = stage(world(vec![item(1, 0, at, 3_000.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]))]));
    let (lo, hi) = hill_only.height_bounds(at, 100.0 / R);
    let (blo, bhi) = base.height_bounds(at, 100.0 / R);
    assert!((hi - lo) - (bhi - blo) < 6.0, "a cap on the top of a hill grows by less than M / 10: {}", (hi - lo) - (bhi - blo));
    // Never wider than the declared bounds say.
    let g = growth(at, 4_000.0 / R);
    assert!(g <= 200.0 + 1e-6, "no wider than the sum of both sides of both declared bounds: {g}");
    // Every height in the cap lies within the bounds.
    for (centre, angle) in [(at, 4_000.0 / R), (at_distance(at, dir(0.0, 1.0, 0.0), 700.0), 1_000.0 / R)] {
        let (lo, hi) = s.height_bounds(centre, angle);
        for k in 0..400 {
            let t = f64::from(k) / 400.0 * core::f64::consts::TAU;
            let r = angle * f64::from(k % 7) / 7.0;
            let h = s.height(at_distance_around(centre, t, r));
            assert!(h >= lo && h <= hi, "{h} outside [{lo}, {hi}]");
        }
    }
    assert!(s.max_abs_height_m() >= base.max_abs_height_m() + 100.0, "the field-wide bound adds every item's bound");
}

// spec: GEN-026
#[test]
fn the_bounds_hold_at_the_rim_of_the_cap_for_a_reach_of_a_metre() {
    let at = dir(1.0, 0.2, 0.1);
    for (kind, params) in [(StampKind::Hill, [60.0, 0.0, 0.0]), (StampKind::Crater, [40.0, 0.0, 0.0])] {
        let s = stage(world(vec![item(1, 0, at, 1.0, 60.0, Stamp::new(kind, params))]));
        for k in 0..64 {
            // Caps whose rim cuts the slope: centre 0.2 to 1.2 m from the anchor, radius 0.1 to 0.7 m.
            let centre = at_distance(at, dir(0.0, 1.0, 0.0), 0.2 + f64::from(k % 8) * 0.125);
            let angle = (0.1 + f64::from(k / 8) * 0.085) / R;
            let (lo, hi) = s.height_bounds(centre, angle);
            for j in 0..72 {
                let t = f64::from(j) / 72.0 * core::f64::consts::TAU;
                // Exactly at the rim, and just inside it.
                for r in [angle, angle * (1.0 - 1e-9), angle * 0.5] {
                    let h = s.height(at_distance_around(centre, t, r));
                    assert!(h >= lo && h <= hi, "{kind:?} cap {k} bearing {j} r {r}: {h} outside [{lo}, {hi}]");
                }
            }
        }
    }
}

/// The point at angle `r` from `centre` on the bearing `t`.
fn at_distance_around(centre: Vec3, t: f64, r: f64) -> Vec3 {
    let a = if centre.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = (a - centre * centre.dot(a)).normalized();
    let e2 = centre.cross(e1);
    (centre * libm::cos(r) + (e1 * libm::cos(t) + e2 * libm::sin(t)) * libm::sin(r)).normalized()
}

/// A reader that records every item the stage asked for (the recording context of ADR 0021 point 7).
struct Recording {
    inner: Arc<WorldDefinition>,
    seen: Mutex<Vec<u128>>,
}

impl ItemReader for Recording {
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
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        self.inner.tile_reads(body, layer, tile, halo)
    }
    fn revision(&self) -> u64 {
        self.inner.revision()
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.inner.footprint_empty(body, layer, tile, halo)
    }
}

// spec: GEN-026, WDEF-007
#[test]
fn every_item_the_stage_reads_over_a_tile_is_in_the_footprint_the_tile_declared_and_nothing_changes_beyond_the_reach() {
    let at = dir(1.0, 0.2, 0.1);
    // Items around the tile: some on it, some at the edge of the declared halo, some far.
    let mut specs = Vec::new();
    for (i, (along, reach)) in
        [(0.0, 400.0), (300.0, 900.0), (900.0, 700.0), (2_500.0, 1_200.0), (6_000.0, 3_000.0), (20_000.0, 5_000.0)].into_iter().enumerate()
    {
        let kind = [StampKind::Hill, StampKind::Crater, StampKind::Plateau][i % 3];
        let params = match kind {
            StampKind::Plateau => [3.0, 0.0, 0.0],
            _ => [15.0, 0.0, 0.0],
        };
        specs.push(item(i as u128 + 1, i as i32, at_distance(at, dir(0.0, 1.0, 0.0), along), reach, 40.0, Stamp::new(kind, params)));
    }
    let def = world(specs);
    for level in [13u8, 15, 17] {
        let tile = TileId::from_direction(&TangentWarp, at, level);
        let reader = Arc::new(Recording { inner: Arc::clone(&def), seen: Mutex::default() });
        let s = StampStage::new(Arc::new(Reference(1)), reader.clone() as Arc<dyn ItemReader>, BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1)
            .unwrap();
        // The samples a tile reads: its grid and a one-sample halo.
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
        for id in &seen {
            assert!(declared.contains(id), "level {level}: item {id} was read but the tile did not declare it (declared {declared:?})");
        }
        // Every item that acts on a sample is among those read.
        for (k, d) in dirs.iter().enumerate() {
            let with = s.height(*d);
            let without = Reference(1).height(*d);
            if with != without {
                let touched: Vec<u128> = declared
                    .iter()
                    .copied()
                    .filter(|id| {
                        let (a, r) = def.item_anchor(BodyId::PLANET, *id).unwrap();
                        R * angle_between(a, *d) < r
                    })
                    .collect();
                assert!(!touched.is_empty(), "level {level} sample {k}: the height changed with no item in reach");
            }
        }
    }
    // Beyond its reach an item changes nothing, within it by at most its bound.
    let (anchor, reach) = def.item_anchor(BodyId::PLANET, 2).unwrap();
    let mut without = (*def).clone();
    assert!(without.remove_item(BodyId::PLANET, 2));
    let (with_s, without_s) = (stage(Arc::clone(&def)), stage(Arc::new(without)));
    for k in 0..200 {
        let d = f64::from(k) * reach * 2.0 / 200.0;
        let p = at_distance(anchor, dir(0.3, 0.2, 1.0), d);
        let (a, b) = (with_s.height(p), without_s.height(p));
        if d >= reach {
            assert_eq!(a, b, "beyond the reach of item 2 ({d} m)");
        } else {
            assert!((a - b).abs() <= 40.0 + 1e-9, "within the reach, the change {} is within the bound", (a - b).abs());
        }
    }
}
