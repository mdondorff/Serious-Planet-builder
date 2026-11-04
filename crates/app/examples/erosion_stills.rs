//! Lit stills of the erosion layer before and after (ADR 0097 point 5, m9-bake preview): the same pose and sun over the base field of the default class source and over the base plus the
//! layer's change, at 300 m and 2 km above the ground of a site. The change is read from the raw tile cores that `bake layer --export DIR` wrote, bilinear on the 101.8 m grid (outside
//! the exported tiles the change is 0). The sea is shaded as seabed (no water pass here). A dev tool: the owner looks, nothing here is a golden.
//!
//! `cargo run --release -p planet --example erosion_stills -- --export DIR --seed 14 --out DIR --site NAME=FACE,TI,TJ[,AZIMUTH_DEG] ...` (`PLANET_ADAPTER=hardware` for the discrete GPU).

use std::collections::HashMap;
use std::path::PathBuf;

use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::{Face, PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::materials::{MaterialParams, NATURAL_STRENGTH};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain_cached, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileGrid;
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::HeightField;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1600, 900);
const CELLS: u32 = 32;
const FINEST: u8 = 24;
const EV: f32 = 14.5;
const FACE_CELLS: usize = 98_304;
const TILE: usize = 3072;

/// The default class source's base, with the layer's change added when `tiles` holds any.
struct Field<'a> {
    inputs: &'a BakeInputs,
    tiles: Option<&'a HashMap<(u8, usize, usize), Vec<f32>>>,
    seed: u64,
    /// Reads the change with Catmull-Rom (C1) instead of bilinear (C0).
    c1: bool,
    /// The change is faded to 0 within this many metres above sea level (0: not faded).
    fade_sea_m: f64,
}

impl Field<'_> {
    /// The change at `d`, bilinear between the cell centres of the stored tile that holds it (indices clamped to that tile).
    fn change(&self, d: Vec3) -> f64 {
        let Some(tiles) = self.tiles else { return 0.0 };
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let n = FACE_CELLS as f64;
        let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
        let (i, j) = (x.floor().clamp(0.0, n - 1.0) as usize, y.floor().clamp(0.0, n - 1.0) as usize);
        let Some(core) = tiles.get(&(face.0, i / TILE, j / TILE)) else { return 0.0 };
        let (i0, j0) = ((i / TILE) * TILE, (j / TILE) * TILE);
        let (fx, fy) = ((x - i as f64).clamp(0.0, 1.0), (y - j as f64).clamp(0.0, 1.0));
        let at = |a: i64, b: i64| {
            let (a, b) = (a.clamp(i0 as i64, (i0 + TILE - 1) as i64) as usize, b.clamp(j0 as i64, (j0 + TILE - 1) as i64) as usize);
            f64::from(core[(b - j0) * TILE + (a - i0)])
        };
        let (i, j) = (i as i64, j as i64);
        if self.c1 {
            let cr = |p: [f64; 4], t: f64| {
                0.5 * (2.0 * p[1]
                    + (p[2] - p[0]) * t
                    + (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t * t
                    + (3.0 * (p[1] - p[2]) + p[3] - p[0]) * t * t * t)
            };
            let row = |dj: i64| cr([at(i - 1, j + dj), at(i, j + dj), at(i + 1, j + dj), at(i + 2, j + dj)], fx);
            return cr([row(-1), row(0), row(1), row(2)], fy);
        }
        (1.0 - fy) * ((1.0 - fx) * at(i, j) + fx * at(i + 1, j)) + fy * ((1.0 - fx) * at(i, j + 1) + fx * at(i + 1, j + 1))
    }
}

impl HeightField for Field<'_> {
    fn height(&self, d: Vec3) -> f64 {
        let base = self.inputs.base(d);
        let change = self.change(d);
        if self.fade_sea_m > 0.0 && change != 0.0 {
            let u = (base / self.fade_sea_m).clamp(0.0, 1.0);
            return base + change * u * u * (3.0 - 2.0 * u);
        }
        base + change
    }
    fn seed(&self) -> u64 {
        self.seed
    }
    fn implementation_id(&self) -> &'static str {
        "erosion-stills"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        6_000.0
    }
    /// A sampled bound (a preview tool, not a contract): the heights at the centre and on two rings of the cap, widened by half their range and 200 m, so a tile's bounding volume is
    /// not the global +-6 km (which keeps the plan from culling anything near the horizon).
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let e = Vec3::new(0.0, 0.0, 1.0).cross(centre);
        let e = if e.length() < 1e-9 { Vec3::new(1.0, 0.0, 0.0).cross(centre) } else { e }.normalized();
        let n = centre.cross(e).normalized();
        let (mut lo, mut hi) = (self.height(centre), self.height(centre));
        for (ring, count) in [(1.0, 12), (0.5, 8)] {
            for k in 0..count {
                let a = f64::from(k) * core::f64::consts::TAU / f64::from(count);
                let t = e * libm::cos(a) + n * libm::sin(a);
                let r = angular_radius * ring;
                let h = self.height((centre * libm::cos(r) + t * libm::sin(r)).normalized());
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        let pad = 0.5 * (hi - lo) + 200.0;
        ((lo - pad).max(-6_000.0), (hi + pad).min(6_000.0))
    }
}

struct Site {
    name: String,
    face: u8,
    ti: usize,
    tj: usize,
    azimuth_deg: Option<f64>,
}

fn fail(m: &str) -> ! {
    eprintln!("erosion_stills: {m}");
    std::process::exit(2)
}

fn main() {
    let (mut export, mut seed, mut out, mut sites) =
        (None::<PathBuf>, 14u64, PathBuf::from("target/review/erosion-preview"), Vec::<Site>::new());
    let (mut c1, mut detail, mut alts) = (false, NATURAL_STRENGTH, vec!["300m".to_string(), "2km".to_string()]);
    let mut skip_before = false;
    let mut fade_sea = 0.0f64;
    let mut k_one = false;
    let mut sun_deg = 30.0f64;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--export" => export = Some(PathBuf::from(value())),
            "--read" => c1 = value() == "c1",
            "--detail" => detail = value().parse().unwrap_or_else(|_| fail("--detail needs a number")),
            "--alts" => alts = value().split(',').map(str::to_string).collect(),
            "--skip-before" => skip_before = true,
            "--fade-sea" => fade_sea = value().parse().unwrap_or_else(|_| fail("--fade-sea needs metres")),
            // SPIKE-99: render with k = 1 in the base; the pose is still searched on the as-built base without the layer, so every base is seen from the same camera.
            "--k-one" => k_one = true,
            // SPIKE-101: the sun's elevation in degrees (default 30).
            "--sun-deg" => sun_deg = value().parse().unwrap_or_else(|_| fail("--sun-deg")),
            // SPIKE-101 candidates (spike branch only).
            "--band-scale" => planet_generators::bake::experiment::set_hm_band_scale(value().parse().unwrap_or_else(|_| fail("--band-scale"))),
            "--ridged" => planet_generators::bake::experiment::set_hm_ridged(true),
            "--centre" => planet_generators::bake::experiment::set_hm_centre(value().parse().unwrap_or_else(|_| fail("--centre"))),
            "--mid-amp" => planet_generators::bake::experiment::set_hm_mid_amp(value().parse().unwrap_or_else(|_| fail("--mid-amp"))),
            "--seed" => seed = value().parse().unwrap_or_else(|_| fail("--seed needs an integer")),
            "--out" => out = PathBuf::from(value()),
            "--site" => {
                let v = value();
                let (name, rest) = v.split_once('=').unwrap_or_else(|| fail("--site NAME=FACE,TI,TJ[,AZIMUTH_DEG]"));
                let p: Vec<&str> = rest.split(',').collect();
                if p.len() < 3 {
                    fail("--site NAME=FACE,TI,TJ[,AZIMUTH_DEG]");
                }
                let n = |s: &str| s.trim().parse::<usize>().unwrap_or_else(|_| fail("--site takes integers"));
                sites.push(Site {
                    name: name.into(),
                    face: n(p[0]) as u8,
                    ti: n(p[1]),
                    tj: n(p[2]),
                    azimuth_deg: p.get(3).map(|s| s.trim().parse().unwrap_or_else(|_| fail("azimuth"))),
                });
            }
            other => fail(&format!("unknown option '{other}'")),
        }
    }
    let export = export.unwrap_or_else(|| fail("--export DIR (from `bake layer --export`) is required"));
    if sites.is_empty() {
        fail("at least one --site is required");
    }
    let mut tiles: HashMap<(u8, usize, usize), Vec<f32>> = HashMap::new();
    for e in std::fs::read_dir(&export).unwrap_or_else(|e| fail(&format!("{}: {e}", export.display()))).flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_prefix("tile-").and_then(|s| s.strip_suffix(".f32")) else { continue };
        let p: Vec<usize> = stem.split('-').filter_map(|s| s.parse().ok()).collect();
        if p.len() != 3 {
            continue;
        }
        let raw = std::fs::read(e.path()).unwrap_or_else(|e| fail(&e.to_string()));
        if raw.len() != TILE * TILE * 4 {
            fail(&format!("{name}: {} bytes, expected {}", raw.len(), TILE * TILE * 4));
        }
        tiles.insert((p[0] as u8, p[1], p[2]), raw.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect());
    }
    println!("{} tile cores read from {}", tiles.len(), export.display());
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&e.to_string()));
    let inputs = BakeInputs::new(seed, planet_streaming::topology::bake_threads(0).map_or(4, |t| t.0));
    let before = Field { inputs: &inputs, tiles: None, seed, c1, fade_sea_m: 0.0 };
    let after = Field { inputs: &inputs, tiles: Some(&tiles), seed, c1, fade_sea_m: fade_sea };
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let grid = TileGrid { cells: FACE_CELLS, tile: TILE, halo: 256, radius_m: R };
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let look = LookParams::default();
    for site in &sites {
        let at_cell = |a: usize, b: usize, n: usize| {
            grid.direction(
                Face(site.face),
                (site.ti * TILE + a * TILE / n + TILE / (2 * n)) as i64,
                (site.tj * TILE + b * TILE / n + TILE / (2 * n)) as i64,
            )
        };
        let frame = |d: Vec3| {
            let e = Vec3::new(0.0, 0.0, 1.0).cross(d).normalized();
            (e, d.cross(e).normalized())
        };
        let cardinals = |d: Vec3| {
            let (e, n) = frame(d);
            [e, n, e * -1.0, n * -1.0]
        };
        planet_generators::bake::experiment::set_k_one(false);
        let mut centre = at_cell(1, 1, 2);
        let mut sea_heading: Option<Vec3> = None;
        if site.name.starts_with("coast") {
            // A coast: for a land lattice point (3 to 400 m up) with sea (below -5 m) at the next lattice point along one of the four cardinal directions, bisect to the coastline and stand
            // 800 m inland of it, looking at the sea; the one nearest to the tile's centre.
            let mut best = f64::MAX;
            let step = f64::from(TILE as u32) / 48.0 * grid.dx();
            let (mut land_pts, mut sea_pts, mut pairs) = (0, 0, 0);
            for a in 0..48 {
                for b in 0..48 {
                    let d = at_cell(a, b, 48);
                    if before.height(d) < -5.0 {
                        sea_pts += 1;
                    }
                    if !(0.5..400.0).contains(&before.height(d)) {
                        continue;
                    }
                    land_pts += 1;
                    for v in cardinals(d) {
                        let far = (d * R + v * step).normalized();
                        if before.height(far) >= 0.0 {
                            continue;
                        }
                        pairs += 1;
                        let (mut lo, mut hi) = (0.0, step);
                        for _ in 0..14 {
                            let mid = 0.5 * (lo + hi);
                            if before.height((d * R + v * mid).normalized()) > 0.0 {
                                lo = mid;
                            } else {
                                hi = mid;
                            }
                        }
                        let stand = (d * R + v * (lo - 800.0).max(0.0)).normalized();
                        let off = (stand - at_cell(24, 24, 48)).length();
                        if off < best {
                            best = off;
                            centre = stand;
                            sea_heading = Some(v);
                        }
                    }
                }
            }
            eprintln!("coast search: {land_pts} land points, {sea_pts} sea points, {pairs} land-sea pairs, step {step:.0} m");
            if sea_heading.is_none() {
                fail(&format!("{}: no land point with the sea 2.5 km away in the tile", site.name));
            }
        } else {
            // Relief: the lattice point on ground over 50 m with the largest height range over the eight points 3 km around it.
            let mut best = f64::MIN;
            for a in 0..24 {
                for b in 0..24 {
                    let d = at_cell(a, b, 24);
                    let h0 = before.height(d);
                    if h0 < 50.0 {
                        continue;
                    }
                    let (e, n) = frame(d);
                    let hs: Vec<f64> = (0..8)
                        .map(|k| {
                            let t = f64::from(k) * core::f64::consts::FRAC_PI_4;
                            before.height((d * R + (e * libm::cos(t) + n * libm::sin(t)) * 3000.0).normalized())
                        })
                        .collect();
                    let range = hs.iter().fold(h0, |m, h| m.max(*h)) - hs.iter().fold(h0, |m, h| m.min(*h));
                    if range > best {
                        best = range;
                        centre = d;
                    }
                }
            }
        }
        let (east, north) = {
            let e = Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized();
            (e, centre.cross(e).normalized())
        };
        let at = |ahead: Vec3, m: f64| (centre * R + ahead * m).normalized();
        // The heading: the azimuth given, else the one of eight that looks at the highest ground 1 to 5 km ahead (ground of the layer, so both stills face the same way).
        let heading = match site.azimuth_deg {
            _ if sea_heading.is_some() => sea_heading.expect("checked"),
            Some(a) => east * libm::sin(a.to_radians()) + north * libm::cos(a.to_radians()),
            None => {
                (0..8)
                    .map(|k| {
                        let a = f64::from(k) * 45f64.to_radians();
                        let h = east * libm::sin(a) + north * libm::cos(a);
                        let mean = (1..=4).map(|s| before.height(at(h, f64::from(s) * 1250.0))).sum::<f64>() / 4.0;
                        (mean, h)
                    })
                    .fold((f64::MIN, north), |b, c| if c.0 > b.0 { c } else { b })
                    .1
            }
        };
        let pose_ground = before.height(centre);
        planet_generators::bake::experiment::set_k_one(k_one);
        // The sun 30 degrees up, from behind the camera's right shoulder.
        let right = heading.cross(centre).normalized() * -1.0;
        let sun = (centre * libm::sin(sun_deg.to_radians()) + (right * 0.6 - heading * 0.8).normalized() * libm::cos(sun_deg.to_radians()))
            .normalized();
        println!(
            "{}: face {} tile {}/{}, ground {pose_ground:.0} m (as built), heading {:.0} deg, k = 1: {k_one}",
            site.name,
            site.face,
            site.ti,
            site.tj,
            libm::atan2(heading.dot(east), heading.dot(north)).to_degrees()
        );
        for (alt_name, eye, pitch_deg) in
            [("300m", 300.0, 12.0), ("2km", 2000.0, 22.0), ("10km", 10000.0, 30.0)].into_iter().filter(|a| alts.iter().any(|x| x == a.0))
        {
            for (variant, field) in [("before", &before), ("after", &after)].into_iter().filter(|v| !(skip_before && v.0 == "before")) {
                // The eye stands `eye` above this field's own ground (an uplifted base raises the ground).
                let ground = field.height(centre);
                let camera = Camera::at_surface_point(
                    PlanetFixed(centre * (R + ground + eye)),
                    heading,
                    f64::to_radians(pitch_deg),
                    60f64.to_radians(),
                    aspect,
                    1.0,
                );
                let lod = LodParams {
                    cells: CELLS,
                    viewport_h_px: f64::from(SIZE.1),
                    tau_px: 2.5,
                    max_level: FINEST,
                    max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m() + 10.0),
                    ..LodParams::earth_1080p()
                };
                eprintln!("planning {} {alt_name} {variant}", site.name);
                let plan = plan_terrain_cached(
                    &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed },
                    field,
                    &mut SelectionCache::new(),
                )
                .expect("plan");
                eprintln!("{} nodes", plan.nodes.len());
                if plan.nodes.len() > 20_000 {
                    fail(&format!("{} {alt_name} {variant}: {} nodes in the plan: too many for the pool", site.name, plan.nodes.len()));
                }
                let block_slots = (plan.nodes.len() as u32 + 64).max(1024);
                let pool = compact_format_pool_with((256 << 20) + 4 * u64::from(block_slots) * slot_bytes(CELLS), CELLS, block_slots, 1)
                    .expect("pool");
                let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, pool);
                r.enable_lit(&ctx);
                r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
                for n in &plan.nodes {
                    if r.slot_of(n.tile).is_none() {
                        r.upload_tile(&ctx, &compact_tile_field(field, &TangentWarp, n.tile, CELLS, R)).expect("upload");
                    }
                }
                let mut lit = LitPlan::fixed(EV).with_atmosphere(&look, R).expect("an atmosphere plan");
                lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
                lit.materials = MaterialParams::EARTH;
                lit.ground_detail = detail;
                let mut p = plan.clone();
                p.lit = Some(lit);
                for _ in 0..2 {
                    r.render(&ctx, &p, false).expect("render");
                }
                let file = out.join(format!("{}_{alt_name}_{variant}.png", site.name));
                r.read_color(&ctx).write_png(&file).expect("png");
                println!("wrote {} ({} nodes)", file.display(), plan.nodes.len());
            }
        }
    }
}
