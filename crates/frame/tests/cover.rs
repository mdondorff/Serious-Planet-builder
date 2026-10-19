//! The cover is the plan's own selection with "resident" as the condition for descending (STRM-002, STRM-016, LOD-007, TEST-018):
//! with the residency set R resident the cover of a camera equals the plan of that camera, node for node and level for level,
//! whatever way the camera is turned; with tiles missing it is the deepest resident tiles, never blank; and the frame draws the
//! plan of an earlier camera only where it covers the frame's view. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Face, PlanetFixed, TileId, Vec3};
use planet_frame::pipeline::{draw_plan_with, DrawKind, DrawPlan, ResidentRecords, ResidentTile};
use planet_frame::{
    cover_selection_bounded, plan_covers_view, plan_terrain_field, residency_set_field, scripted_views_field, Camera, LodParams,
    SelectionCache, TerrainRequest, TerrainView, TileBounds,
};
use planet_generators::height::{HeightField, Reference};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);

fn lod() -> LodParams {
    LodParams {
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        min_level: 0,
        max_height_m: Reference(1).max_abs_height_m(),
        ..LodParams::earth_1080p()
    }
}

fn request(camera: Camera) -> TerrainRequest {
    TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 }
}

fn views() -> Vec<(&'static str, Camera)> {
    scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &Reference(1))
}

fn view(name: &str) -> Camera {
    views().into_iter().find(|(n, _)| *n == name).unwrap().1
}

/// R of a position, every tile of it resident (records from the field), with the bounds the residency set carries.
fn resident_r(position: PlanetFixed) -> (HashMap<TileId, ResidentTile>, TileBounds) {
    let field = Reference(1);
    let lod = lod();
    let r = residency_set_field(&TangentWarp, position, &field, &lod, &mut SelectionCache::new());
    let records = ResidentRecords::new(&field, &TangentWarp, &lod);
    (r.tiles.iter().map(|e| (e.tile, records.record(e.tile))).collect(), r.bounds)
}

fn levels(tiles: impl IntoIterator<Item = TileId>) -> BTreeMap<TileId, u8> {
    tiles.into_iter().map(|t| (t, t.level())).collect()
}

fn cover_of(camera: &Camera, set: &HashMap<TileId, ResidentTile>, bounds: &TileBounds) -> Vec<TileId> {
    cover_selection_bounded(&TangentWarp, camera, &lod(), &|t| set.get(&t).map(|r| r.origin), bounds)
}

/// Fails naming the tiles only one side has (tile, level), not the whole sets.
fn assert_same(label: &str, cover: &BTreeMap<TileId, u8>, plan: &BTreeMap<TileId, u8>) {
    let only = |a: &BTreeMap<TileId, u8>, b: &BTreeMap<TileId, u8>| {
        a.iter().filter(|(t, _)| !b.contains_key(t)).take(6).map(|(t, l)| format!("{t:?}@{l}")).collect::<Vec<_>>()
    };
    assert!(
        cover == plan,
        "{label}: cover {} tiles, plan {} tiles; only in the cover {:?}; only in the plan {:?}",
        cover.len(),
        plan.len(),
        only(cover, plan),
        only(plan, cover)
    );
}

fn plan_levels(camera: Camera) -> BTreeMap<TileId, u8> {
    levels(plan_terrain_field(&request(camera), &Reference(1)).unwrap().nodes.iter().map(|n| n.tile))
}

// spec: STRM-002, STRM-016, LOD-007, TEST-018
#[test]
fn with_r_resident_the_cover_equals_the_plan_of_the_camera_node_for_node_on_the_ten_standard_views() {
    for (name, camera) in views() {
        let (set, bounds) = resident_r(camera.position);
        let cover = cover_of(&camera, &set, &bounds);
        assert_same(name, &levels(cover.iter().copied()), &plan_levels(camera));
        assert!(cover.iter().all(|t| set.contains_key(t)), "{name}: only resident tiles");
    }
}

/// A camera at `position` turned `yaw` radians about the local up and pitched down by `pitch`.
fn turned(position: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(position, hint, pitch, 60f64.to_radians(), f64::from(SIZE.0) / f64::from(SIZE.1), 0.1)
}

// spec: STRM-002, STRM-016, LOD-007, TEST-018
#[test]
fn under_a_rotation_script_the_cover_equals_the_plan_of_every_frame_whatever_the_turn_rate() {
    for name in ["aerial-20km", "ground-200m", "face-edge-1km"] {
        let position = view(name).position;
        let (set, bounds) = resident_r(position);
        for step_deg in [0.2f64, 0.7, 3.0, 15.0] {
            for k in 0..(360.0 / step_deg).min(14.0) as u32 {
                let pitch = [-0.4, -1.0, 0.2][k as usize % 3];
                let camera = turned(position, f64::from(k) * step_deg.to_radians(), pitch);
                let cover = cover_of(&camera, &set, &bounds);
                assert_same(
                    &format!("{name}, {step_deg} degrees a frame, frame {k}"),
                    &levels(cover.iter().copied()),
                    &plan_levels(camera),
                );
            }
        }
    }
}

/// Every third leaf of R resident, with all ancestors.
fn partial_residency(position: PlanetFixed) -> HashMap<TileId, ResidentTile> {
    let (full, _) = resident_r(position);
    let mut leaves: Vec<TileId> = full.keys().copied().filter(|t| !full.keys().any(|o| o.parent() == Some(*t))).collect();
    leaves.sort();
    let mut out = HashMap::new();
    for t in leaves.into_iter().step_by(3) {
        let mut up = Some(t);
        while let Some(a) = up {
            out.entry(a).or_insert(full[&a]);
            up = a.parent();
        }
    }
    out
}

// spec: STRM-002, STRM-016
#[test]
fn with_tiles_missing_the_cover_is_the_deepest_resident_tiles_and_never_blank_with_or_without_bounds() {
    let camera = view("low-2km");
    let (_, bounds) = resident_r(camera.position);
    let plan = plan_levels(camera);
    let set = partial_residency(camera.position);
    for b in [&bounds, &TileBounds::default()] {
        let cover = cover_of(&camera, &set, b);
        assert!(!cover.is_empty() && cover.iter().all(|t| set.contains_key(t)), "only resident tiles");
        let cover_set: BTreeSet<TileId> = cover.iter().copied().collect();
        for t in &cover {
            assert!(!std::iter::successors(t.parent(), |a| a.parent()).any(|a| cover_set.contains(&a)), "{t:?} lies under another");
        }
        // Every node of the plan has a cover tile at it, above it or below it: nothing the camera sees is blank.
        for (t, l) in &plan {
            let above = std::iter::successors(Some(*t), |a| a.parent()).any(|a| cover_set.contains(&a));
            let below = cover.iter().any(|c| std::iter::successors(Some(*c), |a| a.parent()).any(|a| a == *t));
            assert!(above || below, "{t:?} (level {l}) has nothing in the cover");
        }
    }
    // Only the roots resident: the visible roots.
    let records = ResidentRecords::new(&Reference(1), &TangentWarp, &lod());
    let roots: HashMap<TileId, ResidentTile> =
        Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).unwrap()).map(|t| (t, records.record(t))).collect();
    let cover = cover_of(&camera, &roots, &bounds);
    assert!(!cover.is_empty() && cover.iter().all(|t| t.level() == 0));
}

/// The fresh plan of `from`, drawn by a frame of `to` over `set`.
fn draw_from(from: &Camera, to: &Camera, set: &HashMap<TileId, ResidentTile>, bounds: &TileBounds) -> DrawPlan {
    let (a, b) = (request(*from), request(*to));
    let plan = plan_terrain_field(&a, &Reference(1)).unwrap();
    draw_plan_with(&b, Some((&a, &plan)), set, bounds).unwrap()
}

// spec: STRM-016, TEST-018
#[test]
fn a_plan_of_an_earlier_camera_is_drawn_where_it_covers_the_frame_and_the_cover_is_drawn_where_it_does_not() {
    let position = view("ground-200m").position;
    let from = turned(position, 0.0, -0.4);
    let (set, bounds) = resident_r(position);
    // The same camera: the synchronous frame.
    assert_eq!(draw_from(&from, &from, &set, &bounds).kind, DrawKind::Exact);
    // A camera turned well away: the plan does not cover the frame's view, so the frame draws the cover, which is the plan of
    // the frame's camera.
    let away = turned(from.position, 3.0, -0.4);
    let d = draw_from(&from, &away, &set, &bounds);
    assert_eq!(d.kind, DrawKind::Cover);
    assert_eq!(levels(d.plan.nodes.iter().map(|n| n.tile)), plan_levels(away));
    // Small turns: the rule is the stated one (the plan if it draws every tile of the frame's cover or finer fills of it, else
    // the cover), and whichever is drawn, no node of the frame's own plan is blurred or missing.
    let plan = plan_terrain_field(&request(from), &Reference(1)).unwrap();
    let drawn: BTreeSet<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
    let (mut stale, mut cover_frames) = (0, 0);
    for k in 1..60 {
        let to = turned(from.position, f64::from(k) * 0.05f64.to_radians(), -0.4);
        let d = draw_from(&from, &to, &set, &bounds);
        let covers = plan_covers_view(&TangentWarp, &to, &lod(), &bounds, &cover_of(&to, &set, &bounds), &drawn);
        assert_eq!(d.kind, if covers { DrawKind::Stale } else { DrawKind::Cover }, "turn {k} x 0.05 degrees");
        (stale, cover_frames) = (stale + usize::from(covers), cover_frames + usize::from(!covers));
        let got = levels(d.plan.nodes.iter().map(|n| n.tile));
        for (t, l) in plan_levels(to) {
            let at_or_finer = std::iter::successors(Some(t), |a| a.parent()).any(|a| got.get(&a).is_some_and(|g| *g >= l && a == t))
                || got.keys().any(|g| std::iter::successors(Some(*g), |a| a.parent()).any(|a| a == t));
            assert!(at_or_finer, "turn {k}: node {t:?} (level {l}) is blurred or missing");
        }
    }
    assert!(stale > 0 && cover_frames > 0, "the fixture must draw the plan on some frames and the cover on others: {stale}/{cover_frames}");
}

// spec: STRM-016
#[test]
fn the_coverage_test_wants_every_tile_of_the_view_at_its_level_or_finer_and_nothing_coarser() {
    let camera = view("low-2km");
    let (set, bounds) = resident_r(camera.position);
    let cover = cover_of(&camera, &set, &bounds);
    let check = |drawn: &BTreeSet<TileId>| plan_covers_view(&TangentWarp, &camera, &lod(), &bounds, &cover, drawn);
    let all: BTreeSet<TileId> = cover.iter().copied().collect();
    assert!(check(&all), "drawing the cover covers it");
    let gone = *all.iter().next_back().unwrap();
    // A tile missing: a sliver of the view with nothing drawn.
    let missing: BTreeSet<TileId> = all.iter().filter(|t| **t != gone).copied().collect();
    assert!(!check(&missing));
    // A tile replaced by its parent: coarser than the view can be drawn.
    let coarse: BTreeSet<TileId> = all.iter().map(|t| if *t == gone { t.parent().unwrap() } else { *t }).collect();
    assert!(!check(&coarse));
    // A tile replaced by its four children: finer, covered.
    let finer: BTreeSet<TileId> = all.iter().flat_map(|t| if *t == gone { gone.children().unwrap().to_vec() } else { vec![*t] }).collect();
    assert!(check(&finer));
}

/// The level of the drawn node covering `dir`, none when nothing covers it.
fn drawn_level(nodes: &BTreeSet<TileId>, dir: Vec3) -> Option<(u8, TileId)> {
    (0..=22u8).find_map(|l| {
        let t = TileId::from_direction(&TangentWarp, dir, l);
        nodes.contains(&t).then_some((l, t))
    })
}

// spec: STRM-016, LOD-007, TEST-018
#[test]
fn an_approaching_turning_camera_over_a_lagging_plan_and_residency_never_draws_a_coarser_level_than_the_frame_before() {
    // SPIKE-18: the plan worker and the streamer run `PLAN_LAG` and `R_LAG` frames behind (a starved machine); everything the lagging
    // residency set names is resident. Each frame draws what `draw_plan_with` draws from the newest plan (when usable) and the resident
    // set; no point of the planet that stays in view and nears the camera may be drawn coarser than in the frame before.
    const PLAN_LAG: usize = 6;
    const R_LAG: usize = 10;
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let steps: Vec<Camera> = (0..140)
        .map(|k| {
            let alt = 3_000_000.0 * libm::pow(0.99, f64::from(k));
            turned(PlanetFixed(dir * (R + alt)), f64::from(k) * 0.5f64.to_radians(), -1.2)
        })
        .collect();
    let field = Reference(1);
    let records = ResidentRecords::new(&field, &TangentWarp, &lod());
    let plans: Vec<_> = steps.iter().map(|c| plan_terrain_field(&request(*c), &field).unwrap()).collect();
    let sets: Vec<_> = steps.iter().map(|c| residency_set_field(&TangentWarp, c.position, &field, &lod(), &mut SelectionCache::new())).collect();
    let probes: Vec<Vec3> = (0..2000)
        .map(|i| {
            let (u, a) = ((f64::from(i) + 0.5) / 2000.0, 2.399_963_229_728_653 * f64::from(i));
            let theta = 0.45 * libm::sqrt(u);
            let helper = Vec3::new(0.0, 0.0, 1.0);
            let east = dir.cross(helper).normalized();
            let north = dir.cross(east).normalized();
            (dir * libm::cos(theta) + (east * libm::cos(a) + north * libm::sin(a)) * libm::sin(theta)).normalized()
        })
        .collect();
    let mut prev: Vec<Option<(u8, TileId, f64)>> = vec![None; probes.len()];
    let mut worst = (0usize, String::new());
    let mut kinds = (0, 0, 0);
    for t in 0..steps.len() {
        let req = request(steps[t]);
        let plan_t = t.saturating_sub(PLAN_LAG);
        let set_t = t.saturating_sub(R_LAG);
        // What the streamer has built by now: D first (the nodes of the plans up to `R_LAG` frames old, with their ancestors); the
        // rest of R (the tiles out of view) is not built yet.
        let mut resident: HashMap<TileId, ResidentTile> = HashMap::new();
        for plan in &plans[..=set_t] {
            for n in &plan.nodes {
                let mut up = Some(n.tile);
                while let Some(a) = up {
                    resident.entry(a).or_insert_with(|| records.record(a));
                    up = a.parent();
                }
            }
        }
        let base_req = request(steps[plan_t]);
        let base = planet_frame::pipeline::plan_usable(&plans[plan_t], &req).then_some((&base_req, &plans[plan_t]));
        let d = draw_plan_with(&req, base, &resident, &sets[set_t].bounds).unwrap();
        match d.kind {
            DrawKind::Exact => kinds.0 += 1,
            DrawKind::Stale => kinds.1 += 1,
            DrawKind::Cover => kinds.2 += 1,
        }
        let nodes: BTreeSet<TileId> = d.plan.nodes.iter().map(|n| n.tile).collect();
        let mut bad = 0;
        let mut first = String::new();
        for (i, p) in probes.iter().enumerate() {
            let cur = drawn_level(&nodes, *p).map(|(l, tl)| (l, tl, (steps[t].position.0 - *p * R).length()));
            if let (Some((l0, t0, d0)), Some((l1, t1, d1))) = (prev[i], cur) {
                if l1 < l0 && d1 < d0 - 1.0 {
                    bad += 1;
                    if first.is_empty() {
                        first = format!("frame {t} ({:?}): probe {i} L{l0} {t0:?} -> L{l1} {t1:?}", d.kind);
                    }
                }
            }
            prev[i] = cur;
        }
        if bad > worst.0 {
            worst = (bad, first);
        }
    }
    eprintln!("kinds exact/stale/cover {kinds:?}; worst frame {} probes coarser: {}", worst.0, worst.1);
    assert_eq!(worst.0, 0, "the drawn level got coarser while the camera neared: {}", worst.1);
}

// spec: STRM-016, LOD-007, TEST-018
#[test]
fn a_turn_over_what_the_first_view_made_resident_never_draws_a_point_coarser_than_the_view_before() {
    // SPIKE-18: only the tiles of the first view's plan (D) and their ancestors are resident, the rest of R still streams. A camera
    // that turns a little (same position, so no point gets farther) may leave tiles missing where the view is new, but a point drawn at
    // level L in the first view and in view in the second may not be drawn at a level below L: a missing sibling that came into view
    // must not undo the finer tiles that are resident (the cover splits a tile only if every child in view is resident).
    for alt in [4_700_000.0f64, 1_600_000.0, 300_000.0, 60_000.0, 20_000.0, 2_000.0] {
        let name = format!("altitude {alt} m");
        let position = PlanetFixed(Vec3::new(0.3, 0.5, 0.8).normalized() * (R + alt));
        let a = turned(position, 0.0, -1.2);
        let plan_a = plan_terrain_field(&request(a), &Reference(1)).unwrap();
        let records = ResidentRecords::new(&Reference(1), &TangentWarp, &lod());
        let mut resident: HashMap<TileId, ResidentTile> = HashMap::new();
        for n in &plan_a.nodes {
            let mut up = Some(n.tile);
            while let Some(t) = up {
                resident.entry(t).or_insert_with(|| records.record(t));
                up = t.parent();
            }
        }
        let bounds = TileBounds::default();
        let first: BTreeSet<TileId> = cover_of(&a, &resident, &bounds).into_iter().collect();
        let probes: Vec<Vec3> = plan_a.nodes.iter().flat_map(|n| (0..8u32).map(move |i| n.tile.sample_direction(&TangentWarp, 8, i, (i * 3) % 9))).collect();
        let mut worst = (0usize, String::new());
        for (step_deg, zoom) in [(0.5f64, 1.0f64), (1.0, 1.0), (2.0, 0.98), (5.0, 0.98), (10.0, 0.95), (2.0, 0.9), (5.0, 0.9)] {
            let up = position.0.normalized();
            let alt = position.0.length() - R;
            let moved = PlanetFixed(up * (R + alt * zoom));
            let b = turned(moved, step_deg.to_radians(), -1.2);
            let second: BTreeSet<TileId> = cover_of(&b, &resident, &bounds).into_iter().collect();
            let (mut bad, mut ex) = (0, String::new());
            for p in &probes {
                if let (Some((l0, t0)), Some((l1, t1))) = (drawn_level(&first, *p), drawn_level(&second, *p)) {
                    let (d0, d1) = ((a.position.0 - *p * R).length(), (b.position.0 - *p * R).length());
                    if l1 < l0 && d1 <= d0 {
                        bad += 1;
                        if ex.is_empty() {
                            ex = format!("{name}, turn {step_deg} degrees, altitude x{zoom}: L{l0} {t0:?} -> L{l1} {t1:?}");
                        }
                    }
                }
            }
            eprintln!("{name}: turn {step_deg} degrees, altitude x{zoom}: {bad} probes coarser");
            if bad > worst.0 {
                worst = (bad, ex);
            }
        }
        assert_eq!(worst.0, 0, "{} probes drawn coarser after a turn: {}", worst.0, worst.1);
    }
}

// spec: STRM-002, STRM-016, LOD-007
#[test]
fn one_missing_in_view_tile_does_not_take_its_resident_siblings_back_to_their_parent() {
    // SPIKE-18 (the owner's "coarser for a moment"): the cover draws a tile's children only if every child in view is resident, else
    // the parent. With R resident but one leaf `s` (not yet streamed in), the three resident siblings of `s` are drawn as their parent,
    // a level coarser over their whole area, and the resident finer tiles disappear for as long as `s` is missing. Measured in the
    // harness under CPU load: every violation was a one-level drop, previous tile still resident, a sibling of the new tile never
    // resident (`target/coarsen-ever.log` of SPIKE-18). The frame must keep drawing the resident siblings.
    for name in ["aerial-20km", "low-2km", "ground-200m"] {
        let camera = view(name);
        let (full, bounds) = resident_r(camera.position);
        let leaves = cover_of(&camera, &full, &bounds);
        let mut checked = 0;
        for s in leaves.iter().copied().filter(|t| t.level() >= 3).take(40) {
            let Some(parent) = s.parent() else { continue };
            let siblings: Vec<TileId> = parent.children().unwrap().into_iter().filter(|c| *c != s && leaves.contains(c)).collect();
            if siblings.is_empty() {
                continue;
            }
            // `s` has not streamed in: neither it nor anything under it is resident.
            let partial: HashMap<TileId, ResidentTile> = full
                .iter()
                .filter(|(t, _)| !std::iter::successors(Some(**t), |a| a.parent()).any(|a| a == s))
                .map(|(t, r)| (*t, *r))
                .collect();
            let drawn: BTreeSet<TileId> = cover_of(&camera, &partial, &bounds).into_iter().collect();
            for sib in siblings {
                let kept = drawn.iter().any(|d| std::iter::successors(Some(*d), |a| a.parent()).any(|a| a == sib));
                assert!(kept, "{name}: with {s:?} (level {}) missing, its resident sibling {sib:?} is drawn as {:?}, a level coarser", s.level(), parent);
            }
            checked += 1;
        }
        assert!(checked > 0, "{name}: no leaf with a leaf sibling to remove");
    }
}
