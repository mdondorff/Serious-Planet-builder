//! `cargo xtask preview world --seed N [--out DIR]` runs this (GEN-103): the whole-planet preview of the generator's default class source, written as PNGs
//! and a report. CPU only, reads no store, writes only under `--out` (default `target/preview/world-seed-<N>`).

use planet_generators::preview;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut seed, mut out, mut width, mut threads, mut cells) =
        (1u64, None::<PathBuf>, 2048usize, 0usize, planet_generators::classes::CLASS_LATTICE_N);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--seed" => seed = value().parse().unwrap_or_else(|_| fail("--seed needs a non-negative integer")),
            "--out" => out = Some(PathBuf::from(value())),
            "--width" => width = value().parse().unwrap_or_else(|_| fail("--width needs an integer (the image height is half of it)")),
            "--threads" => threads = value().parse().unwrap_or_else(|_| fail("--threads needs an integer")),
            "--lattice-cells" => cells = value().parse().unwrap_or_else(|_| fail("--lattice-cells needs an integer")),
            other => fail(&format!("unknown option '{other}' (known: --seed N, --out DIR, --width W, --threads N, --lattice-cells N)")),
        }
    }
    if width < 64 || width % 2 != 0 {
        fail("--width must be an even number of at least 64");
    }
    if threads == 0 {
        threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    }
    let out = out.unwrap_or_else(|| PathBuf::from(format!("target/preview/world-seed-{seed}")));
    let started = std::time::Instant::now();
    let fields = preview::compute(seed, width, width / 2, cells, threads);
    let computed = started.elapsed();
    for (name, image) in preview::render(&fields) {
        let path = out.join(&name);
        image.write_png(&path).unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", path.display())));
        println!("wrote {}", path.display());
    }
    let report = preview::report(&fields);
    std::fs::write(out.join("report.txt"), &report).unwrap_or_else(|e| fail(&format!("cannot write the report: {e}")));
    print!("{report}");
    println!("fields {:.1} s, total {:.1} s on {threads} threads", computed.as_secs_f64(), started.elapsed().as_secs_f64());
}

fn fail(message: &str) -> ! {
    eprintln!("preview world: {message}");
    std::process::exit(2);
}
