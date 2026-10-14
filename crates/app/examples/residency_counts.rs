//! SPIKE-05 measurement helper (docs/spikes.md): residency counts per standard view. Plans the ten scripted views over a
//! stored global bake with the production planner and counts, for two sets: the draw set (`plan_terrain_cached`, frustum cut,
//! horizon cut) and the position-based surroundings set (`select_nodes_field_cached`: no frustum, horizon cut kept), the nodes,
//! distinct tiles, triangles and vertex bytes (today's 44-byte vertex), and the plan time cold and warm. Also: the share of
//! drawn triangles that produce no visible pixel (a CPU depth pass, any adapter) and generation throughput per thread.
//!
//! Not production code and not part of the build of the binary: `cargo run --release -p planet --example residency_counts --
//! --store DIR --global-res N [--part counts|hidden|throughput]`. The store is a bake made by `generate --global-dir`.

use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{TileId, Vec3};
use planet_frame::{
    plan_terrain_cached, scripted_views_field, select_nodes_field_cached, Camera, LodParams, SelectionCache, TerrainDraw, TerrainRequest,
    TerrainView,
};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{GlobalCoarseGenerator, GridSource, LatticeSource};
use planet_generators::height::HeightField;
use planet_generators::mesh::{grid_indices, tile_mesh_field, vertex_count, MeshVertex};
use planet_generators::terrain::TerrainGenerator;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn lod(field: &dyn HeightField, cells: u32, max_level: u8) -> LodParams {
    LodParams {
        radius_m: RADIUS_M,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        cells,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        max_level,
        ..LodParams::earth_1080p()
    }
}

struct Counts {
    nodes: usize,
    tiles: usize,
    tris: u64,
    mb: f64,
    cold_ms: f64,
    warm_ms: f64,
    worst_morph: f64,
    by_level: Vec<(u8, usize)>,
}

fn counts_of(tiles: &[TileId], cells: u32, cold_ms: f64, warm_ms: f64, worst_morph: f64) -> Counts {
    let set: std::collections::BTreeSet<_> = tiles.iter().copied().collect();
    let per_tri = (grid_indices(cells).len() / 3) as u64;
    let mut h = std::collections::BTreeMap::new();
    for t in tiles {
        *h.entry(t.level()).or_insert(0usize) += 1;
    }
    Counts {
        nodes: tiles.len(),
        tiles: set.len(),
        tris: tiles.len() as u64 * per_tri,
        mb: (set.len() * vertex_count(cells) * MeshVertex::STRIDE as usize) as f64 / 1.0e6,
        cold_ms,
        warm_ms,
        worst_morph,
        by_level: h.into_iter().collect(),
    }
}

fn draw_set(field: &dyn HeightField, cam: Camera, p: &LodParams) -> Result<Counts, String> {
    let req = TerrainRequest {
        camera: cam,
        size: SIZE,
        view: TerrainView::Level,
        lod: p.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    };
    let mut cache = SelectionCache::new();
    let t = Instant::now();
    let plan = plan_terrain_cached(&req, field, &mut cache).map_err(|e| e.to_string())?;
    let cold = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    plan_terrain_cached(&req, field, &mut cache).map_err(|e| e.to_string())?;
    let warm = t.elapsed().as_secs_f64() * 1e3;
    let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
    let worst =
        plan.nodes.iter().filter(|n| n.level > 0).map(|n| f64::from(n.morph_start_m) / f64::from(n.morph_end_m)).fold(0.0, f64::max);
    Ok(counts_of(&tiles, p.cells, cold, warm, worst))
}

fn surroundings_set(field: &dyn HeightField, cam: Camera, p: &LodParams) -> Result<Counts, String> {
    p.validate()?;
    let mut cache = SelectionCache::new();
    let t = Instant::now();
    let sel = select_nodes_field_cached(&TangentWarp, cam.position, field, p, &mut cache);
    let cold = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    select_nodes_field_cached(&TangentWarp, cam.position, field, p, &mut cache);
    let warm = t.elapsed().as_secs_f64() * 1e3;
    let tiles: Vec<TileId> = sel.nodes.iter().map(|n| n.tile).collect();
    let worst = sel.nodes.iter().filter(|n| n.tile.level() > 0).map(|n| n.morph_start_m / n.morph_end_m).fold(0.0, f64::max);
    Ok(counts_of(&tiles, p.cells, cold, warm, worst))
}

fn print_counts(prefix: &str, c: &Result<Counts, String>) {
    match c {
        Ok(c) => {
            let lv: Vec<String> = c.by_level.iter().map(|(l, n)| format!("L{l}:{n}")).collect();
            println!(
                "{prefix} nodes {} tiles {} tris {:.2}M MB {:.1} plan_cold_ms {:.1} plan_warm_ms {:.2} morph_ratio_max {:.3} [{}]",
                c.nodes,
                c.tiles,
                c.tris as f64 / 1e6,
                c.mb,
                c.cold_ms,
                c.warm_ms,
                c.worst_morph,
                lv.join(" ")
            );
        }
        Err(e) => println!("{prefix} REJECTED {}", e.chars().take(160).collect::<String>()),
    }
}

fn counts(label: &str, field: &dyn HeightField) {
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), field);
    for (cells, levels) in [(32u32, [26u8, 23, 22]), (64, [26, 23, 22]), (64, [25, 22, 21])] {
        for level in levels {
            let p = lod(field, cells, level);
            for (name, cam) in &views {
                print_counts(&format!("COUNT {label} cells {cells} L{level} {name} draw"), &draw_set(field, *cam, &p));
                print_counts(&format!("COUNT {label} cells {cells} L{level} {name} surr"), &surroundings_set(field, *cam, &p));
            }
        }
    }
}

// ---- hidden triangles: a CPU depth pass over the drawn tiles ----

/// Project a tile's vertices to clip space `(x, y, w)` relative to the camera (f64, like `Camera::project`).
fn project_tile(cam: &Camera, d: &TerrainDraw, cells: u32, field: &dyn HeightField) -> (Vec<[f64; 3]>, bool) {
    let mesh = tile_mesh_field(field, &TangentWarp, d.tile, cells, RADIUS_M);
    let o = mesh.origin.0 - cam.position.0;
    // A tight bounding sphere of the tile's own vertices: what a frustum cut with per-tile height bounds could use.
    let radius =
        mesh.vertices.iter().map(|v| Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])).length()).fold(0.0, f64::max);
    let kept = !cam.frustum().culls_sphere(mesh.origin.0, radius);
    let clip = mesh
        .vertices
        .iter()
        .map(|v| {
            let rel = o + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
            let c = cam.project(rel);
            [c[0], c[1], c[3]]
        })
        .collect();
    (clip, kept)
}

/// Rasterize one triangle given in clip space (x, y, w); calls `f(pixel, depth)` per covered pixel centre, `depth` = 1 / w
/// (larger is nearer, linear in screen space). Clips at the near plane w = `near`. Returns true when `f` asked to stop.
fn raster(tri: [[f64; 3]; 3], near: f64, f: &mut dyn FnMut(usize, f64) -> bool) -> bool {
    let inside: Vec<bool> = tri.iter().map(|v| v[2] >= near).collect();
    if !inside.iter().any(|b| *b) {
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
    let scr: Vec<[f64; 3]> = poly
        .iter()
        .map(|v| [(v[0] / v[2] * 0.5 + 0.5) * f64::from(SIZE.0), (0.5 - v[1] / v[2] * 0.5) * f64::from(SIZE.1), 1.0 / v[2]])
        .collect();
    for k in 1..scr.len().saturating_sub(1) {
        if raster_screen([scr[0], scr[k], scr[k + 1]], f) {
            return true;
        }
    }
    false
}

fn raster_screen(t: [[f64; 3]; 3], f: &mut dyn FnMut(usize, f64) -> bool) -> bool {
    let min_x = t.iter().map(|v| v[0]).fold(f64::MAX, f64::min);
    let max_x = t.iter().map(|v| v[0]).fold(f64::MIN, f64::max);
    let min_y = t.iter().map(|v| v[1]).fold(f64::MAX, f64::min);
    let max_y = t.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
    let (x0, x1) = ((min_x - 0.5).ceil().max(0.0) as i64, (max_x - 0.5).floor().min(f64::from(SIZE.0) - 1.0) as i64);
    let (y0, y1) = ((min_y - 0.5).ceil().max(0.0) as i64, (max_y - 0.5).floor().min(f64::from(SIZE.1) - 1.0) as i64);
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
                if f((y as usize) * SIZE.0 as usize + x as usize, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// Self-check of the rasterizer on a toy scene: a near and a far triangle covering the same pixels; only the near one is
/// visible, and a triangle wholly behind the camera is neither drawn nor visible.
fn self_check() {
    let near = 0.1;
    let big = |w: f64| [[-w, -w, w], [3.0 * w, -w, w], [-w, 3.0 * w, w]];
    let tris = [big(10.0), big(5.0), [[0.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 1.0, -1.0]]];
    let mut z = vec![0.0f64; (SIZE.0 * SIZE.1) as usize];
    for t in &tris {
        raster(*t, near, &mut |i, d| {
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
            raster(*t, near, &mut |i, d| {
                vis |= d == z[i];
                vis
            });
            vis
        })
        .collect();
    assert_eq!(visible, vec![false, true, false], "rasterizer self-check");
}

fn hidden(label: &str, field: &dyn HeightField, cells: u32, level: u8, view: &str) {
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), field);
    let cam = views.iter().find(|(n, _)| *n == view).expect("view").1;
    let p = lod(field, cells, level);
    let req = TerrainRequest {
        camera: cam,
        size: SIZE,
        view: TerrainView::Level,
        lod: p.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    };
    let plan = plan_terrain_cached(&req, field, &mut SelectionCache::new()).expect("plan");
    let t = Instant::now();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = plan.nodes.len().div_ceil(threads).max(1);
    let projected: Vec<(Vec<[f64; 3]>, bool)> = std::thread::scope(|s| {
        let hs: Vec<_> = plan
            .nodes
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|d| project_tile(&cam, d, cells, field)).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().expect("thread")).collect()
    });
    let tight_kept = projected.iter().filter(|p| p.1).count();
    let proj: Vec<Vec<[f64; 3]>> = projected.iter().map(|p| p.0.clone()).collect();
    let keep: Vec<bool> = projected.iter().map(|p| p.1).collect();
    drop(projected);
    let mesh_s = t.elapsed().as_secs_f64();
    let idx = grid_indices(cells);
    let near = cam.near_m;
    let t = Instant::now();
    let mut z = vec![0.0f64; (SIZE.0 * SIZE.1) as usize];
    let tri_of = |v: &Vec<[f64; 3]>, k: usize| [v[idx[3 * k] as usize], v[idx[3 * k + 1] as usize], v[idx[3 * k + 2] as usize]];
    let ntri = idx.len() / 3;
    for v in &proj {
        for k in 0..ntri {
            raster(tri_of(v, k), near, &mut |i, d| {
                if d > z[i] {
                    z[i] = d;
                }
                false
            });
        }
    }
    let (mut total, mut visible, mut tiles_visible, mut wrongly_culled) = (0u64, 0u64, 0usize, 0usize);
    for (v, kept) in proj.iter().zip(&keep) {
        let mut any = false;
        for k in 0..ntri {
            total += 1;
            let mut vis = false;
            raster(tri_of(v, k), near, &mut |i, d| {
                vis = d == z[i];
                vis
            });
            if vis {
                visible += 1;
                any = true;
            }
        }
        tiles_visible += usize::from(any);
        wrongly_culled += usize::from(any && !kept);
    }
    let covered = z.iter().filter(|d| **d > 0.0).count();
    println!(
        "HIDDEN {label} cells {cells} L{level} {view} {}x{}: nodes {} triangles {:.2}M visible {:.2}M hidden share {:.1} % ; tiles with no visible triangle {} of {} ({:.1} %); \
         tight frustum cut (own bounding sphere) would keep {} tiles ({} with visible triangles wrongly culled); covered pixels {:.1} % ; triangles per covered pixel {:.2}, visible per covered pixel {:.2} (mesh {:.1} s, depth {:.1} s)",
        SIZE.0,
        SIZE.1,
        plan.nodes.len(),
        total as f64 / 1e6,
        visible as f64 / 1e6,
        100.0 * (total - visible) as f64 / total as f64,
        plan.nodes.len() - tiles_visible,
        plan.nodes.len(),
        100.0 * (plan.nodes.len() - tiles_visible) as f64 / plan.nodes.len() as f64,
        tight_kept,
        wrongly_culled,
        100.0 * covered as f64 / z.len() as f64,
        total as f64 / covered.max(1) as f64,
        visible as f64 / covered.max(1) as f64,
        mesh_s,
        t.elapsed().as_secs_f64()
    );
}

// ---- generation throughput ----

fn throughput(label: &str, field: &dyn HeightField) {
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), field);
    let cam = views.iter().find(|(n, _)| *n == "ground-1m").expect("view").1;
    for cells in [32u32, 64] {
        let p = lod(field, cells, 26);
        let req = TerrainRequest {
            camera: cam,
            size: SIZE,
            view: TerrainView::Level,
            lod: p.clone(),
            face_mapping: TangentWarp.id().to_string(),
            seed: field.seed(),
        };
        let plan = plan_terrain_cached(&req, field, &mut SelectionCache::new()).expect("plan");
        // 64 tiles spread over the draw set, built one after the other on one thread, mesh included (heights, normal stencil,
        // morph targets, skirts: what a tile costs to make today).
        let n = plan.nodes.len();
        let pick: Vec<_> = (0..64).map(|i| plan.nodes[i * n / 64].tile).collect();
        let t = Instant::now();
        for id in &pick {
            std::hint::black_box(tile_mesh_field(field, &TangentWarp, *id, cells, RADIUS_M));
        }
        let per_tile = t.elapsed().as_secs_f64() / 64.0;
        // Bare height samples over one tile's grid.
        let t = Instant::now();
        let mut acc = 0.0;
        for id in &pick {
            for j in 0..=cells {
                for i in 0..=cells {
                    acc += field.height(id.sample_direction(&TangentWarp, cells, i, j));
                }
            }
        }
        let samples = 64.0 * f64::from((cells + 1) * (cells + 1));
        let ns = t.elapsed().as_secs_f64() * 1e9 / samples;
        std::hint::black_box(acc);
        // The same tiles on all threads.
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let many: Vec<_> = (0..(threads * 16)).map(|i| plan.nodes[i * n / (threads * 16)].tile).collect();
        let t = Instant::now();
        std::thread::scope(|s| {
            let hs: Vec<_> = many
                .chunks(16)
                .map(|part| {
                    s.spawn(move || {
                        for id in part {
                            std::hint::black_box(tile_mesh_field(field, &TangentWarp, *id, cells, RADIUS_M));
                        }
                    })
                })
                .collect();
            for h in hs {
                h.join().expect("thread");
            }
        });
        let par = many.len() as f64 / t.elapsed().as_secs_f64();
        println!(
            "THROUGHPUT {label} cells {cells}: {:.1} ms per tile, {:.1} tiles/s on one thread, {:.0} ns per bare height sample ({:.0} samples per tile grid); {:.0} tiles/s on {threads} threads",
            per_tile * 1e3,
            1.0 / per_tile,
            ns,
            f64::from((cells + 1) * (cells + 1)),
            par
        );
    }
}

fn main() -> Result<(), String> {
    self_check();
    let dir = arg("--store").ok_or("--store DIR needed")?;
    let res: u32 = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
    let part = arg("--part").unwrap_or_else(|| "counts".into());
    let def = WorldDefinition::single_planet(RADIUS_M, arg("--seed").map_or(1, |s| s.parse().expect("seed")));
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let stored = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
    // `--mem-res N`: read an in-memory bake of N cells instead of the store (no block-cache lock: parallel throughput).
    let grid = match arg("--mem-res") {
        Some(n) => {
            let n: u32 = n.parse().map_err(|e| format!("{e}"))?;
            let g = GlobalCoarseGenerator.field(&GenContext::new(&def), BodyId::PLANET).map_err(|e| format!("{e:?}"))?;
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            Some((n, g.bake(&TangentWarp, n, threads).map_err(|e| format!("{e:?}"))?))
        }
        None => None,
    };
    let mem = grid.as_ref().map(|(_, g)| GridSource(g));
    let (src, res): (&dyn LatticeSource, u32) = match (&mem, &grid) {
        (Some(m), Some((n, _))) => (m, *n),
        _ => (&stored, res),
    };
    let key = world.stage_key(res).0;
    let ctx = GenContext::new(&def);
    let terrain = TerrainGenerator.field(&ctx, BodyId::PLANET, src, key).map_err(|e| format!("{e:?}"))?;
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).map_err(|e| format!("{e:?}"))?;
    let fields: [(&str, &dyn HeightField); 2] = [("eroded", &eroded), ("terrain", &terrain)];
    println!("residency_counts: bake {res} cells per face edge, {}x{}, tau 6 px, part {part}", SIZE.0, SIZE.1);
    for (label, f) in fields {
        match part.as_str() {
            "counts" => counts(label, f),
            "hidden" => {
                for view in ["ground-1m", "ground-200m", "low-2km", "aerial-20km"] {
                    hidden(label, f, 32, 26, view);
                }
                hidden(label, f, 64, 26, "ground-1m");
            }
            "throughput" => throughput(label, f),
            other => return Err(format!("--part counts|hidden|throughput, got {other}")),
        }
    }
    Ok(())
}
