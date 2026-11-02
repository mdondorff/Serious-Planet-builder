//! SPIKE-87 (spike branch only): the editor's hover pick (`pick_surface` through the world's field) at the cube corner of the owner's marker
//! "TheEdgeHill" against an edge and a face centre, on the real store (read only). Per pick: wall time, height calls, march steps; cold then warm.
//! SP87_STORE=<dir> (else the 256-cell generated world).
use planet::worldfield::WorldTerrain;
use planet_core::{PlanetFixed, Vec3, EARTH_RADIUS_M as R};
use planet_frame::pick::{cursor_ray, pick_surface, PickLimits};
use planet_frame::Camera;
use std::cell::Cell;
use std::time::Instant;

fn dir_of(lat: f64, lon: f64) -> Vec3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

fn main() {
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let t = Instant::now();
    let world = match std::env::var("SP87_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let field = world.field().unwrap();
    println!("open {:.1} s", t.elapsed().as_secs_f64());
    let limits = PickLimits::new(R, planet_frame::LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()));
    let sites = [
        ("corner TheEdgeHill 35.2538N 135.0217E", dir_of(35.2538, 135.0217)),
        ("edge 0N 45E", dir_of(0.0, 45.0)),
        ("edge 20N 45E", dir_of(20.0, 45.0)),
        ("face centre 0N 0E", dir_of(0.0, 0.0)),
        ("face 10N 10E", dir_of(10.0, 10.0)),
    ];
    let (w, h) = (2560.0, 1440.0);
    for (name, site) in sites {
        let ground = field.height(site);
        println!("== {name}: dir ({:.4}, {:.4}, {:.4}), ground {ground:.0} m", site.x, site.y, site.z);
        for alt in [100.0, 1_000.0, 10_000.0, 100_000.0] {
            for pitch in [0.3f64, 1.2] {
                let cam = Camera::at_surface_point(PlanetFixed(site * (R + ground.max(0.0) + alt)), Vec3::new(0.0, 0.0, 1.0), pitch, 1.0, w / h, 0.5);
                for pass in ["cold", "warm"] {
                    let (mut worst, mut sum, mut n, mut hits, mut calls_sum, mut worst_calls) = (0.0f64, 0.0f64, 0u32, 0u32, 0u64, 0u64);
                    let mut height_ms = 0.0f64;
                    for gx in 0..9 {
                        for gy in 0..5 {
                            let ray = cursor_ray(&cam, w * (f64::from(gx) + 0.5) / 9.0, h * (f64::from(gy) + 0.5) / 5.0, w, h);
                            let calls = Cell::new(0u64);
                            let hms = Cell::new(0.0f64);
                            let t = Instant::now();
                            let hit = pick_surface(&ray, &limits, &|d| {
                                calls.set(calls.get() + 1);
                                let t1 = Instant::now();
                                let v = field.height(d);
                                hms.set(hms.get() + t1.elapsed().as_secs_f64() * 1000.0);
                                v
                            });
                            let ms = t.elapsed().as_secs_f64() * 1000.0;
                            worst = worst.max(ms);
                            if ms >= worst {
                                worst_calls = calls.get();
                            }
                            sum += ms;
                            n += 1;
                            hits += u32::from(hit.is_some());
                            calls_sum += calls.get();
                            height_ms += hms.get();
                        }
                    }
                    println!(
                        "  alt {alt:>7.0} m pitch {pitch:.1} {pass}: {n} picks, {hits} hits, mean {:.2} ms, worst {worst:.1} ms ({worst_calls} calls), mean calls {:.0}, height share {:.0} %, {:.2} us/call",
                        sum / f64::from(n),
                        calls_sum as f64 / f64::from(n),
                        100.0 * height_ms / sum.max(1e-9),
                        1000.0 * height_ms / (calls_sum.max(1) as f64)
                    );
                }
            }
        }
    }
}
