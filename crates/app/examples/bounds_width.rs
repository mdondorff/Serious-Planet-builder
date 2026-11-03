//! Diagnosis probe (inbox #51, not committed): the extent `hi - lo` of `height_bounds` of caps of several sizes around a site, `cpu-erosion-v1` against
//! `cpu-erosion-v2`, medians over random caps. `cargo run --release -p planet --example bounds_width`

use std::path::PathBuf;

use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_world_def::WorldDefinition;

fn main() -> Result<(), String> {
    let store = PathBuf::from("C:/DEV/SeriousPlanetData/global16384-m9r2");
    let worlds: Vec<WorldTerrain> = ["cpu-erosion-v1", "cpu-erosion-v2"]
        .iter()
        .map(|id| {
            let mut def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
            def.set_height_field(id);
            WorldTerrain::open_store(def, &store, 16384)
        })
        .collect::<Result<_, _>>()?;
    let fields: Vec<_> = worlds.iter().map(|w| w.field()).collect::<Result<_, _>>()?;
    let (la, lo) = (19.1589f64.to_radians(), 155.3686f64.to_radians());
    let centre = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    let mut rng = planet_core::hash::SplitMix64(3);
    println!("| cap radius (m) | v1 median extent (m) | v2 median extent (m) | ratio |\n|---|---|---|---|");
    for theta_m in [40.0, 150.0, 300.0, 600.0, 1200.0, 2400.0] {
        let theta = theta_m / planet_core::EARTH_RADIUS_M;
        let caps: Vec<Vec3> = (0..400)
            .map(|_| (centre + Vec3::new(rng.next_f64() - 0.5, rng.next_f64() - 0.5, rng.next_f64() - 0.5) * 1e-3).normalized())
            .collect();
        let med = |f: &dyn planet_generators::height::HeightField| {
            let mut v: Vec<f64> = caps
                .iter()
                .map(|c| {
                    let (l, h) = f.height_bounds(*c, theta);
                    h - l
                })
                .collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let (a, b) = (med(fields[0].as_ref()), med(fields[1].as_ref()));
        println!("| {theta_m} | {a:.0} | {b:.0} | {:.2} |", b / a);
    }
    Ok(())
}
