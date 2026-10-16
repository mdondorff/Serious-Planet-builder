//! SPIKE-07 measurement helper (docs/archive/M4/spikes.md): the TEST-013 flight with the finest geometry level capped at 22, 23 and 26, each
//! with and without the shader detail term (`planet_render::spike_detail`, visual only). Per frame it plans every cap with the
//! production planner, renders both variants, and writes PNGs, a CSV of node counts and image differences, and (once, at the
//! end) the mesh's deviation from the pure field at the finest tiles of the last frames.
//!
//! Not production code: `cargo run --release -p planet --features spike07 --example finest_level -- [--adapter hardware]
//! [--frames 240] [--size 1280x720] [--cells 32] [--global-res 1024] [--caps 26,23,22] --out DIR [--every N]`.

#![allow(clippy::type_complexity, clippy::disallowed_methods)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use planet::worldfield::WorldTerrain;
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{TileId, Vec3};
use planet_frame::{flight_path_field, plan_terrain_field, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_generators::mesh::{tile_mesh_field, TileMesh};
use planet_render::spike_detail::DetailRenderer;
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
/// Most meshes a renderer keeps resident before it is rebuilt (as the product flight does).
const RESIDENT_CAP: usize = 12_000;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn par_map<T: Send>(tiles: &[TileId], f: &(dyn Fn(TileId) -> T + Sync)) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Mutex<Vec<Option<T>>> = Mutex::new((0..tiles.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= tiles.len() {
                    break;
                }
                let v = f(tiles[k]);
                out.lock().expect("lock")[k] = Some(v);
            });
        }
    });
    out.into_inner().expect("lock").into_iter().map(|v| v.expect("filled")).collect()
}

/// Largest channel difference, mean channel difference (of 255), pixels differing by more than 8 and by more than 32.
fn diff(a: &RgbaFrame, b: &RgbaFrame) -> (u8, f64, usize, usize) {
    let (mut max, mut sum, mut over8, mut over32) = (0u8, 0u64, 0usize, 0usize);
    for (p, q) in a.rgba.chunks(4).zip(b.rgba.chunks(4)) {
        let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().expect("3");
        max = max.max(d);
        sum += u64::from(d);
        over8 += usize::from(d > 8);
        over32 += usize::from(d > 32);
    }
    (max, sum as f64 / (a.rgba.len() / 4) as f64, over8, over32)
}

/// Vertex spacing in metres of a level at `cells` cells per tile edge (the tile edge is a quarter of a great circle on the cube
/// face's middle, shrinking towards the edges; the nominal value is the middle one).
fn spacing_m(level: u8, cells: u32) -> f64 {
    RADIUS_M * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level) / f64::from(cells)
}

/// Deviation of the mesh from the pure field: for every cell of a tile, the field's height at the midpoint of each diagonal
/// against the mean of the diagonal's two vertex heights (what the linear mesh gives there). Returns the largest over both
/// diagonals of all cells, and the mean.
fn mesh_deviation(field: &dyn HeightField, mesh: &TileMesh, cells: u32) -> (f64, f64) {
    let w = (cells + 1) as usize;
    let dir = |i: u32, j: u32| mesh.id.sample_direction(&TangentWarp, cells, i, j);
    let (mut worst, mut sum, mut n) = (0f64, 0f64, 0usize);
    for j in 0..cells {
        for i in 0..cells {
            let h = |a: u32, b: u32| f64::from(mesh.vertices[b as usize * w + a as usize].height);
            for ((a0, b0), (a1, b1)) in [((i, j), (i + 1, j + 1)), ((i + 1, j), (i, j + 1))] {
                let mid: Vec3 = (dir(a0, b0) + dir(a1, b1)).normalized();
                let e = (field.height(mid) - 0.5 * (h(a0, b0) + h(a1, b1))).abs();
                worst = worst.max(e);
                sum += e;
                n += 1;
            }
        }
    }
    (worst, sum / n as f64)
}

struct Case {
    cap: u8,
    renderer: DetailRenderer,
}

fn main() -> Result<(), String> {
    let cells: u32 = arg("--cells").map_or(32, |s| s.parse().expect("cells"));
    let frames: usize = arg("--frames").map_or(240, |s| s.parse().expect("frames"));
    let every: usize = arg("--every").map_or(1, |s| s.parse().expect("every"));
    let global_res: u32 = arg("--global-res").map_or(1024, |s| s.parse().expect("global-res"));
    let caps: Vec<u8> = arg("--caps").unwrap_or_else(|| "26,23,22".into()).split(',').map(|s| s.parse().expect("cap")).collect();
    let size: (u32, u32) = arg("--size").map_or((1280, 720), |s| {
        let (w, h) = s.split_once('x').expect("WxH");
        (w.parse().expect("w"), h.parse().expect("h"))
    });
    let out = std::path::PathBuf::from(arg("--out").ok_or("--out DIR needed")?);
    let policy = if arg("--adapter").as_deref() == Some("hardware") { AdapterPolicy::HighPerformance } else { AdapterPolicy::Software };
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    println!("{}", ctx.adapter_log_line());

    let t = Instant::now();
    let terrain = WorldTerrain::open(WorldDefinition::single_planet(RADIUS_M, 1), global_res)?;
    let field = terrain.field()?;
    println!("{} ({:.1} s to open)", terrain.describe(), t.elapsed().as_secs_f64());
    for &cap in &caps {
        println!(
            "finest level {cap}: vertex spacing {:.4} m at cells {cells} (tile edge {:.3} m)",
            spacing_m(cap, cells),
            spacing_m(cap, cells) * f64::from(cells)
        );
    }

    let cameras = flight_path_field(RADIUS_M, f64::from(size.0) / f64::from(size.1), field.as_ref(), frames);
    let new_case = |cap| Case { cap, renderer: DetailRenderer::new(&ctx, size, cells, RADIUS_M) };
    let mut cases: Vec<Case> = caps.iter().map(|&c| new_case(c)).collect();
    let reference_cap = *caps.iter().max().expect("a cap");
    let name = |cap: u8, detail: bool| format!("l{cap}{}", if detail { "-detail" } else { "" });
    for cap in &caps {
        for detail in [false, true] {
            std::fs::create_dir_all(out.join(name(*cap, detail))).map_err(|e| e.to_string())?;
        }
    }
    let mut csv = String::from("frame,altitude_m");
    for cap in &caps {
        csv.push_str(&format!(",nodes_l{cap},tris_l{cap}"));
    }
    for cap in caps.iter().filter(|c| **c != reference_cap) {
        csv.push_str(&format!(",l{cap}_vs_l{reference_cap}_max,l{cap}_vs_l{reference_cap}_mean,l{cap}_vs_l{reference_cap}_over8"));
    }
    for cap in &caps {
        csv.push_str(&format!(",detail_l{cap}_max,detail_l{cap}_mean,detail_l{cap}_over8"));
    }
    csv.push('\n');

    let mut mesh_cache: HashMap<TileId, TileMesh> = HashMap::new();
    let mut totals: BTreeMap<u8, (usize, usize, u64)> = BTreeMap::new(); // cap -> (max nodes, last-frame nodes, sum nodes)
    let mut last_plans: Vec<(u8, TerrainPlan)> = Vec::new();
    let started = Instant::now();
    for (i, camera) in cameras.iter().enumerate() {
        let mut plans: Vec<TerrainPlan> = Vec::new();
        for c in &cases {
            let lod = LodParams {
                cells,
                viewport_h_px: f64::from(size.1),
                tau_px: 6.0,
                max_level: c.cap,
                radius_m: RADIUS_M,
                max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
                ..LodParams::earth_1080p()
            };
            let req = TerrainRequest {
                camera: *camera,
                size,
                view: TerrainView::Relief,
                lod,
                face_mapping: TangentWarp.id().to_string(),
                seed: field.seed(),
            };
            plans.push(plan_terrain_field(&req, field.as_ref()).map_err(|e| format!("frame {i} cap {}: {e}", c.cap))?);
        }
        // Rebuild a renderer that would exceed the resident cap, then generate what any renderer misses (once per tile).
        for (c, plan) in cases.iter_mut().zip(&plans) {
            let missing = plan.nodes.iter().filter(|n| !c.renderer.has_mesh(n.tile, n.mesh_key)).count();
            if c.renderer.resident_tiles() > 0 && c.renderer.resident_tiles() + missing > RESIDENT_CAP {
                c.renderer = DetailRenderer::new(&ctx, size, cells, RADIUS_M);
            }
        }
        let mut need: Vec<TileId> = Vec::new();
        let mut seen = HashSet::new();
        for (c, plan) in cases.iter().zip(&plans) {
            for n in &plan.nodes {
                if !c.renderer.has_mesh(n.tile, n.mesh_key) && !mesh_cache.contains_key(&n.tile) && seen.insert(n.tile) {
                    need.push(n.tile);
                }
            }
        }
        for m in par_map(&need, &|t| tile_mesh_field(field.as_ref(), &TangentWarp, t, cells, RADIUS_M)) {
            mesh_cache.insert(m.id, m);
        }
        for (c, plan) in cases.iter_mut().zip(&plans) {
            for n in &plan.nodes {
                if !c.renderer.has_mesh(n.tile, n.mesh_key) {
                    c.renderer.upload(&ctx, &mesh_cache[&n.tile]).map_err(|e| e.to_string())?;
                }
            }
        }
        let wanted: HashSet<TileId> = plans.iter().flat_map(|p| p.nodes.iter().map(|n| n.tile)).collect();
        mesh_cache.retain(|t, _| wanted.contains(t));

        // Render: every case plain and with the term.
        let p = camera.position.0;
        let altitude = p.length() - RADIUS_M - field.height(p.normalized());
        let mut frames_out: BTreeMap<(u8, bool), RgbaFrame> = BTreeMap::new();
        for (c, plan) in cases.iter_mut().zip(&plans) {
            for detail in [false, true] {
                c.renderer.render(&ctx, plan, detail).map_err(|e| format!("frame {i}: {e}"))?;
                frames_out.insert((c.cap, detail), c.renderer.read_color(&ctx));
            }
        }
        let tris = |p: &TerrainPlan| p.nodes.len() as u64 * (cells as u64 * cells as u64 * 2);
        let mut line = format!("{i:04},{altitude:.3}");
        for (c, plan) in cases.iter().zip(&plans) {
            line.push_str(&format!(",{},{}", plan.nodes.len(), tris(plan)));
            let e = totals.entry(c.cap).or_default();
            e.0 = e.0.max(plan.nodes.len());
            e.1 = plan.nodes.len();
            e.2 += plan.nodes.len() as u64;
        }
        let reference = &frames_out[&(reference_cap, false)];
        for cap in caps.iter().filter(|c| **c != reference_cap) {
            let (max, mean, o8, _) = diff(&frames_out[&(*cap, false)], reference);
            line.push_str(&format!(",{max},{mean:.4},{o8}"));
        }
        for cap in &caps {
            let (max, mean, o8, _) = diff(&frames_out[&(*cap, true)], &frames_out[&(*cap, false)]);
            line.push_str(&format!(",{max},{mean:.4},{o8}"));
        }
        csv.push_str(&line);
        csv.push('\n');
        if i % every == 0 || i + 1 == frames {
            for ((cap, detail), img) in &frames_out {
                let path = out.join(name(*cap, *detail)).join(format!("frame_{i:04}.png"));
                img.write_png(&path).map_err(|e| format!("writing {}: {e}", path.display()))?;
            }
        }
        if i % 20 == 0 {
            println!(
                "frame {i}/{frames}: altitude {altitude:.2} m, nodes {:?}, {:.0} s",
                plans.iter().map(|p| p.nodes.len()).collect::<Vec<_>>(),
                started.elapsed().as_secs_f64()
            );
        }
        if i + 1 == frames {
            last_plans = cases.iter().map(|c| c.cap).zip(plans).collect();
        }
    }
    std::fs::write(out.join("flight.csv"), &csv).map_err(|e| e.to_string())?;

    println!("NODES per case over the {frames}-frame flight (cells {cells}, tau 6 px, {}x{}): max per frame / last frame (1 m) / sum over frames", size.0, size.1);
    for (cap, (max, last, sum)) in &totals {
        println!("  finest {cap}: {max} / {last} / {sum}");
    }

    // Mesh deviation from the pure field at the finest tiles of the last frame (up to 400 per case, evenly spread).
    println!("MESH DEVIATION from height(dir) in the last frame, finest tiles of each case (metres; both diagonals of every cell):");
    for (cap, plan) in &last_plans {
        let mut tiles: Vec<TileId> = plan.nodes.iter().filter(|n| n.level == *cap).map(|n| n.tile).collect();
        tiles.sort_by_key(|t| t.raw());
        let step = (tiles.len() / 400).max(1);
        let sample: Vec<TileId> = tiles.iter().step_by(step).copied().collect();
        let devs = par_map(&sample, &|t| {
            let m = tile_mesh_field(field.as_ref(), &TangentWarp, t, cells, RADIUS_M);
            mesh_deviation(field.as_ref(), &m, cells)
        });
        let worst = devs.iter().map(|d| d.0).fold(0.0, f64::max);
        let mean = devs.iter().map(|d| d.1).sum::<f64>() / devs.len().max(1) as f64;
        println!(
            "  finest {cap}: {} of {} finest tiles sampled, worst {worst:.3e} m, mean {mean:.3e} m, spacing {:.4} m",
            sample.len(),
            tiles.len(),
            spacing_m(*cap, cells)
        );
    }
    std::io::stdout().flush().ok();
    Ok(())
}
