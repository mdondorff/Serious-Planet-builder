//! lead #203 (R23, spike only, never merged): two calibration windows for the intended-basin mark, drawn like BUILD-066's (#186a) over the seed-14 world's coarse field,
//! the arid stratum only, with the lead's draw seed (2026112403, decimal); a candidate overlapping the five windows or G3-8's 10 (BUILD-066's draw, reproduced here
//! with its own seed) or a calibration window already drawn is rejected and recorded. Writes `crates/cache/data/g3-calibration-windows.toml`.
//! Usage: `calib_draw [--out FILE] [--threads N]`.
use planet_cache::global_store::class_inputs;
use planet_cache::window_draw::{candidates, classify, draw, draw_key, Box, Stats, Stratum, DRAW_SEED, FIVE, REGIONAL, SIZE, WORLD_SEED};
use planet_core::{Face, TangentWarp};
use planet_generators::global::sin_latitude;
use planet_generators::runoff::runoff_mm;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const SAMPLES: usize = 16;
/// The lead's calibration draw seed (#203, 2026-11-24).
const CALIB_SEED: u64 = 2_026_112_403;
const COUNT: usize = 2;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = "crates/cache/data/g3-calibration-windows.toml".to_string();
    let mut threads = 8usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--out" => out = v.clone(),
            "--threads" => threads = v.parse().map_err(|_| format!("--threads N, not '{v}'"))?,
            other => return Err(format!("unknown option '{other}' (usage: calib_draw [--out FILE] [--threads N])")),
        }
    }
    let def = WorldDefinition::single_planet(R, WORLD_SEED);
    let inputs = class_inputs(&def, WORLD_SEED, threads);
    let orography = inputs.orography();
    let dir = |face: u8, i: f64, j: f64| {
        planet_core::cube::face_to_direction(&TangentWarp, Face(face), 2.0 * i / REGIONAL as f64 - 1.0, 2.0 * j / REGIONAL as f64 - 1.0)
    };
    let stats = |b: &Box| {
        let step = (b.i1 - b.i0 + 1) as f64 / SAMPLES as f64;
        let (mut land, mut runoff) = (Vec::new(), 0.0);
        for sj in 0..SAMPLES {
            for si in 0..SAMPLES {
                let d = dir(b.face, b.i0 as f64 + (si as f64 + 0.5) * step, b.j0 as f64 + (sj as f64 + 0.5) * step);
                let e = inputs.base(d);
                if e > 0.0 {
                    land.push(e);
                    runoff += runoff_mm(orography.precipitation_mm(d), orography.temperature_c(d));
                }
            }
        }
        land.sort_by(f64::total_cmp);
        let q = |p: f64| if land.is_empty() { 0.0 } else { land[((land.len() - 1) as f64 * p).round() as usize] };
        let centre = dir(b.face, (b.i0 + b.i1 + 1) as f64 / 2.0, (b.j0 + b.j1 + 1) as f64 / 2.0);
        Stats {
            land: land.len() as f64 / (SAMPLES * SAMPLES) as f64,
            land_p50_m: q(0.5),
            land_p90_m: q(0.9),
            runoff_mm: if land.is_empty() { 0.0 } else { runoff / land.len() as f64 },
            abs_lat_deg: libm::asin(sin_latitude(centre)).abs().to_degrees(),
        }
    };
    let mut classed = Vec::new();
    let mut counts = std::collections::BTreeMap::new();
    for face in 0..6u8 {
        for b in candidates(face) {
            let s = stats(&b);
            if let Some(st) = classify(&b, &s) {
                *counts.entry(st.name()).or_insert(0usize) += 1;
                classed.push((b, s, st));
            }
        }
    }
    println!("WINDOWS-DRAW candidates by stratum: {counts:?}");
    // G3-8's 10, reproduced with BUILD-066's seed (the same rule as g3-windows.toml)
    let ten = draw(&classed, DRAW_SEED)?;
    let mut pool: Vec<&(Box, Stats, Stratum)> = classed.iter().filter(|c| c.2 == Stratum::Arid).collect();
    pool.sort_by_key(|c| (draw_key(CALIB_SEED, &c.0), c.0.face, c.0.i0, c.0.j0));
    let mut taken: Vec<(Box, Stats)> = Vec::new();
    let mut rejected: Vec<(Box, String)> = Vec::new();
    for c in pool {
        if taken.len() == COUNT {
            break;
        }
        let why = if let Some(k) = FIVE.iter().position(|f| f.overlaps(&c.0)) {
            Some(format!("overlaps the five's {}", ["R1h", "R3a", "R3b", "M1", "F1"][k]))
        } else if let Some(d) = ten.iter().find(|d| d.window.overlaps(&c.0)) {
            Some(format!("overlaps G3-8 window {} ({})", d.index, d.stratum.name()))
        } else {
            taken.iter().position(|t| t.0.overlaps(&c.0)).map(|k| format!("overlaps calibration window {k}"))
        };
        match why {
            Some(w) => rejected.push((c.0, w)),
            None => taken.push((c.0, c.1)),
        }
    }
    if taken.len() < COUNT {
        return Err(format!("the arid stratum has {} free candidates, needs {COUNT}", taken.len()));
    }
    let wstr = |w: &Box| format!("{},{},{},{},{},{WORLD_SEED}", w.face, w.i0, w.i1, w.j0, w.j1);
    let mut s = format!(
        "# R23's calibration windows (lead #203, 2026-11-24): the arid stratum of BUILD-066's draw (#186a) over the coarse field of the seed-{WORLD_SEED} world, by the
         # spike example `calib_draw`, with the lead's draw seed. Not G3-8's windows (those are g3-windows.toml and stay unseen until G3-8).
         # A window is `face,i0,i1,j0,j1,seed` in regional cells. Rejected candidates are listed in the draw's order, before and between the windows taken.
         world_seed = {WORLD_SEED}
draw_seed = {CALIB_SEED}
size = {SIZE}
stratum = \"{}\"
",
        Stratum::Arid.name()
    );
    for (w, why) in &rejected {
        s += &format!(
            "
[[rejected]]
window = \"{}\"
reason = \"{why}\"
",
            wstr(w)
        );
    }
    for (k, (w, st)) in taken.iter().enumerate() {
        s += &format!(
            "
[[window]]
index = {k}
class = \"calibration-{}\"
window = \"{}\"
land = {:.3}
land_p50_m = {:.1}
land_p90_m = {:.1}
runoff_mm = {:.1}
abs_lat_deg = {:.2}
",
            Stratum::Arid.name(),
            wstr(w),
            st.land,
            st.land_p50_m,
            st.land_p90_m,
            st.runoff_mm,
            st.abs_lat_deg
        );
    }
    std::fs::write(&out, s).map_err(|e| format!("{out}: {e}"))?;
    for (w, why) in &rejected {
        println!("CALIB-DRAW rejected {}: {why}", wstr(w));
    }
    for (k, (w, _)) in taken.iter().enumerate() {
        println!("CALIB-DRAW {k} {}", wstr(w));
    }
    println!("CALIB-DRAW written: {out}");
    Ok(())
}
