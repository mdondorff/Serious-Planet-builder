//! The planet preview of the lithology field (ADR 0102 point 4, GEN-122): equirectangular maps of the hardness, the strata and the soluble flag, a vertical cross-section of the strata through a
//! mountain belt into a basin, and the numbers that go with the pictures. Every image shows the **lithology field, an input of the carve, not terrain**; the values are start values for the owner's review.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14). The strata maps draw the rock at the class source's base elevation (the surface the carve starts from) and 200 m below it.

use crate::bake::carve::CarveParams;
use crate::classes::{ClassSource, EARTH_RADIUS_M};
use crate::dump::Rgba;
use crate::lithology::{Lithology, BAND_MAX_M, BAND_MIN_M, CAPROCK_HARDNESS, K_HARD, K_SOFT, LITHOLOGY_VERSION};
use planet_core::par::{self, Par};
use planet_core::Vec3;
use std::path::Path;

/// The images and the numbers of a preview.
pub struct Preview {
    /// The hardness `H` of the land: blue hard, white middling, red soft.
    pub hardness: Rgba,
    /// The erodibility `K` at the surface (caprock bands black).
    pub strata_surface: Rgba,
    /// The erodibility `K` 200 m below the surface: the rock the carve meets next.
    pub strata_below: Rgba,
    /// The soluble flag at the surface.
    pub soluble: Rgba,
    /// A vertical cross-section along a great circle through a mountain belt into a basin: `x` distance, `y` height, colour `K`, caprock marked, the surface drawn on top.
    pub section: Rgba,
    /// The numbers: shares, quantiles, the section's coordinates, and the band thickness against the pass-1 per-step height change.
    pub text: String,
}

const OCEAN: [u8; 4] = [20, 30, 60, 255];

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgba(c: [f64; 3]) -> [u8; 4] {
    [libm::round(c[0]) as u8, libm::round(c[1]) as u8, libm::round(c[2]) as u8, 255]
}

/// The hardness ramp: blue (hard), white, red (soft).
fn hardness_colour(h: f64) -> [u8; 4] {
    let (blue, white, red) = ([50.0, 90.0, 200.0], [240.0, 240.0, 235.0], [200.0, 60.0, 45.0]);
    rgba(if h >= 0.5 { lerp(white, blue, (h - 0.5) * 2.0) } else { lerp(red, white, h * 2.0) })
}

/// The erodibility ramp: dark blue (hard rock, low K), pale, red (soft rock, high K).
fn k_colour(k: f64) -> [u8; 4] {
    let t = ((k - K_HARD) / (K_SOFT - K_HARD)).clamp(0.0, 1.0);
    let (dark, pale, red) = ([30.0, 50.0, 140.0], [240.0, 235.0, 200.0], [200.0, 60.0, 40.0]);
    rgba(if t <= 0.5 { lerp(dark, pale, t * 2.0) } else { lerp(pale, red, (t - 0.5) * 2.0) })
}

fn soluble_colour(s: f64) -> [u8; 4] {
    rgba(lerp([55.0, 55.0, 55.0], [40.0, 215.0, 195.0], s.clamp(0.0, 1.0)))
}

/// The unit direction of longitude `lon` and latitude `lat` in radians (+z the north pole, COORD's axes).
fn direction(lon: f64, lat: f64) -> Vec3 {
    let c = libm::cos(lat);
    Vec3::new(c * libm::cos(lon), c * libm::sin(lon), libm::sin(lat))
}

fn lon_lat(d: Vec3) -> (f64, f64) {
    (libm::atan2(d.y, d.x), libm::asin(d.z.clamp(-1.0, 1.0)))
}

/// What one map pixel says.
#[derive(Clone, Copy)]
struct Pixel {
    land: bool,
    hardness: f64,
    k_surface: f64,
    k_below: f64,
    caprock: bool,
    soluble: f64,
}

/// The maps at `km` kilometres a pixel (an equirectangular grid of `2 pi R / km` by half as many pixels), the section and the numbers, on `threads` threads.
pub fn preview(source: &ClassSource, lithology: &Lithology, km: f64, threads: usize) -> Preview {
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    let rows = par::map(par, height, &|y| {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        (0..width)
            .map(|x| {
                let lon = (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
                let d = direction(lon, lat);
                let p = source.eval(d);
                let col = lithology.column(d);
                let (s0, s1) = (col.at(p.e0), col.at(p.e0 - 200.0));
                Pixel {
                    land: p.land_depth > 0.0,
                    hardness: col.hardness,
                    k_surface: s0.k,
                    k_below: s1.k,
                    caprock: s0.caprock,
                    soluble: s0.soluble,
                }
            })
            .collect::<Vec<_>>()
    });
    let image = |colour: &dyn Fn(&Pixel) -> [u8; 4]| {
        let mut data = Vec::with_capacity(width * height * 4);
        for row in &rows {
            for p in row {
                data.extend_from_slice(&if p.land { colour(p) } else { OCEAN });
            }
        }
        Rgba { width: width as u32, height: height as u32, data }
    };
    let hardness = image(&|p| hardness_colour(p.hardness));
    let strata_surface = image(&|p| if p.caprock { [0, 0, 0, 255] } else { k_colour(p.k_surface) });
    let strata_below = image(&|p| k_colour(p.k_below));
    let soluble = image(&|p| soluble_colour(p.soluble));
    let (section, section_text) = cross_section(source, lithology);
    let text = numbers(&rows, width, height, km, lithology.seed(), &section_text);
    Preview { hardness, strata_surface, strata_below, soluble, section, text }
}

/// The shares and quantiles of the land pixels, and the band thickness against the carve's per-step change.
fn numbers(rows: &[Vec<Pixel>], width: usize, height: usize, km: f64, seed: u64, section: &str) -> String {
    let land: Vec<&Pixel> = rows.iter().flatten().filter(|p| p.land).collect();
    let n = land.len().max(1) as f64;
    let share = |f: &dyn Fn(&Pixel) -> bool| land.iter().filter(|p| f(p)).count() as f64 / n;
    let quantile = |v: &mut Vec<f64>, q: f64| {
        v.sort_by(f64::total_cmp);
        v.get(((v.len().max(1) - 1) as f64 * q) as usize).copied().unwrap_or(0.0)
    };
    let mut h: Vec<f64> = land.iter().map(|p| p.hardness).collect();
    let mut k: Vec<f64> = land.iter().map(|p| p.k_surface).collect();
    let mut out = String::new();
    out.push_str(
        "lithology field (input), not terrain: the preview of ADR 0102 / GEN-116; every number is a start value for the owner's review\n",
    );
    out.push_str(&format!(
        "version {LITHOLOGY_VERSION}, seed {seed}, {km} km a pixel, {width} x {height} pixels, land {:.1} % of the planet\n",
        100.0 * n / (width * height) as f64
    ));
    out.push_str(&format!("hardness H over land: hard (H over 0.6) {:.1} %, soft (H under 0.4) {:.1} %; quantiles p5 {:.2}, p25 {:.2}, p50 {:.2}, p75 {:.2}, p95 {:.2}\n", 100.0 * share(&|p| p.hardness > 0.6), 100.0 * share(&|p| p.hardness < 0.4), quantile(&mut h, 0.05), quantile(&mut h, 0.25), quantile(&mut h, 0.5), quantile(&mut h, 0.75), quantile(&mut h, 0.95)));
    out.push_str(&format!("erodibility K at the surface over land: p5 {:.2}, p50 {:.2}, p95 {:.2} (a band of hardness 1 has K {K_HARD}, of hardness 0 has K {K_SOFT})\n", quantile(&mut k, 0.05), quantile(&mut k, 0.5), quantile(&mut k, 0.95)));
    out.push_str(&format!("caprock bands (hardness over {CAPROCK_HARDNESS}) at the surface: {:.1} % of land; soluble (over 0.5) at the surface: {:.1} % of land\n", 100.0 * share(&|p| p.caprock), 100.0 * share(&|p| p.soluble > 0.5)));
    out.push_str(section);
    // The bake can resolve the bands when one pass-1 step changes the height by much less than a band is thick.
    let carve = CarveParams::default();
    let dx = EARTH_RADIUS_M * core::f64::consts::FRAC_PI_2 / 12_288.0;
    out.push_str(&format!("band thickness {BAND_MIN_M:.0} to {BAND_MAX_M:.0} m (nominal {:.0} m); the lithology field is a function of height, read by the carve at the cell's current height in each step\n", crate::lithology::BAND_NOMINAL_M));
    out.push_str(&format!("pass 1 height change a step (k_dt {}, {} steps, cell {:.0} m at 12,288 cells a face edge; the lowering of a cell is k_dt sqrt(A) / dx at its drainage area A):\n", carve.k_dt, carve.steps, dx));
    for area_km2 in [10.0, 100.0, 1_000.0, 10_000.0, 100_000.0] {
        let step = carve.k_dt * libm::sqrt(area_km2 * 1e6) / dx;
        out.push_str(&format!(
            "  A {area_km2:>9.0} km2: {step:>6.2} m a step, {:>6.1} m over the run; the thinnest band takes {:>6.1} steps to cross\n",
            step * carve.steps as f64,
            BAND_MIN_M / step
        ));
    }
    out.push_str("  (a step is small against a band wherever the area is under about 1e3 km2; on the largest rivers (1e5 km2) a thin band is crossed in 1 to 2 steps: marginally resolved, and the carve re-reads K at the new height each step)\n");
    out
}

/// A vertical cross-section along a great circle from a mountain belt into a basin: `x` distance, `y` height, colour `K`, caprock hatched black, the surface a black line on top.
fn cross_section(source: &ClassSource, lithology: &Lithology) -> (Rgba, String) {
    const W: usize = 900;
    const H: usize = 450;
    let provinces = lithology.provinces();
    let n = provinces.cells();
    let node = |f: u8, i: usize, j: usize| {
        let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
        planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(f), s, t)
    };
    // The belts (the highest core terms) and the basins (the highest basin terms) of the lattice nodes, every 4th node.
    let (mut belts, mut basins) = (Vec::new(), Vec::new());
    for f in 0..6u8 {
        for j in (0..=n).step_by(4) {
            for i in (0..=n).step_by(4) {
                let p = provinces.point(planet_core::Face(f), i, j);
                if p[1] > 150 {
                    belts.push((p[1], node(f, i, j)));
                }
                if p[3] > 150 {
                    basins.push((p[3], node(f, i, j)));
                }
            }
        }
    }
    belts.sort_by_key(|b| std::cmp::Reverse(b.0));
    let angle = |a: Vec3, b: Vec3| libm::atan2(a.cross(b).length(), a.dot(b));
    // Take the strongest belts that are spread out; for each, the nearest basin at least 600 km and at most 2,500 km away; keep the path with the most land.
    let mut chosen: Option<(f64, Vec3, Vec3)> = None;
    let mut tried: Vec<Vec3> = Vec::new();
    for (_, b) in belts.iter() {
        if tried.iter().any(|t| angle(*t, *b) < 1_500.0 / (EARTH_RADIUS_M / 1000.0)) {
            continue;
        }
        tried.push(*b);
        if tried.len() > 12 {
            break;
        }
        let near = basins
            .iter()
            .map(|(_, p)| (angle(*b, *p) * EARTH_RADIUS_M / 1000.0, *p))
            .filter(|(km, _)| (600.0..=2_500.0).contains(km))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, basin)) = near {
            let land = (0..40).filter(|k| source.eval(slerp(*b, basin, f64::from(*k) / 39.0)).land_depth > 0.0).count() as f64 / 40.0;
            if chosen.is_none_or(|c| land > c.0) {
                chosen = Some((land, *b, basin));
            }
        }
    }
    let Some((land, belt, basin)) = chosen else {
        return (
            Rgba { width: W as u32, height: H as u32, data: vec![255; W * H * 4] },
            "cross-section: no belt and basin pair found\n".to_string(),
        );
    };
    // The path: from before the belt to beyond the basin (a quarter of the distance on each side).
    let (a, b) = (slerp(belt, basin, -0.25), slerp(belt, basin, 1.25));
    let dirs: Vec<Vec3> = (0..W).map(|x| slerp(a, b, x as f64 / (W - 1) as f64)).collect();
    let surface: Vec<f64> = dirs.iter().map(|d| source.eval(*d).e0).collect();
    let columns: Vec<_> = dirs.iter().map(|d| lithology.column(*d)).collect();
    let (lo, hi) = (surface.iter().cloned().fold(f64::MAX, f64::min) - 300.0, surface.iter().cloned().fold(f64::MIN, f64::max) + 100.0);
    let mut data = vec![255u8; W * H * 4];
    for x in 0..W {
        for y in 0..H {
            let z = hi - (y as f64 + 0.5) / H as f64 * (hi - lo);
            let i = (y * W + x) * 4;
            if z > surface[x] {
                continue;
            }
            let s = columns[x].at(z);
            let c = if s.caprock && (x + y) % 2 == 0 { [0, 0, 0, 255] } else { k_colour(s.k) };
            data[i..i + 4].copy_from_slice(&c);
        }
        // The surface on top: a black line two pixels thick.
        let ys = ((hi - surface[x]) / (hi - lo) * H as f64) as isize;
        for dy in 0..2 {
            let y = ys + dy;
            if (0..H as isize).contains(&y) {
                let i = (y as usize * W + x) * 4;
                data[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    let (b_lon, b_lat) = lon_lat(belt);
    let (p_lon, p_lat) = lon_lat(basin);
    let length_km = angle(a, b) * EARTH_RADIUS_M / 1000.0;
    let text = format!(
        "cross-section: from a mountain belt (lat {:.1}, lon {:.1}) into a basin (lat {:.1}, lon {:.1}), {:.0} km along a great circle, {:.0} % land; height {:.0} m to {:.0} m (the class source's base elevation, not the carved terrain), {W} x {H} pixels\n",
        b_lat.to_degrees(),
        b_lon.to_degrees(),
        p_lat.to_degrees(),
        p_lon.to_degrees(),
        length_km,
        100.0 * land,
        lo,
        hi
    );
    (Rgba { width: W as u32, height: H as u32, data }, text)
}

/// The point at fraction `t` along the great circle from `a` to `b` (`t` outside `[0, 1]` extends it).
fn slerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    let omega = libm::atan2(a.cross(b).length(), a.dot(b)).max(1e-9);
    let (sa, sb) = (libm::sin((1.0 - t) * omega) / libm::sin(omega), libm::sin(t * omega) / libm::sin(omega));
    (a * sa + b * sb).normalized()
}

/// Writes the images and `lithology.txt` into `dir`.
pub fn write(dir: &Path, p: &Preview) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    p.hardness.write_png(&dir.join("hardness.png"))?;
    p.strata_surface.write_png(&dir.join("strata_surface.png"))?;
    p.strata_below.write_png(&dir.join("strata_200m_below.png"))?;
    p.soluble.write_png(&dir.join("soluble.png"))?;
    p.section.write_png(&dir.join("section.png"))?;
    std::fs::write(dir.join("lithology.txt"), &p.text)
}
