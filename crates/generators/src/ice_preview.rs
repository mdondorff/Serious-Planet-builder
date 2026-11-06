//! The planet preview of the ice history (GEN-134; ADR 0106 point 1): the ice-extent map, the equilibrium-line map, the land share under ice against the glacial maximum of Earth, and the ice
//! cover at the terrain scorecard's glaciated sites. Every image shows the **ice history, an input of the bake, not terrain**; the values are start values for the owner's review.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14).

use crate::bake::inputs::BakeInputs;
use crate::classes::EARTH_RADIUS_M;
use crate::dump::Rgba;
use crate::ice::{IceHistory, IcePoint};
use crate::lithology_preview::{direction, lerp, rgba, OCEAN};
use planet_core::par::{self, Par};
use std::path::Path;

/// The images and the numbers of a preview.
pub struct Preview {
    pub images: Vec<(&'static str, Rgba)>,
    pub text: String,
}

/// Earth's land under ice at the last glacial maximum, as a share of the land (about 25 to 30 %: 44 Mkm2 of ice on 149 Mkm2 of land, Clark et al. 2009).
const EARTH_LGM_SHARE: f64 = 25.0;
/// The cover from which ground counts as under ice in the shares.
const UNDER_ICE: f64 = 0.5;

/// The maps at `km` kilometres a pixel and the numbers, on `threads` threads.
pub fn preview(inputs: &BakeInputs, history: &IceHistory, km: f64, threads: usize) -> Preview {
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    let climate = inputs.orography();
    let rows = par::map(par, height, &|y| {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        (0..width)
            .map(|x| {
                let lon = (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
                history.at(climate, direction(lon, lat))
            })
            .collect::<Vec<IcePoint>>()
    });
    let image = |colour: &dyn Fn(&IcePoint) -> [u8; 4]| {
        let mut data = Vec::with_capacity(width * height * 4);
        for row in &rows {
            for p in row {
                data.extend_from_slice(&if p.ground_m > 0.0 { colour(p) } else { OCEAN });
            }
        }
        Rgba { width: width as u32, height: height as u32, data }
    };
    let extent = |p: &IcePoint| {
        rgba(lerp(lerp([120.0, 170.0, 90.0], [150.0, 130.0, 100.0], (p.ground_m / 3000.0).clamp(0.0, 1.0)), [245.0, 250.0, 255.0], p.cover))
    };
    let ela = |p: &IcePoint| {
        // The line from deep blue (under sea level) through green to red (6,000 m).
        let t = ((p.ela_m + 1000.0) / 7000.0).clamp(0.0, 1.0);
        rgba(lerp(lerp([40.0, 60.0, 190.0], [90.0, 190.0, 90.0], (2.0 * t).min(1.0)), [210.0, 60.0, 40.0], (2.0 * t - 1.0).max(0.0)))
    };
    let images = vec![("ice_extent.png", image(&extent)), ("ela.png", image(&ela))];
    // The shares by area (pixels weigh by the cosine of the latitude), over land.
    let (mut land, mut ice, mut ice_high_sheet) = (0.0, 0.0, 0.0);
    let mut elas: Vec<(f64, f64)> = Vec::new();
    // The mid-latitude highlands (the Highlands, the Alps and the fjords of the gate): land of 800 m and more between 40 and 65 degrees of latitude, either hemisphere.
    let (mut high, mut high_ice, mut high_px) = (0.0, 0.0, 0usize);
    for (y, row) in rows.iter().enumerate() {
        let w = libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
        let lat_deg = (core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI).to_degrees().abs();
        for p in row.iter().filter(|p| p.ground_m > 0.0) {
            if (40.0..65.0).contains(&lat_deg) && p.ground_m >= 800.0 {
                high += w;
                high_px += 1;
                high_ice += if p.cover >= UNDER_ICE { w } else { 0.0 };
            }
            land += w;
            if p.cover >= UNDER_ICE {
                ice += w;
                if p.ela_m <= 0.0 {
                    ice_high_sheet += w;
                }
            }
            elas.push((p.ela_m, w));
        }
    }
    // Quantiles by area: walk the cumulative weight of the lines in order.
    elas.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total_w: f64 = elas.iter().map(|e| e.1).sum();
    let q = |f: f64| {
        let mut acc = 0.0;
        for &(v, w) in &elas {
            acc += w;
            if acc >= f * total_w {
                return v;
            }
        }
        f64::NAN
    };
    let params = &history.params;
    let mut text = String::new();
    text.push_str("ice history (input), not terrain: the preview of GEN-134; every number is a start value for the owner's review\n");
    text.push_str(&format!(
        "seed {}, {km} km a pixel, climate lattice {} cells a face edge, {} land pixels\n",
        inputs.seed(),
        climate.cells(),
        elas.len()
    ));
    text.push_str(&format!(
        "parameters (Earth preset): cooling {} C, dry equilibrium temperature {} C at {} mm, {} C a decade of rain, range {} to {} C, cover ramp {} m\n",
        params.lgm_cooling_c,
        params.ela_dry_temp_c,
        params.ela_dry_precip_mm,
        params.ela_temp_per_decade_c,
        params.ela_temp_range_c.0,
        params.ela_temp_range_c.1,
        params.cover_ramp_m
    ));
    text.push_str(&format!(
        "land under ice (cover over {UNDER_ICE}, by area): {:.1} % (Earth at the glacial maximum about {EARTH_LGM_SHARE:.0} %); of it, sheets (the line under sea level): {:.1} % of the land\n",
        100.0 * ice / land.max(1e-9),
        100.0 * ice_high_sheet / land.max(1e-9)
    ));
    text.push_str(&format!(
        "equilibrium line over the land (m, by area): p5 {:.0}, p25 {:.0}, p50 {:.0}, p75 {:.0}, p95 {:.0}\n",
        q(0.05),
        q(0.25),
        q(0.5),
        q(0.75),
        q(0.95)
    ));
    text.push_str(&format!(
        "mid-latitude highlands (40 to 65 degrees, ground of 800 m and more, the analogues of the Highlands, the Alps and the fjords): {high_px} pixels, {:.1} % under ice (by area) (the gate's reference sites were all glaciated)
",
        100.0 * high_ice / high.max(1e-9)
    ));
    Preview { images, text }
}

/// The ice history at a point as one line of text.
pub fn site_line(inputs: &BakeInputs, history: &IceHistory, lat_deg: f64, lon_deg: f64) -> String {
    let p = history.at(inputs.orography(), direction(lon_deg.to_radians(), lat_deg.to_radians()));
    format!(
        "  ({lat_deg:.1}, {lon_deg:.1}): ground {:.0} m, equilibrium line {:.0} m, ice cover {:.2}{}\n",
        p.ground_m,
        p.ela_m,
        p.cover,
        if p.cover >= UNDER_ICE { " (under ice)" } else { "" }
    )
}

/// Writes the images and `ice.txt` (the text, with `extra` appended) into `dir`.
pub fn write(dir: &Path, p: &Preview, extra: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, img) in &p.images {
        img.write_png(&dir.join(name))?;
    }
    std::fs::write(dir.join("ice.txt"), format!("{}{extra}", p.text))
}
