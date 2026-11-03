//! The preview maps of pass 1 (the owner's I5b look): the base and the carved base of the planet as equirectangular images, from the faces' changes, with no store and no viewer.

use super::face::FaceChange;
use crate::dump::Rgba;
use crate::preview::{change_map, elevation_map, Fields, Pixel};
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::{Face, Vec3};

/// The change of the six faces read at a direction: the bilinear interpolant of the cells' values (the cell centres are the samples), clamped at a face's edge.
pub fn change_at(faces: &[FaceChange; 6], d: Vec3) -> f64 {
    let (face, s, t) = direction_to_face(&TangentWarp, d);
    let f = &faces[face.0 as usize];
    debug_assert_eq!(f.face, Face(face.0));
    let n = f.cells as f64;
    let (x, y) = (((s + 1.0) * 0.5 * n - 0.5).clamp(0.0, n - 1.0), ((t + 1.0) * 0.5 * n - 0.5).clamp(0.0, n - 1.0));
    let (i, j) = ((x as usize).min(f.cells - 2), (y as usize).min(f.cells - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let at = |ii: usize, jj: usize| f64::from(f.change[jj * f.cells + ii]);
    let (a, b) = (at(i, j) + (at(i + 1, j) - at(i, j)) * fx, at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fx);
    a + (b - a) * fy
}

/// The maps of a pass 1 over the whole planet: `(base, carved base, change)` as images of `width x width / 2` pixels (elevation with hillshade; the change in a diverging tint),
/// from the base field and the six faces' changes. The base is read per pixel (not per cell), so the maps show what pass 1 adds to the field as the runtime reads it.
pub fn pass1_maps(width: usize, seed: u64, base: &(dyn Fn(Vec3) -> f64 + Sync), faces: &[FaceChange; 6]) -> [(String, Rgba); 3] {
    let height = width / 2;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        for x in 0..width {
            let lon = -core::f64::consts::PI + (x as f64 + 0.5) / width as f64 * 2.0 * core::f64::consts::PI;
            let d = Vec3::from_lat_lon(lat, lon);
            let (e, c) = (base(d), change_at(faces, d));
            pixels.push(Pixel {
                land_depth: e as f32,
                today_m: e as f32,
                new_m: (e + c) as f32,
                k: 0.0,
                terms: [0.0; 6],
                margin_km: 0.0,
                weights: [0.0; crate::classes::CLASS_COUNT],
            });
        }
    }
    let fields = Fields { seed, width, height, pixels, largest_k_step: 0.0, k_edges_over: 0.0, k_step_p99: 0.0 };
    let base_map = elevation_map(&fields, "BAKE PASS 1: BASE ELEVATION", |p| f64::from(p.today_m));
    let carved = elevation_map(&fields, "BAKE PASS 1: CARVED BASE", |p| f64::from(p.new_m));
    let change = change_map(&fields, "BAKE PASS 1: CHANGE (CARVED MINUS BASE)", 400.0, |p| f64::from(p.new_m - p.today_m));
    [("base".into(), base_map), ("carved".into(), carved), ("change".into(), change)]
}
