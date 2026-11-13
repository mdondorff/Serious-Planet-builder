//! A scripted long flight over a commit (E2, the lead's order of 2026-11-13): a path file in the F9 format (`planet-path 1`) shaped like the owner's flights, for
//! `cargo xtask repro <name>-path.txt --window --real` (BUILD-054). From orbit down to 3 km over the site, long travel over and past the region, several altitude
//! changes and stretches at 1 to 50 m, about 26 minutes at 10 poses a second. The ground under each pose is the world's drawn field, so the low stretches keep
//! their height above the terrain. A dev tool; not a test.
//!
//! `cargo run --release -p planet --example long_flight_path -- COMMIT_DIR OUT_PATH_TXT [LAT LON] [--threads T]`

use planet_core::Vec3;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const PERIOD_MS: u64 = 100;

/// One leg: its length in seconds, the height above the ground at its end (the start is the previous leg's end; `log` interpolates the height
/// geometrically, else linearly), the ground speed in m/s, the turn rate in degrees a second and the pitch below the horizon in degrees.
struct Leg {
    secs: f64,
    end_alt_m: f64,
    log: bool,
    speed_mps: f64,
    turn_dps: f64,
    pitch_deg: f64,
}

fn legs() -> Vec<Leg> {
    let l = |secs: f64, end_alt_m: f64, log: bool, speed_mps: f64, turn_dps: f64, pitch_deg: f64| Leg {
        secs,
        end_alt_m,
        log,
        speed_mps,
        turn_dps,
        pitch_deg,
    };
    vec![
        l(90.0, 3_000.0, true, 0.0, 0.0, 60.0),     // orbit (1,500 km) down to 3 km over the site
        l(300.0, 3_000.0, false, 700.0, 0.0, 20.0), // 210 km east at 3 km: over the region and past it
        l(60.0, 50.0, true, 300.0, 0.5, 25.0),      // down to 50 m, turning
        l(300.0, 50.0, false, 80.0, 0.3, 10.0),     // 24 km at 50 m
        l(60.0, 2.0, true, 30.0, 0.0, 8.0),         // down to 2 m
        l(180.0, 3.0, false, 20.0, 0.2, 5.0),       // 3.6 km at 1 to 5 m
        l(60.0, 1_000.0, true, 100.0, 1.5, 15.0),   // up to 1 km, turning back
        l(300.0, 1_000.0, false, 400.0, 0.4, 20.0), // 120 km at 1 km, a slow turn
        l(60.0, 20.0, true, 150.0, 0.0, 15.0),      // down to 20 m
        l(120.0, 20.0, false, 40.0, -0.5, 8.0),     // 4.8 km at 20 m
        l(30.0, 10_000.0, true, 200.0, 0.0, 40.0),  // up to 10 km
    ]
}

fn rotate_about(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    // Rodrigues' rotation of `v` about the unit `axis`.
    let (s, c) = (angle.sin(), angle.cos());
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, out) = (std::path::PathBuf::from(&args[0]), std::path::PathBuf::from(&args[1]));
    let nums: Vec<f64> = args[2..].iter().take_while(|a| !a.starts_with("--")).map(|s| s.parse().expect("LAT LON")).collect();
    let (lat, lon) = (nums.first().copied().unwrap_or(-36.7498), nums.get(1).copied().unwrap_or(-162.2827));
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    let def = WorldDefinition::single_planet(R, 14);
    let world = planet::worldfield::WorldTerrain::open_commit_in_memory(def, &dir, threads)?;
    let field = world.field()?;
    let ground = |d: Vec3| field.height(d.normalized());

    // The state: the direction under the camera, the heading (a unit tangent), the height above the ground.
    let mut dir_v = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
    let east = Vec3::new(0.0, 0.0, 1.0).cross(dir_v).normalized();
    let mut heading = east;
    let mut alt = 1_500_000.0f64;
    let mut t_ms: u64 = 0;
    let mut lines: Vec<String> = Vec::new();
    let mut lowest = f64::MAX;
    for leg in legs() {
        let steps = (leg.secs * 1000.0 / PERIOD_MS as f64).round() as usize;
        let start_alt = alt;
        for k in 1..=steps {
            let f = k as f64 / steps as f64;
            alt = if leg.log { start_alt * (leg.end_alt_m / start_alt).powf(f) } else { start_alt + (leg.end_alt_m - start_alt) * f };
            // Move along the great circle of the heading, then turn.
            let step_m = leg.speed_mps * PERIOD_MS as f64 / 1000.0;
            let axis = dir_v.cross(heading).normalized();
            let angle = step_m / R;
            dir_v = rotate_about(dir_v, axis, angle).normalized();
            heading = rotate_about(heading, axis, angle);
            heading = (heading - dir_v * heading.dot(dir_v)).normalized();
            heading = rotate_about(heading, dir_v, (leg.turn_dps * PERIOD_MS as f64 / 1000.0).to_radians());
            let g = ground(dir_v);
            let position = dir_v * (R + g + alt);
            let pitch = leg.pitch_deg.to_radians();
            let forward = (heading * pitch.cos() - dir_v * pitch.sin()).normalized();
            let up = (dir_v - forward * dir_v.dot(forward)).normalized();
            t_ms += PERIOD_MS;
            lowest = lowest.min(alt);
            lines.push(format!(
                "{t_ms} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                position.x, position.y, position.z, forward.x, forward.y, forward.z, up.x, up.y, up.z, alt, 16.0
            ));
        }
    }
    let mut text = format!("planet-path 1\nwindow_ms {t_ms}\nperiod_ms {PERIOD_MS}\nsamples {}\n", lines.len());
    for l in &lines {
        text += l;
        text.push('\n');
    }
    std::fs::write(&out, text).map_err(|e| format!("writing {}: {e}", out.display()))?;
    let (la, lo) = dir_v.lat_lon();
    println!(
        "wrote {} poses over {:.1} min to {}; lowest {lowest:.1} m above the ground; ends at {:.4} {:.4}",
        lines.len(),
        t_ms as f64 / 60_000.0,
        out.display(),
        la.to_degrees(),
        lo.to_degrees()
    );
    Ok(())
}
