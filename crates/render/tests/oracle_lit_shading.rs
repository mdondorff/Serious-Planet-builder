//! Tier B: the lit terrain's base radiance (`lit_radiance` in the terrain shader) against `look::written_value` of
//! `look::lit_terrain_radiance` at probe inputs (REND-025). Declared bound (presentation data, ADR 0035): |GPU - reference| <=
//! max(1e-4 * |reference|, 1e-5 * pre-exposure factor) per channel, on the pre-exposed value before the f16 store. The function is
//! the shader's own: the terrain module is compiled with one extra compute entry that calls it.

use planet_frame::look;
use planet_render::lit::{probe_lit_radiance, LitProbe};
use planet_render::{AdapterPolicy, GpuContext};

struct Case {
    normal: [f64; 3],
    height: f64,
    range: [f64; 2],
    sun: [f64; 3],
    pre_ev: f64,
}

/// Deterministic probe inputs: a hash-driven spread of normals, heights (also beyond both ends of the range), suns (also grazing and
/// behind the surface) and pre-exposures (low enough to hit the 60,000 clamp).
fn cases() -> Vec<Case> {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut out = Vec::new();
    for k in 0..400 {
        let unit = |u: f64, v: f64| {
            let z = 2.0 * u - 1.0;
            let r = libm::sqrt(1.0 - z * z);
            let a = 2.0 * core::f64::consts::PI * v;
            [r * libm::cos(a), r * libm::sin(a), z]
        };
        let n = unit(next(), next());
        let sun = unit(next(), next());
        // Unnormalised normals too: the shader normalises.
        let scale = 0.5 + 3.0 * next();
        let range = if k % 5 == 0 { [-50.0, 50.0] } else { [-11_000.0, 9_000.0] };
        out.push(Case {
            normal: n.map(|c| c * scale),
            height: range[0] - 500.0 + next() * (range[1] - range[0] + 1000.0),
            range,
            sun,
            pre_ev: [-4.0, 0.0, 8.0, 12.0, 15.0, 18.0][k % 6],
        });
    }
    // Exact corners: facing the sun, exactly edge-on, behind, the ends of the tint.
    for (n, h, ev) in
        [([0.0, 0.0, 1.0], 0.0, 15.0), ([1.0, 0.0, 0.0], 0.0, 15.0), ([0.0, 0.0, -1.0], 100.0, 15.0), ([0.0, 0.0, 1.0], 1.0e5, -4.0)]
    {
        out.push(Case { normal: n, height: h, range: [-100.0, 100.0], sun: [0.0, 0.0, 1.0], pre_ev: ev });
    }
    out
}

// spec: REND-025
#[test]
fn the_lit_radiance_matches_the_f64_reference_within_its_declared_bound() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let cases = cases();
    let probes: Vec<LitProbe> = cases
        .iter()
        .map(|c| LitProbe {
            normal: c.normal.map(|v| v as f32),
            height: c.height as f32,
            range: c.range.map(|v| v as f32),
            sun: c.sun.map(|v| v as f32),
            factor: look::pre_exposure_factor(c.pre_ev) as f32,
        })
        .collect();
    let gpu = probe_lit_radiance(&ctx, &probes);
    let (mut worst, mut clamped, mut dark, mut lit) = (0.0f64, 0, 0, 0);
    for (i, c) in cases.iter().enumerate() {
        let radiance = look::lit_terrain_radiance(c.normal, c.height, c.range, c.sun);
        let want = look::written_value(radiance, c.pre_ev);
        let factor = look::pre_exposure_factor(c.pre_ev);
        for ch in 0..3 {
            let got = f64::from(gpu[i][ch]);
            let bound = (1.0e-4 * want[ch].abs()).max(1.0e-5 * factor);
            let d = (got - want[ch]).abs();
            worst = worst.max(d / bound);
            assert!(
                d <= bound,
                "probe {i} channel {ch}: GPU {got}, reference {} (|d| {d} > bound {bound}); input normal {:?} height {} range {:?} sun {:?} EV {}",
                want[ch],
                c.normal,
                c.height,
                c.range,
                c.sun,
                c.pre_ev
            );
        }
        clamped += i32::from(want.iter().any(|v| *v >= f64::from(look::HDR_CLAMP)));
        dark += i32::from(want == [0.0; 3]);
        lit += i32::from(want.iter().all(|v| *v > 0.0 && *v < f64::from(look::HDR_CLAMP)));
    }
    // The probe set exercises the clamp, the dark side and ordinary light.
    assert!(clamped > 0 && dark > 20 && lit > 100, "probe coverage: {clamped} clamped, {dark} dark, {lit} lit");
    println!("lit radiance: {} probes, worst difference {:.3} of the bound", cases.len(), worst);
}
