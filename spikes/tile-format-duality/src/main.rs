//! CPU-only measurements for the tile-format duality exploration (opus-planner's questions 2 and 3). Throwaway.
//! `tile-format-duality error`   base-surface model error of the compact format by level and number of control points per side
//! `tile-format-duality levels`  which levels the planner draws in the standard views
//! `tile-format-duality steps`   height quantisation step of the compact tile at the coarse levels

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_generators::mesh::tile_mesh_field;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;

type V = [f64; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V, k: f64) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V) -> V {
    scale(a, 1.0 / dot(a, a).sqrt())
}
fn len(a: V) -> f64 {
    dot(a, a).sqrt()
}

/// Lagrange basis on `n` equispaced nodes at t in [0, 1]: values and derivatives.
fn lagrange(n: usize, t: f64) -> (Vec<f64>, Vec<f64>) {
    let nodes: Vec<f64> = (0..n).map(|k| k as f64 / (n - 1) as f64).collect();
    let mut l = vec![1.0; n];
    let mut d = vec![0.0; n];
    for k in 0..n {
        for m in 0..n {
            if m != k {
                l[k] *= (t - nodes[m]) / (nodes[k] - nodes[m]);
            }
        }
        for j in 0..n {
            if j == k {
                continue;
            }
            let mut p = 1.0 / (nodes[k] - nodes[j]);
            for m in 0..n {
                if m != k && m != j {
                    p *= (t - nodes[m]) / (nodes[k] - nodes[m]);
                }
            }
            d[k] += p;
        }
    }
    (l, d)
}

struct Model {
    n: usize,
    ctrl: Vec<V>,
    flip: bool,
}

impl Model {
    fn new(n: usize, id: TileId, origin: V, shell: f64) -> Model {
        let step = CELLS as usize / (n - 1);
        let mut ctrl = Vec::new();
        for b in 0..n {
            for a in 0..n {
                let d = id.sample_direction(&TangentWarp, CELLS, (a * step) as u32, (b * step) as u32);
                ctrl.push(sub(scale([d.x, d.y, d.z], shell), origin));
            }
        }
        let mut m = Model { n, ctrl, flip: false };
        let (_, du, dv) = m.eval(0.5, 0.5);
        let c = norm(add(origin, [0.0; 3]));
        m.flip = dot(cross(du, dv), c) < 0.0;
        m
    }

    fn eval(&self, u: f64, v: f64) -> (V, V, V) {
        let (lu, du) = lagrange(self.n, u);
        let (lv, dv) = lagrange(self.n, v);
        let (mut p, mut pu, mut pv) = ([0.0; 3], [0.0; 3], [0.0; 3]);
        for b in 0..self.n {
            for a in 0..self.n {
                let c = self.ctrl[b * self.n + a];
                p = add(p, scale(c, lu[a] * lv[b]));
                pu = add(pu, scale(c, du[a] * lv[b]));
                pv = add(pv, scale(c, lu[a] * dv[b]));
            }
        }
        (p, pu, pv)
    }

    fn position(&self, u: f64, v: f64, hr: f64) -> V {
        let (p, pu, pv) = self.eval(u, v);
        let mut n = norm(cross(pu, pv));
        if self.flip {
            n = scale(n, -1.0);
        }
        add(p, scale(n, hr))
    }
}

fn sample_tiles(level: u8) -> Vec<TileId> {
    let side = 1u32 << level;
    let mut out = Vec::new();
    let per_face = (side * side) as usize;
    let want = 6usize.max(24);
    let stride = (per_face * 6).div_ceil(want).max(1);
    let mut k = 0usize;
    for f in 0..6u8 {
        for y in 0..side {
            for x in 0..side {
                if k % stride == 0 || level == 0 {
                    out.push(TileId::new(Face(f), level, x, y).unwrap());
                }
                k += 1;
            }
        }
    }
    out
}

fn error_table() {
    let field = Reference(1);
    println!("| level | tile edge | metres per pixel (edge/190) | 3x3 (now) m / px | 5x5 m / px | 9x9 m / px | 17x17 m / px |");
    println!("|---|---|---|---|---|---|---|");
    for level in 0..=8u8 {
        let edge = std::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << level);
        let pix = edge / 190.0;
        let mut worst = [0.0f64; 4];
        for id in sample_tiles(level) {
            let mesh = tile_mesh_field(&field, &TangentWarp, id, CELLS, R);
            let tile = compact_tile_field(&field, &TangentWarp, id, CELLS, R);
            let origin = [mesh.origin.0.x, mesh.origin.0.y, mesh.origin.0.z];
            let shell = R + f64::from(tile.h_origin);
            for (mi, n) in [3usize, 5, 9, 17].into_iter().enumerate() {
                let model = Model::new(n, id, origin, shell);
                let w = CELLS as usize + 1;
                for j in 0..w {
                    for i in 0..w {
                        let v = &mesh.vertices[j * w + i];
                        let hr = f64::from(v.height) - f64::from(tile.h_origin);
                        let p = model.position(i as f64 / CELLS as f64, j as f64 / CELLS as f64, hr);
                        let e = len(sub(p, [f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])]));
                        worst[mi] = worst[mi].max(e);
                    }
                }
            }
        }
        let cell = |e: f64| format!("{e:.3e} / {:.2e}", e / pix);
        println!("| {level} | {edge:.3e} m | {pix:.3e} | {} | {} | {} | {} |", cell(worst[0]), cell(worst[1]), cell(worst[2]), cell(worst[3]));
    }
}

fn levels_table() {
    println!("| view | nodes | levels 0-2 | level 3 | level 4+ | min level | max level |");
    println!("|---|---|---|---|---|---|---|");
    let (mut tot, mut coarse) = (0usize, 0usize);
    for (name, camera) in scripted_views(R, 16.0 / 9.0, 1) {
        let lod = LodParams { cells: CELLS, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 22, ..LodParams::earth_1080p() };
        let plan = plan_terrain(&TerrainRequest { camera, size: (2560, 1440), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
        let lv: Vec<u8> = plan.nodes.iter().map(|n| n.tile.level()).collect();
        let c = lv.iter().filter(|l| **l <= 2).count();
        let l3 = lv.iter().filter(|l| **l == 3).count();
        tot += lv.len();
        coarse += c;
        println!("| {name} | {} | {c} | {l3} | {} | {} | {} |", lv.len(), lv.len() - c - l3, lv.iter().min().unwrap(), lv.iter().max().unwrap());
    }
    println!("\nover the standard views: {coarse} of {tot} drawn nodes are at levels 0-2");
}

fn steps_table() {
    let field = Reference(1);
    println!("| level | tile edge | relief of the worst sampled tile (m) | height step range/65535 (m) | pixel at that level (m, edge/190) |");
    println!("|---|---|---|---|---|");
    for level in 0..=4u8 {
        let edge = std::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << level);
        let (mut relief, mut step) = (0.0f64, 0.0f64);
        for id in sample_tiles(level) {
            let t = compact_tile_field(&field, &TangentWarp, id, CELLS, R);
            relief = relief.max(t.relief_m);
            step = step.max(f64::from(t.hr_step));
        }
        println!("| {level} | {edge:.3e} m | {relief:.0} | {step:.3e} | {:.3e} |", edge / 190.0);
    }
}

/// Largest angle between a unit vector and its octahedral encoding with `bits` per component (snorm, round to nearest).
fn oct_error_deg(bits: u32) -> f64 {
    let levels = f64::from((1u32 << bits) - 1);
    let q = |e: f64| ((e * 0.5 + 0.5) * levels).round() / levels * 2.0 - 1.0;
    let mut worst = 0.0f64;
    let mut state = 0x1234_5678_9abc_def0u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    for k in 0..2_000_000u32 {
        let n = if k < 6 {
            [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]][k as usize]
        } else {
            norm([next() * 2.0 - 1.0, next() * 2.0 - 1.0, next() * 2.0 - 1.0])
        };
        let s = n[0].abs() + n[1].abs() + n[2].abs();
        let (mut x, mut y) = (n[0] / s, n[1] / s);
        if n[2] < 0.0 {
            let (ox, oy) = (x, y);
            x = (1.0 - oy.abs()) * if ox >= 0.0 { 1.0 } else { -1.0 };
            y = (1.0 - ox.abs()) * if oy >= 0.0 { 1.0 } else { -1.0 };
        }
        let (x, y) = (q(x), q(y));
        let mut d = [x, y, 1.0 - x.abs() - y.abs()];
        if d[2] < 0.0 {
            let (ox, oy) = (d[0], d[1]);
            d[0] = (1.0 - oy.abs()) * if ox >= 0.0 { 1.0 } else { -1.0 };
            d[1] = (1.0 - ox.abs()) * if oy >= 0.0 { 1.0 } else { -1.0 };
        }
        let d = norm(d);
        worst = worst.max(dot(n, d).clamp(-1.0, 1.0).acos().to_degrees());
    }
    worst
}

fn normals_table() {
    println!("| bits per component | worst angle (degrees), 2M random + the 6 axes |");
    println!("|---|---|");
    for b in [16u32, 15, 14, 13, 12, 8] {
        println!("| {b} | {:.5} |", oct_error_deg(b));
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("normals") => normals_table(),
        Some("error") => error_table(),
        Some("levels") => levels_table(),
        Some("steps") => steps_table(),
        _ => eprintln!("usage: error | levels | steps"),
    }
}
