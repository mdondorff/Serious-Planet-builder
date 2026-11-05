//! The planet preview of the orographic climate (GEN-130; GEN-117): equirectangular maps of the annual precipitation with the single annual wind and with the two seasons, their difference, the aridity,
//! the seasonality and the wind of both seasons with arrows, the material colour at four regions, and the numbers that decide the model: the windward and lee precipitation ratios across mountain
//! belts. Every image shows the **orographic climate, an input of the bake, not terrain**; the values are start values for the owner's review.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14).

use crate::classes::EARTH_RADIUS_M;
use crate::dump::Rgba;
use crate::lithology::ProvinceLattice;
use crate::lithology_preview::{direction, lerp, lon_lat, rgba, OCEAN};
use crate::orography::{aridity_index, BaseElevation, Orography, ANNUAL_WIND, OROGRAPHY_VERSION};
use planet_core::par::{self, Par};
use planet_core::Vec3;
use std::path::Path;

/// The images and the numbers of a preview.
pub struct Preview {
    /// `(file name, image)` of every map, in the order written.
    pub images: Vec<(&'static str, Rgba)>,
    /// The numbers: land shares by aridity class, precipitation quantiles, the windward and lee ratios.
    pub text: String,
}

/// Where the preview looks at ground level: the two anchors of the integrated preview and two the preview picks itself (the wettest windward flank and a median plain), in latitude and longitude degrees.
const ANCHORS: [(&str, f64, f64); 2] = [("hills", 22.4689, -21.0933), ("coast", -35.0827, -4.2183)];

fn ramp(stops: &[(f64, [f64; 3])], x: f64) -> [u8; 4] {
    let x = x.clamp(stops[0].0, stops[stops.len() - 1].0);
    for w in stops.windows(2) {
        if x <= w[1].0 {
            let t = (x - w[0].0) / (w[1].0 - w[0].0).max(1e-12);
            return rgba(lerp(w[0].1, w[1].1, t));
        }
    }
    rgba(stops[stops.len() - 1].1)
}

/// The precipitation ramp in mm a year: pale sand, green, teal, deep blue.
fn precipitation_colour(p: f64) -> [u8; 4] {
    ramp(
        &[
            (0.0, [235.0, 215.0, 160.0]),
            (500.0, [200.0, 215.0, 120.0]),
            (1200.0, [60.0, 160.0, 90.0]),
            (2500.0, [30.0, 110.0, 190.0]),
            (4500.0, [20.0, 30.0, 120.0]),
        ],
        p,
    )
}

/// The aridity classes of UNEP: hyper-arid under 0.05, arid to 0.2, semi-arid to 0.5, dry sub-humid to 0.65, humid above.
fn aridity_class(ai: f64) -> usize {
    match ai {
        a if a < 0.05 => 0,
        a if a < 0.2 => 1,
        a if a < 0.5 => 2,
        a if a < 0.65 => 3,
        _ => 4,
    }
}

const CLASS_NAMES: [&str; 5] =
    ["hyper-arid (AI < 0.05)", "arid (0.05 to 0.2)", "semi-arid (0.2 to 0.5)", "dry sub-humid (0.5 to 0.65)", "humid (over 0.65)"];
const CLASS_COLOURS: [[f64; 3]; 5] =
    [[150.0, 40.0, 30.0], [225.0, 130.0, 50.0], [235.0, 205.0, 90.0], [170.0, 205.0, 110.0], [50.0, 130.0, 90.0]];

/// An analytic stand-in for the ground material colour at a temperature and a precipitation (sand, steppe, grass, forest, tundra, ice): the preview's reading of the look, not the render's.
fn material_colour(t_c: f64, p_mm: f64) -> [u8; 4] {
    let warm = ramp(
        &[
            (0.0, [226.0, 200.0, 150.0]),
            (250.0, [222.0, 196.0, 140.0]),
            (500.0, [186.0, 172.0, 108.0]),
            (1000.0, [118.0, 158.0, 70.0]),
            (2000.0, [48.0, 108.0, 50.0]),
        ],
        p_mm,
    );
    let cold = ramp(&[(-20.0, [235.0, 238.0, 240.0]), (-8.0, [200.0, 205.0, 200.0]), (2.0, [150.0, 150.0, 128.0])], t_c);
    let w = ((t_c + 2.0) / 10.0).clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (f64::from(a) * (1.0 - w) + f64::from(b) * w).round() as u8;
    [mix(cold[0], warm[0]), mix(cold[1], warm[1]), mix(cold[2], warm[2]), 255]
}

fn image(width: usize, height: usize, f: &dyn Fn(usize, usize) -> [u8; 4]) -> Rgba {
    let mut data = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            data.extend_from_slice(&f(x, y));
        }
    }
    Rgba { width: width as u32, height: height as u32, data }
}

fn pixel_dir(x: usize, y: usize, width: usize, height: usize) -> Vec3 {
    let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
    let lon = (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
    direction(lon, lat)
}

/// A line of one pixel from `(x0, y0)` to `(x1, y1)` (Bresenham), clipped to the image.
fn line(img: &mut Rgba, x0: i64, y0: i64, x1: i64, y1: i64, c: [u8; 4]) {
    let (w, h) = (i64::from(img.width), i64::from(img.height));
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            let i = ((y * w + x) * 4) as usize;
            img.data[i..i + 4].copy_from_slice(&c);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// The ratios of the precipitation 100 to 300 km on the windward side of a belt to that on its lee side, over the crossings of the province lattice's belts (core term above 0.6, the wind across the belt at
/// least 2 m/s, the crest at least 300 m above both sides, both sides land): the number of crossings and the median, p10 and p90 and the shares above 1.5 and 3.
pub struct RatioStats {
    pub crossings: usize,
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
    pub above_1_5: f64,
    pub above_3: f64,
}

pub fn belt_ratios(o: &Orography, src: &dyn BaseElevation, provinces: &ProvinceLattice) -> RatioStats {
    let n = provinces.cells();
    let mut ratios = Vec::new();
    for f in 0..6u8 {
        for j in 0..=n {
            for i in 0..=n {
                if provinces.point(planet_core::Face(f), i, j)[1] < 153 {
                    continue;
                }
                let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
                let b = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(f), s, t);
                // The mean wind of the seasons decides which side is windward.
                let wind = (0..o.seasons().len()).fold(Vec3::ZERO, |a, k| a + o.wind(k, b)) * (1.0 / o.seasons().len() as f64);
                let speed = wind.length();
                if speed < 2.0 {
                    continue;
                }
                let t = wind * (1.0 / speed);
                let at = |km: f64| {
                    let angle = km * 1000.0 / EARTH_RADIUS_M;
                    (b * libm::cos(angle) + t * libm::sin(angle)).normalized()
                };
                // A crossing: both sides, 150 km away, are land and the belt's crest stands at least 300 m above each.
                let crest = src.e0(b);
                let (w150, l150) = (at(-150.0), at(150.0));
                let ok = src.land_depth(w150) > 0.0
                    && src.land_depth(l150) > 0.0
                    && crest > src.e0(w150) + 300.0
                    && crest > src.e0(l150) + 300.0;
                let (mut windward, mut lee) = (0.0, 0.0);
                for km in [100.0, 200.0, 300.0] {
                    windward += o.precipitation_mm(at(-km)) / 3.0;
                    lee += o.precipitation_mm(at(km)) / 3.0;
                }
                if ok && lee > 1.0 {
                    ratios.push(windward / lee);
                }
            }
        }
    }
    ratios.sort_by(f64::total_cmp);
    let q = |f: f64| ratios.get(((ratios.len() as f64 - 1.0).max(0.0) * f) as usize).copied().unwrap_or(0.0);
    let share = |x: f64| ratios.iter().filter(|r| **r > x).count() as f64 / ratios.len().max(1) as f64;
    RatioStats { crossings: ratios.len(), median: q(0.5), p10: q(0.1), p90: q(0.9), above_1_5: share(1.5), above_3: share(3.0) }
}

fn ratio_text(label: &str, s: &RatioStats) -> String {
    format!(
        "windward over lee ratio at {label}: {} belt crossings, median {:.2}, p10 {:.2}, p90 {:.2}, above 1.5 {:.0} %, above 3 {:.0} % (references: Cascades and Sierra Nevada 2 to 4, southern Andes 5 to 10, Himalaya 3 to 5; start target: a median of 1.5 to 4)\n",
        s.crossings,
        s.median,
        s.p10,
        s.p90,
        100.0 * s.above_1_5,
        100.0 * s.above_3
    )
}

/// The maps at `km` kilometres a pixel, the four regions and the numbers. `fine` is a second climate on a finer lattice whose ratio statistics are printed next to the production lattice's (the narrow
/// coastal ranges are a few cells wide on the coarse one).
pub fn preview(
    src: &dyn BaseElevation,
    seed: u64,
    provinces: &ProvinceLattice,
    two: &Orography,
    km: f64,
    fine: Option<&Orography>,
    threads: usize,
) -> Preview {
    let annual = Orography::build(seed, src, two.cells(), &ANNUAL_WIND, threads);
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    struct Px {
        land: bool,
        p_two: f64,
        p_one: f64,
        t: f64,
        ai: f64,
        seasonality: f64,
    }
    let rows = par::map(par, height, &|y| {
        (0..width)
            .map(|x| {
                let d = pixel_dir(x, y, width, height);
                let (p_two, t) = (two.precipitation_mm(d), two.temperature_c(d));
                Px {
                    land: src.land_depth(d) > 0.0,
                    p_two,
                    p_one: annual.precipitation_mm(d),
                    t,
                    ai: aridity_index(p_two, t),
                    seasonality: two.seasonality(d),
                }
            })
            .collect::<Vec<_>>()
    });
    let at = |x: usize, y: usize| &rows[y][x];
    let sea = |c: [u8; 4], land: bool| if land { c } else { OCEAN };
    let mut images: Vec<(&'static str, Rgba)> = Vec::new();
    images.push(("precipitation_annual_wind.png", image(width, height, &|x, y| sea(precipitation_colour(at(x, y).p_one), at(x, y).land))));
    images.push(("precipitation_two_seasons.png", image(width, height, &|x, y| sea(precipitation_colour(at(x, y).p_two), at(x, y).land))));
    images.push((
        "precipitation_difference.png",
        image(width, height, &|x, y| {
            let diff = at(x, y).p_two - at(x, y).p_one;
            sea(ramp(&[(-1000.0, [190.0, 50.0, 40.0]), (0.0, [240.0, 240.0, 235.0]), (1000.0, [40.0, 90.0, 190.0])], diff), at(x, y).land)
        }),
    ));
    images.push((
        "aridity.png",
        image(width, height, &|x, y| {
            let c = CLASS_COLOURS[aridity_class(at(x, y).ai)];
            sea(rgba(c), at(x, y).land)
        }),
    ));
    images.push((
        "seasonality.png",
        image(width, height, &|x, y| {
            sea(ramp(&[(0.0, [230.0, 230.0, 225.0]), (1.0, [110.0, 40.0, 140.0])], at(x, y).seasonality), at(x, y).land)
        }),
    ));
    // The wind of each season with arrows on a grid of 12 degrees; the base is the precipitation of that season.
    for (k, name) in [(0usize, "wind_northern_summer.png"), (1, "wind_northern_winter.png")] {
        let mut img = image(width, height, &|x, y| {
            let d = pixel_dir(x, y, width, height);
            sea(precipitation_colour(2.0 * two.season_precipitation_mm(k, d)), at(x, y).land)
        });
        let step_deg = 12.0;
        let mut lat = -78.0;
        while lat <= 78.0 {
            let mut lon = -174.0;
            while lon <= 174.0 {
                let d = direction(f64::to_radians(lon), f64::to_radians(lat));
                let w = two.wind(k, d);
                let (px, py) = ((lon + 180.0) / 360.0 * width as f64, (90.0 - lat) / 180.0 * height as f64);
                // East is +x and north is -y in the image; the arrow is the wind times 6 pixels per m/s on a map of 2,500 pixels, scaled with the map; orange, to stand out on land and sea.
                let (east, north) = {
                    let e = Vec3::new(-d.y, d.x, 0.0);
                    let east = if e.length() > 1e-9 { e.normalized() } else { Vec3::new(0.0, 1.0, 0.0) };
                    (east, d.cross(east))
                };
                let scale = width as f64 / 2500.0 * 6.0;
                let (dx, dy) = (w.dot(east) * scale, -w.dot(north) * scale);
                let (x1, y1) = (px + dx, py + dy);
                line(&mut img, px as i64, py as i64, x1 as i64, y1 as i64, [255, 140, 0, 255]);
                // The head: two short strokes back from the tip.
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                let (ux, uy) = (dx / len, dy / len);
                for s in [-0.5, 0.5] {
                    let (hx, hy) = (x1 - 4.0 * (ux * 0.87 - uy * s), y1 - 4.0 * (uy * 0.87 + ux * s));
                    line(&mut img, x1 as i64, y1 as i64, hx as i64, hy as i64, [255, 140, 0, 255]);
                }
                lon += step_deg;
            }
            lat += step_deg;
        }
        images.push((name, img));
    }
    // The four regions: the two anchors, the wettest windward flank (the land point of the largest two-season rain) and a median plain.
    let (mut best, mut best_p) = ((0usize, 0usize), -1.0);
    let mut plains: Vec<(f64, usize, usize)> = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, p) in row.iter().enumerate() {
            if p.land && p.p_two.min(4500.0) > best_p {
                best_p = p.p_two.min(4500.0);
                best = (x, y);
            }
            if p.land && p.t > 5.0 && y > height / 6 && y < 5 * height / 6 {
                plains.push((p.p_two, x, y));
            }
        }
    }
    plains.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let plain = plains.get(plains.len() / 2).map_or((0, 0), |p| (p.1, p.2));
    let mut regions: Vec<(String, Vec3)> =
        ANCHORS.iter().map(|(n, la, lo)| ((*n).to_string(), direction(lo.to_radians(), la.to_radians()))).collect();
    regions.push(("wettest flank".into(), pixel_dir(best.0, best.1, width, height)));
    regions.push(("median plain".into(), pixel_dir(plain.0, plain.1, width, height)));
    const R: usize = 100;
    let region_km = 1000.0;
    let mut panel = Rgba { width: (4 * R) as u32, height: (2 * R) as u32, data: vec![0; 4 * R * 2 * R * 4] };
    let mut region_text = String::new();
    for (k, (name, c)) in regions.iter().enumerate() {
        let e = Vec3::new(-c.y, c.x, 0.0);
        let east = if e.length() > 1e-9 { e.normalized() } else { Vec3::new(0.0, 1.0, 0.0) };
        let north = c.cross(east);
        for ry in 0..R {
            for rx in 0..R {
                let (ox, oy) = ((rx as f64 / R as f64 - 0.5) * region_km, (0.5 - ry as f64 / R as f64) * region_km);
                let dir = (*c * 1.0 + east * (ox * 1000.0 / EARTH_RADIUS_M) + north * (oy * 1000.0 / EARTH_RADIUS_M)).normalized();
                let (p, t) = (two.precipitation_mm(dir), two.temperature_c(dir));
                let land = src.land_depth(dir) > 0.0;
                for (row, colour) in [(0usize, precipitation_colour(p)), (1, material_colour(t, p))] {
                    let col = if land { colour } else { OCEAN };
                    let i = (((row * R + ry) * 4 * R) + k * R + rx) * 4;
                    panel.data[i..i + 4].copy_from_slice(&col);
                }
            }
        }
        let (lon, lat) = lon_lat(*c);
        region_text.push_str(&format!(
            "  {name}: lat {:.1}, lon {:.1}: precipitation {:.0} mm, temperature {:.1} C, aridity {:.2}, seasonality {:.2}\n",
            lat.to_degrees(),
            lon.to_degrees(),
            two.precipitation_mm(*c),
            two.temperature_c(*c),
            two.aridity(*c),
            two.seasonality(*c)
        ));
    }
    images.push(("regions.png", panel));
    // The numbers.
    let weight = |y: usize| libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
    let (mut total, mut classes, mut sum_two, mut sum_one) = (0.0, [0.0f64; 5], 0.0, 0.0);
    let (mut warm, mut warm_dry) = (0.0, 0.0);
    let mut ps: Vec<f64> = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for p in row.iter().filter(|p| p.land) {
            total += weight(y);
            classes[aridity_class(p.ai)] += weight(y);
            if p.t > 5.0 {
                warm += weight(y);
                warm_dry += weight(y) * f64::from(u8::from(p.ai < 0.5));
            }
            sum_two += weight(y) * p.p_two;
            sum_one += weight(y) * p.p_one;
            ps.push(p.p_two);
        }
    }
    ps.sort_by(f64::total_cmp);
    let q = |f: f64| ps.get(((ps.len() as f64 - 1.0).max(0.0) * f) as usize).copied().unwrap_or(0.0);
    let mut text = String::new();
    text.push_str(
        "orographic climate (input), not terrain: the preview of GEN-117 / GEN-130; every number is a start value for the owner's review\n",
    );
    text.push_str(&format!(
        "version {OROGRAPHY_VERSION}, seed {seed}, {km} km a pixel, lattice {} cells a face edge, {} seasons, {} land pixels\n",
        two.cells(),
        two.seasons().len(),
        ps.len()
    ));
    text.push_str(&format!(
        "land precipitation (mean by area, quantiles by pixel): two seasons mean {:.0} mm (the annual wind alone {:.0} mm), p5 {:.0}, p25 {:.0}, p50 {:.0}, p75 {:.0}, p95 {:.0}\n",
        sum_two / total.max(1e-9),
        sum_one / total.max(1e-9),
        q(0.05),
        q(0.25),
        q(0.5),
        q(0.75),
        q(0.95)
    ));
    text.push_str("land by aridity class (UNEP; Earth's ice-free land is about 30 to 40 % arid, semi-arid and hyper-arid together):\n");
    for (name, share) in CLASS_NAMES.iter().zip(classes) {
        text.push_str(&format!("  {name}: {:.1} %\n", 100.0 * share / total.max(1e-9)));
    }
    text.push_str(&format!(
        "dry land (aridity under 0.5, hyper-arid, arid and semi-arid): {:.1} % of land\n",
        100.0 * (classes[0] + classes[1] + classes[2]) / total.max(1e-9)
    ));
    text.push_str(&format!(
        "dry land among the land warmer than 5 C (the cold lands have a low PET, so UNEP's index calls them humid): {:.1} % of that land, which is {:.0} % of all land
",
        100.0 * warm_dry / warm.max(1e-9),
        100.0 * warm / total.max(1e-9)
    ));
    // The zonal means of the land by 15 degrees of latitude: the shape of Earth's climate belts (wet tropics, dry subtropics, wet mid-latitudes) is the first check of the model.
    text.push_str(
        "land precipitation by latitude band (mean by pixel, the share of its land that is dry by aridity under 0.5):
",
    );
    for band in 0..12 {
        let (lo, hi) = (-90.0 + 15.0 * band as f64, -75.0 + 15.0 * band as f64);
        let (mut sum, mut count, mut dry) = (0.0, 0usize, 0usize);
        for (y, row) in rows.iter().enumerate() {
            let lat = 90.0 - (y as f64 + 0.5) / height as f64 * 180.0;
            if lat >= lo && lat < hi {
                for p in row.iter().filter(|p| p.land) {
                    sum += p.p_two;
                    count += 1;
                    dry += usize::from(p.ai < 0.5);
                }
            }
        }
        if count > 0 {
            text.push_str(&format!(
                "  {lo:>4.0} to {hi:>4.0}: {:>5.0} mm, dry {:>3.0} % ({count} pixels)
",
                sum / count as f64,
                100.0 * dry as f64 / count as f64
            ));
        }
    }
    text.push_str("regions (the first two are the anchors of the integrated preview, the others are picked by the preview; the material colour is an analytic stand-in of the look, in regions.png: top row precipitation, bottom row material, each 1,000 km across):\n");
    text.push_str(&region_text);
    text.push_str(&ratio_text(&format!("{} cells a face edge", two.cells()), &belt_ratios(two, src, provinces)));
    if let Some(f) = fine {
        text.push_str(&ratio_text(&format!("{} cells a face edge", f.cells()), &belt_ratios(f, src, provinces)));
    }
    Preview { images, text }
}

/// Writes the images and `orography.txt` into `dir`.
pub fn write(dir: &Path, p: &Preview) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, img) in &p.images {
        img.write_png(&dir.join(name))?;
    }
    std::fs::write(dir.join("orography.txt"), &p.text)
}
