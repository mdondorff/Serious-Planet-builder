//! SPIKE-32: bake the realism region (or a variant) and write preview PNGs.
//! `realism_region OUT_DIR TAG [key=value ...]` with keys relief, warp, fluvial, kdt, uplift, thermal, talus, diffuse, seed; with `cache=PATH`
//! the baked file is written there (the render run reads it). `QUICK=1` writes only the hillshade.
use planet_generators::spike_realism::*;
use std::io::BufWriter;

fn png(path: &std::path::Path, w: usize, h: usize, rgb: &[u8]) {
    let f = BufWriter::new(std::fs::File::create(path).unwrap());
    let mut e = png::Encoder::new(f, w as u32, h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[1]);
    let tag = a[2].clone();
    std::fs::create_dir_all(&out).unwrap();
    let mut p = Params::default();
    let mut cache = None;
    for kv in &a[3..] {
        let (k, v) = kv.split_once('=').unwrap();
        let f: f64 = v.parse().unwrap_or(0.0);
        match k {
            "relief" => p.relief_m = f,
            "warp" => p.warp_m = f,
            "fluvial" => p.fluvial_steps = f as u32,
            "kdt" => p.k_dt = f,
            "uplift" => p.uplift_m = f,
            "thermal" => p.thermal_steps = f as u32,
            "talus" => p.talus_deg = f,
            "diffuse" => p.diffuse_steps = f as u32,
            "seed" => p.seed = f as u32,
            "dome" => p.dome = f,
            "acrit" => p.a_crit = f,
            "droplets" => p.droplets = f as u32,
            "life" => p.droplet_life = f as u32,
            "cap" => p.capacity_k = f,
            "diff" => p.diff_per_step = f,
            "cache" => cache = Some(v.to_string()),
            _ => panic!("unknown key {k}"),
        }
    }
    let rules = MaterialRules::default();
    let skip_extra = std::env::var_os("QUICK").is_some();
    let t = std::time::Instant::now();
    let region = synthesize(&p);
    eprintln!("synthesize {:.1}s, max height {:.0} m", t.elapsed().as_secs_f64(), region.max_h);
    // Hillshade: sun from the south-west, plus a height tint.
    let n = region.n;
    let mut rgb = vec![0u8; n * n * 3];
    let half = EXTENT_M / 2.0;
    for j in 0..n {
        for i in 0..n {
            let (x, y) = ((i as f64 + 0.5) * CELL_M - half, (j as f64 + 0.5) * CELL_M - half);
            let (gx, gy) = region.gradient(x, y, 4.0);
            let nrm = [-gx, -gy, 1.0];
            let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + 1.0).sqrt();
            let sun = [-0.5f64, -0.6, 0.62];
            let sl = (sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]).sqrt();
            let d = ((nrm[0] * sun[0] + nrm[1] * sun[1] + nrm[2] * sun[2]) / (l * sl)).max(0.0);
            let t = (region.h[j * n + i] / region.max_h).clamp(0.0, 1.0) as f64;
            let c = [0.45 + 0.5 * t, 0.5 + 0.4 * t, 0.4 + 0.55 * t];
            for k in 0..3 {
                rgb[((n - 1 - j) * n + i) * 3 + k] = ((c[k] * (0.15 + 0.95 * d)).clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    png(&out.join(format!("hillshade-{tag}.png")), n, n, &rgb);
    if skip_extra {
        return;
    }
    let t = std::time::Instant::now();
    let m = build_materials(&region, &rules, p.seed);
    eprintln!("materials {:.1}s", t.elapsed().as_secs_f64());
    let mn = m.n;
    let mut mrgb = vec![0u8; mn * mn * 3];
    // rock grey, scree tan, snow white, grass green, soil brown
    let cols = [[110.0, 105.0, 100.0], [165.0, 150.0, 125.0], [245.0, 248.0, 255.0], [70.0, 120.0, 45.0], [95.0, 70.0, 45.0]];
    for j in 0..mn {
        for i in 0..mn {
            let k = j * mn + i;
            let w = [m.layer0[k * 4], m.layer0[k * 4 + 1], m.layer0[k * 4 + 2], m.layer0[k * 4 + 3], m.layer1[k * 4]];
            let s: f64 = w.iter().map(|&v| f64::from(v)).sum::<f64>().max(1.0);
            for c in 0..3 {
                let v: f64 = (0..5).map(|q| f64::from(w[q]) * cols[q][c]).sum::<f64>() / s;
                mrgb[((mn - 1 - j) * mn + i) * 3 + c] = v as u8;
            }
        }
    }
    png(&out.join(format!("materials-{tag}.png")), mn, mn, &mrgb);
    if let Some(path) = cache {
        let t = std::time::Instant::now();
        let h = build_horizon(&region, 5000.0);
        eprintln!("horizon {:.1}s", t.elapsed().as_secs_f64());
        let baked = Baked { params: p, region, materials: m, horizon: h, seconds: [0.0; 4] };
        baked.save(std::path::Path::new(&path)).unwrap();
        eprintln!("wrote {path}");
    }
}
