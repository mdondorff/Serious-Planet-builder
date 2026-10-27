//! A read-only probe of the coarse horizon over the store (B6, owner test 2026-10-27): around a point it prints the stored horizon toward a
//! sun azimuth and, for the highest cell, the run's heights along the march against the heights the base field draws there.
//!
//! `cargo run --release -p planet --example coarse_probe -- STORE LAT LON HEADING_DEG DIST_KM SUN_AZ_DEG`

use planet::horizonworld::CoarseStanding;
use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_generators::coarsehorizon::geographic_frame;
use planet_generators::coupled::Lattice;
use planet_generators::horizon::dequantize;
use planet_world_def::WorldDefinition;
use std::sync::Arc;

const R: f64 = 6_371_000.0;

fn dir_of(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

/// The point `s` metres from `d` along the azimuth `az` (radians from north).
fn along(d: Vec3, az: f64, s: f64) -> Vec3 {
    let (east, north) = geographic_frame(d);
    let t = north * libm::cos(az) + east * libm::sin(az);
    let th = s / R;
    d * libm::cos(th) + t * libm::sin(th)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let store = std::path::PathBuf::from(&a[1]);
    let (lat, lon, heading, dist_km, sun_az): (f64, f64, f64, f64, f64) =
        (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap());
    let world = Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, 1), &store, 16384).expect("the store opens"));
    world.request_coupled();
    let maps = loop {
        match world.coupled_standing() {
            planet::coupledworld::Standing::Ready(m, _) => break m,
            planet::coupledworld::Standing::Failed(e) => panic!("{e}"),
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    };
    world.request_coarse_horizon();
    let coarse = loop {
        match world.coarse_standing() {
            CoarseStanding::Ready(h) => break h,
            CoarseStanding::Idle | CoarseStanding::Working => std::thread::sleep(std::time::Duration::from_millis(100)),
            s => panic!("{s:?}"),
        }
    };
    let field = world.field().expect("the field");
    let m = coarse.m as usize;
    let az = coarse.azimuths as usize;
    let obs = Lattice::new(m, R);
    let cell_m = std::f64::consts::FRAC_PI_2 * R / m as f64;
    let sun_idx = ((sun_az / 360.0 * az as f64).round() as usize) % az;
    println!(
        "coarse horizon {m} cells per face edge ({:.1} km), {az} azimuths; the sun at {sun_az} deg is azimuth index {sun_idx}",
        cell_m / 1000.0
    );
    let camera = dir_of(lat, lon);
    let centre = along(camera, heading.to_radians(), dist_km * 1000.0);
    let (clat, clon) = (libm::asin(centre.z).to_degrees(), libm::atan2(centre.y, centre.x).to_degrees());
    println!("the point {dist_km} km along {heading} deg: lat {clat:.3} lon {clon:.3}");
    println!("horizon angle toward the sun (deg), rows from the near side to the far, columns left to right across the heading:");
    let mut best = (f64::MIN, 0usize);
    for row in -3i32..=3 {
        let mut line = String::new();
        for col in -3i32..=3 {
            let p =
                along(along(centre, heading.to_radians(), f64::from(row) * cell_m), (heading + 90.0).to_radians(), f64::from(col) * cell_m);
            let cell = obs.cell_at(p);
            let deg = dequantize(coarse.bytes[cell * az + sun_idx]).to_degrees();
            if deg > best.0 {
                best = (deg, cell);
            }
            line += &format!("{deg:6.2}");
        }
        println!("{line}");
    }
    let (deg, cell) = best;
    let d0 = obs.direction(cell);
    println!("highest cell {cell}: {deg:.2} deg toward the sun; the march (4.9 km steps) toward azimuth {sun_az}:");
    println!("{:>6} {:>9} {:>9} {:>9}", "km", "run z", "base h", "diff");
    let t_az = sun_az.to_radians();
    for step in 1..=10 {
        let s = f64::from(step) * std::f64::consts::FRAC_PI_2 * R / 2048.0;
        let p = along(d0, t_az, s);
        let z = f64::from(maps.z(maps.lattice().cell_at(p)));
        let h = field.height(p);
        println!("{:6.1} {:9.1} {:9.1} {:9.1}", s / 1000.0, z, h, z - h);
    }
}
