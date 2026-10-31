//! `generate --plan-check DIR --global-res N`: plan the standard flight at production parameters (cells 32, viewport / tau 240:
//! ADR 0016) over a stored global bake, with the terrain field and the eroded field, and report every view's node count and
//! deepest level. The M3 acceptance check that realistic terrain is not rejected for steepness at production resolution
//! (ADR 0028 point 6, ADR 0030): the tier A tests plan on a 256-cell bake only.

use std::io::Write;
use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::height::HeightField;
use planet_generators::terrain::TerrainGenerator;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::{err, Options};

const OTHER_COMMANDS: [&str; 16] = [
    "--tile",
    "--out",
    "--face-net",
    "--manifest",
    "--smoke",
    "--cache-dir",
    "--verify-cache",
    "--region",
    "--res",
    "--sample",
    "--allow-stale",
    "--global-dir",
    "--verify-global",
    "--global-products",
    "--compact-cache",
    "--cap-bytes",
];

pub fn plan_check(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.plan_check.as_ref().expect("checked by the caller");
    let clash: Vec<&str> = OTHER_COMMANDS.into_iter().chain(["--threads"]).filter(|f| o.given.contains(*f)).collect();
    if !clash.is_empty() {
        return Err(format!("--plan-check takes only --global-res and --world or --seed, not {}", clash.join(", ")));
    }
    if !o.given.contains("--global-res") {
        return Err("--plan-check needs --global-res N, the resolution the bake was made at (a store holds one bake per resolution)".into());
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such store directory (check the path; plan-check reads a bake made by --global-dir)", dir.display()));
    }
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let store = planet_cache::pack::TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
    let src = StoreSource::new(&store, &world, o.global_res).map_err(|e| e.to_string())?;
    let bake_key = world.stage_key(o.global_res).0;
    let radius = GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("{e:?}"))?;
    let terrain = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).map_err(|e| format!("{e:?}"))?;
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).map_err(|e| format!("{e:?}"))?;
    writeln!(out, "plan-check: {} cells per face edge, cells 32, viewport / tau 240, standard flight, {}", o.global_res, dir.display())
        .map_err(err)?;
    let mut failures = 0usize;
    let mut check = |label: &str, field: &dyn HeightField, error: &dyn Fn() -> Option<String>, out: &mut dyn Write| -> Result<(), String> {
        let params = LodParams {
            radius_m: radius,
            max_height_m: field.max_abs_height_m(),
            cells: 32,
            viewport_h_px: 1080.0,
            tau_px: 4.5,
            ..LodParams::earth_1080p()
        };
        // The scripted cameras sit above the terrain: a bake that cannot be read gives NaN heights, so say so before building them.
        let _ = field.height(planet_core::Vec3::new(0.3, 0.5, 0.8).normalized());
        if let Some(e) = error() {
            failures += 1;
            return writeln!(out, "  {label}: the bake could not be read: {e}").map_err(err);
        }
        for (name, cam) in scripted_views_field(radius, 16.0 / 9.0, field) {
            let req = TerrainRequest {
                camera: cam,
                size: (192, 108),
                view: TerrainView::Level,
                lod: params.clone(),
                face_mapping: TangentWarp.id().to_string(),
                seed: field.seed(),
            };
            let t = Instant::now();
            match plan_terrain_field(&req, field) {
                Ok(plan) => writeln!(
                    out,
                    "  {label} {name}: {} nodes, deepest level {}, {:.1} s",
                    plan.nodes.len(),
                    plan.nodes.iter().map(|n| n.level).max().unwrap_or(0),
                    t.elapsed().as_secs_f64()
                )
                .map_err(err)?,
                Err(e) => {
                    failures += 1;
                    writeln!(out, "  {label} {name}: REJECTED: {e}").map_err(err)?;
                }
            }
        }
        if let Some(e) = error() {
            failures += 1;
            writeln!(out, "  {label}: the bake could not be read: {e}").map_err(err)?;
        }
        Ok(())
    };
    check("terrain", &terrain, &|| terrain.error(), out)?;
    check("erosion", &eroded, &|| eroded.error(), out)?;
    if failures > 0 {
        return Err(format!("plan-check: {failures} view(s) rejected or unreadable"));
    }
    writeln!(out, "plan-check: both fields plan every view without steepness rejection").map_err(err)
}

#[cfg(test)]
mod spike82 {
    //! SPIKE-82 (spike code, never merged): the LOD engine on the steeper combined field (SPIKE-80/81). `Steep` wraps the eroded field as the baked
    //! channels of the combined generator would act: `b + w Delta + (1 + w (k - 1)) (e - b) + w hb(k) p`, with `b` the bake, `e` the eroded field,
    //! `Delta` and `k` from SPIKE-81's 0.25-degree fields (bilinear), `p` today's 2-8 km ridged pattern, `w` the coast taper on `b`. Bounds per cap by
    //! interval arithmetic from the bake's and the field's own cap bounds and the cell ranges of `Delta` and `k`: a point function with per-cap
    //! bounds, as ADR 0088's baked channels would be (SPIKE-66 showed a runtime stencil cannot plan).
    use super::*;
    use planet_core::{PlanetFixed, TileId, Vec3};
    use planet_frame::camera::Camera;
    use planet_generators::erosion::ErosionField;

    const FW: usize = 1440;
    const FH: usize = 720;

    struct Steep<'a> {
        inner: &'a ErosionField<'a>,
        f: Vec<f32>,
        nch: usize,
        seed: u64,
        max_abs: f64,
        hb_on: bool,
    }

    fn smooth(x: f64, a: f64, b: f64) -> f64 {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
    fn hb_amp(k: f64) -> f64 {
        30.0 + 145.0 * smooth(k, 1.0, 5.0) + 650.0 * smooth(k, 5.0, 7.5)
    }

    impl Steep<'_> {
        fn cell(&self, d: Vec3) -> (f64, f64) {
            let lat = libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees();
            let lon = libm::atan2(d.y, d.x).to_degrees();
            ((90.0 - lat) / 0.25 - 0.5, (lon + 180.0) / 0.25 - 0.5)
        }
        fn at(&self, y: usize, x: usize, ch: usize) -> f64 {
            f64::from(self.f[(y.min(FH - 1) * FW + x % FW) * self.nch + ch])
        }
        fn look(&self, d: Vec3, ch: usize) -> f64 {
            let (fy, fx) = self.cell(d);
            let fy = fy.clamp(0.0, (FH - 2) as f64);
            let (y0, ty) = (fy as usize, fy - fy.floor());
            let xf = fx.floor();
            let tx = fx - xf;
            let x0 = (xf as i64).rem_euclid(FW as i64) as usize;
            (self.at(y0, x0, ch) * (1.0 - tx) + self.at(y0, x0 + 1, ch) * tx) * (1.0 - ty)
                + (self.at(y0 + 1, x0, ch) * (1.0 - tx) + self.at(y0 + 1, x0 + 1, ch) * tx) * ty
        }
        /// Bounds of channel `ch` over a cap: the least and greatest value of the cells the cap touches (bilinear values lie between them),
        /// intersected with the centre value +- G r, G the bilinear surface's largest gradient over those cells (per radian; a cell is 0.25 degree
        /// in latitude and 0.25 cos(lat) in longitude).
        fn range(&self, centre: Vec3, r: f64, ch: usize) -> (f64, f64) {
            let (fy, fx) = self.cell(centre);
            let lat = libm::asin(centre.z.clamp(-1.0, 1.0));
            let cos_hi = libm::cos(lat.abs() + r).max(0.05);
            let rd = r.to_degrees() / 0.25 + 1e-9;
            let rx = rd / cos_hi;
            if rd > 40.0 || rx > 200.0 {
                return (f64::NEG_INFINITY, f64::INFINITY);
            }
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for y in ((fy - rd).floor().max(0.0) as i64)..=(((fy + rd).floor() as i64) + 1).min(FH as i64 - 1) {
                for x in ((fx - rx).floor() as i64)..=(((fx + rx).floor() as i64) + 1) {
                    let v = self.at(y as usize, x.rem_euclid(FW as i64) as usize, ch);
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            let cell = 0.25f64.to_radians() * cos_hi;
            let g = core::f64::consts::SQRT_2 * (hi - lo) / cell;
            let v = self.look(centre, ch);
            ((v - g * r).max(lo), (v + g * r).min(hi))
        }
    }

    impl HeightField for Steep<'_> {
        fn height(&self, d: Vec3) -> f64 {
            let (b, e) = (self.inner.base_elevation(d), self.inner.height(d));
            let w = smooth(b, 0.0, 400.0);
            let k = self.look(d, 1);
            let hb = if self.hb_on { w * hb_amp(k) * planet_generators::global::spike80_hill_pattern(self.seed, d) } else { 0.0 };
            b + w * self.look(d, 0) + (1.0 + w * (k - 1.0)) * (e - b) + hb
        }
        fn seed(&self) -> u64 {
            self.seed
        }
        fn implementation_id(&self) -> &'static str {
            "spike82-steep"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            self.max_abs
        }
        fn bounds_monotone(&self) -> bool {
            true
        }
        fn height_bounds(&self, c: Vec3, r: f64) -> (f64, f64) {
            let global = (-self.max_abs, self.max_abs);
            let Some((blo, bhi)) = self.inner.spike82_base_bounds(c, r) else {
                return global;
            };
            let (elo, ehi) = self.inner.height_bounds(c, r);
            let (dlo, dhi) = (elo - bhi, ehi - blo);
            let (klo, khi) = self.range(c, r, 1);
            let (zlo, zhi) = self.range(c, r, 0);
            if !(klo.is_finite() && zlo.is_finite() && dlo.is_finite() && dhi.is_finite()) {
                return global;
            }
            let (wlo, whi) = (smooth(blo, 0.0, 400.0), smooth(bhi, 0.0, 400.0));
            let mul = |a: (f64, f64), b: (f64, f64)| {
                let p = [a.0 * b.0, a.0 * b.1, a.1 * b.0, a.1 * b.1];
                (p.iter().cloned().fold(f64::INFINITY, f64::min), p.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
            };
            let wz = mul((wlo, whi), (zlo, zhi));
            let km1 = mul((wlo, whi), (klo - 1.0, khi - 1.0));
            let kd = mul((1.0 + km1.0, 1.0 + km1.1), (dlo, dhi));
            let hb = if self.hb_on {
                let (plo, phi) = planet_generators::global::spike82_hill_pattern_bounds(self.seed, c, r);
                (wlo * hb_amp(klo) * plo, whi * hb_amp(khi) * phi)
            } else {
                (0.0, 0.0)
            };
            ((blo + wz.0 + kd.0 + hb.0).max(-self.max_abs), (bhi + wz.1 + kd.1 + hb.1).min(self.max_abs))
        }
    }

    /// Near-exact bounds for a lever test (not a guaranteed bound): the range of 17 x 17 samples over the cap's tangent square, widened by 5 %
    /// of the range and 1 m. What a min/max pyramid of the baked layer would give the planner.
    struct Tight<'a> {
        inner: &'a dyn HeightField,
    }
    impl HeightField for Tight<'_> {
        fn height(&self, d: Vec3) -> f64 {
            self.inner.height(d)
        }
        fn seed(&self) -> u64 {
            self.inner.seed()
        }
        fn implementation_id(&self) -> &'static str {
            "spike82-tight"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            self.inner.max_abs_height_m()
        }
        fn height_bounds(&self, c: Vec3, r: f64) -> (f64, f64) {
            if r > 0.02 {
                return self.inner.height_bounds(c, r);
            }
            let e = Vec3::new(0.0, 0.0, 1.0).cross(c).normalized();
            let n = c.cross(e).normalized();
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for j in 0..17 {
                for i in 0..17 {
                    let (x, y) = ((i as f64 / 8.0 - 1.0) * r, (j as f64 / 8.0 - 1.0) * r);
                    let h = self.inner.height((c + e * x + n * y).normalized());
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
            let m = 0.05 * (hi - lo) + 1.0;
            (lo - m, hi + m)
        }
    }

    /// `S82_FIELDS=FILE S82_NCH=4 S82_SITES=name:lat:lon,... cargo test -p planet --release --lib spike82_lod -- --ignored --nocapture`; reads
    /// `C:/DEV/SeriousPlanetData/global16384` (read only). Per site, for today's eroded field and the steep field (with and without the class
    /// hill band): the plan at 200 m and 2 km above the ground (pitch 12 and 20 degrees, heading north) at production parameters (`earth_1080p`:
    /// cells 32, tau 6, finest 22): nodes, capped leaves, deepest level; then per level 10-22 the step a tile shows when it splits (p99, max) and
    /// the bound against the sampled range of the centre tile.
    #[test]
    #[ignore = "a spike measurement"]
    fn spike82_lod() {
        let def = WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 1);
        let world = GlobalWorld::from_definition(&def).unwrap();
        let store = planet_cache::pack::TileStore::open(std::path::Path::new("C:/DEV/SeriousPlanetData/global16384"), world.params()).unwrap();
        let src = StoreSource::new(&store, &world, 16384).unwrap();
        let bake_key = world.stage_key(16384).0;
        let radius = GenContext::new(&def).body_radius_m(BodyId::PLANET).unwrap();
        let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).unwrap();
        let nch: usize = std::env::var("S82_NCH").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        let f: Vec<f32> = std::fs::read(std::env::var("S82_FIELDS").unwrap())
            .unwrap()
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        assert_eq!(f.len(), FW * FH * nch);
        let detail = (eroded.max_abs_height_m() - 9000.0).abs().max(1000.0);
        let max_abs = 9000.0 + 900.0 + 7.5 * detail;
        let steep = Steep { inner: &eroded, f: f.clone(), nch, seed: eroded.seed(), max_abs, hb_on: true };
        let steep_nohb = Steep { inner: &eroded, f, nch, seed: eroded.seed(), max_abs, hb_on: false };
        println!("S82 seed {} | eroded max |h| {:.0} m | steep max |h| {:.0} m", eroded.seed(), eroded.max_abs_height_m(), max_abs);
        let map = &TangentWarp;
        for site in std::env::var("S82_SITES").unwrap().split(',') {
            let v: Vec<&str> = site.split(':').collect();
            let (name, lat, lon): (&str, f64, f64) = (v[0], v[1].parse().unwrap(), v[2].parse().unwrap());
            let (tt, ts) = (Tight { inner: &eroded }, Tight { inner: &steep });
            let fields: [(&str, &dyn HeightField); 5] =
                [("today", &eroded), ("steep-nohb", &steep_nohb), ("steep", &steep), ("today-tight", &tt), ("steep-tight", &ts)];
            for (label, field) in fields {
                let d = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
                for (alt, pitch) in [(200.0, 12.0f64), (2000.0, 20.0)] {
                    let pos = PlanetFixed(d * (radius + field.height(d) + alt));
                    let cam = Camera::at_surface_point(pos, Vec3::new(0.0, 0.0, 1.0), pitch.to_radians(), 60f64.to_radians(), 16.0 / 9.0, 0.1);
                    let base = LodParams::earth_1080p();
                    let env = |k: &str, v: f64| std::env::var(k).ok().and_then(|x| x.parse().ok()).unwrap_or(v);
                    let lod = LodParams {
                        radius_m: radius,
                        max_height_m: field.max_abs_height_m(),
                        tau_px: env("S82_TAU", base.tau_px),
                        morph_band: env("S82_MORPH", base.morph_band),
                        cells: env("S82_CELLS", f64::from(base.cells)) as u32,
                        ..base
                    };
                    let req =
                        TerrainRequest { camera: cam, size: (192, 108), view: TerrainView::Level, lod, face_mapping: TangentWarp.id().to_string(), seed: field.seed() };
                    let t = Instant::now();
                    match plan_terrain_field(&req, field) {
                        Ok(plan) => {
                            let capped = plan.nodes.iter().filter(|n| n.capped).count();
                            let mut lv = [0usize; 24];
                            for n in &plan.nodes {
                                lv[usize::from(n.level).min(23)] += 1;
                            }
                            let hist: Vec<String> = (0..24).filter(|&l| lv[l] > 0).map(|l| format!("{l}:{}", lv[l])).collect();
                            println!(
                                "S82PLAN {name} {label} {alt:.0} m | {} nodes, {capped} capped, deepest {} | {:.1} s | levels {}",
                                plan.nodes.len(),
                                plan.nodes.iter().map(|n| n.level).max().unwrap_or(0),
                                t.elapsed().as_secs_f64(),
                                hist.join(" ")
                            );
                        }
                        Err(e) => println!("S82PLAN {name} {label} {alt:.0} m | REJECTED: {e}"),
                    }
                }
                let mut line = format!("S82STEP {name} {label}:");
                for level in (10u8..=22).step_by(2) {
                    let centre = TileId::from_direction(map, d, level);
                    let mut ids = vec![centre];
                    ids.extend(centre.neighbors8());
                    let mut errs = Vec::new();
                    for id in &ids {
                        let dirs: Vec<Vec3> = (0..=64u32).flat_map(|l| (0..=64u32).map(move |k| (k, l))).map(|(k, l)| id.sample_direction(map, 64, k, l)).collect();
                        let fine: Vec<f64> = dirs.iter().map(|x| field.height(*x)).collect();
                        let at = |k: usize, l: usize| fine[l * 65 + k];
                        for l in 0..=64usize {
                            for k in 0..=64usize {
                                let (k0, l0) = ((k / 2) * 2, (l / 2) * 2);
                                let (k1, l1) = ((k0 + 2).min(64), (l0 + 2).min(64));
                                let (fx, fy) = ((k - k0) as f64 / 2.0, (l - l0) as f64 / 2.0);
                                let coarse = at(k0, l0) * (1.0 - fx) * (1.0 - fy) + at(k1, l0) * fx * (1.0 - fy) + at(k0, l1) * (1.0 - fx) * fy + at(k1, l1) * fx * fy;
                                errs.push((at(k, l) - coarse).abs());
                            }
                        }
                    }
                    errs.sort_by(f64::total_cmp);
                    let dirs: Vec<Vec3> = (0..=64u32).flat_map(|l| (0..=64u32).map(move |k| (k, l))).map(|(k, l)| centre.sample_direction(map, 64, k, l)).collect();
                    let fine: Vec<f64> = dirs.iter().map(|x| field.height(*x)).collect();
                    let (smin, smax) = fine.iter().fold((f64::MAX, f64::MIN), |a, &h| (a.0.min(h), a.1.max(h)));
                    let (blo, bhi) = field.height_bounds(centre.center_direction(map), planet_frame::lod::angular_radius(map, centre));
                    let ok = blo <= smin + 1e-6 && bhi >= smax - 1e-6;
                    line += &format!(
                        " L{level} p99 {:.3} max {:.3} range {:.0} bound {:.1}x{};",
                        errs[errs.len() * 99 / 100],
                        errs[errs.len() - 1],
                        smax - smin,
                        (bhi - blo) / (smax - smin).max(1e-3),
                        if ok { "" } else { " MISSES" }
                    );
                }
                println!("{line}");
            }
        }
    }

    // SPIKE-83 (spike code, never merged): the bounds pyramid. A baked layer stands in for ADR 0088's erosion layer: the steep field sampled on the
    // face grid of SPIKE-61 (98,304 cells per face edge, 101.8 m) over a window round a site, read bilinearly; plus a runtime detail octave
    // (gradient noise at about 25 m, amplitude 1.5 k m) with analytic per-cap bounds. Bounds per cap: the min/max of the layer's nodes the cap's face
    // box touches, from a pyramid of 32 x 32 blocks and its coarser levels (raw nodes for boxes under 16 cells), plus the detail's bounds.
    const NF: f64 = 98_304.0;

    struct Win {
        face: u8,
        i0: i64,
        j0: i64,
        n: i64,
        nodes: Vec<f32>,
        /// (block size in cells, blocks per row, (min, max) per block); block `b` covers nodes `b B ..= (b + 1) B`.
        levels: Vec<(i64, i64, Vec<(f32, f32)>)>,
    }

    impl Win {
        fn node(&self, i: i64, j: i64) -> f32 {
            self.nodes[((j - self.j0) * (self.n + 1) + (i - self.i0)) as usize]
        }
        fn inside(&self, i0: i64, i1: i64, j0: i64, j1: i64) -> bool {
            i0 >= self.i0 && j0 >= self.j0 && i1 <= self.i0 + self.n && j1 <= self.j0 + self.n
        }
        fn range(&self, i0: i64, i1: i64, j0: i64, j1: i64) -> (f64, f64) {
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            let w = (i1 - i0).max(j1 - j0);
            if w <= 16 {
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        let v = self.node(i, j);
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            } else {
                // The finest level whose blocks are at least half the box: at most about 3 x 3 blocks.
                let (b, per, mm) = self.levels.iter().find(|(b, _, _)| *b * 2 >= w).unwrap_or(self.levels.last().unwrap());
                let (bi0, bi1) = ((i0 - self.i0) / b, ((i1 - self.i0 - 1).max(0) / b).min(per - 1));
                let (bj0, bj1) = ((j0 - self.j0) / b, ((j1 - self.j0 - 1).max(0) / b).min(per - 1));
                for bj in bj0..=bj1 {
                    for bi in bi0..=bi1 {
                        let (a, z) = mm[(bj * per + bi) as usize];
                        lo = lo.min(a);
                        hi = hi.max(z);
                    }
                }
            }
            (f64::from(lo), f64::from(hi))
        }
    }

    fn build(steep: &Steep<'_>, face: u8, ci: i64, cj: i64, n: i64) -> (Win, f64, f64) {
        let (i0, j0) = (ci - n / 2, cj - n / 2);
        let t = Instant::now();
        let rows: Vec<std::sync::Mutex<Vec<f32>>> = (0..=n).map(|_| std::sync::Mutex::new(Vec::new())).collect();
        let next = std::sync::atomic::AtomicI64::new(0);
        std::thread::scope(|sc| {
            for _ in 0..8 {
                sc.spawn(|| loop {
                    let r = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if r > n {
                        break;
                    }
                    let row: Vec<f32> = (0..=n)
                        .map(|c| {
                            let (s, tt) = (-1.0 + 2.0 * (i0 + c) as f64 / NF, -1.0 + 2.0 * (j0 + r) as f64 / NF);
                            steep.height(planet_core::cube::face_to_direction(&TangentWarp, planet_core::cube::Face(face), s, tt)) as f32
                        })
                        .collect();
                    *rows[r as usize].lock().unwrap() = row;
                });
            }
        });
        let nodes: Vec<f32> = rows.into_iter().flat_map(|m| m.into_inner().unwrap()).collect();
        let t_sample = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut levels = Vec::new();
        let mut b = 32i64;
        // Level 0 from the nodes; each coarser level from four blocks of the one below.
        let per = n / b;
        let mut mm = Vec::with_capacity((per * per) as usize);
        for bj in 0..per {
            for bi in 0..per {
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for j in bj * b..=(bj + 1) * b {
                    for i in bi * b..=(bi + 1) * b {
                        let v = nodes[(j * (n + 1) + i) as usize];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                mm.push((lo, hi));
            }
        }
        levels.push((b, per, mm));
        while levels.last().unwrap().1 >= 2 {
            let (_, per, ref prev) = *levels.last().unwrap();
            let np = per / 2;
            let mut next = Vec::with_capacity((np * np) as usize);
            for bj in 0..np {
                for bi in 0..np {
                    let q = |x: i64, y: i64| prev[(y * per + x) as usize];
                    let v = [q(2 * bi, 2 * bj), q(2 * bi + 1, 2 * bj), q(2 * bi, 2 * bj + 1), q(2 * bi + 1, 2 * bj + 1)];
                    next.push((v.iter().map(|x| x.0).fold(f32::INFINITY, f32::min), v.iter().map(|x| x.1).fold(f32::NEG_INFINITY, f32::max)));
                }
            }
            b *= 2;
            levels.push((b, np, next));
        }
        let t_pyr = t.elapsed().as_secs_f64();
        (Win { face, i0, j0, n, nodes, levels }, t_sample, t_pyr)
    }

    struct Pyr<'a> {
        steep: &'a Steep<'a>,
        wins: Vec<Win>,
        lookups: std::sync::atomic::AtomicU64,
        lookup_ns: std::sync::atomic::AtomicU64,
    }

    const DETAIL_FREQ: f64 = 6_371_000.0 / 25.0;

    impl Pyr<'_> {
        fn locate(&self, d: Vec3) -> Option<(&Win, f64, f64)> {
            let (face, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
            let (x, y) = ((s + 1.0) * 0.5 * NF, (t + 1.0) * 0.5 * NF);
            self.wins.iter().find(|w| w.face == face.0 && w.inside(x.floor() as i64, x.floor() as i64 + 1, y.floor() as i64, y.floor() as i64 + 1)).map(|w| (w, x, y))
        }
        fn detail(&self, d: Vec3) -> f64 {
            1.5 * self.steep.look(d, 1) * planet_generators::global::spike83_detail(self.steep.seed, d, DETAIL_FREQ)
        }
    }

    impl HeightField for Pyr<'_> {
        fn height(&self, d: Vec3) -> f64 {
            let base = match self.locate(d) {
                Some((w, x, y)) => {
                    let (i, j) = (x.floor() as i64, y.floor() as i64);
                    let (fx, fy) = (x - i as f64, y - j as f64);
                    let v = |a: i64, b: i64| f64::from(w.node(a, b));
                    (v(i, j) * (1.0 - fx) + v(i + 1, j) * fx) * (1.0 - fy) + (v(i, j + 1) * (1.0 - fx) + v(i + 1, j + 1) * fx) * fy
                }
                None => self.steep.height(d),
            };
            base + self.detail(d)
        }
        fn seed(&self) -> u64 {
            self.steep.seed
        }
        fn implementation_id(&self) -> &'static str {
            "spike83-pyramid"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            self.steep.max_abs + 15.0
        }
        fn bounds_monotone(&self) -> bool {
            false
        }
        fn height_bounds(&self, c: Vec3, r: f64) -> (f64, f64) {
            let t = Instant::now();
            let (nlo, nhi) = planet_generators::global::spike83_detail_bounds(self.steep.seed, c, r, DETAIL_FREQ);
            let (klo, khi) = self.steep.range(c, r, 1);
            let (klo, khi) = if klo.is_finite() { (klo, khi) } else { (0.0, 7.5) };
            let det = (1.5 * if nlo < 0.0 { khi } else { klo } * nlo, 1.5 * if nhi > 0.0 { khi } else { klo } * nhi);
            let (face, s, tt) = planet_core::cube::direction_to_face(&TangentWarp, c);
            let reach = 2.0 * r + 1e-9;
            let (x0, x1) = (((s - reach + 1.0) * 0.5 * NF).floor() as i64, ((s + reach + 1.0) * 0.5 * NF).floor() as i64 + 1);
            let (y0, y1) = (((tt - reach + 1.0) * 0.5 * NF).floor() as i64, ((tt + reach + 1.0) * 0.5 * NF).floor() as i64 + 1);
            let out = match self.wins.iter().find(|w| r <= 0.05 && s - reach > -1.0 && s + reach < 1.0 && tt - reach > -1.0 && tt + reach < 1.0 && w.face == face.0 && w.inside(x0, x1, y0, y1)) {
                Some(w) => {
                    let (lo, hi) = w.range(x0, x1, y0, y1);
                    (lo + det.0, hi + det.1)
                }
                None => {
                    let (lo, hi) = self.steep.height_bounds(c, r);
                    (lo + det.0 - 1.0, hi + det.1 + 1.0)
                }
            };
            self.lookups.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.lookup_ns.fetch_add(t.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
            out
        }
    }

    /// `S82_FIELDS=FILE S82_NCH=4 S83_SITES=name:lat:lon,... S83_N=2048 cargo test -p planet --release --lib spike83_pyramid -- --ignored --nocapture`:
    /// per site, bake a window of S83_N^2 cells of the steep field, build the pyramid, and plan at 200 m and 2 km with its bounds (production
    /// parameters); the bound against the sampled range per level; the pyramid's size, build time and lookup cost.
    #[test]
    #[ignore = "a spike measurement"]
    fn spike83_pyramid() {
        let def = WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 1);
        let world = GlobalWorld::from_definition(&def).unwrap();
        let store = planet_cache::pack::TileStore::open(std::path::Path::new("C:/DEV/SeriousPlanetData/global16384"), world.params()).unwrap();
        let src = StoreSource::new(&store, &world, 16384).unwrap();
        let bake_key = world.stage_key(16384).0;
        let radius = GenContext::new(&def).body_radius_m(BodyId::PLANET).unwrap();
        let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).unwrap();
        let nch: usize = std::env::var("S82_NCH").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        let f: Vec<f32> = std::fs::read(std::env::var("S82_FIELDS").unwrap()).unwrap().chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let detail = (eroded.max_abs_height_m() - 9000.0).abs().max(1000.0);
        let steep = Steep { inner: &eroded, f, nch, seed: eroded.seed(), max_abs: 9000.0 + 900.0 + 7.5 * detail, hb_on: true };
        let n: i64 = std::env::var("S83_N").ok().and_then(|v| v.parse().ok()).unwrap_or(2048);
        let map = &TangentWarp;
        let sites: Vec<(String, f64, f64)> = std::env::var("S83_SITES")
            .unwrap()
            .split(',')
            .map(|s| {
                let v: Vec<&str> = s.split(':').collect();
                (v[0].to_string(), v[1].parse().unwrap(), v[2].parse().unwrap())
            })
            .collect();
        let mut wins = Vec::new();
        for (name, lat, lon) in &sites {
            let d = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
            let (face, s, t) = planet_core::cube::direction_to_face(map, d);
            let (ci, cj) = (((s + 1.0) * 0.5 * NF) as i64, ((t + 1.0) * 0.5 * NF) as i64);
            let (w, ts, tp) = build(&steep, face.0, ci, cj, n);
            let blocks: usize = w.levels.iter().map(|l| l.2.len()).sum();
            println!(
                "S83BUILD {name}: face {} window {n}^2 cells ({:.0} km), layer sampled in {ts:.1} s, pyramid {} levels, {blocks} blocks ({} B as 2 x f32, {:.3} % of the nodes' f32) built in {:.3} s",
                face.0,
                n as f64 * 101.8 / 1000.0,
                w.levels.len(),
                blocks * 8,
                100.0 * (blocks * 8) as f64 / (w.nodes.len() * 4) as f64,
                tp
            );
            wins.push(w);
        }
        let pyr = Pyr { steep: &steep, wins, lookups: Default::default(), lookup_ns: Default::default() };
        for (name, lat, lon) in &sites {
            let d = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
            for (alt, pitch) in [(200.0, 12.0f64), (2000.0, 20.0)] {
                let pos = PlanetFixed(d * (radius + pyr.height(d) + alt));
                let cam = Camera::at_surface_point(pos, Vec3::new(0.0, 0.0, 1.0), pitch.to_radians(), 60f64.to_radians(), 16.0 / 9.0, 0.1);
                let lod = LodParams { radius_m: radius, max_height_m: pyr.max_abs_height_m(), ..LodParams::earth_1080p() };
                let req = TerrainRequest { camera: cam, size: (192, 108), view: TerrainView::Level, lod, face_mapping: TangentWarp.id().to_string(), seed: pyr.seed() };
                let (l0, n0) = (pyr.lookups.load(std::sync::atomic::Ordering::Relaxed), pyr.lookup_ns.load(std::sync::atomic::Ordering::Relaxed));
                let t = Instant::now();
                match plan_terrain_field(&req, &pyr) {
                    Ok(plan) => {
                        let dl = pyr.lookups.load(std::sync::atomic::Ordering::Relaxed) - l0;
                        let dn = pyr.lookup_ns.load(std::sync::atomic::Ordering::Relaxed) - n0;
                        println!(
                            "S83PLAN {name} pyramid {alt:.0} m | {} nodes, {} capped, deepest {} | {:.2} s | {dl} bounds lookups, {:.2} us each",
                            plan.nodes.len(),
                            plan.nodes.iter().filter(|x| x.capped).count(),
                            plan.nodes.iter().map(|x| x.level).max().unwrap_or(0),
                            t.elapsed().as_secs_f64(),
                            dn as f64 / 1000.0 / dl.max(1) as f64
                        );
                    }
                    Err(e) => println!("S83PLAN {name} pyramid {alt:.0} m | REJECTED: {e}"),
                }
            }
            let mut line = format!("S83BOUND {name}:");
            for level in (10u8..=22).step_by(2) {
                let centre = TileId::from_direction(map, d, level);
                let dirs: Vec<Vec3> = (0..=64u32).flat_map(|l| (0..=64u32).map(move |k| (k, l))).map(|(k, l)| centre.sample_direction(map, 64, k, l)).collect();
                let fine: Vec<f64> = dirs.iter().map(|x| pyr.height(*x)).collect();
                let (smin, smax) = fine.iter().fold((f64::MAX, f64::MIN), |a, &h| (a.0.min(h), a.1.max(h)));
                let (blo, bhi) = pyr.height_bounds(centre.center_direction(map), planet_frame::lod::angular_radius(map, centre));
                let ok = blo <= smin + 1e-6 && bhi >= smax - 1e-6;
                line += &format!(" L{level} range {:.1} bound {:.1}x{};", smax - smin, (bhi - blo) / (smax - smin).max(1e-3), if ok { "" } else { " MISSES" });
            }
            println!("{line}");
        }
    }
}
