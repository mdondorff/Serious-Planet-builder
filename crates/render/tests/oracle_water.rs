//! Tier B: the water shader's own functions against the f64 reference of `planet_frame::look::water` (ADR 0041 points 4 to 6,
//! ADR 0035, ADR 0043): wave displacement and slope within `1 mm + 1e-3 x (sum of the amplitudes)`, the surface radiance within
//! `max(1 % |v|, 1e-4)`, both on the f32-rounded inputs the GPU receives, at camera altitudes from 1 m to 20 km and in a 1 m walk.

use planet_core::hash::{hash_u64s, unit_f64};
use planet_frame::look::water::{surface_radiance, tile_phases, wave_sample, LiquidAppearance, SurfaceInputs, WaterLook, WavePoint, WAVES};
use planet_render::{probe_surface, probe_waves, AdapterPolicy, GpuContext, SurfaceProbe, WaveProbe};
use planet_world_def::LiquidKind;

const R: f64 = 6_371_000.0;

fn unit(seed: u64, k: u64) -> f64 {
    unit_f64(hash_u64s(seed, &[k]))
}

fn norm3(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / l)
}

fn f32s(v: [f64; 3]) -> [f32; 3] {
    v.map(|c| c as f32)
}

fn wide(v: [f32; 3]) -> [f64; 3] {
    v.map(f64::from)
}

/// The wave probes of one camera altitude: points of a tile whose origin is a planet-sized vector, at the distances a camera at
/// `alt` sees the water at (down to the surface under it, up to the distance where the grid stops holding the waves), with every
/// depth of the shoreline blend and the open sea. Returns the probes and the f64 references of their f32-rounded forms.
type WaveRefs = Vec<([f64; 3], [f64; 3])>;

fn wave_cases(alt: f64, count: usize, seed: u64) -> (Vec<WaveProbe>, WaveRefs) {
    let look = WaterLook::EARTH;
    let liquid = LiquidAppearance::new(0, &LiquidKind::WATER, &look);
    let mut probes = Vec::new();
    let mut refs = Vec::new();
    for k in 0..count as u64 {
        // A tile origin anywhere on the planet; the tile-local offset is what the shader sees.
        let o_dir = norm3([unit(seed, 8 * k) - 0.5, unit(seed, 8 * k + 1) - 0.5, unit(seed, 8 * k + 2) - 0.5]);
        let origin = o_dir.map(|c| c * (R + 40.0));
        let reach = (alt * 8.0).clamp(20.0, 3000.0);
        let local =
            [(unit(seed, 8 * k + 3) - 0.5) * 2.0 * reach, (unit(seed, 8 * k + 4) - 0.5) * 2.0 * reach, (unit(seed, 8 * k + 5) - 0.5) * 4.0];
        let up = norm3([o_dir[0] + local[0] / R, o_dir[1] + local[1] / R, o_dir[2] + local[2] / R]);
        let depth = [0.002, 0.05, 0.6, 1.5, 4.0, 30.0, 400.0][(k % 7) as usize] * (0.5 + unit(seed, 8 * k + 6));
        // The vertex spacing of the tile there: from eight vertices per wavelength of the longest wave to fewer than four of the shortest.
        let spacing = 0.4 + 8.0 * unit(seed, 8 * k + 7);
        let t = 1234.5 + 3.7 * k as f64;
        let phases = tile_phases(&liquid, origin, t);
        let probe = WaveProbe {
            liquid,
            local: f32s(local),
            spacing: spacing as f32,
            up: f32s(up),
            depth: depth as f32,
            phases,
            choppy: look.choppy as f32,
        };
        let point = WavePoint {
            local: wide(probe.local),
            up: wide(probe.up),
            phases: probe.phases.map(f64::from),
            spacing: f64::from(probe.spacing),
            depth: f64::from(probe.depth),
        };
        let r = wave_sample(&liquid.as_f32(), &point, f64::from(probe.choppy));
        refs.push((r.disp, r.slope));
        probes.push(probe);
    }
    (probes, refs)
}

// spec: ATMO-014
#[test]
fn the_waves_match_the_reference_within_a_millimetre_plus_a_thousandth_of_the_amplitudes_at_every_altitude() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let sum = LiquidAppearance::new(0, &LiquidKind::WATER, &WaterLook::EARTH).amplitude_sum();
    let bound = 1.0e-3 + 1.0e-3 * sum;
    let mut worst: f64 = 0.0;
    let mut worst_slope: f64 = 0.0;
    let mut nonzero = 0;
    for (a, alt) in [1.0, 10.0, 100.0, 2000.0, 20_000.0].into_iter().enumerate() {
        let (probes, refs) = wave_cases(alt, 400, 77 + a as u64);
        let got = probe_waves(&ctx, &probes);
        assert_eq!(got.len(), probes.len());
        for (i, ((disp, slope), (rd, rs))) in got.iter().zip(&refs).enumerate() {
            for c in 0..3 {
                let e = (f64::from(disp[c]) - rd[c]).abs();
                worst = worst.max(e / bound);
                assert!(e <= bound, "altitude {alt} m, probe {i}: displacement {:?} against {:?} (bound {bound})", disp, rd);
                let es = (f64::from(slope[c]) - rs[c]).abs();
                worst_slope = worst_slope.max(es / 2.0e-4);
                assert!(es <= 2.0e-4, "altitude {alt} m, probe {i}: slope {:?} against {:?}", slope, rs);
            }
            if rd.iter().any(|v| v.abs() > 1.0e-3) {
                nonzero += 1;
            }
        }
    }
    assert!(nonzero > 500, "the cases have waves in them: {nonzero} of 2000 are non-zero");
    println!("waves: worst displacement {worst:.3} of the bound {bound:.4} m, worst slope {worst_slope:.3} of 2e-4");
}

// spec: ATMO-014
#[test]
fn the_waves_hold_a_long_run_and_planet_sized_origins_because_the_phases_are_reduced_on_the_cpu() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let look = WaterLook::EARTH;
    let liquid = LiquidAppearance::new(0, &LiquidKind::WATER, &look);
    let bound = 1.0e-3 + 1.0e-3 * liquid.amplitude_sum();
    // The same tile and points at t = 1e6 s and at the far side of the planet: the reduced phases keep the shader's argument small.
    let mut probes = Vec::new();
    let mut refs = Vec::new();
    for (i, t) in [0.0, 1.0e3, 1.0e6, 3.0e6].into_iter().enumerate() {
        for (j, o_dir) in [[1.0, 0.0, 0.0], [-0.6, 0.64, 0.48], [0.0, 0.0, -1.0]].into_iter().enumerate() {
            let origin = o_dir.map(|c: f64| c * (R + 10.0));
            let local = [12.0 + 7.0 * i as f64, -30.0 + 11.0 * j as f64, 0.5];
            let up = norm3(o_dir);
            let probe = WaveProbe {
                liquid,
                local: f32s(local),
                spacing: 2.0,
                up: f32s(up),
                depth: 25.0,
                phases: tile_phases(&liquid, origin, t),
                choppy: look.choppy as f32,
            };
            let p = WavePoint {
                local: wide(probe.local),
                up: wide(probe.up),
                phases: probe.phases.map(f64::from),
                spacing: 2.0,
                depth: f64::from(probe.depth),
            };
            refs.push(wave_sample(&liquid.as_f32(), &p, look.choppy).disp);
            probes.push(probe);
        }
    }
    let got = probe_waves(&ctx, &probes);
    for ((disp, _), want) in got.iter().zip(&refs) {
        for c in 0..3 {
            assert!((f64::from(disp[c]) - want[c]).abs() <= bound, "{disp:?} against {want:?}");
        }
    }
    // The tile phases are small numbers, however large the time and the origin were.
    assert!(probes.iter().all(|p| p.phases.iter().all(|v| (0.0..=core::f32::consts::TAU).contains(v))));
    let _ = WAVES;
}

fn surface_cases(count: usize, seed: u64) -> (Vec<SurfaceProbe>, Vec<[f64; 3]>) {
    let look = WaterLook::EARTH;
    let water = LiquidAppearance::new(0, &LiquidKind::WATER, &look);
    let lava = LiquidAppearance::new(
        1,
        &LiquidKind {
            colour: [0.8, 0.1, 0.02],
            emission: [40.0, 9.0, 0.5],
            viscosity_pa_s: 1.0e3,
            wave_amplitude_m: 0.0,
            wave_length_m: 0.0,
        },
        &look,
    );
    let (la, lb) = (
        [look.shore_fade_m as f32, look.foam_width_m as f32, look.foam_strength as f32, look.choppy as f32],
        [look.fresnel_f0 as f32, look.glint_roughness as f32, 39.0, 0.0],
    );
    let mut probes = Vec::new();
    let mut refs = Vec::new();
    for k in 0..count as u64 {
        let up = [0.0, 0.0, 1.0];
        let tilt = |i: u64, s: f64| [(unit(seed, 16 * k + i) - 0.5) * s, (unit(seed, 16 * k + i + 1) - 0.5) * s, 1.0];
        let n = norm3(tilt(0, 0.4));
        // Half of the probes look along the sun's mirror direction (the glint), where the distribution is sharpest.
        let sun = norm3([unit(seed, 16 * k + 2) - 0.5, unit(seed, 16 * k + 3) - 0.5, 0.2 + 0.8 * unit(seed, 16 * k + 4)]);
        let v = if k % 2 == 0 {
            norm3([-sun[0] + (unit(seed, 16 * k + 5) - 0.5) * 0.03, -sun[1] + (unit(seed, 16 * k + 6) - 0.5) * 0.03, sun[2]])
        } else {
            norm3([unit(seed, 16 * k + 7) - 0.5, unit(seed, 16 * k + 8) - 0.5, 0.05 + 0.95 * unit(seed, 16 * k + 9)])
        };
        let depth = [0.0, 0.01, 0.04, 0.2, 0.39, 1.0, 3.0, 9.0, 60.0][(k % 9) as usize];
        let liquid = if k % 5 == 4 { lava } else { water };
        let rgb = |i: u64, s: f64| [unit(seed, 16 * k + i) * s, unit(seed, 16 * k + i + 1) * s, unit(seed, 16 * k + i + 2) * s];
        let probe = SurfaceProbe {
            liquid,
            la,
            lb,
            n: f32s(n),
            v: f32s(v),
            up: f32s(up),
            sun: f32s(sun),
            depth: depth as f32,
            foam_mod: unit(seed, 16 * k + 10) as f32,
            seabed: f32s(rgb(11, 0.4)),
            diffuse: f32s(rgb(11, 0.3)),
            sun_t: f32s([0.9, 0.8, 0.7]),
            sky: f32s(rgb(13, 0.9)),
        };
        // The reference on the f32-rounded inputs (unit vectors rounded to f32 are unit within 1e-7: the shader normalises nothing
        // of them but the half vector, so the reference does the same on the rounded values).
        let inp = SurfaceInputs {
            n: wide(probe.n),
            v: wide(probe.v),
            up: wide(probe.up),
            sun: wide(probe.sun),
            depth: f64::from(probe.depth),
            foam_mod: f64::from(probe.foam_mod),
            seabed: wide(probe.seabed),
            diffuse: wide(probe.diffuse),
            sun_t: wide(probe.sun_t),
            sky: wide(probe.sky),
        };
        refs.push(surface_radiance(&liquid.as_f32(), &look.as_f32(), &inp));
        probes.push(probe);
    }
    (probes, refs)
}

// spec: ATMO-014
#[test]
fn the_surface_radiance_matches_the_reference_within_one_percent_including_the_glint_the_foam_and_a_lava_lake() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (probes, refs) = surface_cases(900, 5);
    let got = probe_surface(&ctx, &probes);
    let mut worst: f64 = 0.0;
    let mut bright = 0;
    for (i, (g, r)) in got.iter().zip(&refs).enumerate() {
        for c in 0..3 {
            let bound = (0.01 * r[c].abs()).max(1.0e-4);
            let e = (f64::from(g[c]) - r[c]).abs();
            worst = worst.max(e / bound);
            assert!(
                e <= bound,
                "probe {i} channel {c}: {g:?} against {r:?} (depth {}, view {:?}, sun {:?})",
                probes[i].depth,
                probes[i].v,
                probes[i].sun
            );
        }
        if r.iter().any(|v| *v > 5.0) {
            bright += 1;
        }
    }
    assert!(bright > 50, "the cases include the glint and the lava: {bright} bright probes");
    println!("surface: worst error {worst:.3} of the bound over {} probes ({bright} bright)", probes.len());
}

// spec: ATMO-014
#[test]
fn a_camera_walking_a_millimetre_at_a_time_at_one_metre_changes_the_surface_by_no_more_than_the_reference_does() {
    // CON-11 v1.4, ADR 0043: temporal stability at 1 m. The camera moves 1 mm per frame over 200 frames; the water point stays, the
    // view vector and the camera distance change. No step of the GPU's output may differ from the reference's step by more than the
    // bound of the function.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let look = WaterLook::EARTH;
    let water = LiquidAppearance::new(0, &LiquidKind::WATER, &look);
    let sun = norm3([0.3, -0.5, 0.8]);
    let point = [14.0_f64, 3.0, 0.0];
    let mut surface = Vec::new();
    let mut waves = Vec::new();
    let mut want_surface = Vec::new();
    let mut want_waves = Vec::new();
    let phases = tile_phases(&water, [R * 0.6, R * 0.64, R * 0.48], 4321.0);
    for step in 0..200 {
        // The camera 1 m above the water, 14 m to the south of the point, walking 1 mm a frame towards it.
        let cam = [0.0, -14.0 + step as f64 * 0.001, 1.0];
        let to_cam = [cam[0] - point[0], cam[1] - point[1], cam[2] - point[2]];
        let v = norm3(to_cam);
        let probe = SurfaceProbe {
            liquid: water,
            la: [look.shore_fade_m as f32, look.foam_width_m as f32, look.foam_strength as f32, look.choppy as f32],
            lb: [look.fresnel_f0 as f32, look.glint_roughness as f32, 0.0, 0.0],
            n: f32s(norm3([0.02, -0.03, 1.0])),
            v: f32s(v),
            up: [0.0, 0.0, 1.0],
            sun: f32s(sun),
            depth: 3.0,
            foam_mod: 0.5,
            seabed: [0.2, 0.2, 0.15],
            diffuse: [0.25, 0.25, 0.25],
            sun_t: [0.9, 0.9, 0.9],
            sky: [0.3, 0.4, 0.7],
        };
        let inp = SurfaceInputs {
            n: wide(probe.n),
            v: wide(probe.v),
            up: [0.0, 0.0, 1.0],
            sun: wide(probe.sun),
            depth: 3.0,
            foam_mod: 0.5,
            seabed: wide(probe.seabed),
            diffuse: wide(probe.diffuse),
            sun_t: wide(probe.sun_t),
            sky: wide(probe.sky),
        };
        want_surface.push(surface_radiance(&water.as_f32(), &look.as_f32(), &inp));
        surface.push(probe);
        let wp = WaveProbe {
            liquid: water,
            local: f32s(point),
            spacing: (2.0 + 0.001 * step as f64) as f32,
            up: [0.0, 0.0, 1.0],
            depth: 3.0,
            phases,
            choppy: look.choppy as f32,
        };
        let w = WavePoint {
            local: wide(wp.local),
            up: [0.0, 0.0, 1.0],
            phases: phases.map(f64::from),
            spacing: f64::from(wp.spacing),
            depth: 3.0,
        };
        want_waves.push(wave_sample(&water.as_f32(), &w, look.choppy).disp);
        waves.push(wp);
    }
    let got_surface = probe_surface(&ctx, &surface);
    let got_waves = probe_waves(&ctx, &waves);
    let bound_w = 1.0e-3 + 1.0e-3 * water.amplitude_sum();
    let mut steps = 0;
    for i in 1..200 {
        for c in 0..3 {
            let (dg, dr) = (f64::from(got_surface[i][c]) - f64::from(got_surface[i - 1][c]), want_surface[i][c] - want_surface[i - 1][c]);
            let bound = 2.0 * (0.01 * want_surface[i][c].abs()).max(1.0e-4);
            assert!((dg - dr).abs() <= bound, "frame {i} channel {c}: step {dg} against the reference's {dr}");
            let (wg, wr) = (f64::from(got_waves[i].0[c]) - f64::from(got_waves[i - 1].0[c]), want_waves[i][c] - want_waves[i - 1][c]);
            assert!((wg - wr).abs() <= 2.0 * bound_w, "frame {i} component {c}: displacement step {wg} against {wr}");
            steps += 1;
        }
    }
    assert_eq!(steps, 199 * 3);
}
