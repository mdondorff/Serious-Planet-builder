//! LOD-012: `HeightField::bounds_monotone` — a field that says yes gives bounds that are monotone in the cap: the bounds of a cap inside another lie
//! inside the bounds of the other (up to `MONOTONE_SLACK_M`). Random nested caps, every field that says yes.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::global::{bake, GridSource};
use planet_generators::height::{HeightField, Reference, MONOTONE_SLACK_M};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::stamp::StampStage;
use planet_world_def::footprint::angle_between;
use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
use planet_world_def::{BodyId, GenContext, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

/// The point at angle `r` from `centre` on the bearing `t`.
fn around(centre: Vec3, t: f64, r: f64) -> Vec3 {
    let a = if centre.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = (a - centre * centre.dot(a)).normalized();
    let e2 = centre.cross(e1);
    (centre * libm::cos(r) + (e1 * libm::cos(t) + e2 * libm::sin(t)) * libm::sin(r)).normalized()
}

fn eroded(kind: HeightFieldKind) -> Box<dyn HeightField> {
    let grid = bake(7, &TangentWarp, 128, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let src = Box::leak(Box::new(GridSource(Box::leak(Box::new(grid)))));
    build_field(kind, &GenContext::new(&def), BodyId::PLANET, src, [1; 16]).unwrap()
}

fn item(id: u128, priority: i32, at: Vec3, reach_m: f64, max_delta_m: f64, stamp: Stamp) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority, dir: at, content_hash: 0, reach_m, max_delta_m, stamp: None }
        .with_stamp(stamp)
}

/// The worst excess of an inner cap's bounds over its outer cap's, over `n` random nested pairs near `anchor` (radii from 1e-7 to 0.05 rad).
fn worst_excess(f: &dyn HeightField, anchor: Vec3, seed: u64, n: usize) -> (f64, String) {
    let mut rng = SplitMix64(seed);
    let (mut worst, mut at) = (0.0f64, String::new());
    for _ in 0..n {
        let theta = libm::pow(10.0, -7.0 + 5.7 * rng.next_f64());
        let spread = theta * 50.0 * rng.next_f64();
        let c = around(anchor, rng.next_f64() * std::f64::consts::TAU, spread);
        let inner = theta * rng.next_f64();
        let d = (theta - inner) * rng.next_f64();
        let c2 = around(c, rng.next_f64() * std::f64::consts::TAU, d);
        let (olo, ohi) = f.height_bounds(c, theta);
        let (ilo, ihi) = f.height_bounds(c2, inner);
        let ex = (olo - ilo).max(ihi - ohi);
        if ex > worst {
            worst = ex;
            at = format!("centre {c:?} theta {theta:e}, inner {c2:?} {inner:e}: outer [{olo}, {ohi}] inner [{ilo}, {ihi}]");
        }
    }
    (worst, at)
}

fn stage_over(base: Arc<dyn HeightField>, items: Vec<ItemSpec>) -> StampStage<'static> {
    let mut def = WorldDefinition::single_planet(R, 7);
    for i in items {
        def.put_item_spec(BodyId::PLANET, i).unwrap();
    }
    StampStage::new(base, Arc::new(def), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).unwrap()
}

fn items_near(at: Vec3) -> Vec<ItemSpec> {
    let off = |d: f64, h: Vec3| {
        let t = (h - at * at.dot(h)).normalized();
        (at * libm::cos(d / R) + t * libm::sin(d / R)).normalized()
    };
    let line = |id: u128, kind: StampKind, amount: f64, width: f64, pts: [Vec3; 2]| {
        let far = angle_between(pts[0], pts[1]) * R;
        ItemSpec {
            id,
            layer: DEFAULT_ITEM_LAYER.into(),
            priority: 3,
            dir: pts[0],
            content_hash: 0,
            reach_m: far + width,
            max_delta_m: amount.abs(),
            stamp: None,
        }
        .with_stamp(Stamp::new(kind, [amount, width, 0.0]).with_points(&pts))
    };
    let stroke = |kind: BrushKind, radius_m: f64, strength_m: f64, target_m: Option<f64>, d: f64| Stroke {
        kind,
        radius_m,
        strength_m,
        target_m,
        points: vec![off(d, dir(0.0, 1.0, 0.0)), off(d + 300.0, dir(0.0, 1.0, 0.0))],
    };
    let strokes = [
        stroke(BrushKind::Raise, 200.0, 30.0, None, -500.0),
        stroke(BrushKind::Smooth, 250.0, 20.0, None, -400.0),
        stroke(BrushKind::Flatten, 150.0, 25.0, Some(40.0), -300.0),
        stroke(BrushKind::Lower, 180.0, 15.0, None, -200.0),
    ];
    let brush = ItemSpec {
        id: 9,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 4,
        dir: strokes[0].points[0],
        content_hash: 0,
        reach_m: 1_500.0,
        max_delta_m: 90.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&strokes));
    vec![
        line(5, StampKind::Ridge, 40.0, 400.0, [off(-600.0, dir(1.0, 0.0, 0.0)), off(900.0, dir(0.0, 1.0, 1.0))]),
        line(6, StampKind::Channel, 25.0, 300.0, [off(-300.0, dir(0.0, 1.0, 0.0)), off(700.0, dir(0.0, 0.0, 1.0))]),
        brush,
        item(1, 0, at, 3_000.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0])),
        item(2, 1, off(800.0, dir(0.0, 1.0, 0.0)), 1_500.0, 60.0, Stamp::new(StampKind::Crater, [25.0, 0.0, 0.0])),
        item(3, 2, off(1_000.0, dir(0.0, 0.0, 1.0)), 2_000.0, 200.0, Stamp::new(StampKind::Plateau, [90.0, 0.0, 0.0])),
    ]
}

// spec: LOD-012
#[test]
fn every_field_that_says_its_bounds_are_monotone_gives_nested_bounds_for_nested_caps() {
    let at = dir(1.0, 0.2, 0.1);
    let reference: Arc<dyn HeightField> = Arc::new(Reference(1));
    let er: Arc<dyn HeightField> = Arc::from(eroded(HeightFieldKind::Eroded));
    let te: Arc<dyn HeightField> = Arc::from(eroded(HeightFieldKind::Terrain));
    let st_ref = stage_over(Arc::clone(&reference), items_near(at));
    let st_er = stage_over(Arc::clone(&er), items_near(at));
    let st_te = stage_over(Arc::clone(&te), items_near(at));
    for (name, f) in [
        ("reference", reference.as_ref() as &dyn HeightField),
        ("eroded", er.as_ref()),
        ("terrain", te.as_ref()),
        ("stamp over reference", &st_ref),
        ("stamp over eroded", &st_er),
        ("stamp over terrain", &st_te),
    ] {
        assert!(f.bounds_monotone(), "{name} says its bounds are monotone");
        for seed in [5, 6] {
            let (w, s) = worst_excess(f, at, seed, 6000);
            assert!(w <= MONOTONE_SLACK_M, "{name}: an inner cap exceeds its outer cap's bounds by {w:e} m: {s}");
        }
    }
}

// spec: LOD-012
#[test]
fn a_field_that_does_not_say_so_is_not_monotone_and_a_stage_over_one_is_not_either() {
    struct Plain;
    impl HeightField for Plain {
        fn height(&self, _: Vec3) -> f64 {
            0.0
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "plain"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            1.0
        }
    }
    assert!(!Plain.bounds_monotone(), "the default is no");
    let stage = stage_over(Arc::new(Plain), items_near(dir(1.0, 0.2, 0.1)));
    assert!(!stage.bounds_monotone(), "a stage says yes only over a base that does");
}
