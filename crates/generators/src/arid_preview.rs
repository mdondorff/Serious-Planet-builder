//! The planet preview of the arid class rules (GEN-128; ADR 0104 point 4): the class map with and without the rules, the weights of badlands and sand sea, the land shares of every class against Earth's, and
//! how far the blended `k` steps between neighbouring lattice points before and after. Every image shows the **class weights, an input of the bake, not terrain**; the values are start values for the owner's review.
//!
//! GPU-free, `libm`, integer hashing: the same bytes for any thread count (CON-14).

use crate::arid::{BADLANDS, SAND_SEA};
use crate::bake::inputs::BakeInputs;
use crate::classes::{CLASS_COUNT, CLASS_K, CLASS_NAMES, EARTH_RADIUS_M};
use crate::dump::Rgba;
use crate::lithology_preview::{direction, lerp, rgba, OCEAN};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};
use std::path::Path;

/// The images and the numbers of a preview.
pub struct Preview {
    pub images: Vec<(&'static str, Rgba)>,
    pub text: String,
}

/// The colour of a class on the class map.
const CLASS_COLOURS: [[f64; 3]; CLASS_COUNT] = [
    [150.0, 200.0, 110.0],
    [190.0, 215.0, 130.0],
    [190.0, 150.0, 90.0],
    [140.0, 120.0, 110.0],
    [245.0, 245.0, 250.0],
    [225.0, 170.0, 90.0],
    [200.0, 50.0, 40.0],
    [245.0, 215.0, 90.0],
];

/// Earth's shares of the ice-free land the report compares with: the relief classes of SPIKE-80 (plains, hills, mountains) and SPIKE-77's start targets for badlands and sand sea.
const EARTH_PLAINS: f64 = 0.61;
const EARTH_HILLS: f64 = 0.14;
const EARTH_MOUNTAINS: f64 = 0.25;
const TARGET_BADLANDS: f64 = 0.033;
const TARGET_SAND_SEA: f64 = 0.079;
/// The step of the blended `k` between neighbouring lattice points that the report counts.
const K_STEP_REPORTED: f64 = 0.5;

fn argmax(w: &[f64; CLASS_COUNT]) -> usize {
    (0..CLASS_COUNT).fold(0, |b, c| if w[c] > w[b] { c } else { b })
}

fn k_of(w: &[f64; CLASS_COUNT]) -> f64 {
    w.iter().zip(CLASS_K).map(|(w, k)| w * k).sum()
}

/// The largest step of `k`, its 99th percentile and the share of steps over [`K_STEP_REPORTED`], between neighbouring lattice points of the class lattice, for the weights `read`.
fn k_steps(inputs: &BakeInputs, par: &dyn Par, read: &(dyn Fn(Vec3) -> [f64; CLASS_COUNT] + Sync)) -> (f64, f64, f64) {
    let lattice = inputs.class_lattice();
    let n = lattice.cells();
    let rows = par::map(par, 6 * (n + 1), &|r| {
        let (f, j) = (r / (n + 1), r % (n + 1));
        let k = |i: usize, j: usize| k_of(&read(lattice.direction(Face(f as u8), i, j)));
        let mut steps = Vec::with_capacity(2 * n);
        for i in 0..=n {
            let here = k(i, j);
            if i < n {
                steps.push((here - k(i + 1, j)).abs());
            }
            if j < n {
                steps.push((here - k(i, j + 1)).abs());
            }
        }
        steps
    });
    let mut steps: Vec<f64> = rows.into_iter().flatten().collect();
    steps.sort_by(f64::total_cmp);
    let q = |f: f64| steps.get(((steps.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(0.0);
    let over = steps.iter().filter(|s| **s > K_STEP_REPORTED).count() as f64 / steps.len().max(1) as f64;
    (steps.last().copied().unwrap_or(0.0), q(0.99), over)
}

/// The maps at `km` kilometres a pixel and the numbers, on `threads` threads.
pub fn preview(inputs: &BakeInputs, km: f64, threads: usize) -> Preview {
    let width = libm::round(core::f64::consts::TAU * EARTH_RADIUS_M / 1000.0 / km) as usize;
    let height = width / 2;
    let par: &dyn Par = &threads;
    struct Px {
        land: bool,
        base: [f64; CLASS_COUNT],
        after: [f64; CLASS_COUNT],
    }
    let rows = par::map(par, height, &|y| {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        (0..width)
            .map(|x| {
                let lon = (x as f64 + 0.5) / width as f64 * core::f64::consts::TAU - core::f64::consts::PI;
                let d = direction(lon, lat);
                Px { land: inputs.orography().base_elevation_m(d) > 0.0, base: inputs.base_weights_at(d), after: inputs.weights_at(d) }
            })
            .collect::<Vec<_>>()
    });
    let image = |colour: &dyn Fn(&Px) -> [u8; 4]| {
        let mut data = Vec::with_capacity(width * height * 4);
        for row in &rows {
            for p in row {
                data.extend_from_slice(&if p.land { colour(p) } else { OCEAN });
            }
        }
        Rgba { width: width as u32, height: height as u32, data }
    };
    let class_colour = |w: &[f64; CLASS_COUNT]| rgba(CLASS_COLOURS[argmax(w)]);
    let weight_colour = |w: f64| rgba(lerp([235.0, 235.0, 230.0], [190.0, 40.0, 30.0], (w / 0.8).clamp(0.0, 1.0)));
    let images = vec![
        ("classes_after.png", image(&|p| class_colour(&p.after))),
        ("classes_before.png", image(&|p| class_colour(&p.base))),
        ("badlands_weight.png", image(&|p| weight_colour(p.after[BADLANDS]))),
        ("sand_sea_weight.png", image(&|p| weight_colour(p.after[SAND_SEA]))),
    ];
    // The shares of land by class (the mean weight, by area: pixels weigh by the cosine of the latitude).
    let (mut total, mut before, mut after) = (0.0, [0.0f64; CLASS_COUNT], [0.0f64; CLASS_COUNT]);
    for (y, row) in rows.iter().enumerate() {
        let w = libm::cos(core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI);
        for p in row.iter().filter(|p| p.land) {
            total += w;
            for c in 0..CLASS_COUNT {
                before[c] += w * p.base[c];
                after[c] += w * p.after[c];
            }
        }
    }
    let share = |v: &[f64; CLASS_COUNT], c: usize| 100.0 * v[c] / total.max(1e-9);
    let mut text = String::new();
    text.push_str("arid class rules (input), not terrain: the preview of GEN-128; every number is a start value for the owner's review\n");
    text.push_str(&format!(
        "seed {}, {km} km a pixel, climate lattice {} cells a face edge, {} land pixels\n",
        inputs.seed(),
        inputs.orography().cells(),
        rows.iter().flatten().filter(|p| p.land).count()
    ));
    text.push_str("land share by class (the mean class weight by area), before the rules and after:\n");
    for c in 0..CLASS_COUNT {
        text.push_str(&format!("  {:<14} {:>5.1} % -> {:>5.1} %\n", CLASS_NAMES[c], share(&before, c), share(&after, c)));
    }
    let (plains, hills, mountains) =
        (share(&after, 0) + share(&after, 1) + share(&after, 5), share(&after, 2), share(&after, 3) + share(&after, 4));
    text.push_str(&format!(
        "against Earth's (SPIKE-80; plateaus counted with the plains): plains {plains:.1} % ({:.0} %), hills {hills:.1} % ({:.0} %), mountains {mountains:.1} % ({:.0} %)
",
        100.0 * EARTH_PLAINS,
        100.0 * EARTH_HILLS,
        100.0 * EARTH_MOUNTAINS
    ));
    text.push_str(&format!(
        "against SPIKE-77's start targets: badlands {:.1} % (target {:.1} %), sand sea {:.1} % (target {:.1} %); while the climate is wetter than Earth's (GEN-117's dry share), the shares are low and the rules are not tuned to compensate\n",
        share(&after, BADLANDS),
        100.0 * TARGET_BADLANDS,
        share(&after, SAND_SEA),
        100.0 * TARGET_SAND_SEA
    ));
    let (b_max, b_p99, b_over) = k_steps(inputs, par, &|d| inputs.base_weights_at(d));
    let (a_max, a_p99, a_over) = k_steps(inputs, par, &|d| inputs.weights_at(d));
    text.push_str(&format!(
        "step of the blended k between neighbouring class-lattice points (the share over {K_STEP_REPORTED}): before: largest {b_max:.2}, p99 {b_p99:.2}, over {:.4} %; after: largest {a_max:.2}, p99 {a_p99:.2}, over {:.4} %\n",
        100.0 * b_over,
        100.0 * a_over
    ));
    Preview { images, text }
}

/// The class weights at a point, before and after the rules, as one line of text: `(badlands, sand sea)` and the blended `k`.
pub fn site_line(inputs: &BakeInputs, lat_deg: f64, lon_deg: f64) -> String {
    let d = direction(lon_deg.to_radians(), lat_deg.to_radians());
    let (base, after) = (inputs.base_weights_at(d), inputs.weights_at(d));
    format!(
        "  ({lat_deg:.1}, {lon_deg:.1}): class {} -> {}, badlands {:.2} -> {:.2}, sand sea {:.2} -> {:.2}, k {:.2} -> {:.2}, aridity {:.2}, hardness {:.2}\n",
        CLASS_NAMES[argmax(&base)],
        CLASS_NAMES[argmax(&after)],
        base[BADLANDS],
        after[BADLANDS],
        base[SAND_SEA],
        after[SAND_SEA],
        k_of(&base),
        k_of(&after),
        inputs.orography().aridity(d),
        inputs.lithology().hardness(d)
    )
}

/// Writes the images and `arid.txt` (the text, with `extra` appended) into `dir`.
pub fn write(dir: &Path, p: &Preview, extra: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, img) in &p.images {
        img.write_png(&dir.join(name))?;
    }
    std::fs::write(dir.join("arid.txt"), format!("{}{extra}", p.text))
}
