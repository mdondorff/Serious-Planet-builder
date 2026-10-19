//! Tier B: the lit terrain's base shading (`shade_lit` and `up_at` in the terrain shader) against the f64 reference
//! `look::shading::terrain_radiance` and the exact radial direction (REND-025, REND-027). Class: presentation data (ADR 0035).
//!
//! Declared bounds (they follow from the purpose of the attributes, not from the measured result):
//! - the pre-exposed radiance of a fragment: |GPU - reference| <= max(2e-4 |reference|, 2e-5 x pre-exposure factor) per channel, before
//!   the f16 store (the albedo is a product of smoothsteps of f32 inputs; 2e-4 is a fifth of 1/255 at the brightest albedo);
//! - the up vector at a fragment: within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin
//!   of a tile at 8,848 m or at -11,000 m, anywhere on the planet (the slope features it feeds are three orders of magnitude wider);
//! - temporal stability at 1 m altitude (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart, and the
//!   albedo selected from it by at most 1/255, at the worst case (a tile at the finest level, far from the origin); the shading
//!   uses tile-local quantities only, so it does not depend on the camera.
//!
//! The functions are the shader's own: the terrain module is compiled with extra compute entries that call them.

use planet_frame::look::shading::{terrain_radiance_with_ramp, AlbedoParams, ShadeView};
use planet_frame::look::{self};
use planet_render::lit::{probe_lit_radiance, probe_up_at, LitProbe};
use planet_render::{AdapterPolicy, GpuContext};

struct Case {
    view: ShadeView,
    normal: [f64; 3],
    up: [f64; 3],
    t: f64,
    p: f64,
    h: f64,
    /// The ramp height wanted at a distance of 100 m (REND-030); the probe gets `per_m = ramp / 100` and the distance below, so the ramp
    /// is `shore_ramp_at(ramp / 100, 100 (0.25 + 1.5 morph))` and reaches the 8 m cap; 0 ramps by the table's blend alone.
    ramp: f64,
    /// Scales the fragment's distance from 25 m (0) to 175 m (1).
    morph: f64,
    sun: [f64; 3],
    pre_ev: f64,
    table: AlbedoParams,
}

fn rng(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn unit(u: f64, v: f64) -> [f64; 3] {
    let z = 2.0 * u - 1.0;
    let r = libm::sqrt(1.0 - z * z);
    let a = 2.0 * core::f64::consts::PI * v;
    [r * libm::cos(a), r * libm::sin(a), z]
}

/// A look unlike the default, so the table reaches the shader as data.
fn other_table() -> AlbedoParams {
    AlbedoParams {
        snow: [0.6, 0.62, 0.7],
        desert: [0.3, 0.2, 0.1],
        dry_mm: 400.0,
        wet_mm: 700.0,
        rock_cos_full: 0.4,
        rock_cos_none: 0.95,
        snow_c_full: -2.0,
        snow_c_none: 6.0,
        beach_height_m: 10.0,
        beach_fade_m: 20.0,
        shore_blend_m: 5.0,
        ..AlbedoParams::EARTH
    }
}

/// Deterministic probe inputs: a hash-driven spread of normals (also unnormalised), ups, climates over the whole range, heights above the
/// water (also far below), suns (also grazing and behind the surface), pre-exposures (low enough to hit the 60,000 clamp), all views
/// and two tables.
fn cases() -> Vec<Case> {
    let mut next = rng(0x9e37_79b9_7f4a_7c15);
    let mut out = Vec::new();
    for k in 0..900 {
        let up = unit(next(), next());
        // Mostly near up (the ground), sometimes anything.
        let n = if k % 3 == 0 {
            unit(next(), next())
        } else {
            let tilt = 1.2 * next();
            let side = unit(next(), next());
            [up[0] + tilt * side[0], up[1] + tilt * side[1], up[2] + tilt * side[2]]
        };
        let scale = 0.5 + 3.0 * next();
        out.push(Case {
            view: ShadeView::ALL[k % 5],
            normal: n.map(|c| c * scale),
            up,
            t: -45.0 + 90.0 * next(),
            p: 4500.0 * next() * next(),
            h: if k % 4 == 0 { -40.0 + 80.0 * next() } else { 200.0 * next() * next() - 20.0 },
            // A third of the probes sit at a node with a wider ramp (up to 8 m, the cap), as the coarse nodes of a coastal view do, and
            // half of them at a morph between the node's grid and its parent's.
            ramp: if k % 3 == 1 { 8.0 * next() * next() } else { 0.0 },
            morph: if k % 2 == 0 { next() } else { 0.0 },
            sun: unit(next(), next()),
            pre_ev: [-4.0, 0.0, 8.0, 12.0, 15.0, 18.0][k % 6],
            table: if k % 2 == 0 { AlbedoParams::EARTH } else { other_table() },
        });
    }
    // Exact corners: transitions' edges and midpoints, facing the sun, exactly edge-on, behind.
    let e = AlbedoParams::EARTH;
    for (t, p, cs, h) in [
        (25.0, 50.0, 1.0, 100.0),
        (15.0, 950.0, 1.0, 100.0),
        (15.0, 2500.0, 1.0, 100.0),
        (20.0, 800.0, 0.3, 100.0),
        (-20.0, 800.0, 1.0, 100.0),
        (20.0, 50.0, 1.0, 1.0),
        (20.0, 50.0, 1.0, -5.0),
        (25.0, 0.5 * (e.dry_mm + e.wet_mm), 1.0, 100.0),
        (e.snow_c_full, 100.0, 0.9, 100.0),
        (e.snow_c_none, 100.0, 0.9, 100.0),
        (20.0, 100.0, e.rock_cos_full, 100.0),
        (20.0, 100.0, e.rock_cos_none, 100.0),
        (20.0, 100.0, 1.0, 0.0),
        (20.0, 100.0, 1.0, -e.shore_blend_m),
    ] {
        let up = [0.0, 0.0, 1.0];
        let n = [libm::sqrt(1.0 - cs * cs), 0.0, cs];
        for view in ShadeView::ALL {
            out.push(Case { view, normal: n, up, t, p, h, ramp: 0.0, morph: 0.0, sun: [0.0, 0.0, 1.0], pre_ev: 15.0, table: e });
        }
    }
    // The ramp's own corners (REND-030): heights at its foot, middle and top for a node of 8 m ramp and for one that ramps by the blend
    // alone, and a ramp narrower than the blend (the blend wins).
    for (h, ramp) in [(-8.0, 8.0), (-4.0, 8.0), (-0.5, 8.0), (0.0, 8.0), (-4.0, 0.0), (-0.5, 0.5), (-0.5, 0.0)] {
        for view in ShadeView::ALL {
            out.push(Case {
                view,
                normal: [0.0, 0.0, 1.0],
                up: [0.0, 0.0, 1.0],
                t: 20.0,
                p: 50.0,
                h,
                ramp,
                morph: 0.5,
                sun: [0.0, 0.0, 1.0],
                pre_ev: 15.0,
                table: e,
            });
        }
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
            view: c.view.index(),
            normal: c.normal.map(|v| v as f32),
            up: c.up.map(|v| v as f32),
            temperature_c: c.t as f32,
            precipitation_mm: c.p as f32,
            height_above_water_m: c.h as f32,
            // The case's ramp and morph give the growth per metre and a distance, so the ramp sweeps 0 to 12 m and meets the 8 m cap.
            shore_ramp_per_m: (c.ramp / 100.0) as f32,
            distance_m: (100.0 * (0.25 + 1.5 * c.morph)) as f32,
            sun: c.sun.map(|v| v as f32),
            factor: look::pre_exposure_factor(c.pre_ev) as f32,
            table: c.table.to_words(),
        })
        .collect();
    let gpu = probe_lit_radiance(&ctx, &probes);
    let (mut worst, mut clamped, mut dark, mut lit, mut views) = (0.0f64, 0, 0, 0, [0usize; 5]);
    for (i, c) in cases.iter().enumerate() {
        // The reference sees exactly what the shader received: the f32 inputs and the f32 table.
        let p = &probes[i];
        let radiance = terrain_radiance_with_ramp(
            &c.table.as_f32(),
            c.view,
            p.normal.map(f64::from),
            p.up.map(f64::from),
            f64::from(p.temperature_c),
            f64::from(p.precipitation_mm),
            f64::from(p.height_above_water_m),
            planet_frame::lod::shore_ramp_at(f64::from(p.shore_ramp_per_m), f64::from(p.distance_m)),
            p.sun.map(f64::from),
        );
        let want = look::written_value(radiance, c.pre_ev);
        let factor = look::pre_exposure_factor(c.pre_ev);
        for ch in 0..3 {
            let got = f64::from(gpu[i][ch]);
            let bound = (2.0e-4 * want[ch].abs()).max(2.0e-5 * factor);
            let d = (got - want[ch]).abs();
            worst = worst.max(d / bound);
            assert!(
                d <= bound,
                "probe {i} ({}) channel {ch}: GPU {got}, reference {} (|d| {d} > bound {bound}); T {} P {} h {} normal {:?} up {:?} sun {:?} EV {}",
                c.view.name(),
                want[ch],
                c.t,
                c.p,
                c.h,
                c.normal,
                c.up,
                c.sun,
                c.pre_ev
            );
        }
        views[c.view.index() as usize] += 1;
        clamped += i32::from(want.iter().any(|v| *v >= f64::from(look::HDR_CLAMP)));
        dark += i32::from(want == [0.0; 3]);
        lit += i32::from(want.iter().all(|v| *v > 0.0 && *v < f64::from(look::HDR_CLAMP)));
    }
    // The probe set exercises the clamp, the dark side, ordinary light and every view.
    assert!(clamped > 0 && dark > 20 && lit > 300, "probe coverage: {clamped} clamped, {dark} dark, {lit} lit");
    assert!(views.iter().all(|n| *n > 100), "every view is probed: {views:?}");
    println!("base shading: {} probes, worst difference {:.3} of the bound", cases.len(), worst);
}

/// The unit radial direction of the point `local` metres from an origin at distance `r` along `up0`, in f64.
fn exact_up(up0: [f64; 3], r: f64, local: [f64; 3]) -> [f64; 3] {
    let p = [up0[0] * r + local[0], up0[1] * r + local[1], up0[2] * r + local[2]];
    let l = libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
    p.map(|c| c / l)
}

fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let c = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    // The chord is accurate for tiny angles where acos is not.
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let chord = libm::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if chord < 1e-3 {
        chord
    } else {
        libm::acos(c)
    }
}

// spec: REND-027, CON-11
#[test]
fn the_up_vector_is_the_radial_direction_within_its_bound_at_the_worst_case() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut next = rng(77);
    let radii = [6_371_000.0 + 8_848.0, 6_371_000.0 - 11_000.0, 6_371_000.0];
    let mut probes = Vec::new();
    let mut want = Vec::new();
    // Origins all over the planet, including the poles and the cube corner; offsets up to 1 km in every axis and exactly zero.
    let mut origins: Vec<[f64; 3]> = (0..40).map(|_| unit(next(), next())).collect();
    origins.extend([[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.57735026919, 0.57735026919, 0.57735026919]]);
    for up0 in origins {
        for r in radii {
            for k in 0..12 {
                // Offsets up to 1 km on fine tiles, and up to thousands of km on the coarse ones (level 0 to 3 tiles, whose local positions are that large).
                let reach = [2000.0, 2000.0, 2000.0, 2.0e5, 2.0e6, 8.0e6][k % 6];
                let local = if k == 0 { [0.0; 3] } else { [(next() - 0.5) * reach, (next() - 0.5) * reach, (next() - 0.5) * reach] };
                probes.push(([up0[0] as f32, up0[1] as f32, up0[2] as f32, (1.0 / r) as f32], local.map(|v| v as f32)));
                // The exact direction of the point the shader sees: the f32 local position as given.
                want.push(exact_up(up0, r, local.map(|v| f64::from(v as f32))));
            }
        }
    }
    let got = probe_up_at(&ctx, &probes);
    let mut worst: f64 = 0.0;
    for ((g, w), p) in got.iter().zip(&want).zip(&probes) {
        let a = angle(g.map(f64::from), *w);
        worst = worst.max(a);
        assert!(a <= 5.0e-7, "up differs from the radial direction by {a} rad (> 5e-7): node {:?} local {:?}", p.0, p.1);
        let len = libm::sqrt(g.iter().map(|c| f64::from(*c) * f64::from(*c)).sum::<f64>());
        assert!((len - 1.0).abs() < 2.0e-7, "up is a unit vector: {len}");
    }
    println!("up vector: {} probes, worst {worst:.3e} rad against the bound 5e-7", probes.len());
}

// spec: REND-027, CON-11
#[test]
fn the_slope_input_is_stable_under_one_millimetre_steps_at_the_finest_level_far_from_the_origin() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let e = AlbedoParams::EARTH;
    // The worst case: the camera 1 m above the ground at a point far from the origin (the top of the highest tile, 8,848 m up),
    // moving along a metre of tile-local positions in 1 mm steps; the surface is flat (its normal is the tile origin's up).
    let r = 6_371_000.0 + 8_848.0;
    let up0 = {
        let v = [0.3, -0.8, 0.52];
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        v.map(|c| c / l)
    };
    let node_up = [up0[0] as f32, up0[1] as f32, up0[2] as f32, (1.0 / r) as f32];
    // A line of tile-local positions along three axes, 1 mm apart, 1,001 points each, from a fixed offset inside the tile.
    let mut probes = Vec::new();
    for axis in 0..3 {
        for step in 0..=1000 {
            let mut local = [12.5f64, -3.25, 40.0];
            local[axis] += f64::from(step) * 1.0e-3;
            probes.push((node_up, local.map(|v| v as f32)));
        }
    }
    let got = probe_up_at(&ctx, &probes);
    // The surface normal is tilted from up so that its slope is the middle of the soil-to-rock transition, where the albedo is most
    // sensitive to the slope input.
    let mid = 0.5 * (e.rock_cos_full + e.rock_cos_none);
    let perp = {
        let c = [up0[1], -up0[0], 0.0];
        let l = libm::sqrt(c[0] * c[0] + c[1] * c[1] + c[2] * c[2]);
        c.map(|v| v / l)
    };
    let s = libm::sqrt(1.0 - mid * mid);
    let normal = [up0[0] * mid + perp[0] * s, up0[1] * mid + perp[1] * s, up0[2] * mid + perp[2] * s];
    let cos = |u: [f32; 3]| normal.iter().zip(u).map(|(a, b)| a * f64::from(b)).sum::<f64>();
    let mut worst_step: f64 = 0.0;
    let mut worst_albedo: f64 = 0.0;
    for line in got.chunks(1001) {
        for w in line.windows(2) {
            let (c0, c1) = (cos(w[0]), cos(w[1]));
            worst_step = worst_step.max((c1 - c0).abs());
            let a0 = e.albedo(20.0, 100.0, c0, 100.0);
            let a1 = e.albedo(20.0, 100.0, c1, 100.0);
            for ch in 0..3 {
                worst_albedo = worst_albedo.max((a0[ch] - a1[ch]).abs());
            }
        }
    }
    println!("slope input: worst step between fragments 1 mm apart {worst_step:.3e}, albedo step {worst_albedo:.3e}");
    assert!(worst_step <= 3.0e-7, "the slope input jumps by {worst_step} between 1 mm steps (> 3e-7)");
    assert!(worst_albedo <= 1.0 / 255.0, "the albedo jumps by {worst_albedo} between 1 mm steps (> 1/255)");
    // The same quantity from two different tiles that cover the same ground (neighbouring origins) agrees: the shading does not
    // depend on which tile draws the fragment.
    let near = |tile_origin: [f64; 3], p_abs: [f64; 3]| {
        let rr = libm::sqrt(tile_origin.iter().map(|c| c * c).sum::<f64>());
        let u = tile_origin.map(|c| c / rr);
        let local = [p_abs[0] - tile_origin[0], p_abs[1] - tile_origin[1], p_abs[2] - tile_origin[2]];
        ([u[0] as f32, u[1] as f32, u[2] as f32, (1.0 / rr) as f32], local.map(|v| v as f32))
    };
    let p_abs = up0.map(|c| c * (r + 1.0));
    let a_origin = up0.map(|c| c * r);
    let b_origin = {
        let side = [-up0[1], up0[0], 0.0];
        let l = libm::sqrt(side[0] * side[0] + side[1] * side[1]);
        [a_origin[0] + 37.0 * side[0] / l, a_origin[1] + 37.0 * side[1] / l, a_origin[2]]
    };
    let two = probe_up_at(&ctx, &[near(a_origin, p_abs), near(b_origin, p_abs)]);
    let d = angle(two[0].map(f64::from), two[1].map(f64::from));
    assert!(d <= 5.0e-7, "two tiles disagree about up at the same point by {d} rad");
}
