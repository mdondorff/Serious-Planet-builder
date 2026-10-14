//! Shared by `lod.rs` and `oracle_lod_coverage.rs`: the scripted views, the node selection and the coverage check.
#![allow(dead_code)]

use planet_core::cube::TangentWarp;
use planet_core::{Geo, PlanetFixed, ReferenceSurface, Sphere, TileId};
use planet_frame::{select_nodes, LodParams, NodeSelection};
use std::collections::BTreeSet;

pub fn camera(lat: f64, lon: f64, height: f64) -> PlanetFixed {
    Sphere::EARTH.geo_to_planet_fixed(Geo::new(lat, lon, height))
}

/// Ten scripted views: orbit down to 1 m, both poles, a cube corner and a face edge.
pub fn views() -> Vec<(&'static str, PlanetFixed)> {
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

pub fn params() -> LodParams {
    LodParams::earth_1080p()
}

pub fn select(c: PlanetFixed) -> NodeSelection {
    select_nodes(&TangentWarp, c, &params())
}

/// A sweep's selections share one warm cache (the production path); every 16th step is also selected cold and must be identical.
pub struct WarmSweep {
    cache: planet_frame::SelectionCache,
    step: u32,
}

impl WarmSweep {
    pub fn new() -> WarmSweep {
        WarmSweep { cache: planet_frame::SelectionCache::new(), step: 0 }
    }

    pub fn select(&mut self, c: PlanetFixed) -> NodeSelection {
        let warm = planet_frame::select_nodes_cached(&TangentWarp, c, &params(), &mut self.cache);
        if self.step.is_multiple_of(16) {
            assert_eq!(warm, select(c), "a warm selection differs from a cold one at step {}", self.step);
        }
        self.step += 1;
        warm
    }
}
/// Overlap-free, and every direction up to 98 % of the padded horizon angle (reference horizon plus the horizon of the
/// highest terrain) is held by exactly one leaf.
pub fn assert_covers(name: &str, c: PlanetFixed) {
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
