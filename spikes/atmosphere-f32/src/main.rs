//! SPIKE-09 harness: naive f32 vs altitude-relative primitives (ADR 0040 point 5) against an f64 oracle, on every adapter wgpu
//! finds. Prints markdown tables. Throwaway.
//! `cargo run --release -- [--adapter SUBSTR] [--no-walk]`

use std::collections::BTreeSet;
use wgpu::util::DeviceExt;

const R: f64 = 6_371_000.0;
const H: f64 = 100_000.0;
const RT: f64 = R + H;
const N_VIEW: usize = 24;
const N_SUN: usize = 6;
const BETA_R: [f64; 3] = [5.802e-6, 13.558e-6, 33.1e-6];
const BETA_M_S: f64 = 3.996e-6;
const BETA_M_E: f64 = 4.44e-6;
const SCALE_R: f64 = 8000.0;
const SCALE_M: f64 = 1200.0;
const G: f64 = 0.8;
const ALBEDO: f64 = 0.3;
const PI: f64 = std::f64::consts::PI;

// ---------------------------------------------------------------- f64 oracle (exact geometry, same algorithm as the shader)

#[derive(Clone, Copy)]
struct Geo {
    r: f64,
    h: f64,
    c: f64,
}

fn geo_at(b: Geo, mu: f64, s: f64) -> Geo {
    let r2 = (b.r * b.r + s * s + 2.0 * b.r * mu * s).sqrt();
    let hs = (b.c + s * (s + 2.0 * b.r * mu)) / (r2 + R);
    Geo { r: r2, h: hs, c: hs * (2.0 * R + hs) }
}

fn ground_t(g: Geo, mu: f64) -> f64 {
    if mu >= 0.0 {
        return -1.0;
    }
    let rm = g.r * mu;
    let d = rm * rm - g.c;
    if d < 0.0 {
        return -1.0;
    }
    g.c / (-rm + d.sqrt())
}

fn ctop_of(g: Geo) -> f64 {
    (g.h - H) * (2.0 * R + g.h + H)
}

fn top_exit(g: Geo, mu: f64) -> f64 {
    let rm = g.r * mu;
    let ct = ctop_of(g);
    let d = rm * rm - ct;
    if d < 0.0 {
        return -1.0;
    }
    if mu >= 0.0 && ct < 0.0 {
        return -ct / (rm + d.sqrt());
    }
    -rm + d.sqrt()
}

fn top_entry(g: Geo, mu: f64) -> f64 {
    if mu >= 0.0 {
        return -1.0;
    }
    let rm = g.r * mu;
    let ct = ctop_of(g);
    let d = rm * rm - ct;
    if d < 0.0 {
        return -1.0;
    }
    ct / (-rm + d.sqrt())
}

fn dens(h: f64) -> (f64, f64) {
    let hh = h.max(0.0);
    ((-hh / SCALE_R).exp(), (-hh / SCALE_M).exp())
}

fn phase_r(c: f64) -> f64 {
    3.0 / (16.0 * PI) * (1.0 + c * c)
}

fn phase_m(c: f64) -> f64 {
    let k = 3.0 / (8.0 * PI) * (1.0 - G * G) / (2.0 + G * G);
    k * (1.0 + c * c) / (1.0 + G * G - 2.0 * G * c).powf(1.5)
}

fn sun_t(g: Geo, ms: f64) -> [f64; 3] {
    if ms < 0.0 && ground_t(g, ms) >= 0.0 {
        return [0.0; 3];
    }
    let tt = top_exit(g, ms);
    if tt <= 0.0 {
        return [1.0; 3];
    }
    let ds = tt / N_SUN as f64;
    let mut tau = [0.0; 3];
    for i in 0..N_SUN {
        let gs = geo_at(g, ms, (i as f64 + 0.5) * ds);
        let d = dens(gs.h);
        for k in 0..3 {
            tau[k] += (BETA_R[k] * d.0 + BETA_M_E * d.1) * ds;
        }
    }
    tau.map(|t| (-t).exp())
}

/// Radiance, ground hit, t_end, t_ground, t_start for the exact geometry at altitude `h` (m), inputs rounded as the GPU gets them.
fn oracle(h: f64, mu: f64, ms: f64, cosg: f64) -> ([f64; 3], bool, f64, f64, f64) {
    let g0 = Geo { r: R + h, h, c: h * (2.0 * R + h) };
    let mut t0 = 0.0;
    let mut hit = true;
    if h > H {
        t0 = top_entry(g0, mu);
        if t0 < 0.0 {
            hit = false;
        }
    }
    let tg = ground_t(g0, mu);
    let mut t1 = top_exit(g0, mu);
    if tg >= 0.0 {
        t1 = tg;
    }
    let mut l = [0.0; 3];
    if hit && t1 > t0 {
        let ds = (t1 - t0) / N_VIEW as f64;
        let mut tcur = [1.0; 3];
        let (pr, pm) = (phase_r(cosg), phase_m(cosg));
        for i in 0..N_VIEW {
            let s = t0 + (i as f64 + 0.5) * ds;
            let gs = geo_at(g0, mu, s);
            let ms_s = (g0.r * ms + s * cosg) / gs.r;
            let d = dens(gs.h);
            let st = sun_t(gs, ms_s);
            for k in 0..3 {
                let ext = (BETA_R[k] * d.0 + BETA_M_E * d.1) * ds;
                let tmid = tcur[k] * (-0.5 * ext).exp();
                let sc = BETA_R[k] * d.0 * pr + BETA_M_S * d.1 * pm;
                l[k] += tmid * sc * st[k] * ds;
                tcur[k] *= (-ext).exp();
            }
        }
        if tg >= 0.0 {
            let gg = geo_at(g0, mu, tg);
            let ms_g = (g0.r * ms + tg * cosg) / gg.r;
            let st = sun_t(gg, ms_g);
            for k in 0..3 {
                l[k] += tcur[k] * ALBEDO / PI * ms_g.max(0.0) * st[k];
            }
        }
    }
    (l, tg >= 0.0, t1, tg, t0)
}

// ---------------------------------------------------------------- cases

#[derive(Clone)]
struct Case {
    alt: usize,
    h: f64,
    mu: f64,
    ms: f64,
    phi: f64,
    label: String,
}

impl Case {
    /// The f32-rounded inputs the GPU gets: (ray record, mu, ms, cosg as f64).
    fn gpu(&self) -> ([f32; 8], f64, f64, f64) {
        let (h, mu, ms) = (self.h, self.mu as f32 as f64, self.ms as f32 as f64);
        let (sv, ss) = ((1.0 - mu * mu).max(0.0).sqrt(), (1.0 - ms * ms).max(0.0).sqrt());
        let cosg = (sv * ss * self.phi.cos() + mu * ms) as f32 as f64;
        let c = h * (2.0 * R + h);
        let ctop = (h - H) * (2.0 * R + h + H);
        (
            [h as f32, (R + h) as f32, c as f32, ctop as f32, mu as f32, ms as f32, cosg as f32, 0.0],
            mu,
            ms,
            cosg,
        )
    }
}

const ALTS: [(f64, &str); 4] = [(1.0, "1 m"), (2_000.0, "2 km"), (20_000.0, "20 km"), (20_000_000.0, "20,000 km")];
const SUNS: [(f64, &str); 3] = [(0.8, "noon-ish"), (0.05, "low sun"), (-0.08, "twilight")];

fn mus_for(h: f64) -> Vec<(f64, String)> {
    let r = R + h;
    let c = h * (2.0 * R + h);
    let mu_h = -(c.sqrt()) / r;
    let mut v: Vec<(f64, String)> = vec![
        (-1.0, "mu=-1".into()),
        (-0.5, "mu=-0.5".into()),
        (-0.1, "mu=-0.1".into()),
        (-0.01, "mu=-0.01".into()),
        (mu_h - 1e-4, "horizon-1e-4".into()),
        (mu_h - 1e-6, "horizon-1e-6".into()),
        (mu_h + 1e-6, "horizon+1e-6".into()),
        (mu_h + 1e-4, "horizon+1e-4".into()),
        (-1e-6, "mu=-1e-6".into()),
        (1e-6, "mu=+1e-6".into()),
        (0.01, "mu=0.01".into()),
        (0.5, "mu=0.5".into()),
        (1.0, "mu=+1".into()),
    ];
    if h > H {
        let ct = (h - H) * (2.0 * R + h + H);
        let mu_t = -(ct.sqrt()) / r;
        v.push((mu_t - 1e-6, "top-limb-1e-6".into()));
        v.push((mu_t + 1e-6, "top-limb+1e-6".into()));
        v.push((mu_t - 1e-4, "top-limb-1e-4".into()));
        v.push((mu_t + 1e-4, "top-limb+1e-4".into()));
    }
    v
}

fn static_cases() -> Vec<Case> {
    let mut out = Vec::new();
    for (ai, (h0, _)) in ALTS.iter().enumerate() {
        for k in 0..16 {
            let h = h0 * (1.0 + k as f64 / 16.0);
            for (mu, ml) in mus_for(h) {
                for (ms, sl) in SUNS {
                    for phi in [0.0, PI / 2.0, PI] {
                        out.push(Case { alt: ai, h, mu, ms, phi, label: format!("h={h:.4} {ml} {sl} phi={:.2}", phi) });
                    }
                }
            }
        }
    }
    out
}

const BAND_ALTS: [f64; 5] = [1.0, 10.0, 100.0, 2_000.0, 20_000.0];

/// Horizon-band scan: mu = mu_h + delta for delta = +-10^(-7 + k/4), k = 0..20, at several altitudes (two h values each).
fn band_deltas() -> Vec<f64> {
    (0..=20).flat_map(|k| { let d = 10f64.powf(-7.0 + f64::from(k) / 4.0); [-d, d] }).collect()
}

fn band_cases() -> Vec<Case> {
    let mut out = Vec::new();
    for (ai, h0) in BAND_ALTS.iter().enumerate() {
        for hf in [1.0, 1.3] {
            let h = h0 * hf;
            let mu_h = -((h * (2.0 * R + h)).sqrt()) / (R + h);
            for d in band_deltas() {
                for ms in [0.8, 0.05] {
                    for phi in [0.0, PI / 2.0] {
                        out.push(Case { alt: ai, h, mu: mu_h + d, ms, phi, label: format!("band d={d:e}") });
                    }
                }
            }
        }
    }
    out
}

const WALK_STEPS: usize = 1001;

/// Walk cases, series-major: series s, step k at index s * WALK_STEPS + k. The camera rises from 1.000 m to 2.000 m in 1 mm steps.
fn walk_cases() -> (Vec<Case>, Vec<String>) {
    let mut out = Vec::new();
    let mut names = Vec::new();
    for (mu, ml) in mus_for(1.0) {
        for (ms, sl) in &SUNS[..2] {
            names.push(format!("{ml} {sl}"));
            for k in 0..WALK_STEPS {
                let h = 1.0 + k as f64 * 0.001;
                out.push(Case { alt: 0, h, mu, ms: *ms, phi: PI / 2.0, label: format!("walk h={h:.3}") });
            }
        }
    }
    (out, names)
}

// ---------------------------------------------------------------- GPU

struct Gpu {
    name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

fn make_gpu(adapter: &wgpu::Adapter) -> Result<Gpu, String> {
    let info = adapter.get_info();
    let name = format!("{} [{:?}]", info.name, info.backend);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| format!("{name}: {e}"))?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("atmo"),
        source: wgpu::ShaderSource::Wgsl(include_str!("atmo.wgsl").into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("atmo"),
        layout: None,
        module: &module,
        entry_point: Some("cs"),
        compilation_options: Default::default(),
        cache: None,
    });
    Ok(Gpu { name, device, queue, pipeline })
}

fn run(gpu: &Gpu, rays: &[[f32; 8]], mode: u32) -> Vec<[f32; 8]> {
    let n = rays.len();
    let bytes = |v: &[f32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
    let flat: Vec<f32> = rays.iter().flatten().copied().collect();
    let mut params = Vec::new();
    for v in [mode, n as u32, 0, 0] {
        params.extend_from_slice(&v.to_le_bytes());
    }
    let d = &gpu.device;
    let pbuf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &params, usage: wgpu::BufferUsages::UNIFORM });
    let rbuf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &bytes(&flat), usage: wgpu::BufferUsages::STORAGE });
    let out_bytes = (n * 32) as u64;
    let obuf = d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let rb = d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &gpu.pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: pbuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: rbuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: obuf.as_entire_binding() },
        ],
    });
    let mut enc = d.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
    }
    enc.copy_buffer_to_buffer(&obuf, 0, &rb, 0, out_bytes);
    gpu.queue.submit([enc.finish()]);
    let slice = rb.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let data = slice.get_mapped_range().expect("range");
    let f = |k: usize| f32::from_le_bytes(data[k * 4..k * 4 + 4].try_into().expect("4"));
    let res = (0..n).map(|i| std::array::from_fn(|j| f(i * 8 + j))).collect();
    drop(data);
    rb.unmap();
    res
}

// ---------------------------------------------------------------- metrics

fn ratio(a: f64, b: f64) -> f64 {
    (a - b).abs() / (0.01 * b.abs() + 1e-4)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filter = args.iter().position(|a| a == "--adapter").and_then(|i| args.get(i + 1)).cloned();
    let walk_on = !args.iter().any(|a| a == "--no-walk");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let mut seen = BTreeSet::new();
    let statics = static_cases();
    let (walks, walk_names) = walk_cases();
    let bands = band_cases();
    let all: Vec<Case> = statics.iter().chain(walks.iter()).chain(bands.iter()).cloned().collect();
    let inputs: Vec<([f32; 8], f64, f64, f64)> = all.iter().map(Case::gpu).collect();
    let raw: Vec<[f32; 8]> = inputs.iter().map(|i| i.0).collect();
    let refs: Vec<([f64; 3], bool, f64, f64, f64)> = all.iter().zip(&inputs).map(|(c, i)| oracle(c.h, i.1, i.2, i.3)).collect();
    println!("# SPIKE-09 results\n\n{} static cases, {} walk samples ({} series x {WALK_STEPS})\n", statics.len(), walks.len(), walk_names.len());
    for adapter in &adapters {
        let info = adapter.get_info();
        let name = format!("{} [{:?}]", info.name, info.backend);
        if !seen.insert(name.clone()) {
            continue;
        }
        if filter.as_ref().is_some_and(|f| !name.to_lowercase().contains(&f.to_lowercase())) {
            continue;
        }
        let gpu = match make_gpu(adapter) {
            Ok(g) => g,
            Err(e) => {
                println!("## {name}\n\nskipped: {e}\n");
                continue;
            }
        };
        println!("## {}\n", gpu.name);
        for (mode, form) in [(0u32, "naive"), (1, "primitives")] {
            let out = run(&gpu, &raw, mode);
            println!("### form: {form}\n");
            println!("| altitude | cases | fail >bound | fail % | max err/bound | ground-class mismatches | max rel err t_end | worst case |");
            println!("|---|---|---|---|---|---|---|---|");
            for (ai, (_, an)) in ALTS.iter().enumerate() {
                let (mut n, mut fail, mut worst, mut mism, mut tmax) = (0, 0, 0.0f64, 0, 0.0f64);
                let mut wl = String::new();
                for (idx, c) in statics.iter().enumerate().filter(|(_, c)| c.alt == ai) {
                    n += 1;
                    let (l, gr, t1, _, _) = refs[idx];
                    let o = out[idx];
                    // `run` returns 8 floats per ray: [L.rgb, ground flag, t_end, t_ground, t_start, 0]
                    let e = (0..3).map(|k| ratio(f64::from(o[k]), l[k])).fold(0.0, f64::max);
                    if e > 1.0 {
                        fail += 1;
                    }
                    if e > worst {
                        worst = e;
                        wl = c.label.clone();
                    }
                    if (o[3] > 0.5) != gr {
                        mism += 1;
                    }
                    let te = if t1 > 0.0 { (f64::from(o[4]) - t1).abs() / t1 } else { 0.0 };
                    tmax = tmax.max(te);
                }
                println!(
                    "| {an} | {n} | {fail} | {:.1} | {worst:.3} | {mism} | {tmax:.2e} | {wl} |",
                    100.0 * fail as f64 / n as f64
                );
            }
            if walk_on {
                println!("\nSlow walk, camera rising 1.000 to 2.000 m in 1 mm steps; a step is a frame-to-frame change of the f32 radiance whose difference from the oracle's change exceeds the bound (1 % + 1e-4 of the oracle value).\n");
                println!("| series | max err/bound | frames with error > bound | steps beyond bound | max step/bound |");
                println!("|---|---|---|---|---|");
                let base = statics.len();
                for (si, sn) in walk_names.iter().enumerate() {
                    let (mut worst, mut ebad, mut sbad, mut smax) = (0.0f64, 0, 0, 0.0f64);
                    for k in 0..WALK_STEPS {
                        let idx = base + si * WALK_STEPS + k;
                        let (l, ..) = refs[idx];
                        let o = out[idx];
                        let e = (0..3).map(|c| ratio(f64::from(o[c]), l[c])).fold(0.0, f64::max);
                        worst = worst.max(e);
                        if e > 1.0 {
                            ebad += 1;
                        }
                        if k > 0 {
                            let (lp, ..) = refs[idx - 1];
                            let op = out[idx - 1];
                            let s = (0..3)
                                .map(|c| ((f64::from(o[c]) - f64::from(op[c])) - (l[c] - lp[c])).abs() / (0.01 * l[c].abs() + 1e-4))
                                .fold(0.0, f64::max);
                            smax = smax.max(s);
                            if s > 1.0 {
                                sbad += 1;
                            }
                        }
                    }
                    println!("| {sn} | {worst:.3} | {ebad} | {sbad} | {smax:.3} |");
                }
            }
            // Geometry at 1 m: the distance to the ground or the top, per mu.
            println!("
Geometry at 1 m (max relative error of the ray's end distance t_end over the 16 altitudes 1.0-1.94 m, 3 suns, 3 azimuths; the radiance bound hides a wrong t when the ray is short):
");
            println!("| mu | max rel err t_end | cases with ground/sky class wrong |");
            println!("|---|---|---|");
            let mut labels: Vec<String> = Vec::new();
            for c in statics.iter().filter(|c| c.alt == 0) {
                let ml = c.label.split(' ').nth(1).unwrap_or("").to_string();
                if !labels.contains(&ml) {
                    labels.push(ml);
                }
            }
            for ml in labels {
                let (mut tmax, mut bad) = (0.0f64, 0);
                for (idx, c) in statics.iter().enumerate().filter(|(_, c)| c.alt == 0 && c.label.split(' ').nth(1) == Some(ml.as_str())) {
                    let (_, gr, t1, ..) = refs[idx];
                    let o = out[idx];
                    let _ = c;
                    if (o[3] > 0.5) != gr {
                        bad += 1;
                    }
                    if t1 > 0.0 {
                        tmax = tmax.max((f64::from(o[4]) - t1).abs() / t1);
                    }
                }
                println!("| {ml} | {tmax:.2e} | {bad} |");
            }
            println!("
Horizon band: widest |mu - mu_horizon| (radians) at which any case still exceeds the bound; delta<0 is below the geometric horizon.
");
            println!("| altitude | mu_horizon | widest failing below | widest failing above | cases failing |");
            println!("|---|---|---|---|---|");
            let bbase = statics.len() + walks.len();
            for (ai, h0) in BAND_ALTS.iter().enumerate() {
                let (mut below, mut above, mut nfail) = (0.0f64, 0.0f64, 0);
                for (k, c) in bands.iter().enumerate().filter(|(_, c)| c.alt == ai) {
                    let idx = bbase + k;
                    let (l, ..) = refs[idx];
                    let o = out[idx];
                    let e = (0..3).map(|q| ratio(f64::from(o[q]), l[q])).fold(0.0, f64::max);
                    if e > 1.0 {
                        nfail += 1;
                        let mu_h = -((c.h * (2.0 * R + c.h)).sqrt()) / (R + c.h);
                        let d = c.mu - mu_h;
                        if d < 0.0 { below = below.max(-d) } else { above = above.max(d) }
                    }
                }
                let mu_h = -((h0 * (2.0 * R + h0)).sqrt()) / (R + h0);
                println!("| {h0} m | {mu_h:.3e} | {below:.1e} | {above:.1e} | {nfail} |");
            }
            println!();
        }
    }
}
