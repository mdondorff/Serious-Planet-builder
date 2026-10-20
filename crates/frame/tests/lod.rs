//! LOD-001..004: CPU node selection, snapshots over ten scripted views, coverage, balance, morph continuity.

use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::NodeSelection;
use std::collections::BTreeSet;

#[path = "support/lod.rs"]
mod support;
use support::*;

// spec: LOD-001
#[test]
fn selection_is_deterministic_sorted_and_unique() {
    for (name, c) in views() {
        let a = select(c);
        assert_eq!(a, select(c), "{name}");
        let ids: Vec<u64> = a.nodes.iter().map(|n| n.tile.raw()).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{name}: nodes must be sorted and unique");
    }
}

// spec: LOD-001
#[test]
fn detail_follows_the_camera() {
    let sel = |h: f64| select(camera(20.0, 30.0, h));
    let max_level = |s: &NodeSelection| s.nodes.iter().map(|n| n.tile.level()).max().unwrap();
    let orbit = max_level(&sel(2.0e7));
    let ground = max_level(&sel(1.0));
    assert!(orbit <= 5, "orbit view should stay coarse, got level {orbit}");
    assert!(ground >= 22, "1 m view should reach deep levels, got {ground}");
    // The deepest node is the one under the camera.
    let s = sel(1.0);
    let nadir = camera(20.0, 30.0, 0.0).0.normalized();
    let under = s.nodes.iter().find(|n| n.tile.contains(&TangentWarp, nadir)).unwrap();
    assert_eq!(under.tile.level(), max_level(&s), "the node under the camera must be the deepest");
}

/// Neighbouring leaves differ by at most one level (ancestor walk, across faces).
fn assert_balanced(name: &str, leaves: &BTreeSet<TileId>) {
    let m = TangentWarp;
    for &t in leaves {
        for side in planet_core::Side::ALL {
            let mut a = Some(t.neighbor(&m, side));
            while let Some(x) = a {
                if leaves.contains(&x) {
                    assert!(
                        t.level() - x.level().min(t.level()) <= 1,
                        "{name}: {t:?} (level {}) borders coarser leaf {x:?} (level {})",
                        t.level(),
                        x.level()
                    );
                    break;
                }
                a = x.parent();
            }
        }
    }
}

// spec: LOD-003
#[test]
fn balancing_repairs_hand_built_unbalanced_trees_including_across_faces() {
    let m = TangentWarp;
    for (face, level, x, y) in [(0u8, 5u8, 0u32, 0u32), (3, 6, 31, 17), (5, 5, 31, 31), (2, 4, 0, 15)] {
        // Split exactly the path from the root to one deep tile; everything else stays a coarse leaf.
        let target = TileId::new(planet_core::Face(face), level, x, y).unwrap();
        let mut leaves: BTreeSet<TileId> = planet_core::Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).unwrap()).collect();
        let mut path = vec![target];
        while let Some(p) = path.last().unwrap().parent() {
            path.push(p);
        }
        for t in path.iter().rev() {
            if t.level() < target.level() && leaves.remove(t) {
                leaves.extend(t.children().unwrap());
            }
        }
        let before = leaves.len();
        let mut unbalanced = false;
        for &t in &leaves {
            for side in planet_core::Side::ALL {
                let mut a = Some(t.neighbor(&m, side));
                while let Some(n) = a {
                    if leaves.contains(&n) && t.level() > n.level() + 1 {
                        unbalanced = true;
                    }
                    a = n.parent();
                }
            }
        }
        assert!(unbalanced, "the hand-built tree for {target:?} must start unbalanced");
        planet_frame::balance_leaves(&m, &mut leaves);
        assert!(leaves.len() > before, "balancing must split something");
        assert_balanced("hand-built", &leaves);
    }
}

// spec: LOD-003
#[test]
fn neighbouring_leaves_differ_by_at_most_one_level() {
    let m = TangentWarp;
    for (name, c) in views() {
        let s = select(c);
        let set: BTreeSet<TileId> = s.nodes.iter().map(|n| n.tile).collect();
        let mut checked = 0;
        for n in &s.nodes {
            for side in planet_core::Side::ALL {
                let nb = n.tile.neighbor(&m, side);
                // The leaf covering the neighbour position is nb itself, an ancestor of it, or finer (checked from its side).
                let mut a = Some(nb);
                while let Some(x) = a {
                    if set.contains(&x) {
                        assert!(
                            n.tile.level() - x.level().min(n.tile.level()) <= 1,
                            "{name}: {:?} (level {}) borders coarser leaf {:?} (level {})",
                            n.tile,
                            n.tile.level(),
                            x,
                            x.level()
                        );
                        checked += 1;
                        break;
                    }
                    a = x.parent();
                }
            }
        }
        assert!(checked > 0, "{name}");
    }
}

// spec: LOD-004
#[test]
fn morph_is_bounded_and_continuous_as_the_camera_recedes() {
    let mut sweep = WarmSweep::new();
    let m = TangentWarp;
    let nadir = camera(10.0, 20.0, 0.0).0.normalized();
    let mut h = 1.0f64;
    let mut prev: Option<(u8, f32)> = None;
    let mut merges = 0;
    while h < 2.0e7 {
        let c = camera(10.0, 20.0, h);
        let s = sweep.select(c);
        let n = s.nodes.iter().find(|n| n.tile.contains(&m, nadir)).expect("a leaf contains the nadir");
        assert!((0.0..=1.0).contains(&n.morph), "morph {} at height {h}", n.morph);
        if let Some((pl, pm)) = prev {
            assert!(n.tile.level() <= pl, "level must not increase while receding (height {h})");
            if n.tile.level() == pl {
                assert!((n.morph - pm).abs() <= 0.2, "morph jumped from {pm} to {} within level {pl} at height {h}", n.morph);
            }
            if n.tile.level() < pl {
                // A merge: the child was (almost) fully morphed, the new leaf starts (almost) unmorphed.
                assert!(pm >= 0.8, "merge at height {h}: previous morph only {pm}");
                assert!(n.morph <= 0.5, "after the merge at height {h}: morph {}", n.morph);
                merges += 1;
            }
        }
        prev = Some((n.tile.level(), n.morph));
        h *= 1.03;
    }
    assert!(merges >= 15, "expected many level changes between 1 m and 20000 km, saw {merges}");
}

// spec: LOD-004
#[test]
fn morph_grows_monotonically_within_one_level() {
    let mut sweep = WarmSweep::new();
    let m = TangentWarp;
    let nadir = camera(10.0, 20.0, 0.0).0.normalized();
    let mut by_level: std::collections::BTreeMap<u8, Vec<f32>> = Default::default();
    let mut h = 1.0f64;
    while h < 2.0e7 {
        let s = sweep.select(camera(10.0, 20.0, h));
        let n = s.nodes.iter().find(|n| n.tile.contains(&m, nadir)).unwrap();
        by_level.entry(n.tile.level()).or_default().push(n.morph);
        h *= 1.03;
    }
    for (level, ms) in by_level {
        assert!(ms.windows(2).all(|w| w[1] >= w[0] - 1e-6), "level {level}: morph decreased while receding: {ms:?}");
    }
}

// spec: LOD-002
#[test]
fn scripted_view_summaries_match_the_committed_snapshot() {
    let mut text = String::new();
    for (name, c) in views() {
        let s = select(c);
        assert!(s.nodes.len() < 8000, "{name}: {} nodes exceeds the budget of 8000", s.nodes.len());
        text += &format!("{name}: {}\n", s.summary());
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/lod_views.txt");
    if std::env::var("PLANET_UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(&path, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("snapshot missing; run with PLANET_UPDATE_SNAPSHOTS=1 and review the diff");
    if text != expected {
        let (a, b): (Vec<&str>, Vec<&str>) = (text.lines().collect(), expected.lines().collect());
        let mut diff: Vec<String> = a
            .iter()
            .zip(&b)
            .filter(|(x, y)| x != y)
            .map(|(x, y)| {
                format!(
                    "  now:      {x}
  snapshot: {y}"
                )
            })
            .collect();
        if a.len() != b.len() {
            diff.push(format!("  the number of scripted views changed: {} now, {} in the snapshot", a.len(), b.len()));
        }
        panic!(
            "node selection changed for {} scripted view(s); review and update the snapshot deliberately:\n{}",
            diff.len(),
            diff.join("\n")
        );
    }
}

/// The straightforward algorithm (full rescans, walk up to the root): the reference for `balance_leaves`.
fn reference_balance(m: &TangentWarp, leaves: &mut BTreeSet<TileId>) {
    loop {
        let mut to_split = BTreeSet::new();
        for &t in leaves.iter() {
            if t.level() < 2 {
                continue;
            }
            for side in planet_core::Side::ALL {
                let mut a = t.neighbor(m, side).parent().and_then(|p| p.parent());
                while let Some(x) = a {
                    if leaves.contains(&x) {
                        to_split.insert(x);
                    }
                    a = x.parent();
                }
            }
        }
        if to_split.is_empty() {
            return;
        }
        for t in to_split {
            leaves.remove(&t);
            leaves.extend(t.children().unwrap());
        }
    }
}

// spec: LOD-003
#[test]
fn balancing_equals_the_reference_algorithm_on_random_partitions() {
    let m = TangentWarp;
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    let mut needed_splits = 0;
    for _ in 0..60 {
        // A random quadtree: split random leaves up to level 7 (also across faces), then drop a few leaves (culled holes).
        let mut leaves: BTreeSet<TileId> = planet_core::Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).unwrap()).collect();
        for _ in 0..(40 + next() % 200) {
            let pick = *leaves.iter().nth(next() as usize % leaves.len()).unwrap();
            if pick.level() < 7 {
                leaves.remove(&pick);
                leaves.extend(pick.children().unwrap());
            }
        }
        let holes: Vec<TileId> = leaves.iter().copied().filter(|_| next() % 23 == 0).collect();
        for h in holes {
            leaves.remove(&h);
        }
        let (mut fast, mut slow) = (leaves.clone(), leaves.clone());
        planet_frame::balance_leaves(&m, &mut fast);
        reference_balance(&m, &mut slow);
        needed_splits += usize::from(fast.len() > leaves.len());
        assert_eq!(fast, slow, "balance_leaves differs from the reference on a random partition of {} leaves", leaves.len());
        assert_balanced("random", &fast);
    }
    assert!(needed_splits >= 30, "only {needed_splits} of 60 random trees needed balancing: the test would prove little");
}

/// A field of zero height whose global bound is the eroded 128-bake world's (9,735 m): the plan reads only the bound for the volumes of
/// the coarse tiles, so this is the plan the window makes over that world.
struct Bound9735;

impl planet_generators::height::HeightField for Bound9735 {
    fn height(&self, _: planet_core::Vec3) -> f64 {
        0.0
    }
    fn seed(&self) -> u64 {
        5
    }
    fn implementation_id(&self) -> &'static str {
        "bound-9735"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        9735.0
    }
}

// spec: LOD-003, LOD-011, VIEW-014, VIEW-032
#[test]
fn the_coarse_lod_the_window_derives_is_never_refused_by_the_plan_it_makes() {
    // SPIKE-19: `LodParams::coarse` picks the largest factor `validate` accepts (the morph interval of every level at least 5 % of its
    // end, from the node radius alone: zero margin at the edge, ratio 0.950000 at level 4), but the plan adds the height extent of the
    // neighbourhood to the start (`split + 2 rho`, rho from the volume's height range) and refuses start > 0.95 end. With heights up to
    // 9,735 m the level-4 start is 2,167,273.1 against 0.95 x 2,279,496.0 = 2,165,521.2: the plan refuses the F4 coarse view of the
    // window at a quarter of all orbital poses, and a refusal that arrives after F4 was pressed again aborts the window.
    let size = (320u32, 200u32);
    let field = Bound9735;
    let base = planet_frame::LodParams {
        cells: 32,
        viewport_h_px: f64::from(size.1),
        tau_px: 6.0 * f64::from(size.1) / 1440.0,
        max_height_m: 9735.0,
        ..planet_frame::LodParams::earth_1080p()
    };
    base.validate().unwrap();
    let coarse = base.coarse();
    let mut refused = Vec::new();
    for alt in [3.0e5, 6.0e5, 1.2e6, 1.8e6, 2.4e6] {
        let dir = planet_core::Vec3::new(1.0, 0.0, 0.0);
        let camera = planet_frame::Camera::at_surface_point(
            planet_core::PlanetFixed(dir * (6_371_000.0 + alt)),
            planet_core::Vec3::new(0.0, 1.0, 0.0),
            1.2,
            60f64.to_radians(),
            f64::from(size.0) / f64::from(size.1),
            0.1,
        );
        let req = planet_frame::TerrainRequest {
            camera,
            size,
            view: planet_frame::TerrainView::Level,
            lod: coarse.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 5,
        };
        if let Err(e) = planet_frame::plan_terrain_field(&req, &field) {
            refused.push(format!("alt {alt} m: {e}"));
        }
    }
    assert!(
        refused.is_empty(),
        "the plan refuses the coarse LOD {} (factor {:.4}) at {} of 5 poses: {:?}",
        "of the window",
        base.coarse_factor(),
        refused.len(),
        refused.first()
    );
}

// spec: VIEW-032
#[test]
fn the_coarse_lod_is_refused_at_almost_no_pose_the_base_lod_is_accepted_at_from_20_km_to_orbit() {
    // The SPIKE-19 review: the first test starts at 300 km; the refusals the verifier found sat at 25 to 190 km (levels 8 and 9, where a
    // field with only a global bound has volumes as tall as they are wide). Both parameter sets are the window's, over a field of zero
    // height whose bound is the eroded world's.
    let field = Bound9735;
    let cases = [
        (
            planet_frame::LodParams {
                cells: 32,
                viewport_h_px: 200.0,
                tau_px: 6.0 * 200.0 / 1440.0,
                max_height_m: 9735.0,
                ..planet_frame::LodParams::earth_1080p()
            },
            (320u32, 200u32),
        ),
        (planet_frame::LodParams { max_height_m: 9735.0, ..planet_frame::LodParams::earth_1080p() }, (1920, 1080)),
    ];
    let mut compared = 0;
    let mut refused_only_coarse = Vec::new();
    for (base, size) in cases {
        base.validate().unwrap();
        let coarse = base.coarse();
        assert!(coarse.tau_px > 1.5 * base.tau_px, "the coarse view is a real one: factor {}", coarse.tau_px / base.tau_px);
        for k in 0..40 {
            // 20 km to about 3,400 km in steps of 14 %, so the band of 31 to 39 km is hit; a face centre, a cube corner and a generic direction.
            let alt = 2.0e4 * libm::pow(1.14, f64::from(k));
            for (pitch, dir) in [
                (0.4, planet_core::Vec3::new(1.0, 0.0, 0.0)),
                (1.2, planet_core::Vec3::new(1.0, 1.0, 1.0).normalized()),
                (0.8, planet_core::Vec3::new(0.3, -1.0, 0.5).normalized()),
            ] {
                let camera = planet_frame::Camera::at_surface_point(
                    planet_core::PlanetFixed(dir * (6_371_000.0 + alt)),
                    planet_core::Vec3::new(0.0, 1.0, 0.0),
                    pitch,
                    60f64.to_radians(),
                    f64::from(size.0) / f64::from(size.1),
                    0.1,
                );
                let request = |lod: &planet_frame::LodParams| planet_frame::TerrainRequest {
                    camera,
                    size,
                    view: planet_frame::TerrainView::Level,
                    lod: lod.clone(),
                    face_mapping: "tangent-v1".into(),
                    seed: 5,
                };
                if planet_frame::plan_terrain_field(&request(&base), &field).is_ok() {
                    compared += 1;
                    if let Err(e) = planet_frame::plan_terrain_field(&request(&coarse), &field) {
                        refused_only_coarse.push(format!("{}x{} alt {alt:.0} m pitch {pitch}: {e}", size.0, size.1));
                    }
                }
            }
        }
    }
    assert!(compared >= 150, "the sweep compared {compared} poses");
    // Measured on a denser sweep (70 altitudes, 4 pitches, 5 directions): 0 % for the 320x200 case, 1.6 to 2.7 % for the production ones,
    // all at level 11 between 31 and 39 km (a level the check does not take, whose tiles the plan bounds from above).
    assert!(
        refused_only_coarse.len() * 25 <= compared,
        "{} of {compared} poses refused for the coarse LOD only: {:?}",
        refused_only_coarse.len(),
        refused_only_coarse.first()
    );
}
