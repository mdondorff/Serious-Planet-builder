//! Staircase diagnosis (m9-bake preview): how much of the layer's change gradient is aligned with the grid. `stair_metric FILE.f32 [--crop I0 J0 N --png OUT.png]`: reads one
//! exported tile core (3072 x 3072 f32, row `j` outer), takes the central-difference gradient of the change, keeps the cells whose gradient is in the top decile of the land
//! cells that changed, and reports the share of them whose gradient direction is within 5 degrees of a multiple of 45 degrees (isotropic expectation 22.2 %), the same on cells
//! with a larger incision than 20 m, and the share within 5 degrees of the axes only (expectation 11.1 %). With `--png`, writes the hillshade of the change over a crop.

use planet_generators::bake::map::tile_image;

const T: usize = 3072;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = args.first().expect("a tile core file");
    let raw = std::fs::read(file).expect("readable file");
    assert_eq!(raw.len(), T * T * 4, "a 3072 x 3072 f32 core");
    let c: Vec<f32> = raw.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    let mut grads = Vec::new();
    for j in 1..T - 1 {
        for i in 1..T - 1 {
            let q = j * T + i;
            let (gx, gy) = (f64::from(c[q + 1] - c[q - 1]) * 0.5, f64::from(c[q + T] - c[q - T]) * 0.5);
            let m = libm::hypot(gx, gy);
            if m > 0.0 {
                grads.push((m, libm::atan2(gy, gx).to_degrees(), c[q]));
            }
        }
    }
    let mut mags: Vec<f64> = grads.iter().map(|g| g.0).collect();
    mags.sort_by(f64::total_cmp);
    let p90 = mags[mags.len() * 9 / 10];
    let near = |deg: f64, step: f64| {
        let r = deg.rem_euclid(step);
        r.min(step - r) <= 5.0
    };
    let report = |name: &str, sel: &dyn Fn(&(f64, f64, f32)) -> bool| {
        let v: Vec<&(f64, f64, f32)> = grads.iter().filter(|g| sel(g)).collect();
        let n = v.len().max(1) as f64;
        let d45 = v.iter().filter(|g| near(g.1, 45.0)).count() as f64 / n;
        let d90 = v.iter().filter(|g| near(g.1, 90.0)).count() as f64 / n;
        println!(
            "{name}: {} cells, within 5 deg of a multiple of 45: {:.1} % (isotropic 22.2 %), of the axes: {:.1} % (isotropic 11.1 %)",
            v.len(),
            100.0 * d45,
            100.0 * d90
        );
    };
    println!("{file}: gradient p90 {p90:.2} m a cell");
    report("all gradients", &|_| true);
    report("top decile gradients", &|g| g.0 >= p90);
    report("top decile, incision over 20 m", &|g| g.0 >= p90 && g.2 < -20.0);
    // The same on the change low-passed over 9 x 9 cells (the cell noise gone: what is left is the block and staircase structure of the lattice scale), gradient over 8 cells.
    let blur = {
        let r = 4usize;
        let mut tmp = vec![0.0f32; T * T];
        let mut out = vec![0.0f32; T * T];
        for j in 0..T {
            let row = &c[j * T..(j + 1) * T];
            let mut acc: f32 = row[..=r].iter().sum();
            for i in 0..T {
                tmp[j * T + i] = acc / (2 * r + 1) as f32;
                if i + r + 1 < T {
                    acc += row[i + r + 1];
                }
                if i >= r {
                    acc -= row[i - r];
                }
            }
        }
        for i in 0..T {
            let mut acc: f32 = (0..=r).map(|j| tmp[j * T + i]).sum();
            for j in 0..T {
                out[j * T + i] = acc / (2 * r + 1) as f32;
                if j + r + 1 < T {
                    acc += tmp[(j + r + 1) * T + i];
                }
                if j >= r {
                    acc -= tmp[(j - r) * T + i];
                }
            }
        }
        out
    };
    let mut lg = Vec::new();
    let k = 4usize;
    for j in 16..T - 16 {
        for i in 16..T - 16 {
            let q = j * T + i;
            let (gx, gy) = (f64::from(blur[q + k] - blur[q - k]), f64::from(blur[q + k * T] - blur[q - k * T]));
            let m = libm::hypot(gx, gy);
            if m > 0.0 {
                lg.push((m, libm::atan2(gy, gx).to_degrees(), blur[q]));
            }
        }
    }
    let mut lm: Vec<f64> = lg.iter().map(|g| g.0).collect();
    lm.sort_by(f64::total_cmp);
    let lp90 = lm[lm.len() * 9 / 10];
    let top: Vec<&(f64, f64, f32)> = lg.iter().filter(|g| g.0 >= lp90).collect();
    let nn = top.len().max(1) as f64;
    println!(
        "low-passed (9 x 9), top decile gradients: {} cells, within 5 deg of a multiple of 45: {:.1} % (isotropic 22.2 %), of the axes: {:.1} % (isotropic 11.1 %)",
        top.len(),
        100.0 * top.iter().filter(|g| near(g.1, 45.0)).count() as f64 / nn,
        100.0 * top.iter().filter(|g| near(g.1, 90.0)).count() as f64 / nn
    );
    if let (Some(i), Some(png)) = (args.iter().position(|a| a == "--crop"), args.iter().position(|a| a == "--png")) {
        let (i0, j0, n): (usize, usize, usize) = (args[i + 1].parse().unwrap(), args[i + 2].parse().unwrap(), args[i + 3].parse().unwrap());
        let crop: Vec<f32> = (0..n * n).map(|p| c[(j0 + p / n) * T + i0 + p % n]).collect();
        tile_image(n, 101.8, &vec![0.0; n * n], &crop, 400.0).write_png(std::path::Path::new(&args[png + 1])).expect("png");
        println!("wrote {}", args[png + 1]);
    }
}
