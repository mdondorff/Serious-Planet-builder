//! The planet preview of the shelf profile (GEN-118; ADR 0107, ADR 0108): the bathymetry map, two coast regions at 2 km and 10 km a pixel (one passive and one active margin), and the numbers against
//! Earth's: the shelf share of the ocean, the shelf width by margin type and the depth of the shelf break. Every image shows the **base elevation of the class source, an input of the bake, not terrain**.
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
/// The seaward gradient in m per km from which a walk from the coast counts the shelf break as reached (the shelf's own gradient is about 1 m per km, the slope's 20 to 70).
const BREAK_GRADIENT_M_PER_KM: f64 = 10.0;
/// The walk from a coast pixel gives up after this many km (a shelf wider than this is reported as the cap).
const WALK_CAP_KM: f64 = 800.0;
/// The side of a region image in pixels.
const REGION_PX: usize = 256;

fn e0(source: &ClassSource, d: Vec3) -> f64 {
    source.eval(d).e0
}

/// The colour of a base elevation: the sea from turquoise (shallow) to navy (deep) with the shelf break marked by a lighter band, the land by its height.
fn colour(e: f64) -> [u8; 4] {
    if e <= 0.0 {
        let t = (libm::sqrt(-e) / libm::sqrt(4500.0)).clamp(0.0, 1.0);
        rgba(lerp([110.0, 215.0, 215.0], [15.0, 25.0, 85.0], t))
    } else {
        let t = (libm::sqrt(e) / libm::sqrt(3000.0)).clamp(0.0, 1.0);
        rgba(lerp([120.0, 170.0, 90.0], [235.0, 230.0, 225.0], t))
    }
}

/// A walk seaward from a coast pixel: `(width_km, depth_at_break_m)` where the gradient first exceeds [`BREAK_GRADIENT_M_PER_KM`], or the cap.
fn walk(source: &ClassSource, from: (f64, f64), way: (f64, f64), km: f64) -> (f64, f64) {
    let mut prev = e0(source, direction(from.0, from.1));
    let per_px = km / (EARTH_RADIUS_M / 1000.0);
    let (dlon, dlat) = (way.0 * per_px / libm::cos(from.1).max(0.05), way.1 * per_px);
    let steps = (WALK_CAP_KM / km) as usize;
    // The shelf starts at the first sea pixel: the land's own fall to the coast is not the shelf's.
    let mut coast_at = None;
    for i in 1..=steps {
        let lat = (from.1 + dlat * i as f64).clamp(-core::f64::consts::FRAC_PI_2 + 1e-6, core::f64::consts::FRAC_PI_2 - 1e-6);
        let here = e0(source, direction(from.0 + dlon * i as f64, lat));
        let Some(start) = coast_at else {
            if here < 0.0 {
                coast_at = Some(i);
            }
            prev = here;
            continue;
        };
        if (prev - here) / km > BREAK_GRADIENT_M_PER_KM {
            return ((i - start) as f64 * km, prev);
        }
        prev = here;
    }
    (WALK_CAP_KM, prev)
}

fn quantile(v: &mut [f64], f: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(f64::NAN)
}

/// One coast pixel with its measured shelf.
struct Coast {
    lon: f64,
    lat: f64,
    active: f64,
    width_km: f64,
    break_m: f64,
}

/// The region of `REGION_PX` pixels of `km` km centred on a coast point, as an image.
fn region(source: &ClassSource, c: &Coast, km: f64) -> Rgba {
    let per_px = km / (EARTH_RADIUS_M / 1000.0);
    let cos_lat = libm::cos(c.lat).max(0.05);
    let mut data = Vec::with_capacity(REGION_PX * REGION_PX * 4);
    for y in 0..REGION_PX {
        for x in 0..REGION_PX {
            let lat = c.lat - (y as f64 - REGION_PX as f64 / 2.0) * per_px;
            let lon = c.lon + (x as f64 - REGION_PX as f64 / 2.0) * per_px / cos_lat;
            data.extend_from_slice(&colour(e0(source, direction(lon, lat.clamp(-1.5, 1.5)))));
        }
    }
    Rgba { width: REGION_PX as u32, height: REGION_PX as u32, data }
}

/// The maps at `km` kilometres a pixel and the numbers, on `threads` threads.
pub fn preview(source: &ClassSource, km: f64, threads: usize) -> Preview {
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    let rows = par::map(par, height, &|y| {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        (0..width)
            .map(|x| {
                let lon = (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
                e0(source, direction(lon, lat))
            })
            .collect::<Vec<f64>>()
    });
    let mut data = Vec::with_capacity(width * height * 4);
    for row in &rows {
        for &e in row {
            data.extend_from_slice(&colour(e));
        }
    }
    let mut images = vec![("bathymetry.png".to_string(), Rgba { width: width as u32, height: height as u32, data })];
    // The shelf share of the ocean by area (pixels weigh by the cosine of the latitude).
    let (mut ocean, mut shelf) = (0.0, 0.0);
    for (y, row) in rows.iter().enumerate() {
        let w = libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
        for &e in row.iter().filter(|e| **e < 0.0) {
            ocean += w;
            if e > -SHELF_ISOBATH_M {
                shelf += w;
            }
        }
    }
    // The coast pixels (a land pixel with an ocean neighbour east, west, north or south) and a walk seaward from each: the realised shelf width and the depth at its break.
    let at = |x: usize, y: usize| rows[y][x % width];
    let dir_of = |x: usize, y: usize| {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        ((x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI, lat)
    };
    let mut starts: Vec<(usize, usize, (f64, f64))> = Vec::new();
    // Every 3rd pixel along and across, so that the walks stay a few thousand.
    for y in (2..height - 2).step_by(3) {
        for x in (0..width).step_by(3) {
            if at(x, y) <= 0.0 {
                continue;
            }
            // The way seaward as (east, north) signs; row y grows southward.
            if at(x + 1, y) < 0.0 {
                starts.push((x, y, (1.0, 0.0)));
            } else if at(x + width - 1, y) < 0.0 {
                starts.push((x, y, (-1.0, 0.0)));
            } else if at(x, y + 1) < 0.0 {
                starts.push((x, y, (0.0, -1.0)));
            } else if at(x, y - 1) < 0.0 {
                starts.push((x, y, (0.0, 1.0)));
            }
        }
    }
    let coasts = par::map(par, starts.len(), &|i| {
        let (x, y, step) = starts[i];
        let (lon, lat) = dir_of(x, y);
        let d = direction(lon, lat);
        let active = source.margin_activity(d);
        let (w, b) = walk(source, (lon, lat), step, km);
        Coast { lon, lat, active, width_km: w, break_m: b }
    });
    let mut text = String::new();
    text.push_str("shelf profile (input), not terrain: the preview of GEN-118; every number is a start value for the owner's review\n");
    text.push_str(&format!("seed {}, {km} km a pixel, {} coast pixels walked seaward (axis directions)\n", source.seed(), coasts.len()));
    let p = source.shelf();
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
    for (name, lo, hi, earth) in [
        ("passive (active weight < 0.2)", 0.0, 0.2, "Earth: tens to a few hundred km"),
        ("in between", 0.2, 0.8, ""),
        ("active (active weight > 0.8)", 0.8, 1.01, "Earth: a few tens of km"),
    ] {
        let mut w: Vec<f64> = coasts.iter().filter(|c| c.active >= lo && c.active < hi).map(|c| c.width_km).collect();
        let mut b: Vec<f64> = coasts.iter().filter(|c| c.active >= lo && c.active < hi).map(|c| c.break_m).collect();
        let n = w.len();
        text.push_str(&format!(
            "  {name:<30} {n:>5} coasts: width p10 {:.0} / p50 {:.0} / p90 {:.0} km, break depth p50 {:.0} m  {earth}\n",
            quantile(&mut w, 0.1),
            quantile(&mut w, 0.5),
            quantile(&mut w, 0.9),
            -quantile(&mut b, 0.5)
        ));
    }
    let capped = coasts.iter().filter(|c| c.width_km >= WALK_CAP_KM).count();
    text.push_str(&format!("walks that reached the {WALK_CAP_KM} km cap without a break: {capped}\n"));
    // Two regions: the passive and the active coast within 60 degrees of the equator (a lat/lon image stretches beyond) whose width is nearest its group's median (the first on ties).
    let in_group = |c: &Coast, lo: f64, hi: f64| c.active >= lo && c.active < hi && c.lat.abs() < core::f64::consts::FRAC_PI_3;
    for (label, lo, hi) in [("passive", 0.0, 0.2), ("active", 0.8, 1.01)] {
        let mut w: Vec<f64> = coasts.iter().filter(|c| in_group(c, lo, hi)).map(|c| c.width_km).collect();
        if w.is_empty() {
            text.push_str(&format!("no {label} coast pixel: no {label} region image\n"));
            continue;
        }
        let med = quantile(&mut w, 0.5);
        let best = coasts
            .iter()
            .filter(|c| in_group(c, lo, hi))
            .fold(None::<&Coast>, |b, c| match b {
                Some(b) if (b.width_km - med).abs() <= (c.width_km - med).abs() => Some(b),
                _ => Some(c),
            })
            .expect("a non-empty group");
        text.push_str(&format!(
            "region {label}: ({:.1} N, {:.1} E), shelf {:.0} km, break {:.0} m\n",
            best.lat.to_degrees(),
            best.lon.to_degrees(),
            best.width_km,
            -best.break_m
        ));
        for res in [2.0, 10.0] {
            images.push((format!("region_{label}_{res:.0}km.png"), region(source, best, res)));
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
