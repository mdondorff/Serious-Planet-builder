//! The planet preview of the uplift field (GEN-125; GEN-123): equirectangular maps of the uplift and of the uplift over the lithology's erodibility at the surface, a section along a great circle
//! from an active belt over a back-arc to a craton, and the numbers. Every image shows the **uplift field, an input of the carve, not terrain**; the values are start values for the owner's review.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14).

use crate::classes::{ClassSource, EARTH_RADIUS_M};
use crate::dump::Rgba;
use crate::lithology::Lithology;
use crate::lithology_preview::{direction, lerp, lon_lat, rgba, slerp, OCEAN};
use crate::uplift::{Uplift, TERM_NAMES, UPLIFT_VERSION, U_BACKARC, U_BELT, U_CRATON, U_FLAT};
use planet_core::par::{self, Par};
use planet_core::Vec3;
use std::path::Path;

/// Today's uniform ratio `U / K` of the carve, the reference of the maps.
const UNIFORM: f64 = 20.0;

/// The images and the numbers of a preview.
pub struct Preview {
    /// The uplift `U` of the land: dark (about 0) through yellow to red (the belt value), a map of the ratio `U / K` at K 1.
    pub uplift: Rgba,
    /// `U` over the lithology's `K` at the surface, relative to the land mean of that ratio: the slope the carve settles at (`S = U / (K sqrt(A))`) against the mean; blue gentler, red steeper.
    pub steepness: Rgba,
    /// A section along a great circle from a belt over a back-arc to a craton: the uplift (a line, with today's uniform 20 dashed) above and the base elevation (filled) below.
    pub section: Rgba,
    /// The numbers: the land mean and quantiles, the shares by term, the mean ratio, the section's place.
    pub text: String,
}

/// The uplift ramp: dark blue (0), yellow (about half the belt value), red (the belt value).
fn uplift_colour(u: f64) -> [u8; 4] {
    let t = (u / U_BELT).clamp(0.0, 1.0);
    let (dark, yellow, red) = ([25.0, 35.0, 90.0], [245.0, 215.0, 90.0], [200.0, 40.0, 30.0]);
    rgba(if t <= 0.5 { lerp(dark, yellow, t * 2.0) } else { lerp(yellow, red, (t - 0.5) * 2.0) })
}

/// The steepness ramp: blue (a quarter of the mean) through white (the mean) to red (four times), on a log scale.
fn steepness_colour(rel: f64) -> [u8; 4] {
    let t = (libm::log2(rel.max(1e-6)) / 2.0).clamp(-1.0, 1.0);
    let (blue, white, red) = ([50.0, 90.0, 200.0], [240.0, 240.0, 235.0], [200.0, 60.0, 45.0]);
    rgba(if t >= 0.0 { lerp(white, red, t) } else { lerp(white, blue, -t) })
}

#[derive(Clone, Copy)]
struct Pixel {
    land: bool,
    u: f64,
    ratio: f64,
    term: usize,
}

/// The maps at `km` kilometres a pixel (an equirectangular grid of `2 pi R / km` by half as many pixels), the section and the numbers, on `threads` threads.
pub fn preview(source: &ClassSource, lithology: &Lithology, uplift: &Uplift, km: f64, threads: usize) -> Preview {
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
                let u = uplift.at(lithology.provinces(), d);
                let k = lithology.k_at(d, p.e0);
                let cand = Uplift::candidates(&lithology.provinces().at(d));
                let term = (0..cand.len()).fold(0, |best, i| if cand[i] > cand[best] { i } else { best });
                Pixel { land: p.land_depth > 0.0, u, ratio: u / k, term }
            })
            .collect::<Vec<_>>()
    });
    // The land mean of the ratio, weighted by the cosine of the latitude (an equirectangular pixel covers less ground towards a pole).
    let weight = |y: usize| libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
    let (mut total, mut mean_u, mut mean_ratio) = (0.0, 0.0, 0.0);
    for (y, row) in rows.iter().enumerate() {
        for p in row.iter().filter(|p| p.land) {
            total += weight(y);
            mean_u += weight(y) * p.u;
            mean_ratio += weight(y) * p.ratio;
        }
    }
    let (mean_u, mean_ratio) = (mean_u / total.max(1e-9), mean_ratio / total.max(1e-9));
    let image = |colour: &dyn Fn(&Pixel) -> [u8; 4]| {
        let mut data = Vec::with_capacity(width * height * 4);
        for row in &rows {
            for p in row {
                data.extend_from_slice(&if p.land { colour(p) } else { OCEAN });
            }
        }
        Rgba { width: width as u32, height: height as u32, data }
    };
    let uplift_map = image(&|p| uplift_colour(p.u));
    let steepness = image(&|p| steepness_colour(p.ratio / mean_ratio));
    let (section, section_text) = cross_section(source, lithology, uplift);
    let text = numbers(&rows, height, km, source.seed(), (mean_u, mean_ratio, total), &section_text);
    Preview { uplift: uplift_map, steepness, section, text }
}

fn numbers(rows: &[Vec<Pixel>], height: usize, km: f64, seed: u64, means: (f64, f64, f64), section: &str) -> String {
    let (mean_u, mean_ratio, total) = means;
    let weight = |y: usize| libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
    let mut shares = [0.0f64; 4];
    let mut sum_by = [0.0f64; 4];
    let mut us: Vec<f64> = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for p in row.iter().filter(|p| p.land) {
            shares[p.term] += weight(y);
            sum_by[p.term] += weight(y) * p.u;
            us.push(p.u);
        }
    }
    us.sort_by(f64::total_cmp);
    let q = |f: f64| us.get(((us.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(0.0);
    let mut out = String::new();
    out.push_str(
        "uplift field (input), not terrain: the preview of GEN-123 / GEN-125; every number is a start value for the owner's review\n",
    );
    out.push_str(&format!("version {UPLIFT_VERSION}, seed {seed}, {km} km a pixel, {} land pixels\n", us.len()));
    out.push_str(&format!(
        "uplift U (the ratio U / K at K 1, today's uniform value {UNIFORM}) over land (mean by area, quantiles by pixel): mean {mean_u:.1} ({:.0} % of the uniform), p5 {:.1}, p25 {:.1}, p50 {:.1}, p75 {:.1}, p95 {:.1}\n",
        100.0 * mean_u / UNIFORM,
        q(0.05),
        q(0.25),
        q(0.5),
        q(0.75),
        q(0.95)
    ));
    out.push_str(&format!("table: craton {U_CRATON}, flat {U_FLAT}, belt or arc coast {U_BELT}, back-arc {U_BACKARC} (see uplift.rs); times 1 plus or minus 0.2 noise\n"));
    for i in 0..TERM_NAMES.len() {
        out.push_str(&format!(
            "  {:<18} {:>5.1} % of land, mean U {:>5.1}\n",
            TERM_NAMES[i],
            100.0 * shares[i] / total.max(1e-9),
            sum_by[i] / shares[i].max(1e-9)
        ));
    }
    out.push_str(&format!(
        "U / K at the surface (the lithology's K at the class source's base elevation), land mean by area {mean_ratio:.1}; the steepness map shows the ratio against that mean, log scale, from a quarter to four times\n"
    ));
    out.push_str("the carve settles at S = U / (K sqrt(A)): the ratio is the relative slope of a river of the same drainage area\n");
    out.push_str(section);
    out
}

/// A section along a great circle from a belt over a back-arc to a craton: the uplift above and the base elevation below.
fn cross_section(source: &ClassSource, lithology: &Lithology, uplift: &Uplift) -> (Rgba, String) {
    const W: usize = 900;
    const H: usize = 400;
    let provinces = lithology.provinces();
    let n = provinces.cells();
    let node = |f: u8, i: usize, j: usize| {
        let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
        planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(f), s, t)
    };
    let (mut belts, mut cratons) = (Vec::new(), Vec::new());
    for f in 0..6u8 {
        for j in (0..=n).step_by(4) {
            for i in (0..=n).step_by(4) {
                let p = provinces.point(planet_core::Face(f), i, j);
                if p[1] > 150 {
                    belts.push((p[1], node(f, i, j)));
                }
                if p[0] > 220 && p[1] < 10 {
                    cratons.push(node(f, i, j));
                }
            }
        }
    }
    belts.sort_by_key(|b| std::cmp::Reverse(b.0));
    let angle = |a: Vec3, b: Vec3| libm::atan2(a.cross(b).length(), a.dot(b));
    let mut chosen: Option<(f64, Vec3, Vec3)> = None;
    let mut tried: Vec<Vec3> = Vec::new();
    for (_, b) in belts.iter() {
        if tried.iter().any(|t| angle(*t, *b) < 1_500.0 / (EARTH_RADIUS_M / 1000.0)) {
            continue;
        }
        tried.push(*b);
        if tried.len() > 16 {
            break;
        }
        let near = cratons
            .iter()
            .map(|c| (angle(*b, *c) * EARTH_RADIUS_M / 1000.0, *c))
            .filter(|(km, _)| (800.0..=2_500.0).contains(km))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, craton)) = near {
            // The score: the largest back-arc term on the path, then the land on it.
            let score = (0..40)
                .map(|k| {
                    let d = slerp(*b, craton, f64::from(k) / 39.0);
                    provinces.at(d).back_arc + 0.1 * f64::from(source.eval(d).land_depth > 0.0)
                })
                .sum::<f64>();
            if chosen.is_none_or(|c| score > c.0) {
                chosen = Some((score, *b, craton));
            }
        }
    }
    let Some((_, belt, craton)) = chosen else {
        return (
            Rgba { width: W as u32, height: H as u32, data: vec![255; W * H * 4] },
            "section: no belt and craton pair found\n".to_string(),
        );
    };
    let (a, b) = (slerp(belt, craton, -0.15), slerp(belt, craton, 1.15));
    let dirs: Vec<Vec3> = (0..W).map(|x| slerp(a, b, x as f64 / (W - 1) as f64)).collect();
    let us: Vec<f64> = dirs.iter().map(|d| uplift.at(provinces, *d)).collect();
    let es: Vec<f64> = dirs.iter().map(|d| source.eval(*d).e0).collect();
    let (e_lo, e_hi) = (es.iter().cloned().fold(f64::MAX, f64::min), es.iter().cloned().fold(f64::MIN, f64::max));
    let half = H / 2;
    let mut data = vec![255u8; W * H * 4];
    let put = |data: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| {
        let i = (y * W + x) * 4;
        data[i..i + 4].copy_from_slice(&c);
    };
    let u_max = U_BELT * 1.25;
    let y_of_u = |u: f64| (half as f64 - 1.0 - (u / u_max).clamp(0.0, 1.0) * (half as f64 - 2.0)) as usize;
    for x in 0..W {
        // The uniform 20, dashed.
        if (x / 6) % 2 == 0 {
            put(&mut data, x, y_of_u(UNIFORM), [150, 150, 150, 255]);
        }
        // The uplift: a filled area under the line, light orange, the line red.
        let yu = y_of_u(us[x]);
        for y in yu..half {
            put(&mut data, x, y, [250, 225, 190, 255]);
        }
        for dy in 0..2 {
            put(&mut data, x, (yu + dy).min(half - 1), [200, 40, 30, 255]);
        }
        // The base elevation below, grey, with its range stretched over the lower half.
        let ye = half + 4 + ((e_hi - es[x]) / (e_hi - e_lo).max(1.0) * (half as f64 - 8.0)) as usize;
        for y in ye..H {
            put(&mut data, x, y.min(H - 1), [120, 120, 125, 255]);
        }
    }
    let (b_lon, b_lat) = lon_lat(belt);
    let (c_lon, c_lat) = lon_lat(craton);
    let length_km = libm::atan2(a.cross(b).length(), a.dot(b)) * EARTH_RADIUS_M / 1000.0;
    let text = format!(
        "section: from an active belt (lat {:.1}, lon {:.1}) over a back-arc to a craton (lat {:.1}, lon {:.1}), {:.0} km along a great circle; top: U (red line, 0 to {:.0}) with today's uniform {UNIFORM} dashed; bottom: the class source's base elevation {:.0} m to {:.0} m (not the carved terrain), {W} x {H} pixels\n",
        b_lat.to_degrees(),
        b_lon.to_degrees(),
        c_lat.to_degrees(),
        c_lon.to_degrees(),
        length_km,
        u_max,
        e_lo,
        e_hi
    );
    (Rgba { width: W as u32, height: H as u32, data }, text)
}

/// Writes the images and `uplift.txt` into `dir`.
pub fn write(dir: &Path, p: &Preview) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    p.uplift.write_png(&dir.join("uplift.png"))?;
    p.steepness.write_png(&dir.join("steepness.png"))?;
    p.section.write_png(&dir.join("section.png"))?;
    std::fs::write(dir.join("uplift.txt"), &p.text)
}
