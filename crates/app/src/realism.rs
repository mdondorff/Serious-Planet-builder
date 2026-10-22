//! SPIKE-32 (spikes/realism): the realism capture scene. NOT production code, never merged.
//!
//! `REALISM=1 planet test-render --scene capture-flights --allow-memory-bake --global-res 512 --out-dir DIR` renders the region stills.
//! Environment: `REALISM_REGION=FILE` (the baked region of `realism_region`; without it the run is the baseline: main as it is),
//! `REALISM_PHOTO=DIR` (the CC0 sets: `rock`, `scree`, `snow`, `grass`, `soil`, each with `color.png`, `normal.png`, `rough.png`,
//! `disp.png`), `REALISM_STEPS=0,1,3,7` (the flag sets to draw: 1 shadows, 2 material map, 4 photo textures), `REALISM_SUNS=sunset,noon`,
//! `REALISM_POSES=a,b` (names from `poses`), `REALISM_TAG=text`, `REALISM_DETAIL=n` (octaves of sub-grid relief), `REALISM_TIMING=1`.

use planet_core::{PlanetFixed, Vec3};
use planet_generators::height::HeightField;
use planet_generators::spike_realism::{Baked, Params, RegionStage, Site, EXTENT_M, SKIRT_M};
use planet_render::spike_realism as rz;
use std::sync::{Arc, OnceLock};

struct Active {
    site: Site,
    baked: Arc<Baked>,
    octaves: u32,
}

static ACTIVE: OnceLock<Active> = OnceLock::new();
static SITE: OnceLock<Site> = OnceLock::new();

pub(crate) fn enabled() -> bool {
    std::env::var_os("REALISM").is_some()
}

/// Wrap a height field with the region when the run has one (called by `WorldTerrain::field`).
pub(crate) fn wrap<'a>(f: Box<dyn HeightField + 'a>) -> Box<dyn HeightField + 'a> {
    match ACTIVE.get() {
        Some(a) => Box::new(RegionStage::new(f, a.site, a.baked.clone(), a.octaves)),
        None => f,
    }
}

fn env_f(k: &str, d: f64) -> f64 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Choose the site on the world's own field (the highest land that is flat over 3 km), then install the region and the resources.
pub(crate) fn prepare(field: &dyn HeightField, radius_m: f64, level_m: f64, is_sea: &dyn Fn(Vec3) -> bool, out: &mut dyn std::io::Write) -> Result<(), String> {
    let n = 60000usize;
    let mut best: Option<(f64, Vec3, f64)> = None;
    for i in 0..n {
        let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
        let rr = (1.0 - z * z).sqrt();
        let a = std::f64::consts::PI * (3.0 - 5f64.sqrt()) * i as f64;
        let d = Vec3::new(rr * a.cos(), rr * a.sin(), z);
        let h0 = field.height(d);
        if h0 < level_m + 500.0 || h0 > level_m + 1400.0 || is_sea(d) {
            continue;
        }
        let site = Site::new(d, radius_m, h0);
        let mut worst = 0.0f64;
        for (x, y) in [(3000.0, 0.0), (-3000.0, 0.0), (0.0, 3000.0), (0.0, -3000.0), (2100.0, 2100.0), (-2100.0, -2100.0)] {
            worst = worst.max((field.height(site.dir_at(x, y)) - h0).abs());
        }
        if worst < 70.0 && best.is_none_or(|b| h0 > b.0) {
            best = Some((h0, d, worst));
        }
    }
    let (h0, d, worst) = best.ok_or("no flat high land for the region")?;
    let (lat, lon) = (d.z.asin().to_degrees(), d.y.atan2(d.x).to_degrees());
    // The floor: the lowest base height over the square's rim (the region is the base there).
    let site0 = Site::new(d, radius_m, h0);
    let mut floor = h0;
    for k in 0..16 {
        let a = k as f64 * std::f64::consts::TAU / 16.0;
        floor = floor.min(field.height(site0.dir_at(2700.0 * a.cos(), 2700.0 * a.sin())));
    }
    let site = Site::new(d, radius_m, floor);
    writeln!(out, "realism site: lat {lat:.3} lon {lon:.3}, base height {h0:.0} m, floor {floor:.0} m, worst rim change {worst:.0} m").map_err(|e| e.to_string())?;
    let _ = SITE.set(site);
    let Ok(path) = std::env::var("REALISM_REGION") else {
        writeln!(out, "realism: no REALISM_REGION, the baseline run (the world as main draws it)").map_err(|e| e.to_string())?;
        return Ok(());
    };
    let baked = Baked::load(std::path::Path::new(&path), Params::default()).ok_or_else(|| format!("REALISM_REGION {path}: not a baked region"))?;
    let octaves = env_f("REALISM_DETAIL", 3.0) as u32;
    let photo = match std::env::var("REALISM_PHOTO") {
        Ok(dir) => Some(load_photo(std::path::Path::new(&dir))?),
        Err(_) => None,
    };
    rz::install(rz::Resources {
        mat_n: baked.materials.n as u32,
        mat0: baked.materials.layer0.clone(),
        mat1: baked.materials.layer1.clone(),
        hor_n: baked.horizon.n as u32,
        hor: baked.horizon.layers.clone(),
        photo,
        extent_m: EXTENT_M,
    });
    let baked = Arc::new(baked);
    ACTIVE.set(Active { site, baked, octaves }).map_err(|_| "the region was set twice".to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Photo sets

fn decode(path: &std::path::Path) -> Result<(u32, u32, Vec<u8>, png::ColorType), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("no buffer size")?];
    let info = r.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    buf.truncate(info.buffer_size());
    Ok((info.width, info.height, buf, info.color_type))
}

/// Channel `c` of every pixel as bytes (gray images repeat their channel).
fn channel(img: &(u32, u32, Vec<u8>, png::ColorType), c: usize) -> Vec<u8> {
    let step = match img.3 {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => 3,
    };
    let c = if step <= 2 { 0 } else { c };
    img.2.chunks_exact(step).map(|p| p[c]).collect()
}

fn srgb_to_lin(v: u8) -> f32 {
    let x = f32::from(v) / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn lin_to_srgb(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let v = if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0 + 0.5) as u8
}

fn mips(base: Vec<u8>, size: usize, srgb_rgb: bool) -> Vec<Vec<u8>> {
    let mut out = vec![base];
    let mut s = size;
    while s > 1 {
        let prev = out.last().expect("a level");
        let ns = s / 2;
        let mut next = vec![0u8; ns * ns * 4];
        for y in 0..ns {
            for x in 0..ns {
                for c in 0..4 {
                    let tap = |dx: usize, dy: usize| prev[((y * 2 + dy) * s + x * 2 + dx) * 4 + c];
                    let v = if srgb_rgb && c < 3 {
                        let l = (srgb_to_lin(tap(0, 0)) + srgb_to_lin(tap(1, 0)) + srgb_to_lin(tap(0, 1)) + srgb_to_lin(tap(1, 1))) / 4.0;
                        lin_to_srgb(l)
                    } else {
                        ((u32::from(tap(0, 0)) + u32::from(tap(1, 0)) + u32::from(tap(0, 1)) + u32::from(tap(1, 1)) + 2) / 4) as u8
                    };
                    next[(y * ns + x) * 4 + c] = v;
                }
            }
        }
        out.push(next);
        s = ns;
    }
    out
}

fn load_photo(dir: &std::path::Path) -> Result<rz::Photo, String> {
    let names = ["rock", "scree", "snow", "grass", "soil"];
    let (mut albedo, mut normal) = (Vec::new(), Vec::new());
    let mut size = 0u32;
    for name in names {
        let d = dir.join(name);
        let color = decode(&d.join("color.png"))?;
        let nrm = decode(&d.join("normal.png"))?;
        let rough = decode(&d.join("rough.png"))?;
        let disp = decode(&d.join("disp.png"))?;
        if color.0 != color.1 {
            return Err(format!("{name}: not square"));
        }
        size = size.max(color.0);
        let n = (color.0 * color.1) as usize;
        if [&nrm, &rough, &disp].iter().any(|i| i.0 != color.0 || i.1 != color.1) {
            return Err(format!("{name}: maps differ in size"));
        }
        let (r, g, b) = (channel(&color, 0), channel(&color, 1), channel(&color, 2));
        let h = channel(&disp, 0);
        let (nx, ny) = (channel(&nrm, 0), channel(&nrm, 1));
        let ro = channel(&rough, 0);
        let mut a = vec![0u8; n * 4];
        let mut m = vec![0u8; n * 4];
        for i in 0..n {
            a[i * 4..i * 4 + 4].copy_from_slice(&[r[i], g[i], b[i], h[i]]);
            m[i * 4..i * 4 + 4].copy_from_slice(&[nx[i], ny[i], ro[i], 255]);
        }
        albedo.push(mips(a, color.0 as usize, true));
        normal.push(mips(m, color.0 as usize, false));
    }
    Ok(rz::Photo { size, albedo, normal })
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Stills

struct Pose {
    name: &'static str,
    x: f64,
    y: f64,
    eye: f64,
    heading_deg: f64,
    pitch_deg: f64,
}

/// Poses in the region frame (x east, y north, metres; heading clockwise from north; pitch down positive).
fn poses() -> Vec<Pose> {
    let mut v = vec![
        Pose { name: "valley-300m", x: -1500.0, y: -1700.0, eye: 300.0, heading_deg: 40.0, pitch_deg: 8.0 },
        Pose { name: "valley-30m", x: -1500.0, y: -1700.0, eye: 30.0, heading_deg: 40.0, pitch_deg: 4.0 },
        Pose { name: "valley-1.8m", x: -1500.0, y: -1700.0, eye: 1.8, heading_deg: 40.0, pitch_deg: 3.0 },
        Pose { name: "slope-300m", x: 300.0, y: -900.0, eye: 300.0, heading_deg: 20.0, pitch_deg: 12.0 },
        Pose { name: "slope-30m", x: 300.0, y: -900.0, eye: 30.0, heading_deg: 20.0, pitch_deg: 8.0 },
        Pose { name: "slope-1.8m", x: 300.0, y: -900.0, eye: 1.8, heading_deg: 20.0, pitch_deg: 14.0 },
    ];
    if let Ok(spec) = std::env::var("REALISM_POSE") {
        // name:x,y,eye,heading,pitch;...
        for item in spec.split(';') {
            let Some((name, rest)) = item.split_once(':') else { continue };
            let p: Vec<f64> = rest.split(',').filter_map(|t| t.parse().ok()).collect();
            if p.len() == 5 {
                v.push(Pose { name: Box::leak(name.to_string().into_boxed_str()), x: p[0], y: p[1], eye: p[2], heading_deg: p[3], pitch_deg: p[4] });
            }
        }
    }
    v
}

pub(crate) fn run(
    rig: &super::Rig<'_>,
    setup: &mut super::LitSetup,
    r: &mut planet_render::TerrainRenderer,
    world: &super::WorldTerrain,
    dir: &std::path::Path,
    out: &mut dyn std::io::Write,
) -> Result<(), String> {
    let err = |e: std::io::Error| e.to_string();
    let site = *SITE.get().ok_or("no site")?;
    let steps: Vec<u32> = std::env::var("REALISM_STEPS").ok().map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect()).unwrap_or_else(|| vec![0, 1, 3, 7]);
    let sun_names: Vec<String> = std::env::var("REALISM_SUNS").ok().map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| vec!["sunset".into(), "noon".into()]);
    let want: Option<Vec<String>> = std::env::var("REALISM_POSES").ok().map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let tag = std::env::var("REALISM_TAG").unwrap_or_default();
    let timing = std::env::var_os("REALISM_TIMING").is_some();
    let aspect = f64::from(rig.size.0) / f64::from(rig.size.1);
    let radius = rig.radius_m;
    let region_on = ACTIVE.get().is_some();
    let mut cache = planet_frame::SelectionCache::new();
    std::fs::create_dir_all(dir).map_err(err)?;
    writeln!(out, "realism: region {}, steps {steps:?}, suns {sun_names:?}", if region_on { "ON" } else { "off (baseline)" }).map_err(err)?;
    for pose in poses() {
        if want.as_ref().is_some_and(|w| !w.iter().any(|n| n == pose.name)) {
            continue;
        }
        let d = site.dir_at(pose.x, pose.y);
        let ground = rig.field.height(d);
        let pos = PlanetFixed(d * (radius + ground + pose.eye));
        let az = pose.heading_deg.to_radians();
        let hint = site.east * az.sin() + site.north * az.cos();
        let camera = planet_frame::Camera::at_surface_point(pos, hint, pose.pitch_deg.to_radians(), 60f64.to_radians(), aspect, 0.1);
        let plan = planet_frame::plan_terrain_cached(&super::request(rig, camera), rig.field, &mut cache).map_err(|e| e.to_string())?;
        let started = std::time::Instant::now();
        let new = crate::skeleton::upload_missing_meshes(rig.ctx, Some(rig.world), r, &plan, rig.field, radius, rig.cells)?;
        writeln!(out, "{}: ground {ground:.1} m at ({:.0}, {:.0}), {} nodes, {new} new meshes in {:.1} s", pose.name, pose.x, pose.y, plan.nodes.len(), started.elapsed().as_secs_f64()).map_err(err)?;
        // The camera in the region frame, decided in f64.
        let o = site.centre * radius;
        let rel = pos.0 - o;
        let cam = [rel.dot(site.east) as f32, rel.dot(site.north) as f32, rel.dot(site.centre) as f32, (1.0 / radius) as f32];
        for sun in &sun_names {
            let (elev, az_deg) = match sun.as_str() {
                "sunset" => (3.5f64, 262.0f64),
                "noon" => (58.0, 180.0),
                _ => (env_f("REALISM_SUN_EL", 20.0), env_f("REALISM_SUN_AZ", 200.0)),
            };
            let (el, sa) = (elev.to_radians(), az_deg.to_radians());
            let sun_dir = site.north * (el.cos() * sa.cos()) + site.east * (el.cos() * sa.sin()) + site.centre * el.sin();
            for &flags in &steps {
                rz::set_frame(rz::Frame {
                    cam,
                    east: [site.east.x as f32, site.east.y as f32, site.east.z as f32, (EXTENT_M / 2.0) as f32],
                    north: [site.north.x as f32, site.north.y as f32, site.north.z as f32, SKIRT_M as f32],
                    up: [site.centre.x as f32, site.centre.y as f32, site.centre.z as f32, 0.0],
                    flags: [if region_on { flags } else { 0 }, env_f("REALISM_DEBUG", 0.0) as u32, 0, 0],
                    tune: [env_f("REALISM_SCALE", 1.0) as f32, env_f("REALISM_BLEND", 0.25) as f32, env_f("REALISM_MACRO", 1.0) as f32, 0.0],
                });
                let mut lit = setup.plan(world, rig.field, &plan, super::WATER_T0, 1.0, true)?;
                if let Some(l) = lit.lit.as_mut() {
                    l.sun_dir = [sun_dir.x as f32, sun_dir.y as f32, sun_dir.z as f32];
                }
                for _ in 0..4 {
                    r.render(rig.ctx, &lit, false).map_err(|e| e.to_string())?;
                }
                let image = r.read_color(rig.ctx);
                let name = format!("{}_{}_{}{}.png", pose.name, sun, if region_on { format!("s{flags}") } else { "main".to_string() }, tag);
                image.write_png(&dir.join(&name)).map_err(|e| format!("writing {name}: {e}"))?;
                let mut line = format!("  {name}");
                if timing {
                    let mut gpu = Vec::new();
                    let mut span = Vec::new();
                    for k in 0..70 {
                        let (t, _) = r.render(rig.ctx, &lit, true).map_err(|e| e.to_string())?;
                        if k >= 10 {
                            gpu.push(t.gpu_ms.unwrap_or(f64::NAN));
                            span.push(t.passes.span_ms.unwrap_or(f64::NAN));
                        }
                    }
                    gpu.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    span.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    line += &format!(": terrain pass {:.3} ms (p90 {:.3}), gpu span {:.3} ms", gpu[30], gpu[54], span[30]);
                }
                writeln!(out, "{line}").map_err(err)?;
            }
        }
    }
    Ok(())
}
