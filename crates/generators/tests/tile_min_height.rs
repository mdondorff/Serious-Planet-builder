//! The lowest height of a tile's own lattice vertices is the exact minimum of the terrain the tile draws (ATMO-020): the mesh is linear
//! between vertices, the morph only averages own vertices, and the skirts repeat their border vertex's height. GPU-free.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::{grid_vertex_count, tile_mesh_field, vertex_count, MeshVertex};
use planet_generators::stress::Steep;

const R: f64 = 6_371_000.0;

fn tiles() -> Vec<TileId> {
    let mut v = Vec::new();
    for (face, level, x, y) in [(0, 3, 2, 5), (2, 6, 30, 41), (4, 10, 700, 99), (1, 15, 20_000, 9_000), (5, 20, 500_000, 700_001)] {
        v.push(TileId::new(Face(face), level, x, y).unwrap());
    }
    v
}

/// The vertex shader's lattice minimum and maximum of a shaded tile, and every claim about the morph and the skirts.
fn check(name: &str, id: TileId, cells: u32, shaded: &[MeshVertex], min_m: f32) {
    assert_eq!(shaded.len(), vertex_count(cells), "{name}");
    let grid = grid_vertex_count(cells);
    let lo = shaded[..grid].iter().map(|v| v.height).fold(f32::INFINITY, f32::min);
    let hi = shaded[..grid].iter().map(|v| v.height).fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(min_m, lo, "{name} {id:?}: the tile's minimum is the lowest own lattice vertex");
    // Own heights, morph targets (the parent grid's: averages of own vertices), skirt vertices and every morph in between.
    for (k, v) in shaded.iter().enumerate() {
        assert!(v.height >= lo && v.height <= hi, "{name} {id:?} vertex {k}: height {} outside [{lo}, {hi}]", v.height);
        assert!(
            v.coarse_height >= lo && v.coarse_height <= hi,
            "{name} {id:?} vertex {k}: morphed height {} outside [{lo}, {hi}]",
            v.coarse_height
        );
        for m in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let h = v.height + (v.coarse_height - v.height) * m;
            // The shader's mix in f32 may round one ulp past an end; the cut's margin (4 eps) covers it.
            let slack = 2.0 * f32::EPSILON * lo.abs().max(hi.abs()).max(1.0);
            assert!(h >= lo - slack, "{name} {id:?} vertex {k} morph {m}: {h} under the minimum {lo}");
        }
    }
    // A skirt vertex hangs below the border in position only: its height (the water's depth reads it) is its border vertex's.
    let w = cells as usize + 1;
    for (r, run) in shaded[grid..].chunks(w).enumerate() {
        for (t, s) in run.iter().enumerate() {
            let (i, j) = match r {
                0 => (t, 0),
                1 => (t, cells as usize),
                2 => (0, t),
                _ => (cells as usize, t),
            };
            let border = &shaded[j * w + i];
            assert_eq!(s.height.to_bits(), border.height.to_bits(), "{name} {id:?} skirt {r}/{t}: own height");
            assert!(s.coarse_height >= lo, "{name} {id:?} skirt {r}/{t}: morphed height under the minimum");
        }
    }
}

// spec: ATMO-020
#[test]
fn the_minimum_of_a_tiles_own_vertices_bounds_every_height_the_tile_draws_morph_and_skirts_included_in_both_formats() {
    let fields: [(&str, Box<dyn HeightField>); 2] = [("reference", Box::new(Reference(1))), ("steep", Box::new(Steep(3)))];
    for (fname, field) in &fields {
        for id in tiles() {
            for cells in [16u32, 32] {
                let mesh = tile_mesh_field(field.as_ref(), &TangentWarp, id, cells, R);
                check(&format!("{fname} vertex format, {cells} cells"), id, cells, &mesh.vertices, mesh.min_height_m());
                // The vertex format's height is the field's, rounded once to f32.
                let n = cells + 1;
                let field_min = (0..n)
                    .flat_map(|j| (0..n).map(move |i| (i, j)))
                    .map(|(i, j)| field.height(id.sample_direction(&TangentWarp, cells, i, j)))
                    .fold(f64::INFINITY, f64::min);
                assert!((f64::from(mesh.min_height_m()) - field_min).abs() <= 1.0e-6 * field_min.abs().max(1.0), "{fname} {id:?}");
                let compact = compact_tile_field(field.as_ref(), &TangentWarp, id, cells, R);
                check(&format!("{fname} compact format, {cells} cells"), id, cells, &compact.shade_all(), compact.min_height_m());
                assert_eq!(compact.clone().min_height_m(), compact.min_height_m());
                // The compact format's minimum differs from the vertex format's by the 16-bit quantisation (half a step) at most.
                let d = (f64::from(compact.min_height_m()) - f64::from(mesh.min_height_m())).abs();
                assert!(d <= f64::from(compact.hr_step) + 1.0e-5 * field_min.abs().max(1.0), "{fname} {id:?}: {d}");
            }
        }
    }
}
