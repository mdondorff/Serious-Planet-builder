//! Tier B: the atmosphere's GPU passes against the f64 reference (ATMO-001, ATMO-010; ADR 0040 points 3-5, ADR 0043). The reference runs
//! on the f32-rounded inputs the GPU receives (`rounded_f32`) with the same texel mapping, step counts and placement, so the bound
//! measures arithmetic: `|GPU - ref| <= 1 % |ref| + 1e-4` per channel. Camera altitudes 1 m, 2 km and 20 km with three suns including
//! sunset, the per-pixel orbit march at 20,000 km including the planet's limb, and a camera walking at 1 m altitude in millimetre
//! steps (temporal stability: no frame-to-frame step the reference does not have).
#![allow(clippy::needless_range_loop)]

use planet_core::{PlanetFixed, Vec3};
use planet_frame::look::atmosphere::*;
use planet_frame::Camera;
use planet_render::{AdapterPolicy, AtmospherePath, GpuContext, ProbeKind};

const R: f64 = 6_371_000.0;
const REL: f64 = 0.01;
const ABS: f64 = 1.0e-4;

fn within(got: f64, want: f64) -> bool {
    (got - want).abs() <= REL * want.abs() + ABS
}

struct Rig {
    ctx: GpuContext,
    atm: Atmosphere,
    luts: StaticLuts,
    path: AtmospherePath,
}

/// One device for all the tests of this binary, used one at a time: several WARP devices in one process crash it (the learning of
/// 2026-10-16), and the path's uniform block is shared state. `cargo xtask test` runs one process per test anyway.
fn rig() -> std::sync::MutexGuard<'static, Rig> {
    static RIG: std::sync::OnceLock<std::sync::Mutex<Rig>> = std::sync::OnceLock::new();
    RIG.get_or_init(|| {
        let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
        let atm = Atmosphere::earth(R);
        let luts = StaticLuts::generate(&atm);
        let mut path = AtmospherePath::standalone(&ctx, (64, 48));
        path.upload_tables(&ctx, &atm, &luts);
        std::sync::Mutex::new(Rig { ctx, atm, luts, path })
    })
    .lock()
    .unwrap_or_else(|e| e.into_inner())
}

fn camera(h: f64) -> Camera {
    Camera::at_surface_point(PlanetFixed(Vec3::new(0.0, 0.0, R + h)), Vec3::new(1.0, 0.0, 0.0), 0.0, 60f64.to_radians(), 4.0 / 3.0, 0.1)
}

fn frame(atm: &Atmosphere, cam: &Camera, mu_sun: f64) -> FrameGeometry {
    let sun = [libm::sqrt(1.0 - mu_sun * mu_sun), 0.0, mu_sun];
    FrameGeometry::new(atm, cam, sun)
}

impl Rig {
    fn set_frame(&self, geometry: &FrameGeometry) {
        self.path.write_frame(&self.ctx, &self.atm, geometry, 1.0, AtmosphereView::Composite);
        self.path.run_products(&self.ctx);
    }

    fn rounded(&self) -> (Atmosphere, StaticLuts) {
        (self.atm.rounded_f32(), self.luts.rounded_f32())
    }
}

// spec: ATMO-001
#[test]
fn the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns() {
    let r = rig();
    let (atm, luts) = r.rounded();
    let mut worst = (0.0f64, String::new());
    for h in [1.0, 2000.0, 20_000.0] {
        for mu_sun in [0.8, 0.05, -0.08] {
            let geometry = frame(&r.atm, &camera(h), mu_sun);
            r.set_frame(&geometry);
            let gpu = r.path.read_sky_view(&r.ctx);
            let g = geometry.rounded_f32();
            for iy in (0..SKY_H).step_by(2) {
                for ix in (0..SKY_W).step_by(5) {
                    let want = g.sky_view_texel(&atm, &luts, ix, iy);
                    let got = gpu[iy * SKY_W + ix];
                    for k in 0..3 {
                        let excess = (f64::from(got[k]) - want[k]).abs() / (REL * want[k].abs() + ABS);
                        if excess > worst.0 {
                            worst = (excess, format!("h {h} sun {mu_sun} texel ({ix},{iy}) channel {k}: {} vs {}", got[k], want[k]));
                        }
                    }
                }
            }
        }
    }
    eprintln!("sky view: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "sky-view table exceeds the bound by {}: {}", worst.0, worst.1);
}

// spec: ATMO-001
#[test]
fn the_aerial_perspective_volume_matches_the_reference_at_three_altitudes_and_three_suns() {
    let r = rig();
    let (atm, luts) = r.rounded();
    let mut worst = (0.0f64, String::new());
    for h in [1.0, 2000.0, 20_000.0] {
        for mu_sun in [0.8, 0.05, -0.08] {
            let geometry = frame(&r.atm, &camera(h), mu_sun);
            r.set_frame(&geometry);
            let gpu = r.path.read_aerial_perspective(&r.ctx);
            let g = geometry.rounded_f32();
            for (ix, iy) in [(16, 16), (3, 20), (28, 9), (16, 28), (1, 30)] {
                for iz in (0..AP_D).step_by(3) {
                    let want = g.ap_texel(&atm, &luts, ix, iy, iz);
                    let got = gpu[(iz * AP_H + iy) * AP_W + ix];
                    for k in 0..4 {
                        let abs = if k == 3 { 1e-3 } else { ABS };
                        let excess = (f64::from(got[k]) - want[k]).abs() / (REL * want[k].abs() + abs);
                        if excess > worst.0 {
                            worst = (excess, format!("h {h} sun {mu_sun} froxel ({ix},{iy},{iz}) channel {k}: {} vs {}", got[k], want[k]));
                        }
                    }
                }
            }
        }
    }
    eprintln!("aerial perspective: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "aerial-perspective volume exceeds the bound by {}: {}", worst.0, worst.1);
}

/// Directions seen from `h` above the pole: `angle` from the nadir towards +x.
fn orbit_dir(angle: f64) -> [f64; 3] {
    [libm::sin(angle), 0.0, -libm::cos(angle)]
}

// spec: ATMO-001
#[test]
fn the_orbit_march_matches_the_reference_per_ray_including_the_limb_at_20000_km() {
    let r = rig();
    let (atm, luts) = r.rounded();
    let h = 2.0e7;
    let mut worst = (0.0f64, String::new());
    for mu_sun in [0.8, 0.05, -0.08] {
        let geometry = frame(&r.atm, &camera(h), mu_sun);
        r.path.write_frame(&r.ctx, &r.atm, &geometry, 1.0, AtmosphereView::Composite);
        let g = geometry.rounded_f32();
        let limb = libm::asin((R + 100_000.0) / (R + h));
        let ground_limb = libm::asin(R / (R + h));
        let mut angles: Vec<f64> = (0..60).map(|k| limb * (k as f64 + 0.5) / 60.0).collect();
        for d in [-1e-4, -1e-5, -1e-6, 1e-6, 1e-5, 1e-4] {
            angles.push(limb + d);
            angles.push(ground_limb + d);
        }
        // The rays are given by their cosines to the up and to the sun, rounded to f32 as the shader's dot products round them (the
        // limb at 20,000 km moves by 2.5e-7 rad = 0.04 px with that rounding, which is the input's precision, not the march's).
        let cosines: Vec<[f32; 2]> = angles
            .iter()
            .map(|a| {
                let d = orbit_dir(*a);
                [d[2] as f32, (d[0] * geometry.sun[0] + d[2] * geometry.sun[2]) as f32]
            })
            .collect();
        for t_geo in [-1.0f32, 2.0e7 - 5.0e4] {
            let queries: Vec<[f32; 4]> = cosines.iter().map(|c| [c[0], c[1], t_geo, 0.0]).collect();
            let out = r.path.probe(&r.ctx, ProbeKind::OrbitCosines, &queries);
            for (i, c) in cosines.iter().enumerate() {
                let want = g.orbit_ray_cosines(&atm, &luts, f64::from(c[0]), f64::from(c[1]), (t_geo >= 0.0).then_some(f64::from(t_geo)));
                let (a, b) = (out[2 * i], out[2 * i + 1]);
                // Whether the ray crosses the atmosphere or reaches the ground is a discrete decision at the limb: a ray the
                // reference puts within f32 rounding of the edge may fall on either side; its values are then compared as
                // both outcomes would agree within the bound only away from the edge.
                let near_edge = [limb, ground_limb].iter().any(|e| (angles[i] - e).abs() < 3e-7);
                if near_edge && ((b[3] > 0.5) != want.enters || (a[3] > 0.5) != want.reaches_ground) {
                    continue;
                }
                assert_eq!(b[3] > 0.5, want.enters, "ray {i} (angle {}): crosses the atmosphere", angles[i]);
                for k in 0..3 {
                    for (got, want, what) in
                        [(f64::from(a[k]), want.radiance[k], "radiance"), (f64::from(b[k]), want.transmittance[k], "transmittance")]
                    {
                        // Within 2e-6 rad of the ground's limb (0.3 px at 1440p, 60 degrees) the product r mu of an f32 cosine (rounded to
                        // 1.5 m at 20,000 km) moves the tangent by more than the march resolves: the bound there is three times as wide
                        // (ADR 0040 point 6: the limb-stable discriminant is the next step if the real frames show it).
                        let excess = (got - want).abs() / (REL * want.abs() + ABS);
                        if excess > worst.0 {
                            worst = (
                                excess,
                                format!("sun {mu_sun} t_geo {t_geo} ray {i} angle {} channel {k} {what}: {got} vs {want}", angles[i]),
                            );
                        }
                    }
                }
            }
        }
    }
    eprintln!("orbit march: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "orbit march exceeds the bound by {}: {}", worst.0, worst.1);
}

// spec: ATMO-001
#[test]
fn the_sky_view_coordinates_and_the_lighting_functions_match_the_reference() {
    let r = rig();
    let (atm, luts) = r.rounded();
    // Sky-view coordinates of directions around the horizon at 1 m, to a hundredth of a texel (the rows are finest at the horizon).
    let geometry = frame(&r.atm, &camera(1.0), 0.3);
    r.path.write_frame(&r.ctx, &r.atm, &geometry, 1.0, AtmosphereView::Composite);
    let g = geometry.rounded_f32();
    let mut dirs = Vec::new();
    for k in 0..200 {
        let theta = 1.2 + 0.7 * f64::from(k) / 200.0;
        for phi in [0.0, 0.3, 1.5, 3.0] {
            dirs.push([libm::sin(theta) * libm::cos(phi), libm::sin(theta) * libm::sin(phi), libm::cos(theta)]);
        }
    }
    let queries: Vec<[f32; 4]> = dirs.iter().map(|d| [d[0] as f32, d[1] as f32, d[2] as f32, 0.0]).collect();
    let out = r.path.probe(&r.ctx, ProbeKind::SkyCoords, &queries);
    let mut worst_rows = 0.0f64;
    for (d, o) in dirs.iter().zip(&out) {
        let want = g.sky_view_coords(d.map(|v| f64::from(v as f32)));
        let du = (f64::from(o[0]) - want.0).abs() * (SKY_W - 1) as f64;
        let dv = (f64::from(o[1]) - want.1).abs() * (SKY_H - 1) as f64;
        worst_rows = worst_rows.max(dv);
        assert!(du < 0.15 && dv < 0.15, "direction {d:?}: texel offset ({du}, {dv}) from the reference");
    }
    eprintln!("sky-view coordinates: worst row offset {worst_rows:.4} texels");
    // Sun transmittance, planet shadow and the ground's radiance by altitude and sun cosine.
    let mut q = Vec::new();
    for h in [0.0, 1.0, 500.0, 5000.0, 30_000.0, 99_000.0] {
        for mu in [-0.2, -0.05, -0.005, 0.0, 0.005, 0.1, 0.5, 1.0] {
            q.push([h as f32, mu as f32, 0.0, 0.0]);
        }
    }
    // Inside the penumbra at altitude (where the width w R / r matters).
    for h in [30_000.0f64, 99_000.0] {
        let mu_h = -libm::sqrt(h * (2.0 * R + h)) / (R + h);
        let w = 0.004_675 * R / (R + h);
        for k in [-0.9, -0.577, 0.0, 0.577, 0.9] {
            q.push([h as f32, (mu_h + k * w) as f32, 0.0, 0.0]);
        }
    }
    let t = r.path.probe(&r.ctx, ProbeKind::Transmittance, &q);
    let gr = r.path.probe(&r.ctx, ProbeKind::Ground, &q);
    for (i, qq) in q.iter().enumerate() {
        let (h, mu) = (f64::from(qq[0]), f64::from(qq[1]));
        let want = atm.transmittance(&luts, h, mu);
        let shadow = atm.earth_shadow(h, mu);
        let ground = atm.ground_radiance(&luts, mu);
        for k in 0..3 {
            assert!(within(f64::from(t[i][k]), want[k]), "transmittance h {h} mu {mu} channel {k}: {} vs {}", t[i][k], want[k]);
            assert!(within(f64::from(gr[i][k]), ground[k]), "ground h {h} mu {mu} channel {k}: {} vs {}", gr[i][k], ground[k]);
        }
        assert!((f64::from(t[i][3]) - shadow).abs() <= 1e-3, "shadow h {h} mu {mu}: {} vs {shadow}", t[i][3]);
    }
    // The sun's disc across its rim, from f32-rounded directions (the coverage ramps over 6 % of its radius).
    let w = atm.params.sun_angular_radius_rad;
    let sun = g.sun;
    let east = g.sun_e2;
    let mut dq = Vec::new();
    let mut dirs = Vec::new();
    for k in -6..=6 {
        let angle = w * (1.0 + 0.01 * f64::from(k));
        let d: [f64; 3] = [0, 1, 2].map(|c| libm::cos(angle) * sun[c] + libm::sin(angle) * east[c]);
        let d32 = d.map(|v| f64::from(v as f32));
        dq.push([d32[0] as f32, d32[1] as f32, d32[2] as f32, 0.0]);
        dirs.push(d32);
    }
    let cov = r.path.probe(&r.ctx, ProbeKind::SunDisc, &dq);
    for (c, d) in cov.iter().zip(&dirs) {
        let want = sun_disc_coverage(*d, sun, w);
        assert!((f64::from(c[0]) - want).abs() <= 0.03, "disc at {d:?}: {} vs {want}", c[0]);
    }
}

// spec: ATMO-010
#[test]
fn a_camera_walking_at_one_metre_in_millimetre_steps_makes_no_step_the_reference_does_not() {
    // ADR 0043: temporal stability at 1 m altitude. The camera rises from 1.000 m to 1.200 m in 1 mm steps (200 frames); the
    // sky-view texels of the horizon band and the aerial-perspective texels along the ground are compared frame to frame: the GPU's
    // change between two frames must equal the reference's change within the bound (no stepping beyond it).
    let r = rig();
    let (atm, luts) = r.rounded();
    let texels: Vec<(usize, usize)> = (50..58).flat_map(|iy| [10, 60, 120, 180].map(|ix| (ix, iy))).collect();
    let aerial: Vec<(usize, usize, usize)> = vec![(16, 30, 4), (16, 28, 10), (8, 31, 20), (24, 29, 31), (16, 31, 1)];
    type Walk = (Vec<[f64; 3]>, Vec<[f64; 3]>, Vec<[f64; 4]>, Vec<[f64; 4]>);
    let mut prevs: Vec<Option<Walk>> = vec![None, None, None];
    let (mut steps, mut worst) = (0usize, 0.0f64);
    for frame_index in 0..200 {
        let h = 1.0 + 0.001 * f64::from(frame_index);
        // The sun high, on the horizon and just below it (inside the penumbra: no sample may jump).
        for (si, mu_sun) in [0.3, 0.0, -0.004].into_iter().enumerate() {
            let geometry = frame(&r.atm, &camera(h), mu_sun);
            r.set_frame(&geometry);
            let sky = r.path.read_sky_view(&r.ctx);
            let ap = r.path.read_aerial_perspective(&r.ctx);
            let g = geometry.rounded_f32();
            let got_sky: Vec<[f64; 3]> =
                texels.iter().map(|&(ix, iy)| sky[iy * SKY_W + ix]).map(|t| [t[0], t[1], t[2]].map(f64::from)).collect();
            let want_sky: Vec<[f64; 3]> = texels.iter().map(|&(ix, iy)| g.sky_view_texel(&atm, &luts, ix, iy)).collect();
            let got_ap: Vec<[f64; 4]> = aerial.iter().map(|&(ix, iy, iz)| ap[(iz * AP_H + iy) * AP_W + ix].map(f64::from)).collect();
            let want_ap: Vec<[f64; 4]> = aerial.iter().map(|&(ix, iy, iz)| g.ap_texel(&atm, &luts, ix, iy, iz)).collect();
            if let Some((ps, pw, pa, pwa)) = &prevs[si] {
                for i in 0..texels.len() {
                    for k in 0..3 {
                        let (dg, dw) = (got_sky[i][k] - ps[i][k], want_sky[i][k] - pw[i][k]);
                        let excess = (dg - dw).abs() / (REL * want_sky[i][k].abs() + ABS);
                        worst = worst.max(excess);
                        if excess > 1.0 {
                            steps += 1;
                        }
                    }
                }
                for i in 0..aerial.len() {
                    for k in 0..3 {
                        let (dg, dw) = (got_ap[i][k] - pa[i][k], want_ap[i][k] - pwa[i][k]);
                        let excess = (dg - dw).abs() / (REL * want_ap[i][k].abs() + ABS);
                        worst = worst.max(excess);
                        if excess > 1.0 {
                            steps += 1;
                        }
                    }
                }
            }
            prevs[si] = Some((got_sky, want_sky, got_ap, want_ap));
        }
    }
    eprintln!("walk at 1 m: {steps} steps beyond the bound over 199 frame pairs and three suns, worst {worst:.3} of the bound");
    assert_eq!(steps, 0, "frame-to-frame steps beyond the bound (worst {worst})");
}

// spec: ATMO-001
#[test]
fn the_terrain_lighting_matches_the_reference() {
    let r = rig();
    let (atm, luts) = r.rounded();
    let geometry = frame(&r.atm, &camera(1.0), 0.3);
    r.path.write_frame(&r.ctx, &r.atm, &geometry, 1.0, AtmosphereView::Composite);
    let norm = |v: [f64; 3]| {
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        v.map(|x| x / l)
    };
    let mut queries = Vec::new();
    let mut inputs = Vec::new();
    for h in [0.0, 3.0, 800.0, 12_000.0, 90_000.0] {
        for mu_sun in [-0.3, -0.06, -0.01, 0.0, 0.02, 0.4, 1.0] {
            for tilt in [0.0, 0.6, 1.4, 2.5] {
                let up = [0.0, 0.0, 1.0];
                let sun = [libm::sqrt(1.0 - mu_sun * mu_sun), 0.0, mu_sun];
                let normal = norm([libm::sin(tilt), 0.3 * libm::sin(tilt), libm::cos(tilt)]);
                let f = |v: [f64; 3]| v.map(|x| f64::from(x as f32));
                let (n32, u32_, s32) = (f(normal), f(up), f(sun));
                queries.push([n32[0] as f32, n32[1] as f32, n32[2] as f32, h as f32]);
                queries.push([u32_[0] as f32, u32_[1] as f32, u32_[2] as f32, 0.0]);
                queries.push([s32[0] as f32, s32[1] as f32, s32[2] as f32, 0.0]);
                inputs.push((n32, u32_, s32, h));
            }
        }
    }
    let out = r.path.probe(&r.ctx, ProbeKind::TerrainLighting, &queries);
    let mut worst = 0.0f64;
    for (o, (n, u, s, h)) in out.iter().zip(&inputs) {
        let want = atm.terrain_lighting(&luts, *n, *u, *h, *s);
        for k in 0..3 {
            let excess = (f64::from(o[k]) - want[k]).abs() / (REL * want[k].abs() + ABS);
            worst = worst.max(excess);
            assert!(excess <= 1.0, "h {h} normal {n:?} sun {s:?} channel {k}: {} vs {}", o[k], want[k]);
        }
    }
    eprintln!("terrain lighting: worst {worst:.3} of the bound");
}

fn pixel_ray(g: &FrameGeometry, px: usize, py: usize, w: usize, h: usize) -> [f64; 3] {
    let a = 2.0 * (px as f64 + 0.5) / w as f64 - 1.0;
    let b = 1.0 - 2.0 * (py as f64 + 0.5) / h as f64;
    [0, 1, 2].map(|k| g.fwd[k] + g.right[k] * a + g.upv[k] * b)
}

fn unit(v: [f64; 3]) -> ([f64; 3], f64) {
    let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    (v.map(|x| x / l), l)
}

// spec: ATMO-001, ATMO-002
#[test]
fn the_composite_applies_the_sky_and_the_aerial_perspective_as_the_reference_does() {
    for (h, mu_sun) in [(2000.0, 0.3), (1.0, 0.8), (20_000.0, 0.05), (2000.0, -0.08)] {
        composite_inside(h, mu_sun);
    }
}

fn composite_inside(altitude: f64, mu_sun: f64) {
    composite_inside_at(altitude, mu_sun, 8000.0);
}

/// The composite with the terrain `terrain_m` along the view axis (right half of the frame), against the reference.
fn composite_inside_at(altitude: f64, mu_sun: f64, terrain_m: f64) {
    let r = rig();
    let (w, h) = (64usize, 48usize);
    let cam = camera(altitude);
    let geometry = frame(&r.atm, &cam, mu_sun);
    r.set_frame(&geometry);
    let g = geometry.rounded_f32();
    let sky: Vec<[f64; 3]> = r.path.read_sky_view(&r.ctx).iter().map(|t| [t[0], t[1], t[2]].map(f64::from)).collect();
    let ap: Vec<[f64; 4]> = r.path.read_aerial_perspective(&r.ctx).iter().map(|t| t.map(f64::from)).collect();
    // The right half of the frame is terrain 8 km along the view axis, the left half is sky.
    let colour = [0.2f32, 0.1, 0.05, 1.0];
    let scene: Vec<[f32; 4]> = (0..w * h).map(|i| if i % w >= w / 2 { colour } else { [0.0; 4] }).collect();
    let depth: Vec<f32> = (0..w * h).map(|i| if i % w >= w / 2 { (0.1 / terrain_m) as f32 } else { 0.0 }).collect();
    r.path.upload_scene(&r.ctx, &scene);
    r.path.upload_depth(&r.ctx, &depth);
    let out = r.path.composite_to_image(&r.ctx);
    let (mut worst, mut seen_sky, mut seen_geo) = (0.0f64, 0, 0);
    for py in 0..h {
        for px in 0..w {
            let (dir, len) = unit(pixel_ray(&g, px, py, w, h));
            let got = out[py * w + px];
            let want: [f64; 3] = if px >= w / 2 {
                seen_geo += 1;
                let a = g.ap_lookup(&ap, [(px as f64 + 0.5) / w as f64, (py as f64 + 0.5) / h as f64], terrain_m * len);
                [0, 1, 2].map(|k| f64::from(colour[k]) * a[3] + a[k])
            } else {
                seen_sky += 1;
                g.sky_view_lookup(&sky, dir)
            };
            for k in 0..3 {
                let excess = (f64::from(got[k]) - want[k]).abs() / (REL * want[k].abs() + ABS);
                worst = worst.max(excess);
                assert!(excess <= 1.0, "pixel ({px},{py}) channel {k}: {} vs {}", got[k], want[k]);
            }
        }
    }
    eprintln!(
        "composite inside at {altitude} m, sun {mu_sun}: {seen_sky} sky and {seen_geo} terrain pixels, worst {worst:.3} of the bound"
    );
    if terrain_m != 8000.0 {
        return; // the colour-shift check below is the near-ground case's
    }
    // Distant terrain is hazier and bluer than near terrain (ATMO-002): compare the same colour at 1 km and 100 km.
    let hazy = |t: f64| {
        let a = g.ap_lookup(&ap, [0.75, 0.5], t);
        [0, 1, 2].map(|k| f64::from(colour[k]) * a[3] + a[k])
    };
    if mu_sun < 0.1 {
        return;
    }
    let (near, far) = (hazy(1000.0), hazy(100_000.0));
    assert!(far[2] / far[0] > near[2] / near[0], "the distance shifts the colour towards blue: {near:?} {far:?}");
    assert!(far[2] > near[2], "and brightens the blue channel of this dark terrain: {near:?} {far:?}");
}

// spec: ATMO-021
#[test]
fn the_composite_reads_the_frames_range_for_terrain_beyond_256_km() {
    // A camera at 99 km sees terrain 600 km away; the frame's range is its horizon (about 1,130 km), not 256 km, in the volume the
    // shader reads and the reference reads.
    composite_inside_at(99_000.0, 0.7, 600_000.0);
    composite_inside_at(20_000.0, 0.7, 400_000.0);
}

// spec: ATMO-001
#[test]
fn the_composite_from_orbit_marches_each_pixel_as_the_reference_does() {
    let r = rig();
    let (atm, luts) = r.rounded();
    let (w, h) = (64usize, 48usize);
    let cam = Camera::looking_at_centre(
        PlanetFixed(Vec3::new(0.0, 0.0, R + 2.0e7)),
        Vec3::new(1.0, 0.0, 0.0),
        60f64.to_radians(),
        4.0 / 3.0,
        0.1,
    );
    let geometry = frame(&r.atm, &cam, 0.4);
    r.path.write_frame(&r.ctx, &r.atm, &geometry, 1.0, AtmosphereView::Composite);
    let g = geometry.rounded_f32();
    r.path.upload_scene(&r.ctx, &vec![[0.0; 4]; w * h]);
    r.path.upload_depth(&r.ctx, &vec![0.0; w * h]);
    let out = r.path.composite_to_image(&r.ctx);
    let (limb, ground_limb) = (libm::asin((R + 100_000.0) / g.r), libm::asin(R / g.r));
    let (mut worst, mut compared) = (0.0f64, 0);
    for py in 0..h {
        for px in 0..w {
            let (dir, _) = unit(pixel_ray(&g, px, py, w, h));
            let (mu, _) = g.view_cosines(dir);
            let angle = libm::acos(-mu);
            if [limb, ground_limb].iter().any(|e| (angle - e).abs() < 3e-4) {
                continue;
            }
            let o = g.orbit_ray(&atm, &luts, dir, None);
            let mut l = o.radiance;
            if o.reaches_ground {
                let gr = atm.ground_radiance(&luts, o.ground_mu_sun);
                for k in 0..3 {
                    l[k] += o.transmittance[k] * gr[k];
                }
            }
            let got = out[py * w + px];
            for k in 0..3 {
                let excess = (f64::from(got[k]) - l[k]).abs() / (REL * l[k].abs() + ABS);
                worst = worst.max(excess);
                assert!(excess <= 1.0, "pixel ({px},{py}) channel {k}: {} vs {}", got[k], l[k]);
            }
            compared += 1;
        }
    }
    eprintln!("composite orbit: {compared} pixels, worst {worst:.3} of the bound");
    assert!(compared > 500);
}

/// Ad hoc cost of the passes at 1440p on the adapter in use (indicative only: wall time of submit-and-wait over 40 runs, not a
/// perf-window measurement). Run with `--ignored --nocapture`.
// spec: ATMO-001
#[test]
#[ignore = "ad hoc timing, not a verdict"]
fn adhoc_cost_of_the_atmosphere_passes_at_1440p() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let atm = Atmosphere::earth(R);
    let luts = StaticLuts::generate(&atm);
    let mut path = AtmospherePath::standalone(&ctx, (2560, 1440));
    path.upload_tables(&ctx, &atm, &luts);
    let n = 2560 * 1440;
    path.upload_scene(&ctx, &vec![[0.1, 0.1, 0.1, 1.0]; n]);
    path.upload_depth(&ctx, &(0..n).map(|i| if (i / 2560) % 2 == 0 { 0.0 } else { 1.0e-5 }).collect::<Vec<_>>());
    for (name, cam) in [
        ("ground 1 m", camera(1.0)),
        ("20 km", camera(20_000.0)),
        (
            "orbit 20,000 km",
            Camera::looking_at_centre(PlanetFixed(Vec3::new(0.0, 0.0, R + 2.0e7)), Vec3::new(1.0, 0.0, 0.0), 1.0, 16.0 / 9.0, 0.1),
        ),
    ] {
        let g = frame(&atm, &cam, 0.4);
        path.write_frame(&ctx, &atm, &g, 1.0, AtmosphereView::Composite);
        let time = |f: &dyn Fn()| {
            f();
            let t = std::time::Instant::now();
            for _ in 0..40 {
                f();
            }
            t.elapsed().as_secs_f64() * 1000.0 / 40.0
        };
        let products = time(&|| {
            path.run_products(&ctx);
            let _ = ctx.adapter_name();
            pollster_wait(&ctx);
        });
        let composite = time(&|| {
            path.composite_only(&ctx);
            pollster_wait(&ctx);
        });
        eprintln!(
            "{} on {}: sky view + aerial perspective {products:.3} ms, composite {composite:.3} ms (wall, submit and wait)",
            name,
            ctx.adapter_name()
        );
    }
}

fn pollster_wait(ctx: &GpuContext) {
    ctx.wait_idle();
}
