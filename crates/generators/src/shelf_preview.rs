//! The planet preview of the shelf profile (GEN-118, GEN-136; ADR 0107, ADR 0108): the bathymetry map, two coast regions at 2 km and 10 km a pixel (one passive and one active margin), and the numbers
//! against Earth's: the shelf share of the ocean and, by margin type, where the shelf break and the foot of the slope lie from the coast of the field. Every image shows the **base elevation of the
//! class source, an input of the bake, not terrain**.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14).

use crate::classes::{ClassSource, EARTH_RADIUS_M};
use crate::dump::Rgba;
use crate::lithology_preview::{direction, lerp, rgba};
use planet_core::par::{self, Par};
use planet_core::Vec3;
use std::path::Path;

/// The images and the numbers of a preview.
pub struct Preview {
    pub images: Vec<(String, Rgba)>,
    pub text: String,
}

/// Earth's shelf (shallower than 200 m) share of the ocean area (Kennett 1982; Harris et al. 2014: about 7 to 9 %).
const EARTH_SHELF_SHARE: (f64, f64) = (7.0, 9.0);
/// The depth in metres the shelf share counts (the conventional 200 m isobath).
const SHELF_ISOBATH_M: f64 = 200.0;
/// The relative band around the shelf break's isobath, and around the foot of the slope's (narrower: the abyssal depth lies 10 % beyond it), within which a sea pixel counts as lying on it.
const BREAK_BAND: f64 = 0.15;
const SLOPE_BAND: f64 = 0.05;
/// The fraction of the abyssal depth the foot of the slope is taken at.
const SLOPE_END_FRACTION: f64 = 0.9;
/// The side of a region image in pixels.
const REGION_PX: usize = 256;

fn e0(source: &ClassSource, d: Vec3) -> f64 {
    source.eval(d).e0
}

/// The colour of a base elevation: the sea from turquoise (shallow) to navy (deep), the land by its height.
fn colour(e: f64, abyssal_m: f64) -> [u8; 4] {
    if e <= 0.0 {
        let t = (libm::sqrt(-e) / libm::sqrt(abyssal_m)).clamp(0.0, 1.0);
        rgba(lerp([110.0, 215.0, 215.0], [15.0, 25.0, 85.0], t))
    } else {
        let t = (libm::sqrt(e) / libm::sqrt(3000.0)).clamp(0.0, 1.0);
        rgba(lerp([120.0, 170.0, 90.0], [235.0, 230.0, 225.0], t))
    }
}

fn quantile(v: &mut [f64], f: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(f64::NAN)
}

/// The region of `REGION_PX` pixels of `km` km centred on `(lon, lat)` (radians), as an image.
fn region(source: &ClassSource, lon: f64, lat: f64, km: f64) -> Rgba {
    let per_px = km / (EARTH_RADIUS_M / 1000.0);
    let cos_lat = libm::cos(lat).max(0.05);
    let mut data = Vec::with_capacity(REGION_PX * REGION_PX * 4);
    for y in 0..REGION_PX {
        for x in 0..REGION_PX {
            let la = lat - (y as f64 - REGION_PX as f64 / 2.0) * per_px;
            let lo = lon + (x as f64 - REGION_PX as f64 / 2.0) * per_px / cos_lat;
            data.extend_from_slice(&colour(e0(source, direction(lo, la.clamp(-1.5, 1.5))), source.shelf().abyssal_depth_m));
        }
    }
    Rgba { width: REGION_PX as u32, height: REGION_PX as u32, data }
}

/// The maps at `km` kilometres a pixel and the numbers, on `threads` threads.
pub fn preview(source: &ClassSource, km: f64, threads: usize) -> Preview {
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    let abyss = source.shelf().abyssal_depth_m;
    let (break_m, end_m) = (source.shelf().break_depth_m, SLOPE_END_FRACTION * abyss);
    let lat_of = |y: usize| core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
    let lon_of = |x: usize| (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
    // Per pixel: the base elevation and, for a pixel near the shelf break or the foot of the slope, the distance to the coast of the field and the margin activity there (the coast lattice).
    let rows = par::map(par, height, &|y| {
        (0..width)
            .map(|x| {
                let d = direction(lon_of(x), lat_of(y));
                let e = e0(source, d);
                let near = |iso: f64, band: f64| e < 0.0 && libm::fabs(-e - iso) < band * iso;
                let (dist, act) = if near(break_m, BREAK_BAND) || near(end_m, SLOPE_BAND) { source.coast_read(d) } else { (0.0, 0.0) };
                (e, dist, act)
            })
            .collect::<Vec<(f64, f64, f64)>>()
    });
    let mut data = Vec::with_capacity(width * height * 4);
    for row in &rows {
        for &(e, _, _) in row {
            data.extend_from_slice(&colour(e, abyss));
        }
    }
    let mut images = vec![("bathymetry.png".to_string(), Rgba { width: width as u32, height: height as u32, data })];
    // The shelf share of the ocean by area (pixels weigh by the cosine of the latitude).
    let (mut ocean, mut shelf) = (0.0, 0.0);
    for (y, row) in rows.iter().enumerate() {
        let w = libm::cos(lat_of(y));
        for &(e, _, _) in row.iter().filter(|p| p.0 < 0.0) {
            ocean += w;
            if e > -SHELF_ISOBATH_M {
                shelf += w;
            }
        }
    }
    let p = source.shelf();
    let mut text = String::new();
    text.push_str(
        "shelf profile (input), not terrain: the preview of GEN-118 and GEN-136; every number is a start value for the owner's review\n",
    );
    text.push_str(&format!(
        "seed {}, {km} km a pixel, distances from the coast lattice of {} cells a face edge\n",
        source.seed(),
        p.coast_lattice_cells
    ));
    text.push_str(&format!(
        "parameters (Earth preset): break {} m, abyssal {} m, passive width {} km, active width {} km, slope {} km\n",
        p.break_depth_m, p.abyssal_depth_m, p.passive_width_km, p.active_width_km, p.slope_width_km
    ));
    text.push_str(&format!(
        "shelf share of the ocean (shallower than {SHELF_ISOBATH_M} m, by area): {:.1} % (Earth {:.0} to {:.0} %)\n",
        100.0 * shelf / ocean.max(1e-9),
        EARTH_SHELF_SHARE.0,
        EARTH_SHELF_SHARE.1
    ));
    text.push_str(&format!(
        "distance from the coast of the field to the shelf break (pixels within {:.0} % of {break_m} m) and to the foot of the slope (within {:.0} % of {end_m:.0} m), by margin type of the coast:\n",
        100.0 * BREAK_BAND,
        100.0 * SLOPE_BAND
    ));
    let groups = [
        ("passive (active weight < 0.2)", 0.0, 0.2, "Earth: tens to a few hundred km"),
        ("in between", 0.2, 0.8, ""),
        ("active (active weight > 0.8)", 0.8, 1.01, "Earth: a few tens of km"),
    ];
    for (name, lo, hi, earth) in groups {
        let pick = |iso: f64, band: f64| -> Vec<f64> {
            rows.iter()
                .flatten()
                .filter(|(e, _, a)| *e < 0.0 && libm::fabs(-*e - iso) < band * iso && *a >= lo && *a < hi)
                .map(|(_, d, _)| *d)
                .collect()
        };
        let (mut shelf_d, mut slope_d) = (pick(break_m, BREAK_BAND), pick(end_m, SLOPE_BAND));
        let n = shelf_d.len();
        text.push_str(&format!(
            "  {name:<30} {n:>6} break pixels: p10 {:.0} / p50 {:.0} / p90 {:.0} km; foot of the slope p50 {:.0} km  {earth}\n",
            quantile(&mut shelf_d, 0.1),
            quantile(&mut shelf_d, 0.5),
            quantile(&mut shelf_d, 0.9),
            quantile(&mut slope_d, 0.5)
        ));
    }
    // Two regions: the coast pixel (a land pixel with a sea neighbour, within 60 degrees of the equator, where a lat/lon image stretches beyond) in the middle of its margin type's group.
    for (label, lo, hi) in [("passive", 0.0, 0.2), ("active", 0.8, 1.01)] {
        let mut coasts: Vec<(usize, usize)> = Vec::new();
        for y in 1..height - 1 {
            if libm::fabs(lat_of(y)) > core::f64::consts::FRAC_PI_3 {
                continue;
            }
            for x in 0..width {
                let neighbours = [rows[y][(x + 1) % width].0, rows[y][(x + width - 1) % width].0, rows[y + 1][x].0, rows[y - 1][x].0];
                if rows[y][x].0 > 0.0 && neighbours.iter().any(|e| *e < 0.0) {
                    let a = source.coast_read(direction(lon_of(x), lat_of(y))).1;
                    if a >= lo && a < hi {
                        coasts.push((x, y));
                    }
                }
            }
        }
        let Some(&(x, y)) = coasts.get(coasts.len() / 2) else {
            text.push_str(&format!("no {label} coast pixel: no {label} region image\n"));
            continue;
        };
        text.push_str(&format!(
            "region {label}: ({:.1} N, {:.1} E), the middle of {} coast pixels of that type\n",
            lat_of(y).to_degrees(),
            lon_of(x).to_degrees(),
            coasts.len()
        ));
        for res in [2.0, 10.0] {
            images.push((format!("region_{label}_{res:.0}km.png"), region(source, lon_of(x), lat_of(y), res)));
        }
    }
    Preview { images, text }
}

/// Writes the images and `shelf.txt` into `dir`.
pub fn write(dir: &Path, p: &Preview) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, img) in &p.images {
        img.write_png(&dir.join(name))?;
    }
    std::fs::write(dir.join("shelf.txt"), &p.text)
}
