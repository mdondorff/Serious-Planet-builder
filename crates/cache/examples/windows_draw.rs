//! BUILD-066 (lead #186a): G3-8's out-of-sample windows, a seeded stratified draw over the seed-14 world's coarse field (`cargo xtask windows-draw`). Each candidate window
//! (`planet_cache::window_draw::candidates`) is sampled on a 16 x 16 grid of its regional cells' centres: the class inputs' base elevation (land above 0 m), the orography's
//! precipitation and temperature (runoff by `runoff_mm`), and the latitude of the box's centre; then classed and drawn by `window_draw`. Writes the data file (TOML).
//! Usage: `windows_draw [--out FILE] [--threads N]`.
use planet_cache::global_store::class_inputs;
use planet_cache::window_draw::{candidates, classify, draw, to_toml, Box, Stats, DRAW_SEED, REGIONAL, WORLD_SEED};
use planet_core::{Face, TangentWarp};
use planet_generators::global::sin_latitude;
use planet_generators::runoff::runoff_mm;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const SAMPLES: usize = 16;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = "crates/cache/data/g3-windows.toml".to_string();
    let mut threads = 8usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--out" => out = v.clone(),
            "--threads" => threads = v.parse().map_err(|_| format!("--threads N, not '{v}'"))?,
            other => return Err(format!("unknown option '{other}' (usage: windows_draw [--out FILE] [--threads N])")),
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
    let drawn = draw(&classed, DRAW_SEED)?;
    std::fs::write(&out, to_toml(&drawn, DRAW_SEED)).map_err(|e| format!("{out}: {e}"))?;
    for d in &drawn {
        let w = d.window;
        println!("WINDOWS-DRAW {} {} {},{},{},{},{},{WORLD_SEED}", d.index, d.stratum.name(), w.face, w.i0, w.i1, w.j0, w.j1);
    }
    println!("WINDOWS-DRAW written: {out}");
    Ok(())
}
