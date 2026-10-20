//! Tier A: the preview of a brush stroke (VIEW-036, REND-033): a stroke is a chain of hill (raise, lower) or plateau (flatten) preview stamps
//! along its line, a smooth is not previewed, a brush item that only gained strokes adds only the new ones, and a transient preview flags the
//! nodes it touches whether or not their tiles are stale.

use planet_core::Vec3;
use planet_frame::preview::{
    brush_chain, preview_delta, preview_delta_on, PreviewStamp, BRUSH_CHAIN_STAMPS, MAX_DELTA_STAMPS, MAX_PREVIEW_STAMPS,
};
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
    // Each dab makes up what the chain falls short of the strength at its end point.
    for (cap, end) in [(&chain[13], east(0.0)), (&chain[14], east(600.0))] {
        assert_eq!(cap.reach_m, radius);
        let short = strength - delta_at(&chain[..13], end);
        assert!((cap.amount_m - short).abs() < 1e-9 && cap.amount_m > 0.0, "{} against {short}", cap.amount_m);
    }
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
    // Changed otherwise (a stroke's strength): the old state taken away and the new one added, as one unit of the cap.
    let dab = stroke(BrushKind::Raise, 100.0, 5.0, None, &[0.0]);
    let (dab_a, dab_b) =
        (items(&[brush_spec(1, std::slice::from_ref(&dab))]), items(&[brush_spec(1, &[Stroke { strength_m: 9.0, ..dab.clone() }])]));
    let changed = preview_delta_on(&dab_a, &dab_b, cam, R);
    assert_eq!(changed.stamps.iter().map(|s| (s.sign, s.amount_m)).collect::<Vec<_>>(), [(-1, 5.0), (1, 9.0)]);
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
    let kept = MAX_DELTA_STAMPS / per;
    assert_eq!(list.stamps.len(), kept * per, "{} of {} entries", list.stamps.len(), 4 * per);
    assert_eq!(list.dropped, (4 - kept) * per);
    assert!(list.stamps.len() <= MAX_DELTA_STAMPS);
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
    // With a transient stamp the nodes under it are flagged although every tile is exact, and the frame is still exact. An exact tile
    // already holds the edit in flight, so the node takes only the transient stamps.
    let d = draw_plan_transient(&req, base, &exact, &bounds, &[], &none, &[stamp]).unwrap();
    let flagged: Vec<_> = d.plan.nodes.iter().filter(|n| n.preview).map(|n| n.tile).collect();
    assert!(flagged.contains(&near) && flagged.len() < d.plan.nodes.len(), "{} of {}", flagged.len(), d.plan.nodes.len());
    assert!(d.plan.nodes.iter().filter(|n| n.preview).all(|n| n.transient_only));
    assert_eq!((d.kind, d.substituted, d.blank), (plain.kind, 0, 0));
    assert_eq!((d.preview.clone(), d.preview_delta_len), (vec![stamp], 0));
    // A delta stamp over the same exact tile does not change what the exact node applies: it is still transient-only, after the delta's.
    let hill = PreviewStamp { amount_m: 30.0, ..stamp };
    let d = draw_plan_transient(&req, base, &exact, &bounds, &[hill], &none, &[stamp]).unwrap();
    assert_eq!((d.preview.clone(), d.preview_delta_len), (vec![hill, stamp], 1));
    assert!(
        d.plan.nodes.iter().find(|n| n.tile == near).is_some_and(|n| n.preview && n.transient_only),
        "exact: the delta is not applied twice"
    );
    // A stale tile under both gets both; a stale tile under the transient stamp alone only that one; a stale tile under the delta alone is
    // not transient-only.
    let stale_near: HashMap<_, _> = plan
        .nodes
        .iter()
        .map(|n| (n.tile, ResidentTile { mesh_key: if n.tile == near { n.mesh_key ^ 1 } else { n.mesh_key }, origin: n.origin }))
        .collect();
    let node = |d: &planet_frame::pipeline::DrawPlan| d.plan.nodes.iter().find(|n| n.tile == near).copied().unwrap();
    let both = draw_plan_transient(&req, base, &stale_near, &bounds, &[hill], &none, &[stamp]).unwrap();
    assert!(node(&both).preview && !node(&both).transient_only, "stale under both: all the stamps");
    let transient_alone = draw_plan_transient(&req, base, &stale_near, &bounds, &[], &none, &[stamp]).unwrap();
    assert!(node(&transient_alone).preview && node(&transient_alone).transient_only);
    let delta_alone = draw_plan_transient(&req, base, &stale_near, &bounds, &[hill], &none, &[]).unwrap();
    assert!(node(&delta_alone).preview && !node(&delta_alone).transient_only);
    // The delta's stamps (at most MAX_DELTA_STAMPS) and a transient preview (at most BRUSH_CHAIN_STAMPS) fit the table whole, none cut.
    let delta: Vec<PreviewStamp> = (0..MAX_DELTA_STAMPS).map(|k| PreviewStamp { amount_m: k as f64, ..stamp }).collect();
    let chain: Vec<PreviewStamp> = (0..BRUSH_CHAIN_STAMPS).map(|k| PreviewStamp { amount_m: 100.0 + k as f64, ..stamp }).collect();
    let d = draw_plan_transient(&req, base, &exact, &bounds, &delta, &none, &chain).unwrap();
    assert_eq!((d.preview.len(), d.preview_delta_len), (MAX_PREVIEW_STAMPS, MAX_DELTA_STAMPS));
    assert_eq!(d.preview, delta.iter().chain(&chain).copied().collect::<Vec<_>>());
}

// spec: VIEW-036
#[test]
fn a_brush_items_strokes_keep_their_order_and_one_large_item_is_kept_stroke_by_stroke_under_the_cap() {
    let cam = east(0.0);
    // Order: a flatten and then a lower in one item are applied in that order (a plateau reads the running height).
    let flatten = stroke(BrushKind::Flatten, 100.0, 10.0, Some(0.0), &[0.0, 200.0]);
    let lower = stroke(BrushKind::Lower, 100.0, 5.0, None, &[0.0, 200.0]);
    let item = items(&[brush_spec(1, &[flatten.clone(), lower.clone()])]);
    let list = preview_delta_on(&[], &item, cam, R);
    let kinds: Vec<StampKind> = list.stamps.iter().map(|s| s.kind).collect();
    let last_plateau = kinds.iter().rposition(|k| *k == StampKind::Plateau).unwrap();
    let first_hill = kinds.iter().position(|k| *k == StampKind::Hill).unwrap();
    assert!(last_plateau < first_hill, "the flatten's plateaus come before the lower's hills: {kinds:?}");
    // The other way round keeps the other order.
    let swapped = preview_delta_on(&[], &items(&[brush_spec(1, &[lower, flatten])]), cam, R);
    assert_eq!(swapped.stamps[0].kind, StampKind::Hill);
    // One item with three strokes of 11 stamps (33 entries, over the cap): whole strokes are kept, nearest first, not nothing.
    let strokes: Vec<Stroke> =
        (0..3).map(|k| stroke(BrushKind::Raise, 100.0, 5.0, None, &[1_000.0 * f64::from(k), 1_000.0 * f64::from(k) + 400.0])).collect();
    let per = brush_chain(&strokes[0], 1, R).len();
    let list = preview_delta_on(&[], &items(&[brush_spec(1, &strokes)]), cam, R);
    let kept = MAX_DELTA_STAMPS / per;
    assert!(kept >= 1 && !list.stamps.is_empty(), "{} stamps", list.stamps.len());
    assert_eq!((list.stamps.len(), list.dropped), (kept * per, (3 - kept) * per));
    assert!(list.stamps.iter().all(|s| arc(s.dir, cam) < 1_000.0 * kept as f64), "the nearest strokes");
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
    assert!(worst <= strokes.strength_m / 3.0, "worst error {worst}");
    assert!(sum_abs / n <= 0.1 * strokes.strength_m, "mean error {}", sum_abs / n);
}

/// The summed delta of `stamps` at `p` as the vertex stage adds it (hills and plateaus; `h0` the height there before the preview).
fn shape_at(stamps: &[PreviewStamp], p: Vec3, h0: f64) -> f64 {
    let smooth01 = |t: f64| t * t * (3.0 - 2.0 * t);
    let mut total = 0.0;
    for s in stamps {
        let x = arc(s.dir, p) / s.reach_m;
        if x >= 1.0 {
            continue;
        }
        let profile = match s.kind {
            StampKind::Hill => s.amount_m * (1.0 - x * x) * (1.0 - x * x),
            StampKind::Plateau => {
                (1.0 - smooth01(((x - 0.7) / 0.3).clamp(0.0, 1.0))) * (s.amount_m - (h0 + total)).clamp(-s.max_delta_m, s.max_delta_m)
            }
            StampKind::Crater => unreachable!("a brush chain has no crater"),
            _ => 0.0,
        };
        total += f64::from(s.sign) * profile;
    }
    total
}

/// Compare a chain with the stage over a grid around a stroke: the worst and the mean error in metres over the points either moves.
fn fidelity(stroke: &Stroke, metres: &[(f64, f64)], span: (f64, f64)) -> (f64, f64, f64) {
    use planet_generators::height::{HeightField, Reference};
    use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
    use std::sync::Arc;

    let at = |e: f64, n: f64| -> Vec3 { (east(e) + Vec3::new(0.0, 0.0, 1.0) * (n / R)).normalized() };
    let _ = metres;
    let mut def = WorldDefinition::single_planet(R, 1);
    def.put_item_spec(BodyId::PLANET, brush_spec(1, std::slice::from_ref(stroke))).unwrap();
    let base: Arc<dyn HeightField> = Arc::new(Reference(1));
    let stage = StampStage::new(Arc::clone(&base), Arc::new(def), BodyId::PLANET, "brush", 0, STAMP_STAGE_VERSION).unwrap();
    let chain = brush_chain(stroke, 1, R);
    let (mut worst, mut sum, mut n) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..70 {
        for j in 0..70 {
            let p = at(span.0 + (span.1 - span.0) * f64::from(i) / 69.0, span.0 + (span.1 - span.0) * f64::from(j) / 69.0);
            let h = base.height(p);
            let exact = stage.height(p) - h;
            let preview = shape_at(&chain, p, h);
            if exact.abs() > 1e-9 || preview.abs() > 0.5 {
                worst = worst.max((exact - preview).abs());
                sum += (exact - preview).abs();
                n += 1.0;
            }
        }
    }
    (worst, sum / n, n)
}

// spec: VIEW-036
#[test]
fn the_chain_of_a_flatten_stays_within_the_strength_of_the_stage_s_and_does_not_stack_its_bound() {
    use planet_generators::height::{HeightField, Reference};
    let strength = 10.0;
    let ground = Reference(1).height(east(0.0));
    // A target far above the ground (more than the strength) and one within the strength.
    for (label, target) in [("far", ground + 40.0), ("near", ground + 6.0)] {
        let line =
            Stroke { target_m: Some(target), ..stroke(BrushKind::Flatten, 300.0, strength, Some(target), &[0.0, 400.0, 800.0, 1_200.0]) };
        let (worst, mean, n) = fidelity(&line, &[], (-400.0, 1_600.0));
        println!("FLATTEN {label}: worst {worst:.2} m, mean {mean:.2} m over {n} points");
        assert!(worst <= strength, "{label}: the preview is within the strength of the stage's delta everywhere (worst {worst})");
        // On the line itself the plateaus do not pile up: the preview moves the ground by at most the strength, as the stage does.
        let on_line = |m: f64| shape_at(&brush_chain(&line, 1, R), east(m), ground);
        for m in [200.0, 600.0, 1_000.0] {
            assert!(on_line(m).abs() <= strength * 1.05, "{label}: at {m} m the preview moves the ground by {}", on_line(m));
        }
    }
}

// spec: VIEW-036
#[test]
fn a_stroke_of_the_longest_length_the_tool_records_is_within_a_third_of_the_strength_of_the_stage() {
    // 64 points a quarter of the radius apart: about 16 radii.
    let radius = 100.0;
    let line = stroke(BrushKind::Raise, radius, 40.0, None, &[0.0, 63.0 * radius / 4.0]);
    let (worst, mean, n) = fidelity(&line, &[], (-300.0, 1_900.0));
    println!("LONG: worst {worst:.2} m, mean {mean:.2} m over {n} points");
    assert!(worst <= 40.0 / 3.0, "worst error {worst}");
    assert!(mean <= 40.0 * 0.1, "mean error {mean}");
}
