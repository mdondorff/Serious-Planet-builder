//! BUILD-066: the G3-8 windows' seeded stratified draw (lead #186a).
use planet_cache::window_draw::*;

fn stats(land: f64, p50: f64, p90: f64, runoff: f64, lat: f64) -> Stats {
    Stats { land, land_p50_m: p50, land_p90_m: p90, runoff_mm: runoff, abs_lat_deg: lat }
}

// spec: BUILD-066, TEST-149
#[test]
fn windows_are_classed_by_the_rules_in_order() {
    let interior = Box { face: 0, i0: 3100, i1: 3719, j0: 3100, j1: 3719 };
    let edge = Box { face: 0, i0: 0, i1: 619, j0: 3100, j1: 3719 };
    let corner = Box { face: 0, i0: REGIONAL - SIZE, i1: REGIONAL - 1, j0: 0, j1: 619 };
    assert_eq!((interior.edges_near(), edge.edges_near(), corner.edges_near()), (0, 1, 2));
    assert_eq!(classify(&corner, &stats(0.2, 0.0, 0.0, 0.0, 0.0)), Some(Stratum::CubeCorner));
    assert_eq!(classify(&corner, &stats(0.19, 0.0, 0.0, 0.0, 0.0)), None, "a corner window needs 20 % land");
    assert_eq!(classify(&edge, &stats(0.3, 2000.0, 3000.0, 10.0, 45.0)), Some(Stratum::CubeEdge), "the band rule comes first");
    assert_eq!(classify(&interior, &stats(0.8, 1500.0, 2500.0, 10.0, 45.0)), Some(Stratum::HighMountain));
    assert_eq!(classify(&interior, &stats(0.9, 400.0, 900.0, 49.0, 20.0)), Some(Stratum::Arid));
    assert_eq!(classify(&interior, &stats(0.5, 80.0, 299.0, 400.0, 45.0)), Some(Stratum::FlatCoast), "flat coast before mixed");
    assert_eq!(classify(&interior, &stats(0.5, 200.0, 600.0, 400.0, 45.0)), Some(Stratum::MixedMidLatitude));
    assert_eq!(classify(&interior, &stats(0.5, 200.0, 600.0, 400.0, 10.0)), None, "no stratum at 10 degrees");
    assert_eq!(classify(&interior, &stats(1.0, 200.0, 600.0, 400.0, 45.0)), None, "all land at mid latitude is not mixed");
}

// spec: BUILD-066, TEST-149
#[test]
fn the_candidates_cover_each_face_and_reach_both_far_edges() {
    let c = candidates(3);
    let per_axis = REGIONAL / SIZE + 1;
    assert_eq!(c.len(), per_axis * per_axis);
    assert!(c.iter().all(|b| b.face == 3 && b.i1 - b.i0 + 1 == SIZE && b.j1 < REGIONAL));
    assert!(c.iter().any(|b| b.i0 == 0 && b.j0 == 0) && c.iter().any(|b| b.i1 == REGIONAL - 1 && b.j1 == REGIONAL - 1));
}

// spec: BUILD-066, TEST-149
#[test]
fn the_draw_is_seeded_fills_each_quota_and_overlaps_nothing() {
    // every candidate of two faces, classed by a stand-in rule so each stratum has many
    let mut classed = Vec::new();
    for face in [0u8, 1] {
        for (k, b) in candidates(face).into_iter().enumerate() {
            let st = match b.edges_near() {
                2.. => Stratum::CubeCorner,
                1 => Stratum::CubeEdge,
                _ => Stratum::ALL[2 + k % 4],
            };
            classed.push((b, stats(0.5, 0.0, 0.0, 0.0, 0.0), st));
        }
    }
    let a = draw(&classed, DRAW_SEED).unwrap();
    assert_eq!(a.len(), 10);
    for st in Stratum::ALL {
        assert_eq!(a.iter().filter(|d| d.stratum == st).count(), st.quota(), "{st:?}");
    }
    for (i, d) in a.iter().enumerate() {
        assert_eq!(d.index, i);
        assert!(!FIVE.iter().any(|f| f.overlaps(&d.window)), "no overlap with the five windows: {d:?}");
        assert!(a[..i].iter().all(|e| !e.window.overlaps(&d.window)), "no overlap with an earlier draw: {d:?}");
    }
    // the same seed draws the same; another seed draws differently
    assert_eq!(draw(&classed, DRAW_SEED).unwrap(), a);
    assert_ne!(draw(&classed, DRAW_SEED + 1).unwrap(), a);
    // a stratum without candidates is refused, naming it
    let few: Vec<_> = classed.iter().filter(|c| c.2 != Stratum::Arid).cloned().collect();
    assert!(draw(&few, DRAW_SEED).unwrap_err().contains("arid-endorheic has 0 free candidates"));
    // the data file names the seeds and every window
    let t = to_toml(&a, DRAW_SEED);
    assert!(
        t.contains(&format!("draw_seed = {DRAW_SEED}")) && t.matches("[[window]]").count() == 10 && t.contains("class = \"cube-corner\""),
        "{t}"
    );
}
