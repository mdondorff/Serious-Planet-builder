//! LOD-001..004: CPU node selection, snapshots over ten scripted views, coverage, balance, morph continuity.

use planet_core::cube::TangentWarp;
use planet_core::{Geo, PlanetFixed, ReferenceSurface, Sphere, TileId};
use planet_frame::{select_nodes, LodParams, NodeSelection};
use std::collections::BTreeSet;

fn camera(lat: f64, lon: f64, height: f64) -> PlanetFixed {
    Sphere::EARTH.geo_to_planet_fixed(Geo::new(lat, lon, height))
}

/// Ten scripted views: orbit down to 1 m, both poles, a cube corner and a face edge.
fn views() -> Vec<(&'static str, PlanetFixed)> {
    vec![
        ("orbit-20000km", camera(20.0, 30.0, 2.0e7)),
        ("high-2000km", camera(20.0, 30.0, 2.0e6)),
        ("aerial-200km", camera(20.0, 30.0, 2.0e5)),
        ("aerial-20km", camera(20.0, 30.0, 2.0e4)),
        ("low-2km", camera(20.0, 30.0, 2.0e3)),
        ("ground-200m", camera(20.0, 30.0, 200.0)),
        ("ground-1m", camera(20.0, 30.0, 1.0)),
        ("north-pole-1km", camera(90.0, 0.0, 1000.0)),
        ("cube-corner-1km", camera(35.264_389_682_754_654, 45.0, 1000.0)),
        ("face-edge-1km", camera(0.0, 45.0, 1000.0)),
    ]
}

fn params() -> LodParams {
    LodParams::earth_1080p()
}

fn select(c: PlanetFixed) -> NodeSelection {
    select_nodes(&TangentWarp, c, &params())
}

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

/// Overlap-free, and every direction up to 98 % of the padded horizon angle (reference horizon plus the horizon of the
/// highest terrain) is held by exactly one leaf.
fn assert_covers(name: &str, c: PlanetFixed) {
    let s = select(c);
    let set: BTreeSet<TileId> = s.nodes.iter().map(|n| n.tile).collect();
    for n in &s.nodes {
        let mut a = n.tile.parent();
        while let Some(p) = a {
            assert!(!set.contains(&p), "{name}: {p:?} is a leaf and an ancestor of {:?}", n.tile);
            a = p.parent();
        }
    }
    let r = 6_371_000.0;
    let pr = params();
    let (cam_dir, d) = (c.0.normalized(), c.0.length().max(r * (1.0 + 1e-12)));
    let limit = libm::acos(r / d) + libm::acos(r / (r + pr.max_height_m));
    let mut rng = planet_core::hash::SplitMix64(77);
    let mut tested = 0;
    for _ in 0..3000 {
        let v = planet_core::Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        let u = v - cam_dir * v.dot(cam_dir);
        if u.length() < 0.05 {
            continue;
        }
        let a = rng.range(0.0, limit.min(3.1) * 0.98);
        let dir = cam_dir * libm::cos(a) + u.normalized() * libm::sin(a);
        let holders = s.nodes.iter().filter(|n| n.tile.contains(&TangentWarp, dir)).count();
        assert_eq!(holders, 1, "{name}: direction {dir:?} at {a} rad from the camera is held by {holders} leaves");
        tested += 1;
    }
    assert!(tested > 2000, "{name}: only {tested} directions tested");
}

// spec: LOD-003
#[test]
fn leaves_cover_the_visible_surface_exactly_once() {
    for (name, c) in views() {
        assert_covers(name, c);
        let area: u128 = select(c).nodes.iter().map(|n| 1u128 << (2 * (28 - u32::from(n.tile.level())))).sum();
        assert!(area < 6u128 << 56, "{name}: horizon culling must drop something");
    }
}

// spec: LOD-003
#[test]
fn culling_stays_conservative_for_known_hard_cameras() {
    // Regression cameras from review: a tile whose chord radius underestimated its angular radius was culled while still visible.
    assert_covers("review-random", camera(-86.427, 127.912, 673.0));
    for angle in [60.0f64, 70.0, 80.0, 89.0] {
        // On the great circle from the +Z face centre through its (+X,+Y) corner, beyond the corner.
        let a = angle.to_radians();
        let corner = planet_core::Vec3::new(1.0, 1.0, 1.0).normalized();
        let dir = planet_core::Vec3::new(0.0, 0.0, 1.0) * libm::cos(a)
            + (corner - planet_core::Vec3::new(0.0, 0.0, corner.z)).normalized() * libm::sin(a);
        for h in [5_000.0, 100_000.0, 1_000_000.0] {
            assert_covers(&format!("beyond-corner {angle} deg {h} m"), PlanetFixed(dir * (6_371_000.0 + h)));
        }
    }
    // A camera below the reference sphere is treated as sitting on it: still covered, and within the node budget.
    for h in [-1.0, -400.0, -10_000.0] {
        let c = camera(20.0, 30.0, h);
        assert_covers("below-surface", c);
        assert!(select(c).nodes.len() < 8000, "below-surface camera at {h} m selects {} nodes", select(c).nodes.len());
    }
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
    let m = TangentWarp;
    let nadir = camera(10.0, 20.0, 0.0).0.normalized();
    let mut h = 1.0f64;
    let mut prev: Option<(u8, f32)> = None;
    let mut merges = 0;
    while h < 2.0e7 {
        let c = camera(10.0, 20.0, h);
        let s = select(c);
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
    let m = TangentWarp;
    let nadir = camera(10.0, 20.0, 0.0).0.normalized();
    let mut by_level: std::collections::BTreeMap<u8, Vec<f32>> = Default::default();
    let mut h = 1.0f64;
    while h < 2.0e7 {
        let s = select(camera(10.0, 20.0, h));
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
