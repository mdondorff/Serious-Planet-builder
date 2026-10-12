//! SPIKE-04: three erosion filter variants over the global bake, rendered in the relief view. Throwaway.
//!
//! Every variant is a pure function of the direction: bake elevation (and the bake's slope) plus detail. Run:
//! `cargo run --release -p erosion-variants` (writes target/review/erosion-spike/).

use std::path::Path;
use std::time::Instant;

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::global::{bake, read_direction, GridSource};
use planet_generators::height::{layer_noise_with_gradient, HeightField};
use planet_generators::mesh::{tile_mesh_field, TileMesh};
use planet_generators::terrain::TerrainGenerator;
use planet_render::{execute_terrain, AdapterPolicy, GpuContext, RgbaFrame};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const BAKE_RES: u32 = 512;
const SIZE: (u32, u32) = (640, 360);
const LAYER: u64 = 0x4552_0001;

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The bake as a base elevation with a slope estimate (central differences over `cells` bake cells).
struct Base<'a> {
    src: &'a GridSource<'a>,
}

impl Base<'_> {
    fn elevation(&self, d: Vec3) -> f64 {
        f64::from(read_direction(self.src, &TangentWarp, d).expect("bake").elevation_m)
    }

    fn basis(d: Vec3) -> (Vec3, Vec3) {
        let a = if d.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
        let e1 = d.cross(a).normalized();
        (e1, d.cross(e1))
    }

    /// Gradient of the elevation in metres per radian (3D, tangent to the sphere).
    fn gradient(&self, d: Vec3, cells: f64) -> Vec3 {
        let delta = cells * core::f64::consts::FRAC_PI_2 / f64::from(BAKE_RES);
        let (e1, e2) = Base::basis(d);
        let at = |v: Vec3| self.elevation(v.normalized());
        let g1 = (at(d + e1 * delta) - at(d + e1 * -delta)) / (2.0 * delta);
        let g2 = (at(d + e2 * delta) - at(d + e2 * -delta)) / (2.0 * delta);
        e1 * g1 + e2 * g2
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// A: fBm whose octaves are damped by the gradient accumulated so far (base slope included).
    Damped,
    /// B: ridged creases incised deeper where the bake is steep.
    Incised,
    /// C: B with the lattice displaced by smooth vector noise (meandering creases).
    Flow,
}

struct Variant<'a> {
    kind: Kind,
    base: Base<'a>,
    /// A bound on |gradient| of the height in metres per radian (measured, see `measure_lipschitz`; spike only).
    lip: f64,
    name: &'static str,
}

const OCTAVES: u32 = 12;
const F0: f64 = 256.0;

impl Variant<'_> {
    fn detail(&self, d: Vec3, elev: f64) -> f64 {
        let land = smoothstep(-200.0, 300.0, elev);
        match self.kind {
            Kind::Damped => {
                let a0 = 100.0 + 800.0 * land;
                let g_base = self.base.gradient(d, 2.0);
                let mut g_acc = g_base;
                let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
                let mut f = F0;
                let mut weights = Vec::with_capacity(OCTAVES as usize);
                for o in 0..OCTAVES {
                    let (n, g) = layer_noise_with_gradient(SEED, LAYER, o, d * f);
                    let w = 1.0 / (1.0 + (g_acc.length() / 1.27e6).powi(2));
                    weights.push((n, g, amp, f, w));
                    norm += amp;
                    let step = amp * a0 * f * w;
                    g_acc = g_acc + Vec3::new(g[0], g[1], g[2]) * (step / norm.max(1e-9));
                    sum += w * amp * n;
                    amp *= 0.55;
                    f *= 2.0;
                }
                a0 * sum / norm
            }
            Kind::Incised | Kind::Flow => {
                let slope = self.base.gradient(d, 3.0).length() / R;
                let carve = 0.15 + 0.85 * smoothstep(0.01, 0.2, slope);
                let a0 = (80.0 + 900.0 * land) * carve;
                let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, F0);
                for o in 0..OCTAVES {
                    let mut p = d * f;
                    if self.kind == Kind::Flow {
                        // Meander: displace the lattice position by a smooth vector noise of 0.8 cells (rivers, not straight lines).
                        let q = p * 0.35;
                        let w = Vec3::new(
                            planet_generators::height::layer_noise(SEED, LAYER + 1, o, q),
                            planet_generators::height::layer_noise(SEED, LAYER + 2, o, q),
                            planet_generators::height::layer_noise(SEED, LAYER + 3, o, q),
                        );
                        p = p + w * 0.8;
                    }
                    let (n, _) = layer_noise_with_gradient(SEED, LAYER, o, p);
                    let v = (1.0 - n.abs()).powi(3);
                    sum += amp * (0.25 - v);
                    norm += amp;
                    amp *= 0.55;
                    f *= 2.0;
                }
                a0 * sum / norm
            }
        }
    }
}

impl HeightField for Variant<'_> {
    fn height(&self, d: Vec3) -> f64 {
        let e = self.base.elevation(d);
        e + self.detail(d, e)
    }
    fn seed(&self) -> u64 {
        SEED
    }
    fn implementation_id(&self) -> &'static str {
        self.name
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        9000.0 + 1000.0
    }
    /// Lipschitz bounds about the centre value with the measured constant. NOT a sound bound (the constant is measured, not
    /// proved); good enough to answer whether the plan survives the steepness check.
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let h = self.height(centre);
        let r = self.lip * angular_radius + 1e-3 + h.abs() * 1.2e-7;
        ((h - r).max(-self.max_abs_height_m()), (h + r).min(self.max_abs_height_m()))
    }
}

/// Largest |h(d + eps t) - h(d)| / eps over random points and directions, times a safety factor of 3.
fn measure_lipschitz(f: &dyn HeightField) -> f64 {
    let mut rng = SplitMix64(99);
    let mut worst: f64 = 0.0;
    for _ in 0..1500 {
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let (e1, e2) = Base::basis(d);
        for eps in [3e-7, 3e-6, 3e-5, 3e-4] {
            let a = rng.range(0.0, core::f64::consts::TAU);
            let t = e1 * libm::cos(a) + e2 * libm::sin(a);
            let d2 = (d + t * eps).normalized();
            worst = worst.max((f.height(d2) - f.height(d)).abs() / eps);
        }
    }
    3.0 * worst
}

fn ns_per_sample(f: &dyn HeightField) -> f64 {
    let mut rng = SplitMix64(5);
    let dirs: Vec<Vec3> =
        (0..60_000).map(|_| Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized()).collect();
    let t = Instant::now();
    let mut acc = 0.0;
    for d in &dirs {
        acc += f.height(*d);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / dirs.len() as f64;
    assert!(acc.is_finite());
    ns
}

/// Camera positions found in the bake: a coast with relief on both sides, the highest peak, and the foot of the peak.
fn find_places(base: &Base<'_>) -> Vec<(&'static str, Vec3, f64, f64, f64)> {
    let mut best_peak = (f64::MIN, Vec3::new(1.0, 0.0, 0.0));
    let mut best_coast = (0.0, Vec3::new(1.0, 0.0, 0.0));
    let n = 360;
    let mut rng = SplitMix64(3);
    for _ in 0..200_000 {
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let e = base.elevation(d);
        if e > best_peak.0 && d.z.abs() < 0.8 {
            best_peak = (e, d);
        }
        if (-30.0..80.0).contains(&e) && d.z.abs() < 0.7 {
            // Land on one side and sea on the other, within about 100 km, and land that rises: a coast with relief.
            let (e1, e2) = Base::basis(d);
            let ring: Vec<f64> = (0..8)
                .map(|k| {
                    let a = f64::from(k) * core::f64::consts::FRAC_PI_4;
                    base.elevation((d + (e1 * libm::cos(a) + e2 * libm::sin(a)) * 0.012).normalized())
                })
                .collect();
            let (lo, hi) = (ring.iter().copied().fold(f64::MAX, f64::min), ring.iter().copied().fold(f64::MIN, f64::max));
            if lo < -100.0 && hi > 150.0 && hi - lo > best_coast.0 {
                best_coast = (hi - lo, d);
            }
        }
    }
    let _ = n;
    eprintln!("peak {:.0} m at {:?}; coast relief {:.0} m at {:?}", best_peak.0, best_peak.1, best_coast.0, best_coast.1);
    // (name, direction, height above ground, pitch in degrees, offset south in radians)
    vec![
        ("coast-1500m", best_coast.1, 1500.0, 35.0, 0.0),
        ("mountain-2km", best_peak.1, 2000.0, 30.0, 0.0015),
        ("ground-1m", best_peak.1, 1.0, 5.0, 0.0012),
        ("hills-400m", best_peak.1, 400.0, 30.0, 0.003),
    ]
}

fn camera_at(f: &dyn HeightField, d: Vec3, h: f64, pitch_deg: f64, south: f64) -> Camera {
    let (e1, e2) = Base::basis(d);
    let _ = e1;
    // Offset towards the south (-z projected on the sphere) so the camera looks north at the place.
    let north = Vec3::new(0.0, 0.0, 1.0);
    let t = (north - d * north.dot(d)).normalized();
    let _ = e2;
    let p = (d + t * -south).normalized();
    let pos = PlanetFixed(p * (R + f.height(p) + h));
    Camera::at_surface_point(pos, north, pitch_deg.to_radians(), 60f64.to_radians(), f64::from(SIZE.0) / f64::from(SIZE.1), 0.1)
}

fn render(ctx: &GpuContext, f: &dyn HeightField, cam: Camera, lod: &LodParams) -> Result<(RgbaFrame, usize), String> {
    let req = TerrainRequest {
        camera: cam,
        size: SIZE,
        view: TerrainView::Relief,
        lod: lod.clone(),
        face_mapping: "tangent-v1".into(),
        seed: SEED,
    };
    let plan = plan_terrain_field(&req, f).map_err(|e| e.to_string())?;
    let nodes: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let meshes: Vec<TileMesh> = std::thread::scope(|s| {
        let hs: Vec<_> = nodes
            .chunks(nodes.len().div_ceil(threads).max(1))
            .map(|part| s.spawn(move || part.iter().map(|t| tile_mesh_field(f, &TangentWarp, *t, lod.cells, R)).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let refs: Vec<&TileMesh> = meshes.iter().collect();
    let (frame, _) = execute_terrain(ctx, &plan, &refs).map_err(|e| format!("{e:?}"))?;
    Ok((frame, nodes.len()))
}

/// Top-down hillshade of a square patch of `size_m` metres around `c` (vertical exaggeration `exag`), so the erosion character
/// shows independently of the relief shader's mild lighting.
fn hillshade(f: &dyn HeightField, c: Vec3, size_m: f64, exag: f64, n: u32) -> RgbaFrame {
    let (e1, e2) = Base::basis(c);
    let step = size_m / f64::from(n);
    let at = |x: f64, y: f64| f.height((c + (e1 * x + e2 * y) * (1.0 / R)).normalized());
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut hs = vec![0.0f64; (n * n) as usize];
    for j in 0..n {
        for i in 0..n {
            let h = at((f64::from(i) - f64::from(n) / 2.0) * step, (f64::from(j) - f64::from(n) / 2.0) * step);
            hs[(j * n + i) as usize] = h;
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    let sun = Vec3::new(-0.6, 0.6, 0.55).normalized();
    let mut rgba = vec![255u8; (n * n * 4) as usize];
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let h = |di: i32, dj: i32| hs[((j as i32 + dj) as u32 * n + (i as i32 + di) as u32) as usize];
            let dx = (h(1, 0) - h(-1, 0)) / (2.0 * step) * exag;
            let dy = (h(0, 1) - h(0, -1)) / (2.0 * step) * exag;
            let nrm = Vec3::new(-dx, -dy, 1.0).normalized();
            let l = (0.25 + 0.75 * nrm.dot(sun).max(0.0)).min(1.0);
            let t = if hi > lo { ((h(0, 0) - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
            let tint = [0.30 + 0.55 * t, 0.42 + 0.40 * t, 0.25 + 0.55 * t];
            let o = ((j * n + i) * 4) as usize;
            for k in 0..3 {
                rgba[o + k] = (255.0 * tint[k] * l).clamp(0.0, 255.0) as u8;
            }
        }
    }
    RgbaFrame { width: n, height: n, rgba }
}

fn main() {
    let out = Path::new("target").join("review").join("erosion-spike");
    std::fs::create_dir_all(&out).unwrap();
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    let grid = bake(seed, &TangentWarp, BAKE_RES, 4).unwrap();
    let src = GridSource(&grid);
    let ctx_gpu = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("{}", ctx_gpu.adapter_log_line());

    let baseline = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
    let mk = |kind, name| Variant { kind, base: Base { src: &src }, lip: 0.0, name };
    let mut variants: Vec<Variant<'_>> = vec![mk(Kind::Damped, "A-damped"), mk(Kind::Incised, "B-incised"), mk(Kind::Flow, "C-meander")];
    for v in &mut variants {
        v.lip = measure_lipschitz(v);
    }
    let base_lip = measure_lipschitz(&baseline);
    let mut fields: Vec<(&str, &dyn HeightField, f64)> = vec![("0-baseline", &baseline, base_lip)];
    for v in &variants {
        fields.push((v.name, v, v.lip));
    }
    let places = find_places(&Base { src: &src });
    // Production steepness parameters: cells 32, viewport / tau = 240 (viewport 360 px, tau 1.5 px).
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: f64::from(SIZE.1) / 240.0,
        max_height_m: 10_000.0,
        ..LodParams::earth_1080p()
    };
    assert_eq!(lod.viewport_h_px / lod.tau_px, 240.0);

    let mut sheet = vec![0u8; (SIZE.0 * 4 * SIZE.1 * places.len() as u32 * fields.len() as u32) as usize];
    let sheet_w = SIZE.0 * fields.len() as u32;
    let sheet_h = SIZE.1 * places.len() as u32;
    for (fi, (name, f, lip)) in fields.iter().enumerate() {
        let ns = ns_per_sample(*f);
        println!("{name}: {ns:.0} ns/sample, measured Lipschitz constant {lip:.3e} m/rad (x3 safety)");
        for (pi, (pname, d, h, pitch, south)) in places.iter().enumerate() {
            let cam = camera_at(*f, *d, *h, *pitch, *south);
            match render(&ctx_gpu, *f, cam, &lod) {
                Ok((frame, nodes)) => {
                    println!("  {pname}: plans, {nodes} nodes");
                    frame.write_png(&out.join(format!("{name}_{pname}.png"))).unwrap();
                    for y in 0..SIZE.1 {
                        let src_row = (y * SIZE.0 * 4) as usize;
                        let dst = (((pi as u32 * SIZE.1 + y) * sheet_w + fi as u32 * SIZE.0) * 4) as usize;
                        sheet[dst..dst + (SIZE.0 * 4) as usize].copy_from_slice(&frame.rgba[src_row..src_row + (SIZE.0 * 4) as usize]);
                    }
                }
                Err(e) => println!("  {pname}: DOES NOT PLAN: {e}"),
            }
        }
    }
    RgbaFrame { width: sheet_w, height: sheet_h, rgba: sheet }.write_png(&out.join("contact-sheet.png")).unwrap();
    // Second sheet: top-down hillshades (coast 40 km, mountain flank 8 km, mountain 600 m), exaggeration 3 / 3 / 1.
    let patches: Vec<(&str, Vec3, f64, f64)> = vec![
        ("coast-40km", places[0].1, 40_000.0, 3.0),
        ("mountain-8km", places[1].1, 8_000.0, 3.0),
        ("mountain-600m", places[1].1, 600.0, 1.0),
    ];
    let n = 512u32;
    let mut sheet2 = vec![0u8; (n * fields.len() as u32 * n * patches.len() as u32 * 4) as usize];
    let w2 = n * fields.len() as u32;
    for (fi, (name, f, _)) in fields.iter().enumerate() {
        for (pi, (pname, c, size, exag)) in patches.iter().enumerate() {
            let img = hillshade(*f, *c, *size, *exag, n);
            img.write_png(&out.join(format!("{name}_hillshade_{pname}.png"))).unwrap();
            for y in 0..n {
                let s = (y * n * 4) as usize;
                let d = (((pi as u32 * n + y) * w2 + fi as u32 * n) * 4) as usize;
                sheet2[d..d + (n * 4) as usize].copy_from_slice(&img.rgba[s..s + (n * 4) as usize]);
            }
        }
    }
    RgbaFrame { width: w2, height: n * patches.len() as u32, rgba: sheet2 }.write_png(&out.join("contact-sheet-hillshade.png")).unwrap();
    println!(
        "columns: {}; rows: {}",
        fields.iter().map(|f| f.0).collect::<Vec<_>>().join(", "),
        places.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")
    );
    println!("wrote {}", out.join("contact-sheet.png").display());
}
