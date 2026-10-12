//! TEST-015, tier B: the morph closes the gap between levels on the steep stress field in every scripted view. Moved out of
//! `steep.rs` unchanged (it meshes every planned node of ten views, about 10 s under load) so that `cargo xtask test fast`
//! stays quick; it still runs in `cargo xtask test` and in CI. The other steep tests stay in `steep.rs`.

use planet_core::cube::TangentWarp;
use planet_core::{Side, TileId, Vec3};
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const STEEP: Steep = Steep(1);

// spec: TEST-015
#[test]
fn morph_closes_the_gap_between_levels_on_steep_terrain_in_every_scripted_view() {
    let map = TangentWarp;
    let params = LodParams { cells: 4, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let n = params.cells as usize;
    let cells = params.cells;
    let (mut coarse_cases, mut fine_cases) = (0, 0);
    for (name, cam) in scripted_views_field(R, 4.0 / 3.0, &STEEP) {
        let req = TerrainRequest {
            camera: cam,
            size: (128, 96),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        };
        let plan = plan_terrain_field(&req, &STEEP).unwrap();
        let leaves: BTreeMap<TileId, usize> = plan.nodes.iter().enumerate().map(|(i, d)| (d.tile, i)).collect();
        // Meshes in parallel (each a pure function of its tile; collected in node order), so tier A stays fast.
        let nodes: Vec<_> = plan.nodes.iter().filter(|d| d.level > 0).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let meshes: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = nodes
                .chunks(nodes.len().div_ceil(threads).max(1))
                .map(|part| {
                    s.spawn(move || part.iter().map(|d| tile_mesh_field(&STEEP, &TangentWarp, d.tile, cells, R)).collect::<Vec<_>>())
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for (d, mesh) in nodes.into_iter().zip(meshes) {
            let morph_at = |i: usize, j: usize| {
                let v = mesh.vertices[j * (n + 1) + i];
                let rel = mesh.origin.0 - cam.position.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
                ((rel.length() - f64::from(d.morph_start_m)) / f64::from((d.morph_end_m - d.morph_start_m).max(1e-6))).clamp(0.0, 1.0)
            };
            for side in Side::ALL {
                let nb = d.tile.neighbor(&map, side);
                let border: Vec<(usize, usize)> = (0..=n)
                    .map(|k| match side {
                        Side::East => (n, k),
                        Side::West => (0, k),
                        Side::North => (k, n),
                        Side::South => (k, 0),
                    })
                    .collect();
                if nb.parent().is_some_and(|p| leaves.contains_key(&p)) {
                    let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(1.0f64, f64::min);
                    assert!(
                        worst > 0.999,
                        "{name}: {:?} (level {}) borders a coarser leaf on {side:?} but only morphs to {worst}",
                        d.tile,
                        d.level
                    );
                    coarse_cases += 1;
                } else if !leaves.contains_key(&nb) && nb.children().is_some_and(|c| c.iter().any(|x| leaves.contains_key(x))) {
                    let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(0.0f64, f64::max);
                    assert!(
                        worst < 1e-3,
                        "{name}: {:?} (level {}) borders finer leaves on {side:?} but already morphs ({worst})",
                        d.tile,
                        d.level
                    );
                    fine_cases += 1;
                }
            }
        }
    }
    assert!(coarse_cases > 50 && fine_cases > 50, "too few level transitions: {coarse_cases} coarser, {fine_cases} finer");
}
