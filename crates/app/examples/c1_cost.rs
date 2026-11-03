//! CPU cost of the C1 base read against the bilinear one (GEN-107, inbox #50): ns per `height(dir)`, ms per tile of samples and ns per `height_bounds`,
//! `cpu-erosion-v1` against `cpu-erosion-v2` on the same store and build, interleaved, the median of the repeats. Not a perf window; indicative.
//! `cargo run --release -p planet --example c1_cost -- [--store DIR] [--reps N]`

use std::path::PathBuf;
use std::time::Instant;

use planet::worldfield::WorldTerrain;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_world_def::WorldDefinition;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn site(lat: f64, lon: f64) -> Vec3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

fn main() -> Result<(), String> {
    let mut store = PathBuf::from("C:/DEV/SeriousPlanetData/global16384-m9r2");
    let mut reps = 7usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--store" => store = it.next().ok_or("--store")?.into(),
            "--reps" => reps = it.next().ok_or("--reps")?.parse().map_err(|e| format!("{e}"))?,
            o => return Err(format!("unknown {o}")),
        }
    }
    let worlds: Vec<WorldTerrain> = ["cpu-erosion-v1", "cpu-erosion-v2"]
        .iter()
        .map(|id| {
            let mut def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
            def.set_height_field(id);
            WorldTerrain::open_store(def, &store, 16384)
        })
        .collect::<Result<_, _>>()?;
    let fields: Vec<_> = worlds.iter().map(|w| w.field()).collect::<Result<_, _>>()?;
    let centre = site(18.97, 155.34);
    // Random directions within about 4 km of the site, the same for both fields.
    let mut rng = planet_core::hash::SplitMix64(1);
    let near = |rng: &mut planet_core::hash::SplitMix64, spread: f64| {
        (centre + Vec3::new(rng.next_f64() - 0.5, rng.next_f64() - 0.5, rng.next_f64() - 0.5) * spread).normalized()
    };
    let dirs: Vec<Vec3> = (0..20_000).map(|_| near(&mut rng, 1.2e-3)).collect();
    // A tile of the flight: 35 x 35 samples (32 cells and a halo of one) over a patch of 1.2 km, level 13.
    let (face, s0, t0) = planet_core::cube::direction_to_face(&TangentWarp, centre);
    let patch = 2.0 / f64::from(1u32 << 13);
    let tile: Vec<Vec3> = (0..35 * 35)
        .map(|k| {
            let (c, r) = ((k % 35) as f64 - 1.0, (k / 35) as f64 - 1.0);
            face_to_direction(&TangentWarp, Face(face.0), s0 + (c / 32.0 - 0.5) * patch, t0 + (r / 32.0 - 0.5) * patch)
        })
        .collect();
    let caps: Vec<(Vec3, f64)> = (0..3000).map(|i| (near(&mut rng, 1.2e-3), [1e-5, 1e-4, 1e-3][i % 3])).collect();
    // Warm the block cache of both fields over every direction the timings read, then time interleaved.
    for f in &fields {
        for d in &dirs {
            f.height(*d);
        }
        f.heights_until(&tile, &|| false);
        for (c, t) in &caps {
            f.height_bounds(*c, *t);
        }
    }
    let (mut single, mut tiles, mut bounds) = ([vec![], vec![]], [vec![], vec![]], [vec![], vec![]]);
    for _ in 0..reps {
        for (k, f) in fields.iter().enumerate() {
            let t = Instant::now();
            let mut acc = 0.0;
            for d in &dirs {
                acc += f.height(*d);
            }
            single[k].push(t.elapsed().as_secs_f64() * 1e9 / dirs.len() as f64);
            let t = Instant::now();
            let h = f.heights_until(&tile, &|| false);
            tiles[k].push(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            let mut b = 0.0;
            for (c, th) in &caps {
                let (lo, hi) = f.height_bounds(*c, *th);
                b += hi - lo;
            }
            bounds[k].push(t.elapsed().as_secs_f64() * 1e9 / caps.len() as f64);
            std::hint::black_box((acc, h.len(), b));
        }
    }
    println!("| measure | cpu-erosion-v1 | cpu-erosion-v2 | ratio |\n|---|---|---|---|");
    for (name, v) in
        [("ns per height(dir)", &single), ("ms per 35 x 35 tile (1,225 samples, gathered)", &tiles), ("ns per height_bounds", &bounds)]
    {
        let (a, b) = (median(v[0].clone()), median(v[1].clone()));
        println!("| {name} | {a:.1} | {b:.1} | {:.2} |", b / a);
    }
    println!("median of {reps} interleaved repeats; {} heights, {} caps per repeat", dirs.len(), caps.len());
    Ok(())
}
