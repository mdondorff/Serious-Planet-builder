//! SPIKE-86 (spike branch only): the window's BASE LOD over the real store (read only) against the generated world and the global-bound
//! stub: how many orbital-to-ground poses does the plan refuse, by altitude band and terrain class? Port of SPIKE-22's sweep.
//! SP86_SOURCE = store:<dir> | generated | stub; SP86_N poses; SP86_W/SP86_H window size; SP86_SEED pose seed.
use planet::worldfield::WorldTerrain;
use planet_core::{PlanetFixed, Vec3, EARTH_RADIUS_M as R};
use planet_frame::{plan_terrain_field, production_finest_level, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn main() {
    let n: usize = env("SP86_N", 3000);
    let size: (u32, u32) = (env("SP86_W", 1280), env("SP86_H", 720));
    let source = std::env::var("SP86_SOURCE").unwrap_or_else(|_| "generated".into());
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let world = match source.strip_prefix("store:") {
        Some(dir) => WorldTerrain::open_store(def, std::path::Path::new(dir), 16384).unwrap(),
        None => WorldTerrain::open(def, 256).unwrap(),
    }
    .bounding_water();
    let real = world.field().unwrap();
    let stub = Bound9735;
    let field: &dyn HeightField = if source == "stub" { &stub } else { real.as_ref() };
    // The window's LodParams (window.rs run_window): tau 6 at 1440 rows, scaled with the window height; production finest level.
    let tau = 6.0 * f64::from(size.1) / 1440.0;
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level: production_finest_level(32),
        radius_m: R,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().unwrap();
    println!(
        "source {source}, max_abs_height {:.0} m, viewport/tau {:.1}, max_level {}, {n} poses at {}x{}",
        field.max_abs_height_m(),
        lod.viewport_h_px / lod.tau_px,
        lod.max_level,
        size.0,
        size.1
    );
    let mut s: u64 = 0x9e3779b97f4a7c15 ^ env::<u64>("SP86_SEED", 0);
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let bands = [(0.0, 100.0), (100.0, 1_000.0), (1_000.0, 10_000.0), (10_000.0, 31_000.0), (31_000.0, 100_000.0), (100_000.0, 1.0e6), (1.0e6, 1.0e9)];
    let classes = ["ocean", "plain", "hills", "mountains"];
    let mut tally = vec![(0usize, 0usize); bands.len()];
    let mut by_class = vec![(0usize, 0usize); classes.len()];
    let mut low = vec![(0usize, 0usize); classes.len()]; // poses below 31 km only
    let mut levels = std::collections::BTreeMap::<String, usize>::new();
    // ADR 0074: a tile too steep to morph is capped (not refined) instead of refusing the plan; count poses with a capped leaf
    let mut capped_band = vec![(0usize, 0usize); bands.len()]; // (poses with a capped leaf, capped leaves)
    let mut capped_class = vec![(0usize, 0usize); classes.len()];
    let mut capped_levels = std::collections::BTreeMap::<u8, usize>::new();
    let mut first: Vec<String> = Vec::new();
    let t0 = std::time::Instant::now();
    for i in 0..n {
        // the same pose stream for every source: draw every random number first
        let (u_alt, side, u_pitch) = (next(), next(), next());
        // SP86_ALT=min,max: log-uniform altitude in that band (default 2 m to 30,000 km)
        let alt = match std::env::var("SP86_ALT").ok().and_then(|v| v.split_once(',').map(|(a, b)| (a.parse::<f64>().unwrap(), b.parse::<f64>().unwrap()))) {
            Some((lo, hi)) => lo * libm::pow(hi / lo, u_alt),
            None => 2.0 * libm::pow(10.0, u_alt * 7.2),
        };
        // SP86_STEEP: every pose is redrawn (up to 400 times) until its nadir is land with a bound range of 1,500 m or more over 10 km
        let want_steep = std::env::var("SP86_STEEP").is_ok();
        // SP86_LAND: every other pose is redrawn (up to 40 times) until its nadir is land, so the land classes get poses
        let want_land = std::env::var("SP86_LAND").is_ok() && i % 2 == 0;
        let (mut dir, mut a) = (Vec3::new(0.0, 0.0, 1.0), 0.0);
        for _ in 0..(if want_steep { 400 } else { 40 }) {
            let z = 2.0 * next() - 1.0;
            a = std::f64::consts::TAU * next();
            let r = libm::sqrt(1.0 - z * z);
            dir = Vec3::new(r * libm::cos(a), r * libm::sin(a), z);
            let ok = if want_steep {
                let (lo, hi) = real.height_bounds(dir, 10_000.0 / R);
                real.height(dir) > 0.0 && hi - lo >= 1500.0
            } else {
                !want_land || real.height(dir) > 0.0
            };
            if ok {
                break;
            }
        }
        // terrain class at the nadir: sea level or below is ocean; else the local range over about 10 km (the field's own bound)
        let h = real.height(dir);
        let (lo, hi) = real.height_bounds(dir, 10_000.0 / R);
        let ci = if h <= 0.0 { 0 } else if hi - lo < 300.0 { 1 } else if hi - lo < 1500.0 { 2 } else { 3 };
        let ground = if source == "stub" { 0.0 } else { h.max(0.0) };
        let heading = dir.cross(Vec3::new(libm::cos(a * 3.0), libm::sin(a * 3.0), 0.3)).normalized();
        let pitch = if side < 0.5 { 0.05 + u_pitch * 1.3 } else { -(0.05 + u_pitch * 1.3) };
        let cam = Camera::at_surface_point(PlanetFixed(dir * (R + ground + alt)), heading, pitch, 60f64.to_radians(), f64::from(size.0) / f64::from(size.1), 0.1);
        let bi = bands.iter().position(|b| alt >= b.0 && alt < b.1).unwrap();
        tally[bi].0 += 1;
        by_class[ci].0 += 1;
        if alt < 31_000.0 {
            low[ci].0 += 1;
        }
        let req = TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: field.seed() };
        let res = plan_terrain_field(&req, field);
        if let Ok(plan) = &res {
            let c: Vec<u8> = plan.nodes.iter().filter(|d| d.capped).map(|d| d.level).collect();
            if !c.is_empty() {
                capped_band[bi].0 += 1;
                capped_band[bi].1 += c.len();
                capped_class[ci].0 += 1;
                capped_class[ci].1 += c.len();
                for l in &c {
                    *capped_levels.entry(*l).or_default() += 1;
                }
                if first.len() < 4 {
                    first.push(format!("alt {alt:.0} m, class {}: {} capped leaves at levels {:?}", classes[ci], c.len(), c));
                }
                // SP86_PROBE: per capped leaf, the field's bound extent against the sampled 17 x 17 range, over the leaf and its 4 neighbours
                if std::env::var("SP86_PROBE").is_ok() {
                    use planet_core::cube::TangentWarp;
                    use planet_core::tile::Side;
                    let map = TangentWarp;
                    for d in plan.nodes.iter().filter(|d| d.capped).take(6) {
                        let mut tiles = vec![d.tile];
                        tiles.extend(Side::ALL.iter().map(|s| d.tile.neighbor(&map, *s)));
                        let (mut bmax, mut smax) = (0f64, 0f64);
                        for t in tiles {
                            let c = t.center_direction(&map);
                            let ang = (0..4).map(|k| libm::acos(c.dot(t.sample_direction(&map, 1, k % 2, k / 2)).min(1.0))).fold(0.0, f64::max);
                            let (lo, hi) = field.height_bounds(c, ang);
                            let (mut a, mut b) = (f64::MAX, f64::MIN);
                            for k in 0..17 {
                                for l in 0..17 {
                                    let h = field.height(t.sample_direction(&map, 16, k, l));
                                    a = a.min(h);
                                    b = b.max(h);
                                }
                            }
                            bmax = bmax.max(hi - lo);
                            smax = smax.max(b - a);
                        }
                        let edge = 10_007_543.0 / f64::from(1u32 << d.level);
                        println!("probe: alt {alt:.0} m, level {}, tile edge {edge:.0} m, bound extent {bmax:.1} m, sampled range {smax:.1} m, ratio {:.1}, sampled slope ~{:.0} deg", d.level, bmax / smax.max(1e-9), libm::atan(smax / (edge / 16.0) / 2.0).to_degrees());
                    }
                }
            }
        }
        if let Err(e) = res {
            tally[bi].1 += 1;
            by_class[ci].1 += 1;
            if alt < 31_000.0 {
                low[ci].1 += 1;
            }
            let m = e.to_string();
            let key = m.split("(level ").nth(1).and_then(|x| x.split(')').next()).map_or_else(|| m.chars().take(60).collect(), |l| format!("level {l}"));
            *levels.entry(key).or_default() += 1;
            if first.len() < 4 {
                first.push(format!("alt {alt:.0} m, class {}: {m}", classes[ci]));
            }
        }
        if (i + 1) % 500 == 0 {
            eprintln!("{} poses, {:.0} s", i + 1, t0.elapsed().as_secs_f64());
        }
    }
    for (b, t) in bands.iter().zip(&tally) {
        println!("altitude {:>10.0} to {:>10.0} m: {:>5} poses, {:>4} refused ({:.1} %)", b.0, b.1, t.0, t.1, 100.0 * t.1 as f64 / t.0.max(1) as f64);
    }
    for ((c, t), l) in classes.iter().zip(&by_class).zip(&low) {
        println!("class {c:>9}: {:>5} poses, {:>4} refused; below 31 km {:>5} poses, {:>4} refused", t.0, t.1, l.0, l.1);
    }
    println!("refused by level: {levels:?}");
    for (b, c) in bands.iter().zip(&capped_band) {
        println!("capped: altitude {:>10.0} to {:>10.0} m: {:>4} poses with a capped leaf, {:>5} capped leaves", b.0, b.1, c.0, c.1);
    }
    for (k, c) in classes.iter().zip(&capped_class) {
        println!("capped: class {k:>9}: {:>4} poses with a capped leaf, {:>5} capped leaves", c.0, c.1);
    }
    println!("capped leaves by level: {capped_levels:?}");
    for f in first {
        println!("{f}");
    }
    println!("wall {:.0} s", t0.elapsed().as_secs_f64());
}

/// A field with only the global bound (the default `height_bounds`): the stub of the original finding.
struct Bound9735;
impl HeightField for Bound9735 {
    fn height(&self, _: Vec3) -> f64 {
        0.0
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "bound-9735"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        9735.0
    }
}
