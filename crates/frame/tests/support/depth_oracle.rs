//! Test helper (LOD-007): a CPU depth pass over a set of tiles, the oracle for "does this tile contribute a visible pixel".
//! Ported from SPIKE-05's `residency_counts` example (docs/archive/M4/spikes.md): every tile's grid and skirt triangles are projected
//! in f64 (`Camera::project`), rasterized at pixel centres into one depth buffer (larger `1/w` is nearer, near plane
//! clipping), and a tile is visible when one of its triangles owns the nearest depth of a covered pixel.

use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::Camera;
use planet_generators::height::HeightField;
use planet_generators::mesh::{grid_indices, skirt_depth, tile_mesh_field};
use std::collections::BTreeSet;

type Clip = Vec<[f64; 3]>;

/// Rasterize one triangle given in clip space `(x, y, w)`; calls `f(pixel, depth)` per covered pixel centre, `depth` = `1/w`.
/// Clips at `w = near`. Returns true when `f` asked to stop.
fn raster(size: (usize, usize), tri: [[f64; 3]; 3], near: f64, f: &mut dyn FnMut(usize, f64) -> bool) -> bool {
    if !tri.iter().any(|v| v[2] >= near) {
        return false;
    }
    let mut poly: Vec<[f64; 3]> = Vec::with_capacity(4);
    for i in 0..3 {
        let (a, b) = (tri[i], tri[(i + 1) % 3]);
        if a[2] >= near {
            poly.push(a);
        }
        if (a[2] >= near) != (b[2] >= near) {
            let t = (near - a[2]) / (b[2] - a[2]);
            poly.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, near]);
        }
    }
    let (w, h) = (size.0 as f64, size.1 as f64);
    let scr: Vec<[f64; 3]> = poly.iter().map(|v| [(v[0] / v[2] * 0.5 + 0.5) * w, (0.5 - v[1] / v[2] * 0.5) * h, 1.0 / v[2]]).collect();
    for k in 1..scr.len().saturating_sub(1) {
        if raster_screen(size, [scr[0], scr[k], scr[k + 1]], f) {
            return true;
        }
    }
    false
}

fn raster_screen(size: (usize, usize), t: [[f64; 3]; 3], f: &mut dyn FnMut(usize, f64) -> bool) -> bool {
    let (w, h) = (size.0 as f64, size.1 as f64);
    let min_x = t.iter().map(|v| v[0]).fold(f64::MAX, f64::min);
    let max_x = t.iter().map(|v| v[0]).fold(f64::MIN, f64::max);
    let min_y = t.iter().map(|v| v[1]).fold(f64::MAX, f64::min);
    let max_y = t.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
    let (x0, x1) = ((min_x - 0.5).ceil().max(0.0) as i64, (max_x - 0.5).floor().min(w - 1.0) as i64);
    let (y0, y1) = ((min_y - 0.5).ceil().max(0.0) as i64, (max_y - 0.5).floor().min(h - 1.0) as i64);
    if x0 > x1 || y0 > y1 {
        return false;
    }
    let area = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
    if area == 0.0 {
        return false;
    }
    let inv = 1.0 / area;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let w0 = ((t[1][0] - px) * (t[2][1] - py) - (t[1][1] - py) * (t[2][0] - px)) * inv;
            let w1 = ((t[2][0] - px) * (t[0][1] - py) - (t[2][1] - py) * (t[0][0] - px)) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let z = w0 * t[0][2] + w1 * t[1][2] + w2 * t[2][2];
                if f(y as usize * size.0 + x as usize, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// The positions of `tile_mesh_field`'s vertices (grid, then the four skirt borders), without its normals, geomorph targets,
/// climate and key. The oracle only needs positions, and the five-point normal stencil makes the full mesh about five times
/// dearer. The arithmetic repeats the mesh's own, so every position is bit for bit the mesh's (`positions_match_the_mesh` checks).
fn tile_positions(field: &dyn HeightField, tile: TileId, cells: u32, radius_m: f64) -> (planet_core::Vec3, Vec<[f32; 3]>) {
    let map = &TangentWarp;
    let n = i64::from(cells);
    let width = (cells + 1) as usize;
    let mut dirs = Vec::with_capacity(width * width + 1);
    for j in 0..=n {
        for i in 0..=n {
            dirs.push(tile.sample_direction(map, cells, i as u32, j as u32));
        }
    }
    let centre_dir = tile.center_direction(map);
    dirs.push(centre_dir);
    let all = field.heights(&dirs);
    let count = width * width;
    let origin = centre_dir * (radius_m + all[count]);
    let mut pos: Vec<[f32; 3]> = (0..count)
        .map(|k| {
            let d = dirs[k] * (radius_m + all[k]) - origin;
            [d.x as f32, d.y as f32, d.z as f32]
        })
        .collect();
    let depth = skirt_depth(tile.level(), radius_m);
    let c = cells as usize;
    let border: Vec<usize> = (0..=c)
        .chain((0..=c).map(|i| c * (c + 1) + i))
        .chain((0..=c).map(|j| j * (c + 1)))
        .chain((0..=c).map(|j| j * (c + 1) + c))
        .collect();
    for &b in &border {
        let v = pos[b];
        let radial = (origin + planet_core::Vec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]))).normalized() * depth;
        pos.push([(f64::from(v[0]) - radial.x) as f32, (f64::from(v[1]) - radial.y) as f32, (f64::from(v[2]) - radial.z) as f32]);
    }
    (origin, pos)
}

fn project_tile(cam: &Camera, tile: TileId, cells: u32, radius_m: f64, field: &dyn HeightField) -> Clip {
    let (origin, pos) = tile_positions(field, tile, cells, radius_m);
    let o = origin - cam.position.0;
    pos.iter()
        .map(|v| {
            let rel = o + planet_core::Vec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
            let c = cam.project(rel);
            [c[0], c[1], c[3]]
        })
        .collect()
}

/// `tile_positions` equals the mesh's origin and vertex positions bit for bit, for the given tiles.
pub fn positions_match_the_mesh(tiles: &[TileId], cells: u32, radius_m: f64, field: &dyn HeightField) {
    for &t in tiles {
        let mesh = tile_mesh_field(field, &TangentWarp, t, cells, radius_m);
        let (origin, pos) = tile_positions(field, t, cells, radius_m);
        assert_eq!(origin, mesh.origin.0, "origin of {t:?}");
        let mp: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.pos).collect();
        assert_eq!(pos, mp, "positions of {t:?}");
    }
}

/// The tiles of `tiles` that own the nearest depth of at least one pixel of a `size` image (grid and skirt triangles).
/// Projection, the depth pass and the ownership pass each run on all cores; the depth buffer is the per-pixel maximum, so the
/// result does not depend on the thread count or the order of the triangles.
pub fn visible_tiles(
    cam: &Camera,
    size: (usize, usize),
    tiles: &[TileId],
    cells: u32,
    radius_m: f64,
    field: &dyn HeightField,
) -> BTreeSet<TileId> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = tiles.len().div_ceil(threads).max(1);
    let proj: Vec<Clip> = std::thread::scope(|s| {
        let hs: Vec<_> = tiles
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|&t| project_tile(cam, t, cells, radius_m, field)).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().expect("projection thread")).collect()
    });
    let idx = grid_indices(cells);
    let tri = |v: &Clip, k: usize| [v[idx[3 * k] as usize], v[idx[3 * k + 1] as usize], v[idx[3 * k + 2] as usize]];
    let ntri = idx.len() / 3;
    let near = cam.near_m;
    // Depth pass: each thread fills its own buffer from its share of the tiles; the buffers merge by per-pixel maximum.
    let bufs: Vec<Vec<f64>> = std::thread::scope(|s| {
        let hs: Vec<_> = proj
            .chunks(chunk)
            .map(|part| {
                let tri = &tri;
                s.spawn(move || {
                    let mut z = vec![0.0f64; size.0 * size.1];
                    for v in part {
                        for k in 0..ntri {
                            raster(size, tri(v, k), near, &mut |i, d| {
                                if d > z[i] {
                                    z[i] = d;
                                }
                                false
                            });
                        }
                    }
                    z
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("depth thread")).collect()
    });
    let mut z = vec![0.0f64; size.0 * size.1];
    for b in &bufs {
        for (a, &d) in z.iter_mut().zip(b) {
            if d > *a {
                *a = d;
            }
        }
    }
    drop(bufs);
    // Ownership pass: a tile is visible when one of its triangles owns the nearest depth of a pixel (z is read only).
    let z = &z;
    let owners: Vec<bool> = std::thread::scope(|s| {
        let hs: Vec<_> = proj
            .chunks(chunk)
            .map(|part| {
                let tri = &tri;
                s.spawn(move || {
                    part.iter()
                        .map(|v| {
                            let mut any = false;
                            for k in 0..ntri {
                                raster(size, tri(v, k), near, &mut |i, d| {
                                    any = d == z[i];
                                    any
                                });
                                if any {
                                    break;
                                }
                            }
                            any
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().expect("ownership thread")).collect()
    });
    tiles.iter().zip(owners).filter_map(|(&t, any)| any.then_some(t)).collect()
}

/// Self-check of the rasterizer on a toy scene: of a far and a near triangle covering the same pixels only the near one is
/// visible, and a triangle wholly behind the camera is neither drawn nor visible.
pub fn self_check() {
    let size = (64usize, 64usize);
    let near = 0.1;
    let big = |w: f64| [[-w, -w, w], [3.0 * w, -w, w], [-w, 3.0 * w, w]];
    let tris = [big(10.0), big(5.0), [[0.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 1.0, -1.0]]];
    let mut z = vec![0.0f64; size.0 * size.1];
    for t in &tris {
        raster(size, *t, near, &mut |i, d| {
            if d > z[i] {
                z[i] = d;
            }
            false
        });
    }
    let visible: Vec<bool> = tris
        .iter()
        .map(|t| {
            let mut vis = false;
            raster(size, *t, near, &mut |i, d| {
                vis |= d == z[i];
                vis
            });
            vis
        })
        .collect();
    assert_eq!(visible, vec![false, true, false], "rasterizer self-check");
}
