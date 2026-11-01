//! SPIKE-88 (spike branch only): the cost of a cold `field.height` on the main thread (the camera's ground height) on the real store (read only),
//! alone and while the K view's coupled run loads on its own thread (`set_coupled_lens`, STRM-044). SP88_STORE=<dir>; SP88_N cold reads per phase.
use planet::worldfield::WorldTerrain;
use planet_core::{Vec3, EARTH_RADIUS_M as R};
use planet_generators::coupled::CoupledLens;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let n: usize = std::env::var("SP88_N").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let dir = std::env::var("SP88_STORE").expect("SP88_STORE");
    let world = Arc::new(WorldTerrain::open_store(def, std::path::Path::new(&dir), 16384).unwrap());
    let mut s: u64 = 0x2545F4914F6CDD1D;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut random_dir = move || {
        let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
        let r = libm::sqrt(1.0 - z * z);
        Vec3::new(r * libm::cos(a), r * libm::sin(a), z)
    };
    // one cold read per random direction (far apart: each meets blocks nobody decoded), timed one by one
    let phase = |label: &str, world: &WorldTerrain, dirs: &[Vec3]| {
        let field = world.field().unwrap();
        let mut ms: Vec<f64> = dirs
            .iter()
            .map(|d| {
                let t = Instant::now();
                std::hint::black_box(field.height(*d));
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        let warm: Vec<f64> = dirs
            .iter()
            .map(|d| {
                let t = Instant::now();
                std::hint::black_box(field.height(*d));
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        ms.sort_by(f64::total_cmp);
        let p = |q: f64| ms[((ms.len() - 1) as f64 * q) as usize];
        println!(
            "{label}: {} cold reads, p50 {:.1} ms, p90 {:.1} ms, max {:.1} ms, sum {:.0} ms; warm mean {:.3} ms; coupled {:?}",
            ms.len(),
            p(0.5),
            p(0.9),
            p(1.0),
            ms.iter().sum::<f64>(),
            warm.iter().sum::<f64>() / warm.len() as f64,
            standing(world)
        );
    };
    let a: Vec<Vec3> = (0..n).map(|_| random_dir()).collect();
    phase("alone", &world, &a);
    // A field rebuild (an edit or a lens change builds a new field): does it keep the decoded blocks? One direction, read on field 1 twice, then on field 2.
    {
        let d = random_dir();
        let f1 = world.field().unwrap();
        let t = Instant::now();
        std::hint::black_box(f1.height(d));
        let cold = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        std::hint::black_box(f1.height(d));
        let warm = t.elapsed().as_secs_f64() * 1000.0;
        let f2 = world.field().unwrap();
        let t = Instant::now();
        std::hint::black_box(f2.height(d));
        let rebuilt = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        std::hint::black_box(f2.height(d * 1.0 + Vec3::new(1e-7, 0.0, 0.0)));
        let near = t.elapsed().as_secs_f64() * 1000.0;
        println!("rebuild: same direction cold {cold:.2} ms, warm {warm:.4} ms, on a new field {rebuilt:.4} ms, a neighbour on the new field {near:.4} ms");
    }
    // The tile refresh flood: SP88_LOAD threads read cold random directions on their own fields while the main thread measures.
    let load: usize = std::env::var("SP88_LOAD").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    if load > 0 {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let handles: Vec<_> = (0..load)
            .map(|k| {
                let (w, stop, reads) = (Arc::clone(&world), Arc::clone(&stop), Arc::clone(&reads));
                std::thread::spawn(move || {
                    let f = w.field().unwrap();
                    let mut s = 0x9E3779B97F4A7C15u64 ^ (k as u64 + 1) * 0x1234567;
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        s ^= s << 13;
                        s ^= s >> 7;
                        s ^= s << 17;
                        let u = (s >> 11) as f64 / (1u64 << 53) as f64;
                        s ^= s << 13;
                        s ^= s >> 7;
                        s ^= s << 17;
                        let v = (s >> 11) as f64 / (1u64 << 53) as f64;
                        let (z, a) = (2.0 * u - 1.0, std::f64::consts::TAU * v);
                        let r = libm::sqrt(1.0 - z * z);
                        std::hint::black_box(f.height(Vec3::new(r * libm::cos(a), r * libm::sin(a), z)));
                        reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                })
            })
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let b: Vec<Vec3> = (0..n).map(|_| random_dir()).collect();
        phase(&format!("under {load} loader threads"), &world, &b);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        for h in handles {
            let _ = h.join();
        }
        println!("loader reads {}", reads.load(std::sync::atomic::Ordering::Relaxed));
        return;
    }
    let t = Instant::now();
    world.set_coupled_lens_readonly(CoupledLens::Height);
    for round in 0..6 {
        let b: Vec<Vec3> = (0..n).map(|_| random_dir()).collect();
        phase(&format!("K on, round {round} at {:.1} s", t.elapsed().as_secs_f64()), &world, &b);
        if standing(&world) != "loading" {
            break;
        }
    }
    let mut waited = 0;
    while standing(&world) == "loading" && waited < 600 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        waited += 1;
    }
    println!("coupled run {} after {:.1} s", standing(&world), t.elapsed().as_secs_f64());
    let c: Vec<Vec3> = (0..n).map(|_| random_dir()).collect();
    phase("K on, run settled", &world, &c);
}

fn standing(w: &WorldTerrain) -> String {
    match w.coupled_standing() {
        planet::coupledworld::Standing::Ready(..) => "ready".into(),
        planet::coupledworld::Standing::Failed(e) => format!("failed: {e}"),
        _ => "loading".into(),
    }
}
