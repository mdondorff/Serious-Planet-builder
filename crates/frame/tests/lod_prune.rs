//! LOD-012 (ADR 0074): over a field whose bounds nest (`HeightField::bounds_monotone`) the selection skips the subtrees that one bounds query proves
//! hold no tile too steep to morph, and does no ceiling work where nothing is steep. The result is the one of the full path, node for node and
//! ceiling for ceiling, on every field and camera; the saving is real where the ground is gentle. Tier A, no GPU.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::lod::{select_nodes_field, select_nodes_in_view_cached};
use planet_frame::{Camera, LodParams, SelectionCache};
use planet_generators::height::HeightField;

use super::lod_capping::Spike;

const R: f64 = 6_371_000.0;

/// A field that forwards to `0` but does not say its bounds nest: the selection takes the full path over it.
struct Plain<'a>(&'a dyn HeightField);

impl HeightField for Plain<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.height(dir)
    }
    fn seed(&self) -> u64 {
        self.0.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.0.implementation_id()
    }
    fn version(&self) -> u32 {
        self.0.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.max_abs_height_m()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.0.height_bounds(centre, angular_radius)
    }
    fn inputs_hash(&self) -> u64 {
        self.0.inputs_hash()
    }
}

fn lod() -> LodParams {
    LodParams { viewport_h_px: 720.0, tau_px: 6.0, min_level: 0, max_height_m: 9_000.0, ..LodParams::earth_1080p() }
}

/// The point at angle `r` from `centre` on the bearing `t`.
fn around(centre: Vec3, t: f64, r: f64) -> Vec3 {
    let a = if centre.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = (a - centre * centre.dot(a)).normalized();
    let e2 = centre.cross(e1);
    (centre * libm::cos(r) + (e1 * libm::cos(t) + e2 * libm::sin(t)) * libm::sin(r)).normalized()
}

fn position(over: Vec3, offset_m: f64, bearing: f64, altitude_m: f64) -> PlanetFixed {
    PlanetFixed(around(over, bearing, offset_m / R) * (R + altitude_m))
}

// spec: LOD-012
#[test]
fn the_selection_over_a_field_with_nested_bounds_equals_the_full_path_node_for_node_and_ceiling_for_ceiling() {
    let lod = lod();
    let mut rng = SplitMix64(31);
    let (mut steep_cases, mut cases) = (0, 0);
    for k in 0..90 {
        let at = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized();
        let (height_m, radius_m) = (30.0 + 3_900.0 * rng.next_f64() * rng.next_f64(), 60.0 + 3_000.0 * rng.next_f64());
        let spike = Spike { at, height_m, radius_m };
        // From a metre to forty kilometres up, over the spike and up to a few spike radii away.
        let altitude = libm::pow(10.0, rng.next_f64() * 4.6);
        let offset = rng.next_f64() * 4.0 * radius_m;
        let pos = position(at, offset, rng.next_f64() * std::f64::consts::TAU, altitude);
        let fast = select_nodes_field(&TangentWarp, pos, &spike, &lod);
        let full = select_nodes_field(&TangentWarp, pos, &Plain(&spike), &lod);
        assert!(
            fast == full,
            "case {k}: spike {height_m:.0} m over {radius_m:.0} m, camera {altitude:.0} m up {offset:.0} m off: {} against {} nodes",
            fast.nodes.len(),
            full.nodes.len()
        );
        cases += 1;
        steep_cases += usize::from(!full.ceiling.is_empty());
    }
    assert!(steep_cases >= 8, "the cases include steep ground: {steep_cases} of {cases}");
    assert!(steep_cases < cases, "and gentle ground: {steep_cases} of {cases}");
}

// spec: LOD-012
#[test]
fn over_gentle_ground_the_selection_visits_fewer_tiles_than_the_full_path_and_selects_the_same_nodes() {
    let lod = lod();
    // A field that can be 3 km high (so every level reads the bounds of its neighbours) but is flat where the camera is.
    let over = Vec3::new(0.3, 0.8, 0.5).normalized();
    let reference = Spike { at: around(over, 0.0, 1.0e6 / R), height_m: 3_000.0, radius_m: 20_000.0 };
    for (name, altitude) in [("ground", 2.0), ("aerial", 20_000.0), ("high", 2_000_000.0)] {
        let pos = position(over, 0.0, 0.0, altitude);
        let (mut a, mut b) = (SelectionCache::new(), SelectionCache::new());
        let camera = Camera::at_surface_point(pos, Vec3::new(0.0, 0.0, 1.0), -0.3, 60f64.to_radians(), 16.0 / 9.0, 0.1);
        let fast = select_nodes_in_view_cached(&TangentWarp, &camera, &reference, &lod, &mut a);
        let full = select_nodes_in_view_cached(&TangentWarp, &camera, &Plain(&reference), &lod, &mut b);
        assert!(fast == full, "{name}: the same selection");
        assert!(a.misses() < b.misses(), "{name}: {} tiles visited against {} on the full path", a.misses(), b.misses());
    }
}

/// Several spikes: the highest of them at each point, so that the refinement jumps several levels across a few tiles.
struct Many(Vec<Spike>);

impl HeightField for Many {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.iter().map(|s| s.height(dir)).fold(0.0, f64::max)
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "test-many-spikes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.iter().map(|s| s.height_m).fold(0.0, f64::max)
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        // The highest of the spikes: the bounds are the highest of their bounds (every one is 0 at least, so the lows are too).
        self.0.iter().map(|s| s.height_bounds(centre, angular_radius)).fold((0.0, 0.0), |a, b| (a.0.max(b.0), a.1.max(b.1)))
    }
    fn bounds_monotone(&self) -> bool {
        true
    }
    fn inputs_hash(&self) -> u64 {
        9
    }
}

// spec: LOD-012
#[test]
fn the_selection_equals_the_full_path_at_marginal_parameters_over_clusters_of_tall_narrow_spikes() {
    let mut rng = SplitMix64(77);
    let (mut steep_cases, mut cases) = (0, 0);
    for (cells, viewport) in [(32u32, 720.0), (48, 720.0), (32, 600.0), (24, 900.0)] {
        let lod = LodParams { cells, viewport_h_px: viewport, ..lod() };
        for k in 0..24 {
            let over = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized();
            let spikes: Vec<Spike> = (0..5)
                .map(|_| Spike {
                    at: around(over, rng.next_f64() * std::f64::consts::TAU, rng.next_f64() * 6_000.0 / R),
                    height_m: 100.0 + 3_000.0 * rng.next_f64(),
                    radius_m: 40.0 + 600.0 * rng.next_f64(),
                })
                .collect();
            let field = Many(spikes);
            let altitude = libm::pow(10.0, rng.next_f64() * 4.0);
            let pos = position(over, rng.next_f64() * 4_000.0, rng.next_f64() * std::f64::consts::TAU, altitude);
            let fast = select_nodes_field(&TangentWarp, pos, &field, &lod);
            let full = select_nodes_field(&TangentWarp, pos, &Plain(&field), &lod);
            assert!(fast == full, "cells {cells}, viewport {viewport}, case {k}: {} against {} nodes", fast.nodes.len(), full.nodes.len());
            cases += 1;
            steep_cases += usize::from(!full.ceiling.is_empty());
        }
    }
    assert!(steep_cases >= 10 && steep_cases < cases, "steep and gentle cases both occur: {steep_cases} of {cases}");
}
