//! Shared by the frustum-cut tests (LOD-007): `frustum_cut.rs` (tier A) and `oracle_frustum_cut_sweep.rs` (tier B).

#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{select_nodes_field_cached, select_nodes_in_view_field, Camera, LodParams, SelectionCache};
use planet_generators::mesh::tile_mesh_field;

pub const R: f64 = 6_371_000.0;

/// The tiles of `all` that the cut dropped for real: not in the cut selection, with no ancestor and no descendant in it (a
/// leaf that balancing replaced by its children, or the reverse, is still drawn).
pub fn dropped(all: &[TileId], kept: &[TileId]) -> Vec<TileId> {
    let kept_set: BTreeSet<TileId> = kept.iter().copied().collect();
    let mut related: BTreeSet<TileId> = BTreeSet::new();
    for &k in kept {
        let mut t = k;
        while let Some(p) = t.parent() {
            related.insert(p);
            t = p;
        }
    }
    all.iter()
        .copied()
        .filter(|&t| {
            let mut a = t;
            while let Some(p) = a.parent() {
                if kept_set.contains(&p) {
                    return false;
                }
                a = p;
            }
            !kept_set.contains(&t) && !related.contains(&t)
        })
        .collect()
}

/// Cameras: every standard view and its pitch and heading variants, so that tiles sit at the edges of the frustum.
pub fn cameras(views: Vec<(&'static str, Camera)>) -> Vec<(String, Camera)> {
    let mut out = Vec::new();
    for (name, cam) in views {
        out.push((name.to_string(), cam));
        if name == "orbit-20000km" {
            continue;
        }
        for (hint, pitch) in [(Vec3::new(0.0, 0.0, 1.0), 5.0f64), (Vec3::new(0.0, 1.0, 0.0), 35.0), (Vec3::new(1.0, 0.3, -0.5), 70.0)] {
            let c = Camera::at_surface_point(cam.position, hint, pitch.to_radians(), cam.fov_y_rad, cam.aspect, cam.near_m);
            out.push((format!("{name}/{hint:?}/{pitch}"), c));
        }
    }
    out
}

/// `f` over `items` on up to 16 scoped threads, results in the order of `items`. Panics propagate with their message.
fn par_map<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync) -> Vec<U> {
    let chunk = items.len().div_ceil(16).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items.chunks(chunk).map(|c| scope.spawn(|| c.iter().map(&f).collect::<Vec<U>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    })
}

/// The world-space vertices (positions and morph targets, skirts included) of one tile.
fn mesh_points(field: &dyn planet_generators::height::HeightField, cells: u32, t: TileId) -> Vec<Vec3> {
    let mesh = tile_mesh_field(field, &TangentWarp, t, cells, R);
    mesh.vertices
        .iter()
        .flat_map(|v| [v.pos, v.coarse])
        .map(|p| mesh.origin.0 + Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
        .collect()
}

/// The meshes dominate this file's run time and are pure functions of the tile, and the heading variants of one view cut nearly
/// the same tiles. So: select for every camera (in parallel), build each distinct tile's mesh once (in parallel), then check
/// every camera against the shared meshes. Same cameras and assertions at any `stride` (every
/// `stride`-th dropped tile of each camera is checked).
pub fn check_field(
    field: &(dyn planet_generators::height::HeightField + Sync),
    views: Vec<(&'static str, Camera)>,
    cells: u32,
    max_level: u8,
    tau: f64,
    stride: usize,
    camera_filter: &dyn Fn(&str) -> bool,
) {
    let lod = LodParams { cells, viewport_h_px: 360.0, tau_px: tau, max_level, ..LodParams::earth_1080p() };
    let lod = LodParams { max_height_m: lod.max_height_m.max(field.max_abs_height_m()), ..lod };
    let cams: Vec<(String, Camera)> = cameras(views).into_iter().filter(|(name, _)| camera_filter(name)).collect();
    let cuts: Vec<Vec<TileId>> = par_map(&cams, |(_, cam)| {
        let all: Vec<TileId> = select_nodes_field_cached(&TangentWarp, cam.position, field, &lod, &mut SelectionCache::new())
            .nodes
            .iter()
            .map(|n| n.tile)
            .collect();
        let kept: Vec<TileId> = select_nodes_in_view_field(&TangentWarp, cam, field, &lod).nodes.iter().map(|n| n.tile).collect();
        dropped(&all, &kept).into_iter().enumerate().filter(|(i, _)| i % stride == 0).map(|(_, t)| t).collect()
    });
    let distinct: Vec<TileId> = cuts.iter().flatten().copied().collect::<BTreeSet<_>>().into_iter().collect();
    let meshes: HashMap<TileId, Vec<Vec3>> = distinct.iter().copied().zip(par_map(&distinct, |&t| mesh_points(field, cells, t))).collect();
    let jobs: Vec<(&(String, Camera), &Vec<TileId>)> = cams.iter().zip(&cuts).collect();
    let checked: usize = par_map(&jobs, |((name, cam), tiles)| {
        let frustum = cam.frustum();
        for t in tiles.iter() {
            // Every vertex of the cut tile, skirts and morph targets included, lies outside one and the same side plane (what
            // a sphere test guarantees; "outside some plane each" would also pass for a tile straddling the frustum).
            let points = &meshes[t];
            assert!(
                frustum.normals.iter().any(|n| points.iter().all(|p| (*p - frustum.origin).dot(*n) < 0.0)),
                "{name}: cut tile {t:?} has vertices on the inside of every side plane"
            );
        }
        tiles.len()
    })
    .into_iter()
    .sum();
    assert!(checked > 100, "the cameras must cut enough tiles to mean something (checked {checked})");
}
