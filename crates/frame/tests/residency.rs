//! STRM-015, tier A: the residency set R is a pure function of the camera position, the LOD parameters and the horizon cut.
//! Yaw and pitch do not change it; every draw set from the position lies inside it; it is closed under parents.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{residency_set, residency_set_field, select_nodes_field, select_nodes_in_view_field, Camera, LodParams, SelectionCache};
use planet_generators::height::{HeightField, Reference};

const R: f64 = 6_371_000.0;

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 360.0, tau_px: 4.0, max_level: 16, min_level: 0, ..LodParams::earth_1080p() }
}

fn position(height: f64) -> PlanetFixed {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    PlanetFixed(dir * (R + height))
}

fn camera(pos: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(pos, hint, pitch, 60f64.to_radians(), 16.0 / 9.0, 0.1)
}

// spec: STRM-015
#[test]
fn r_is_sorted_closed_under_parents_and_holds_the_selected_leaves() {
    let field = Reference(1);
    let params = LodParams { max_height_m: field.max_abs_height_m(), ..lod() };
    let pos = position(500.0);
    let r = residency_set_field(&TangentWarp, pos, &field, &params, &mut SelectionCache::new());
    assert!(!r.is_empty());
    assert!(r.tiles.windows(2).all(|w| w[0].tile < w[1].tile), "ascending tile-id order, no duplicates");
    for e in &r.tiles {
        if let Some(p) = e.tile.parent() {
            assert!(r.contains(p), "{:?} is in R but its parent {:?} is not", e.tile, p);
        }
        assert!(e.distance_m.is_finite() && e.distance_m >= 0.0);
    }
    let leaves: Vec<TileId> = select_nodes_field(&TangentWarp, pos, &field, &params).nodes.iter().map(|n| n.tile).collect();
    assert_eq!(r.tiles.iter().filter(|e| e.leaf).map(|e| e.tile).collect::<Vec<_>>(), leaves);
    // An ancestor's distance is that of its nearest descendant leaf.
    for e in r.tiles.iter().filter(|e| !e.leaf) {
        assert!(r.tiles.iter().any(|l| l.leaf && l.distance_m == e.distance_m));
    }
}

// spec: STRM-015
#[test]
fn r_is_a_pure_function_of_the_position_and_independent_of_cache_state() {
    let field = Reference(1);
    let params = LodParams { max_height_m: field.max_abs_height_m(), ..lod() };
    let mut warm = SelectionCache::new();
    let a = residency_set_field(&TangentWarp, position(500.0), &field, &params, &mut warm);
    let _elsewhere = residency_set_field(&TangentWarp, position(5.0e5), &field, &params, &mut warm);
    let b = residency_set_field(&TangentWarp, position(500.0), &field, &params, &mut warm);
    let c = residency_set_field(&TangentWarp, position(500.0), &field, &params, &mut SelectionCache::new());
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// spec: STRM-015, LOD-007
#[test]
fn every_draw_set_from_the_position_is_inside_r_whatever_the_yaw_and_pitch() {
    let field = Reference(1);
    let params = LodParams { max_height_m: field.max_abs_height_m(), ..lod() };
    for height in [2.0, 500.0, 3.0e4, 2.0e6] {
        let pos = position(height);
        let r = residency_set_field(&TangentWarp, pos, &field, &params, &mut SelectionCache::new());
        let mut smallest_d = usize::MAX;
        for k in 0..24 {
            let (yaw, pitch) = (f64::from(k) * 0.2618, [-1.2, -0.4, 0.0, 0.5][k as usize % 4]);
            let d = select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), &field, &params);
            smallest_d = smallest_d.min(d.nodes.len());
            for n in &d.nodes {
                assert!(r.contains(n.tile), "height {height}, yaw {yaw}: draw tile {:?} is not in R", n.tile);
            }
        }
        if height < 1.0e5 {
            assert!(smallest_d < r.tiles.iter().filter(|e| e.leaf).count(), "height {height}: a view must draw fewer tiles than R holds");
        }
    }
}

// spec: STRM-015
#[test]
fn the_flat_variant_gives_leaves_and_ancestors() {
    let params = lod();
    let flat = residency_set(&TangentWarp, position(300.0), &params, &mut SelectionCache::new());
    assert!(flat.len() > 100);
    assert!(flat.tiles.iter().any(|e| e.leaf) && flat.tiles.iter().any(|e| !e.leaf));
}
