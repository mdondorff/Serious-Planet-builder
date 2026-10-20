//! Tier A: the preview of a brush stroke (VIEW-036, REND-033): a stroke is a chain of hill (raise, lower) or plateau (flatten) preview stamps
//! along its line, a smooth is not previewed, a brush item that only gained strokes adds only the new ones, and a transient preview flags the
//! nodes it touches whether or not their tiles are stale.

use planet_core::Vec3;
use planet_frame::preview::{brush_chain, preview_delta, preview_delta_on, PreviewStamp, BRUSH_CHAIN_STAMPS, MAX_PREVIEW_STAMPS};
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

/// The unit direction `m` metres east of (0, 0) on the equator.
fn east(m: f64) -> Vec3 {
    let a = m / R;
    Vec3::new(libm::cos(a), libm::sin(a), 0.0)
}

fn stroke(kind: BrushKind, radius: f64, strength: f64, target: Option<f64>, metres: &[f64]) -> Stroke {
    Stroke { kind, radius_m: radius, strength_m: strength, target_m: target, points: metres.iter().map(|m| east(*m)).collect() }
}

fn brush_spec(id: u128, strokes: &[Stroke]) -> ItemSpec {
    let at = strokes[0].points[0];
    let far = strokes
        .iter()
        .flat_map(|s| s.points.iter().map(|p| R * libm::atan2(at.cross(*p).length(), at.dot(*p)) + s.radius_m))
        .fold(0.0f64, f64::max);
    ItemSpec {
        id,
        layer: "brush".into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m: far + 2.0,
        max_delta_m: strokes.iter().map(|s| s.strength_m).sum(),
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(strokes))
}

fn items(specs: &[ItemSpec]) -> Vec<FootprintItem> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for s in specs {
        d.put_item_spec(BodyId::PLANET, s.clone()).unwrap_or_else(|e| panic!("{e:?}"));
    }
    d.footprint_items_cap(BodyId::PLANET, "brush", Cap { centre: east(0.0), angle: core::f64::consts::PI }).unwrap()
}

fn arc(a: Vec3, b: Vec3) -> f64 {
    R * libm::atan2(a.cross(b).length(), a.dot(b))
}

/// The summed delta of `stamps` at `p` (the hill profile `(1 - x^2)^2`, signed).
fn delta_at(stamps: &[PreviewStamp], p: Vec3) -> f64 {
    stamps
        .iter()
        .filter(|s| s.kind == StampKind::Hill)
        .map(|s| {
            let x = arc(s.dir, p) / s.reach_m;
            if x >= 1.0 {
                0.0
            } else {
                f64::from(s.sign) * s.amount_m * (1.0 - x * x) * (1.0 - x * x)
            }
        })
        .sum()
}

// spec: VIEW-036
#[test]
fn a_raise_is_hills_a_lower_the_same_with_the_sign_turned_a_flatten_plateaus_and_a_smooth_nothing() {
    let one = |kind, target| brush_chain(&stroke(kind, 120.0, 6.0, target, &[0.0]), 1, R);
    let raise = one(BrushKind::Raise, None);
    assert_eq!(raise.len(), 1, "a click is one stamp");
    assert_eq!((raise[0].sign, raise[0].kind, raise[0].amount_m, raise[0].reach_m), (1, StampKind::Hill, 6.0, 120.0), "the whole strength");
    let lower = one(BrushKind::Lower, None);
    assert_eq!((lower[0].sign, lower[0].kind, lower[0].amount_m), (-1, StampKind::Hill, 6.0));
    let flatten = one(BrushKind::Flatten, Some(812.0));
    assert_eq!((flatten[0].sign, flatten[0].kind, flatten[0].amount_m, flatten[0].max_delta_m), (1, StampKind::Plateau, 812.0, 6.0));
    assert!(one(BrushKind::Smooth, None).is_empty(), "a smooth is not previewed: it shows when the exact tiles arrive");
    // Taken away: a raise's stamps turn their sign, a removed flatten is not previewed (as a removed plateau is not).
    let s = stroke(BrushKind::Raise, 120.0, 6.0, None, &[0.0, 300.0]);
    let gone = brush_chain(&s, -1, R);
    assert!(gone.iter().all(|p| p.sign == -1) && gone.len() == brush_chain(&s, 1, R).len());
    assert!(brush_chain(&stroke(BrushKind::Flatten, 120.0, 6.0, Some(1.0), &[0.0, 300.0]), -1, R).is_empty());
    assert!(brush_chain(&Stroke { points: Vec::new(), ..stroke(BrushKind::Raise, 120.0, 6.0, None, &[0.0]) }, 1, R).is_empty());
}

// spec: VIEW-036
#[test]
fn a_chain_adds_up_to_the_strength_on_the_line_and_a_long_stroke_gets_a_wider_spacing_not_more_stamps() {
    let (radius, strength) = (100.0, 5.0);
    let line = stroke(BrushKind::Raise, radius, strength, None, &[0.0, 600.0]);
    let chain = brush_chain(&line, 1, R);
    // A stamp every half radius, both ends included, then a dab of half the strength on each end (the rounded ends).
    assert_eq!(chain.len(), 13 + 2);
    for w in chain[..13].windows(2) {
        assert!((arc(w[0].dir, w[1].dir) - 50.0).abs() < 1e-3);
    }
    assert!(chain[..13].iter().all(|s| s.reach_m > radius), "the chain's bumps reach further than the radius");
    assert_eq!((chain[13].amount_m, chain[13].reach_m, chain[14].amount_m), (0.5 * strength, radius, 0.5 * strength));
    assert!(arc(chain[13].dir, east(0.0)) < 1e-3 && arc(chain[14].dir, east(600.0)) < 1e-3);
    // Along the middle of the line the sum is the strength within a fifth (the ripple of the spacing), its mean within 3 %.
    let samples: Vec<f64> = (0..400).map(|k| delta_at(&chain, east(100.0 + 400.0 * f64::from(k) / 399.0))).collect();
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    assert!((mean - strength).abs() < 0.03 * strength, "mean {mean}");
    assert!(
        samples.iter().all(|v| (v - strength).abs() < 0.2 * strength),
        "{:?}",
        samples.iter().cloned().fold((f64::MAX, f64::MIN), |a, v| (a.0.min(v), a.1.max(v)))
    );
    // Off the line the delta falls to nothing at the radius.
    assert!(delta_at(&chain, east(300.0) + Vec3::new(0.0, 0.0, 1.0) * (0.5 * radius / R)) > 0.0);
    assert!(
        delta_at(&chain, Vec3::new(libm::cos(0.5 / R * 1000.0), 0.0, libm::sin(0.5 / R * 1000.0))).abs() < 1e-9,
        "far from it: nothing"
    );
    // 50 km at a 100 m radius: at most BRUSH_CHAIN_STAMPS with the caps, a wider spacing, the amplitude no larger than the strength.
    let long = brush_chain(&stroke(BrushKind::Raise, radius, strength, None, &[0.0, 25_000.0, 50_000.0]), 1, R);
    assert!(long.len() <= BRUSH_CHAIN_STAMPS, "{}", long.len());
    assert!(long[0].amount_m <= strength + 1e-12 && long[0].amount_m > 0.0);
    assert!(arc(long[0].dir, long[1].dir) > 50.0);
    // The last stamp is the cap at the end of the stroke.
    assert!(arc(long.last().unwrap().dir, east(50_000.0)) < 1.0);
}

// spec: VIEW-036
#[test]
fn a_brush_item_added_is_its_chains_removed_the_same_turned_and_one_that_gained_strokes_adds_only_the_new_ones() {
    let cam = east(0.0);
    let first = stroke(BrushKind::Raise, 100.0, 5.0, None, &[0.0, 400.0]);
    let second = stroke(BrushKind::Lower, 100.0, 3.0, None, &[200.0, 600.0]);
    let one = items(&[brush_spec(1, std::slice::from_ref(&first))]);
    let two = items(&[brush_spec(1, &[first.clone(), second.clone()])]);
    let none: Vec<FootprintItem> = Vec::new();
    // Added, removed.
    let added = preview_delta_on(&none, &one, cam, R);
    assert_eq!(added.stamps, brush_chain(&first, 1, R));
    let removed = preview_delta_on(&one, &none, cam, R);
    assert_eq!(removed.stamps, brush_chain(&first, -1, R));
    // Gained a stroke at the end: only the new stroke's stamps, nothing taken away.
    let gained = preview_delta_on(&one, &two, cam, R);
    assert_eq!(gained.stamps, brush_chain(&second, 1, R));
    assert!(gained.stamps.iter().all(|s| s.sign == -1), "a lower adds its hills turned");
    // The way back (the undo of the join) takes the new stroke away and nothing else.
    let back = preview_delta_on(&two, &one, cam, R);
    assert_eq!(back.stamps, brush_chain(&second, -1, R));
    assert!(back.stamps.iter().all(|s| s.sign == 1), "the lower's hills turned back");
    // Changed otherwise (the first stroke's strength): the old state taken away and the new one added.
    let stronger = items(&[brush_spec(1, &[Stroke { strength_m: 9.0, ..first.clone() }])]);
    let changed = preview_delta_on(&one, &stronger, cam, R);
    assert_eq!(changed.stamps.len(), 2 * brush_chain(&first, 1, R).len());
    // The plain delta (no radius) leaves brushes out, as before.
    assert!(preview_delta(&none, &one, cam).stamps.is_empty());
    // An unchanged item is absent.
    assert!(preview_delta_on(&one, &one, cam, R).stamps.is_empty());
}

// spec: VIEW-036
#[test]
fn brush_chains_beyond_the_cap_keep_whole_items_nearest_the_camera() {
    let cam = east(0.0);
    // Four items of one stroke each, each a chain of 12 stamps (a 450 m stroke at a 100 m radius, the end dabs included): the cap holds two whole.
    let make =
        |k: u128| brush_spec(k + 1, &[stroke(BrushKind::Raise, 100.0, 5.0, None, &[2_000.0 * k as f64, 2_000.0 * k as f64 + 450.0])]);
    let all = items(&(0..4).map(make).collect::<Vec<_>>());
    let list = preview_delta_on(&[], &all, cam, R);
    let per = brush_chain(&stroke(BrushKind::Raise, 100.0, 5.0, None, &[0.0, 450.0]), 1, R).len();
    assert_eq!(per, 12);
    let kept = MAX_PREVIEW_STAMPS / per;
    assert_eq!(list.stamps.len(), kept * per, "{} of {} entries", list.stamps.len(), 4 * per);
    assert_eq!(list.dropped, (4 - kept) * per);
    assert!(list.stamps.len() <= MAX_PREVIEW_STAMPS);
    // The farthest items are the ones left out.
    assert!(list.stamps.iter().all(|s| arc(s.dir, cam) < 3_000.0));
}

// spec: VIEW-036
#[test]
fn a_transient_stamp_flags_every_node_it_touches_stale_or_exact_and_nothing_else_changes() {
    use planet_core::cube::TangentWarp;
    use planet_frame::pipeline::{draw_plan_edited, draw_plan_transient, ResidentTile};
    use planet_frame::TileBounds;
    use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
    use planet_generators::height::Reference;
    use std::collections::{BTreeSet, HashMap};

    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[6];
    let req = TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    };
    let plan = plan_terrain_field(&req, &Reference(1)).unwrap();
    let near = {
        let by = |n: &&planet_frame::TerrainDraw| n.tile.center_direction(&TangentWarp).dot(camera.position.0.normalized());
        let mut v: Vec<&planet_frame::TerrainDraw> = plan.nodes.iter().collect();
        v.sort_by(|a, b| by(b).total_cmp(&by(a)));
        v[0].tile
    };
    let stamp = PreviewStamp {
        sign: 1,
        kind: StampKind::Hill,
        amount_m: 30.0,
        dir: near.center_direction(&TangentWarp),
        reach_m: 300.0,
        max_delta_m: 60.0,
    };
    // Every tile is resident with the plan's key: the frame is exact.
    let exact: HashMap<_, _> = plan.nodes.iter().map(|n| (n.tile, ResidentTile { mesh_key: n.mesh_key, origin: n.origin })).collect();
    let bounds = TileBounds::default();
    let base = Some((&req, &plan));
    let none = BTreeSet::new();
    let plain = draw_plan_edited(&req, base, &exact, &bounds, &[], &none).unwrap();
    assert!(plain.plan.nodes.iter().all(|n| !n.preview));
    // An empty transient list is the same draw plan.
    assert_eq!(draw_plan_transient(&req, base, &exact, &bounds, &[], &none, &[]).unwrap(), plain);
    // With a transient stamp the nodes under it are flagged although every tile is exact, and the frame is still exact.
    let d = draw_plan_transient(&req, base, &exact, &bounds, &[], &none, &[stamp]).unwrap();
    let flagged: Vec<_> = d.plan.nodes.iter().filter(|n| n.preview).map(|n| n.tile).collect();
    assert!(flagged.contains(&near) && flagged.len() < d.plan.nodes.len(), "{} of {}", flagged.len(), d.plan.nodes.len());
    assert_eq!((d.kind, d.substituted, d.blank), (plain.kind, 0, 0));
    assert_eq!(d.preview, vec![stamp]);
    // The delta's stamps come first, the transient ones after, and the transient ones are kept when the cap is reached.
    let delta: Vec<PreviewStamp> = (0..MAX_PREVIEW_STAMPS).map(|k| PreviewStamp { amount_m: k as f64, sign: -1, ..stamp }).collect();
    let d = draw_plan_transient(&req, base, &exact, &bounds, &delta, &none, &[stamp]).unwrap();
    assert_eq!(d.preview.len(), MAX_PREVIEW_STAMPS);
    assert_eq!(*d.preview.last().unwrap(), stamp);
    assert_eq!(d.preview[0].amount_m, 0.0);
}

// spec: VIEW-036
#[test]
fn the_chain_of_a_raise_is_close_to_the_stage_s_delta_along_and_across_the_stroke() {
    use planet_generators::height::{HeightField, Reference};
    use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
    use std::sync::Arc;

    // A bent stroke of four segments, radius 300 m, strength 40 m.
    let metres = [(0.0, 0.0), (400.0, 0.0), (800.0, 250.0), (1_000.0, 650.0), (1_000.0, 1_000.0)];
    let at = |e: f64, n: f64| -> Vec3 {
        let p = east(e);
        let north = Vec3::new(0.0, 0.0, 1.0);
        (p + north * (n / R)).normalized()
    };
    let strokes = Stroke {
        kind: BrushKind::Raise,
        radius_m: 300.0,
        strength_m: 40.0,
        target_m: None,
        points: metres.iter().map(|(e, n)| at(*e, *n)).collect(),
    };
    let spec = brush_spec(1, std::slice::from_ref(&strokes));
    let mut def = WorldDefinition::single_planet(R, 1);
    def.put_item_spec(BodyId::PLANET, spec).unwrap();
    let base: Arc<dyn HeightField> = Arc::new(Reference(1));
    let stage = StampStage::new(Arc::clone(&base), Arc::new(def), BodyId::PLANET, "brush", 0, STAMP_STAGE_VERSION).unwrap();
    let chain = brush_chain(&strokes, 1, R);
    let (mut worst, mut sum_abs, mut n, mut sum_exact) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..60 {
        for j in 0..60 {
            let p = at(-400.0 + 1_800.0 * f64::from(i) / 59.0, -400.0 + 1_800.0 * f64::from(j) / 59.0);
            let exact = stage.height(p) - base.height(p);
            let preview = delta_at(&chain, p);
            if exact > 0.0 || preview > 0.5 {
                worst = worst.max((exact - preview).abs());
                sum_abs += (exact - preview).abs();
                sum_exact += exact.abs();
                n += 1.0;
            }
        }
    }
    println!(
        "FIDELITY chain of {}: worst {worst:.2} m, mean abs error {:.2} m over {n} points, mean exact {:.2} m",
        chain.len(),
        sum_abs / n,
        sum_exact / n
    );
    assert!(worst <= 0.35 * strokes.strength_m, "worst error {worst}");
    assert!(sum_abs / n <= 0.1 * strokes.strength_m, "mean error {}", sum_abs / n);
}
