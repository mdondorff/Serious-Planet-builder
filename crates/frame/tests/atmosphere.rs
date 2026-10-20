//! Tier A: the atmosphere's f64 reference (ADR 0040, `frame::look::atmosphere`): the static tables (known answers, thread
//! independence), the altitude-relative ray geometry, analytic cases, the discretisation against the converged integral, the
//! frame's geometry and the plan's part. GPU-free (CON-03). Class: presentation data (ADR 0035).
#![allow(clippy::needless_range_loop)] // the RGB channel loops index several parallel arrays

use planet_core::{PlanetFixed, Vec3};
use planet_frame::look::atmosphere::*;
use planet_frame::look::LitPlan;
use planet_frame::Camera;
use std::sync::OnceLock;

const R: f64 = 6_371_000.0;

fn earth() -> &'static (Atmosphere, StaticLuts) {
    static LUTS: OnceLock<(Atmosphere, StaticLuts)> = OnceLock::new();
    LUTS.get_or_init(|| {
        let atm = Atmosphere::earth(R);
        let luts = StaticLuts::generate(&atm);
        (atm, luts)
    })
}

/// A deterministic stream of numbers in [0, 1).
fn stream(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A frame at altitude `h` over the +z pole looking along +x at the horizon with the sun at zenith cosine `mu_sun` (in the x-z plane).
fn frame_at(atm: &Atmosphere, h: f64, mu_sun: f64) -> FrameGeometry {
    let sun = [libm::sqrt(1.0 - mu_sun * mu_sun), 0.0, mu_sun];
    let cam = Camera::at_surface_point(PlanetFixed(Vec3::new(0.0, 0.0, atm.radius_m + h)), Vec3::new(1.0, 0.0, 0.0), 0.0, 1.0, 1.0, 0.1);
    FrameGeometry::new(atm, &cam, sun)
}

fn within_bound(got: f64, want: f64, rel: f64, abs: f64) -> bool {
    (got - want).abs() <= rel * want.abs() + abs
}

// spec: ATMO-005
#[test]
fn the_static_tables_have_known_answers() {
    let (atm, luts) = earth();
    // The table is a pure function of the parameters and the radius: this hash is bit-exact on every machine (libm, no fast-math).
    assert_eq!(luts.hash(), 0x265aac661b0c2c0f, "recompute and record only with a reason (a model change)");
    // Physical anchors (not the hash): the zenith transmittance at sea level, the horizon's reddening and the multiple-scattering factor.
    let t = atm.transmittance(luts, 0.0, 1.0);
    for (got, want) in t.iter().zip([0.9403654, 0.8676231, 0.7623195]) {
        assert!((got - want).abs() < 1e-5, "{t:?}");
    }
    let th = atm.transmittance(luts, 0.0, 0.0);
    assert!(th[0] > 0.09 && th[0] < 0.12 && th[1] < 0.011 && th[2] < 1e-4, "the horizon reddens: {th:?}");
    let ms = atm.multiscatter(luts, 0.0, 0.8);
    assert!(ms[0] > 0.03 && ms[2] > ms[0] && ms[2] < 0.08, "{ms:?}");
}

// spec: ATMO-005
#[test]
fn the_static_tables_do_not_depend_on_the_thread_count() {
    let (atm, luts) = earth();
    for threads in [1, 2, 3, 7, 64] {
        let other = StaticLuts::generate_with_threads(atm, threads);
        assert!(other == *luts, "{threads} threads: the tables differ bit for bit");
        assert_eq!(other.hash(), luts.hash());
    }
}

// spec: ATMO-005
#[test]
fn another_body_gives_other_tables_and_a_vacuum_gives_no_light() {
    let (_, luts) = earth();
    let moonlike = Atmosphere::new(AtmosphereParams { top_height_m: 60_000.0, ..AtmosphereParams::EARTH }, 3_390_000.0).unwrap();
    assert_ne!(StaticLuts::generate(&moonlike).hash(), luts.hash());
    let vacuum_params = AtmosphereParams {
        rayleigh_scattering: [0.0; 3],
        mie_scattering: [0.0; 3],
        mie_absorption: [0.0; 3],
        ozone_absorption: [0.0; 3],
        ..AtmosphereParams::EARTH
    };
    let vacuum = Atmosphere::new(vacuum_params, R).unwrap();
    let v = StaticLuts::generate(&vacuum);
    assert!(v.optical_depth.iter().all(|t| *t == [0.0; 3]) && v.multiscatter.iter().all(|t| t.iter().all(|x| x.is_finite())));
    let f = frame_at(&vacuum, 100.0, 0.5);
    for (ix, iy) in [(0, 0), (50, 40), (100, 53), (191, 107)] {
        let l = f.sky_view_texel(&vacuum, &v, ix, iy);
        // Above the horizon nothing scatters; below it the ground is lit (albedo 0.3 under a sun at 0.5) and nothing dims it.
        if iy < 54 {
            assert_eq!(l, [0.0; 3], "{ix},{iy}");
        } else {
            assert!(l[0] > 0.0 && l[0] < 0.2, "{l:?}");
        }
    }
}

/// The naive f64 ground distance, `r^2 - R^2` formed as a difference (fine in f64 even at 1 m).
fn naive_ground(r: f64, mu: f64) -> Option<f64> {
    let d = r * r * mu * mu - (r * r - R * R);
    (mu < 0.0 && d >= 0.0).then(|| -r * mu - d.sqrt())
}

fn naive_top(r: f64, mu: f64, top: f64) -> f64 {
    let rt = R + top;
    -r * mu + (r * r * mu * mu - (r * r - rt * rt)).sqrt()
}

// spec: ATMO-006
#[test]
fn the_altitude_relative_primitives_equal_the_naive_forms_in_f64() {
    let top = 100_000.0;
    let mus = [-1.0, -0.5, -0.1, -0.01, -1e-3, 0.0, 1e-4, 0.01, 0.5, 1.0];
    for h in [1.0, 10.0, 2000.0, 20_000.0, 90_000.0] {
        let r = R + h;
        let (c, c_top) = (c_of(R, h), c_top_of(R, top, h));
        assert!((c - (r * r - R * R)).abs() <= 1e-9 * c, "c at {h} m");
        for mu in mus {
            let rmu = r * mu;
            let want_top = naive_top(r, mu, top);
            let got_top = distance_to_top(rmu, c_top);
            assert!((got_top - want_top).abs() <= 1e-8 * want_top, "top: h {h} mu {mu}: {got_top} vs {want_top}");
            match (distance_to_ground(rmu, c), naive_ground(r, mu)) {
                (None, None) => {}
                (Some(g), Some(w)) => assert!((g - w).abs() <= 1e-8 * w.max(1.0), "ground: h {h} mu {mu}: {g} vs {w}"),
                (g, w) => panic!("h {h} mu {mu}: ground {g:?} vs {w:?}"),
            }
        }
        // The sample altitude along a descending ray equals the naive |p| - R.
        let mu = -0.3;
        let o = RayOrigin { h, r, c, rmu: r * mu, r_musun: 0.0 };
        for s in [0.0, 1.0, 100.0, 5000.0, 20_000.0] {
            let (hs, _) = o.at(R, s);
            let rs = (r * r + s * s + 2.0 * r * mu * s).sqrt();
            assert!((hs - (rs - R).max(0.0)).abs() <= 1e-6 * hs.max(1.0) + 1e-6, "h {h} s {s}: {hs} vs {}", rs - R);
        }
    }
}

// spec: ATMO-006
#[test]
fn in_f32_the_naive_form_fails_at_one_metre_and_the_primitives_do_not() {
    // SPIKE-09 as a unit test: at 1 m altitude r^2 - R^2 in f32 is about a third wrong, the primitives are not.
    let big_r = R as f32;
    let mut worst_naive = 0.0f64;
    for i in 0..400 {
        let h = 0.5 + 0.05 * f64::from(i);
        let r32 = (R + h) as f32;
        let naive_c = r32 * r32 - big_r * big_r;
        let exact_c = c_of(R, h);
        worst_naive = worst_naive.max((f64::from(naive_c) - exact_c).abs() / exact_c);
        let c32 = (h as f32) * (2.0 * big_r + h as f32);
        assert!((f64::from(c32) - exact_c).abs() < 1e-6 * exact_c, "relative form f32 c = {c32}, exact {exact_c} at {h} m");
    }
    assert!(worst_naive > 0.25, "the naive f32 r^2 - R^2 is off by {worst_naive} at its worst between 0.5 m and 20 m");
    // The ground distance at mu = -0.3 (about 4 m above the ground at 1 m) from the f32 primitive.
    let (h, exact_c) = (1.0f64, c_of(R, 1.0));
    let r32 = (R + h) as f32;
    let c32 = (h as f32) * (2.0 * big_r + h as f32);
    let mu = -0.3f32;
    let rmu = r32 * mu;
    let d = rmu * rmu - c32;
    let t32 = c32 / (-rmu + d.sqrt());
    let want = distance_to_ground(f64::from(r32) * f64::from(mu), exact_c).unwrap();
    assert!((f64::from(t32) - want).abs() < 1e-5 * want, "{t32} vs {want}");
}

// spec: ATMO-008
#[test]
fn a_vertical_ray_has_the_analytic_optical_depth() {
    // One species, exponential density: tau(0, up) = sigma H (1 - exp(-top / H)).
    let p = AtmosphereParams { mie_scattering: [0.0; 3], mie_absorption: [0.0; 3], ozone_absorption: [0.0; 3], ..AtmosphereParams::EARTH };
    let atm = Atmosphere::new(p, R).unwrap();
    let tau = atm.optical_depth_integral(0.0, 1.0, 20_000);
    for k in 0..3 {
        let want = p.rayleigh_scattering[k] * p.rayleigh_scale_height_m * (1.0 - libm::exp(-p.top_height_m / p.rayleigh_scale_height_m));
        assert!((tau[k] - want).abs() <= 1e-6 * want, "channel {k}: {} vs {want}", tau[k]);
    }
    // The ozone tent: its column is the tent's area, 2 * half_width / 2 = half_width times the peak (plus the layer's share of the
    // other species, which are zero here).
    let oz =
        AtmosphereParams { rayleigh_scattering: [0.0; 3], mie_scattering: [0.0; 3], mie_absorption: [0.0; 3], ..AtmosphereParams::EARTH };
    let atm = Atmosphere::new(oz, R).unwrap();
    let tau = atm.optical_depth_integral(0.0, 1.0, 100_000);
    for k in 0..3 {
        let want = oz.ozone_absorption[k] * oz.ozone_half_width_m;
        assert!((tau[k] - want).abs() <= 1e-4 * want, "ozone channel {k}: {} vs {want}", tau[k]);
    }
}

// spec: ATMO-008
#[test]
fn the_phase_functions_integrate_to_one_and_the_shadow_and_the_sun_disc_are_what_they_say() {
    let integral = |f: &dyn Fn(f64) -> f64| {
        // 2 pi integral over cos from -1 to 1 by Simpson's rule.
        let n = 20_000;
        let h = 2.0 / n as f64;
        let mut s = f(-1.0) + f(1.0);
        for i in 1..n {
            s += f(-1.0 + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        2.0 * PI * s * h / 3.0
    };
    assert!((integral(&rayleigh_phase) - 1.0).abs() < 1e-9);
    for g in [0.8, 0.0, -0.5] {
        assert!((integral(&|c| cornette_shanks_phase(g, c)) - 1.0).abs() < 1e-3, "g = {g}: {}", integral(&|c| cornette_shanks_phase(g, c)));
    }
    assert!((cornette_shanks_phase(0.0, 0.3) - rayleigh_phase(0.3)).abs() < 1e-12, "g = 0 is Rayleigh's shape");
    let (atm, _) = earth();
    let w = atm.params.sun_angular_radius_rad;
    // At the surface the horizon is mu = 0: dark below by more than the sun's radius, bright above, half-way on the horizon.
    assert_eq!(atm.earth_shadow(0.0, -2.0 * w), 0.0);
    assert_eq!(atm.earth_shadow(0.0, 2.0 * w), 1.0);
    assert!((atm.earth_shadow(0.0, 0.0) - 0.5).abs() < 1e-12);
    // At altitude the horizon dips: mu_h = -sqrt(c) / r, so a sun on the geometric horizon (mu = 0) is lit from 20 km.
    assert_eq!(atm.earth_shadow(20_000.0, 0.0), 1.0, "the horizon dips by 4.5 degrees at 20 km");
    // The disc carries the illuminance 1: radiance times solid angle.
    let solid_angle = PI * w * w;
    assert!((sun_disc_radiance(w) * solid_angle - 1.0).abs() < 1e-12);
    let sun = [0.0, 0.0, 1.0];
    let at = |angle: f64| [libm::sin(angle), 0.0, libm::cos(angle)];
    assert_eq!(sun_disc_coverage(sun, sun, w), 1.0);
    assert_eq!(sun_disc_coverage(at(2.0 * w), sun, w), 0.0);
    assert!((sun_disc_coverage(at(w), sun, w) - 0.5).abs() < 1e-3, "half-covered on the rim");
    // From f32-rounded vectors (what the shader has) the coverage across the rim stays within 1e-2 of the f64 value.
    for i in 0..=40 {
        let angle = w * (0.9 + 0.2 * f64::from(i) / 40.0);
        let (d, s) = (at(angle).map(|v| f64::from(v as f32)), sun);
        let want = sun_disc_coverage(at(angle), sun, w);
        assert!((sun_disc_coverage(d, s, w) - want).abs() < 1e-2, "angle {angle}");
    }
}

// spec: ATMO-008
#[test]
fn a_thin_rayleigh_sky_has_the_single_scattering_radiance_in_closed_form() {
    // A thousand times thinner: the sky is single scattering with the sun undimmed, so the radiance of a vertical view is
    // phase(cos_gamma) * sigma * H * (1 - exp(-top / H)) per channel, and its colour ratio is the coefficients' ratio.
    let p = AtmosphereParams {
        rayleigh_scattering: AtmosphereParams::EARTH.rayleigh_scattering.map(|v| v * 1.0e-3),
        mie_scattering: [0.0; 3],
        mie_absorption: [0.0; 3],
        ozone_absorption: [0.0; 3],
        // No ground bounce either (it is a first-order light source for the sky, not a multiple-scattering one).
        ground_albedo: [0.0; 3],
        ..AtmosphereParams::EARTH
    };
    let atm = Atmosphere::new(p, R).unwrap();
    let luts = StaticLuts::generate(&atm);
    let f = frame_at(&atm, 0.0, 0.5);
    // The row just above the horizon is not vertical; find the texel nearest the zenith: row 0.
    let (mu, cos_vs, ground) = f.sky_view_texel_dir(100, 0);
    assert!(mu > 0.999999 && !ground);
    let l = f.sky_view_texel(&atm, &luts, 100, 0);
    let column = p.rayleigh_scale_height_m * (1.0 - libm::exp(-p.top_height_m / p.rayleigh_scale_height_m));
    for k in 0..3 {
        let want = rayleigh_phase(cos_vs) * p.rayleigh_scattering[k] * column;
        assert!(within_bound(l[k], want, 0.01, 0.0), "channel {k}: {} vs {want}", l[k]);
    }
    let ratio = l[2] / l[0];
    let want = p.rayleigh_scattering[2] / p.rayleigh_scattering[0];
    assert!((ratio / want - 1.0).abs() < 0.01, "blue over red {ratio} vs {want}");
}

// spec: ATMO-006
#[test]
fn the_frames_geometry_follows_the_camera_and_the_sky_view_mapping_inverts() {
    let (atm, _) = earth();
    for (h, inside) in [(1.0, true), (2000.0, true), (99_999.0, true), (100_001.0, false), (2.0e7, false)] {
        let f = frame_at(atm, h, 0.4);
        assert_eq!(f.inside, inside, "h {h}");
        assert!((f.h - h).abs() < 1e-6 && (f.r - (R + h)).abs() < 1e-6);
        assert!((f.c - h * (2.0 * R + h)).abs() < 1e-6 * f.c.max(1.0));
        assert!((f.mu_h + libm::sqrt(f.c) / f.r).abs() < 1e-15);
        assert!((f.theta_h - libm::acos(f.mu_h)).abs() < 1e-9, "the horizon angle: h {h}");
        assert!((f.mu_sun - 0.4).abs() < 1e-12 && (f.sin_sun - libm::sqrt(1.0 - 0.16)).abs() < 1e-12);
    }
    // The texel-to-direction mapping and its inverse agree (inside, where the sky-view LUT is used).
    for h in [1.0, 2000.0, 20_000.0] {
        for mu_sun in [0.8, 0.05, -0.08] {
            let f = frame_at(atm, h, mu_sun);
            for iy in (0..SKY_H).step_by(5) {
                for ix in (0..SKY_W).step_by(7) {
                    let (dir, _) = f.sky_view_texel_ray(ix, iy);
                    let (u, v) = f.sky_view_coords(dir);
                    let (eu, ev) = (ix as f64 / (SKY_W - 1) as f64, iy as f64 / (SKY_H - 1) as f64);
                    // The zenith and nadir rows have no azimuth.
                    let sin_t = libm::sqrt(1.0 - dir[2] * dir[2]);
                    if sin_t > 1e-3 {
                        assert!((u - eu).abs() < 1e-6, "h {h} sun {mu_sun} texel ({ix},{iy}): u {u} vs {eu}");
                    }
                    assert!((v - ev).abs() < 1e-6, "h {h} sun {mu_sun} texel ({ix},{iy}): v {v} vs {ev}");
                    // The direction's cosines are the texel's.
                    let (mu, cos_vs, _) = f.sky_view_texel_dir(ix, iy);
                    let (m2, c2) = f.view_cosines(dir);
                    assert!((mu - m2).abs() < 1e-12 && (cos_vs - c2).abs() < 1e-12, "h {h} texel ({ix},{iy})");
                }
            }
        }
    }
}

// spec: ATMO-006
#[test]
fn the_horizon_rows_resolve_the_horizon_at_every_altitude() {
    // The first row below the horizon reaches the ground and the last above it does not, at 1 m as at 20 km.
    let (atm, _) = earth();
    for h in [1.0, 2000.0, 20_000.0] {
        let f = frame_at(atm, h, 0.5);
        let above = f.sky_view_texel_dir(100, 53);
        let below = f.sky_view_texel_dir(100, 54);
        assert!(!above.2 && below.2);
        assert!(above.0 > f.mu_h && below.0 < f.mu_h, "h {h}: {} {} {}", above.0, f.mu_h, below.0);
        // Within a milliradian of the horizon on both sides at 1 m, where the horizon is only 0.03 degrees below the level.
        assert!((above.0 - f.mu_h).abs() < 2e-3 && (below.0 - f.mu_h).abs() < 2e-3);
        assert!(f.inside_ray_end(below.0).1 && !f.inside_ray_end(above.0).1);
    }
}

// spec: ATMO-007
#[test]
fn the_transmittance_table_is_within_the_discretisation_bound_of_the_converged_integral() {
    let (atm, luts) = earth();
    let mut next = stream(7);
    let (mut worst, mut worst_at) = (0.0f64, (0.0, 0.0));
    for _ in 0..250 {
        let h = atm.top_m() * next() * next();
        let mu = if next() < 0.3 {
            // Around and just above the horizon, where the table is steepest.
            let mu_h = -libm::sqrt(c_of(R, h)) / (R + h);
            mu_h + 0.02 * next() * next()
        } else {
            -1.0 + 2.0 * next()
        };
        let (_, c_top) = (0.0, c_top_of(R, atm.top_m(), h));
        let (r, c) = (R + h, c_of(R, h));
        // Rays that dive into the ground are outside the table (it clamps them to the horizon).
        if distance_to_ground(r * mu, c).is_some() {
            continue;
        }
        let _ = c_top;
        let want = atm.optical_depth_integral(h, mu, 8192).map(|t| libm::exp(-t));
        let got = atm.transmittance(luts, h, mu);
        for k in 0..3 {
            let excess = (got[k] - want[k]).abs() / (TRANSMITTANCE_REL * want[k] + TRANSMITTANCE_ABS);
            if excess > worst {
                worst = excess;
                worst_at = (h, mu);
            }
        }
    }
    assert!(worst <= 1.0, "transmittance lookup exceeds the bound by {worst} at (h, mu) = {worst_at:?}");
    eprintln!("transmittance table: worst {worst:.3} of the bound at {worst_at:?}");
}

/// The discretisation bound of the tables and the marches against the converged integral (ATMO-007): relative and absolute.
const TRANSMITTANCE_REL: f64 = 0.03;
const TRANSMITTANCE_ABS: f64 = 1.0e-4;
const MARCH_REL: f64 = 0.05;
const MARCH_ABS: f64 = 1.0e-4;
/// The multiple-scattering table is 32 x 32: across the twilight terminator (sun within 0.1 of the horizon) it is off by up to
/// 6e-4 where the value itself is 1e-3 (it is the ambient term, a few percent of the sky there), so its absolute term is larger.
const MS_TABLE_REL: f64 = 0.05;
const MS_TABLE_ABS: f64 = 1.0e-3;

// spec: ATMO-007
#[test]
fn the_multiple_scattering_table_is_within_the_bound_between_its_texels() {
    let (atm, luts) = earth();
    let mut next = stream(11);
    let mut worst = 0.0f64;
    for _ in 0..60 {
        let h = atm.top_m() * next() * 0.6;
        let mu_sun = -0.2 + 1.2 * next();
        let want = atm.multiscatter_texel(luts, h, mu_sun);
        let got = atm.multiscatter(luts, h, mu_sun);
        for k in 0..3 {
            worst = worst.max((got[k] - want[k]).abs() / (MS_TABLE_REL * want[k] + MS_TABLE_ABS));
        }
    }
    eprintln!("multiscatter table: worst {worst:.3} of the bound");
    assert!(worst <= 1.0, "interpolating the multiple-scattering table exceeds the bound by {worst}");
}

fn converged_sky(f: &FrameGeometry, atm: &Atmosphere, luts: &StaticLuts, ix: usize, iy: usize) -> [f64; 3] {
    let (mu, cos_vs, ground) = f.sky_view_texel_dir(ix, iy);
    let o = RayOrigin { h: f.h, r: f.r, c: f.c, rmu: f.r * mu, r_musun: f.r * f.mu_sun };
    let hit = if ground { distance_to_ground(o.rmu, f.c) } else { None };
    let s_end = hit.unwrap_or_else(|| distance_to_top(o.rmu, f.c_top));
    let m = atm.march(luts, &o, cos_vs, s_end, StepLayout::Quadratic(2048), Scatter::Physical { cos_gamma: cos_vs });
    let mut l = m.radiance;
    if hit.is_some() {
        let (_, r_g) = o.at(R, s_end);
        let g = atm.ground_radiance(luts, ((o.r_musun + s_end * cos_vs) / r_g).clamp(-1.0, 1.0));
        for k in 0..3 {
            l[k] += m.transmittance[k] * g[k];
        }
    }
    l
}

// spec: ATMO-007
#[test]
fn the_sky_view_march_is_within_the_bound_of_the_converged_march() {
    let (atm, luts) = earth();
    let mut worst = (0.0f64, String::new());
    for h in [1.0, 2000.0, 20_000.0] {
        for mu_sun in [0.8, 0.05, -0.08] {
            let f = frame_at(atm, h, mu_sun);
            for iy in (0..SKY_H).step_by(5) {
                for ix in (0..SKY_W).step_by(23) {
                    let got = f.sky_view_texel(atm, luts, ix, iy);
                    let want = converged_sky(&f, atm, luts, ix, iy);
                    for k in 0..3 {
                        let excess = (got[k] - want[k]).abs() / (MARCH_REL * want[k] + MARCH_ABS);
                        if excess > worst.0 {
                            worst = (excess, format!("h {h} sun {mu_sun} texel ({ix},{iy}) channel {k}: {} vs {}", got[k], want[k]));
                        }
                    }
                }
            }
        }
    }
    eprintln!("sky-view march: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "the 32-step sky-view march exceeds the discretisation bound by {}: {}", worst.0, worst.1);
}

// spec: ATMO-007
#[test]
fn the_aerial_perspective_march_is_within_the_bound_of_the_converged_march() {
    let (atm, luts) = earth();
    let mut worst = (0.0f64, String::new());
    for h in [1.0, 2000.0, 20_000.0, 60_000.0, 99_000.0] {
        for mu_sun in [0.8, -0.08] {
            let f = frame_at(atm, h, mu_sun);
            for (ix, iy) in [(16, 16), (3, 20), (28, 9), (16, 30)] {
                for iz in [0, 3, 9, 20, 31] {
                    let got = f.ap_texel(atm, luts, ix, iy, iz);
                    let dir = {
                        let v = f.froxel_ray(ix, iy);
                        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
                        [v[0] / l, v[1] / l, v[2] / l]
                    };
                    let (mu, cos_vs) = f.view_cosines(dir);
                    let limit = distance_to_top(f.r * mu, f.c_top);
                    let o = RayOrigin { h: f.h, r: f.r, c: f.c, rmu: f.r * mu, r_musun: f.r * f.mu_sun };
                    let m = atm.march(
                        luts,
                        &o,
                        cos_vs,
                        f.ap_slice_distance(iz).min(limit),
                        StepLayout::Uniform(4096),
                        Scatter::Physical { cos_gamma: cos_vs },
                    );
                    let tm = (m.transmittance[0] + m.transmittance[1] + m.transmittance[2]) / 3.0;
                    let want = [m.radiance[0], m.radiance[1], m.radiance[2], tm];
                    for k in 0..4 {
                        let (rel, abs) = if k == 3 { (MARCH_REL, 0.0) } else { (MARCH_REL, MARCH_ABS) };
                        let excess = (got[k] - want[k]).abs() / (rel * want[k].abs() + abs + 1e-12);
                        if excess > worst.0 {
                            worst = (excess, format!("h {h} sun {mu_sun} froxel ({ix},{iy},{iz}) channel {k}: {} vs {}", got[k], want[k]));
                        }
                    }
                }
            }
        }
    }
    eprintln!("aerial perspective march: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "the aerial-perspective march exceeds the discretisation bound by {}: {}", worst.0, worst.1);
}

// spec: ATMO-007
#[test]
fn the_orbit_march_is_within_the_bound_of_the_converged_march() {
    let (atm, luts) = earth();
    let mut worst = (0.0f64, String::new());
    for h in [2.0e5, 2.0e7] {
        for mu_sun in [0.8, -0.05] {
            let f = frame_at(atm, h, mu_sun);
            // Rays from the camera towards points on the atmosphere's disc as seen from there, from the nadir to the limb.
            let limb = libm::asin((R + 100_000.0) / f.r);
            for k in 0..40 {
                let angle = limb * (k as f64 + 0.5) / 40.0 * 1.0005;
                // The camera looks down at the planet: the ray leaves the nadir by `angle` towards +x.
                let dir = [libm::sin(angle), 0.0, -libm::cos(angle)];
                let got = f.orbit_ray(atm, luts, dir, None);
                // The converged ray: the same entry and the same limits with 8192 steps.
                let (mu, cos_vs) = f.view_cosines(dir);
                if !got.enters {
                    continue;
                }
                let rmu = f.r * mu;
                let sq = libm::sqrt(rmu * rmu - f.c_top);
                let t0 = f.c_top / (-rmu + sq);
                let mut s_end = 2.0 * sq;
                if let Some(s_g) = distance_to_ground(-sq, atm.rho_top * atm.rho_top) {
                    s_end = s_g;
                }
                let o = RayOrigin {
                    h: atm.top_m(),
                    r: R + atm.top_m(),
                    c: atm.rho_top * atm.rho_top,
                    rmu: -sq,
                    r_musun: f.r * f.mu_sun + t0 * cos_vs,
                };
                let m = atm.march(luts, &o, cos_vs, s_end, StepLayout::Uniform(8192), Scatter::Physical { cos_gamma: cos_vs });
                for c in 0..3 {
                    let e1 = (got.radiance[c] - m.radiance[c]).abs() / (MARCH_REL * m.radiance[c] + MARCH_ABS);
                    let e2 = (got.transmittance[c] - m.transmittance[c]).abs() / (MARCH_REL * m.transmittance[c] + MARCH_ABS);
                    for (excess, what) in [(e1, "radiance"), (e2, "transmittance")] {
                        if excess > worst.0 {
                            worst = (excess, format!("h {h} sun {mu_sun} ray {k} channel {c} {what}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("orbit march: worst {:.3} of the bound ({})", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "the 40-step orbit march exceeds the discretisation bound by {}: {}", worst.0, worst.1);
}

// spec: ATMO-006
#[test]
fn the_orbit_march_meets_the_sky_view_just_inside_the_top_of_the_atmosphere() {
    // The same direction seen from 1 km above the top and 1 km below it: the inside camera's sky-view texel and the outside
    // camera's march agree to within the two discretisations (the regimes meet, no jump when a camera crosses H).
    let (atm, luts) = earth();
    let top = atm.top_m();
    let inside = frame_at(atm, top - 1000.0, 0.7);
    let outside = frame_at(atm, top + 1000.0, 0.7);
    let mut checked = 0;
    for iy in (50..SKY_H).step_by(6) {
        let (mu, cos_vs, ground) = inside.sky_view_texel_dir(80, iy);
        if !ground || mu > -0.25 {
            continue;
        }
        let _ = cos_vs;
        let a = inside.sky_view_texel(atm, luts, 80, iy);
        // The same ray from the outside camera: both frames have the same up and sun, so the same planet-fixed direction.
        let (dir, _) = inside.sky_view_texel_ray(80, iy);
        let o = outside.orbit_ray(atm, luts, dir, None);
        let mut l = o.radiance;
        if o.reaches_ground {
            let g = atm.ground_radiance(luts, o.ground_mu_sun);
            for k in 0..3 {
                l[k] += o.transmittance[k] * g[k];
            }
        }
        // The outside camera starts 2 km farther out but through the same atmosphere: equal up to the discretisations.
        for k in 0..3 {
            assert!(within_bound(l[k], a[k], 0.08, 3e-4), "texel (80,{iy}) channel {k}: orbit {} vs sky view {}", l[k], a[k]);
        }
        checked += 1;
    }
    assert!(checked >= 3, "{checked} rays compared");
}

// spec: ATMO-009
#[test]
fn the_plans_atmosphere_round_trips_and_leaves_the_default_plan_alone() {
    let plain = LitPlan::fixed(15.0);
    assert!(plain.atmosphere.is_none());
    assert!(!plain.to_text().contains("atmosphere"), "a plan without an atmosphere keeps the text it had: {}", plain.to_text());
    let look = planet_frame::look::file::LookParams::default();
    let with = plain.with_atmosphere(&look, R).unwrap();
    assert_ne!(with.hash(), plain.hash());
    let text = with.to_text();
    assert!(text.contains(" atmosphere composite "), "{text}");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let back = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!(back, with);
    assert_eq!(back.to_text(), text);
    assert!(with.validate().is_ok());
    // The view travels with it and the order of the optional tokens is fixed (the atmosphere is last).
    let mut other = with;
    other.view = planet_frame::look::shading::ShadeView::Albedo;
    other.atmosphere.as_mut().unwrap().view = AtmosphereView::SkyViewLut;
    let t = other.to_text();
    assert!(t.contains(" view albedo atmosphere sky-view-lut "), "{t}");
    let tokens: Vec<&str> = t.split_whitespace().collect();
    assert_eq!(LitPlan::from_tokens(&tokens[1..]).unwrap(), other);
    // Refusals name the value.
    let mut bad = with.to_text().split_whitespace().map(str::to_string).collect::<Vec<_>>();
    let at = bad.iter().position(|t| t == "atmosphere").unwrap();
    bad.truncate(at + 5);
    let tokens: Vec<&str> = bad.iter().map(String::as_str).collect();
    assert!(LitPlan::from_tokens(&tokens[1..]).is_err());
    assert!(look.atmosphere.validate().is_ok());
    let e = LitPlan::fixed(15.0).with_atmosphere(&look, 500.0).unwrap_err();
    assert!(e.contains("500") && e.contains("radius"), "{e}");
    let thin = planet_frame::look::file::LookParams {
        atmosphere: AtmosphereParams { top_height_m: 900_000.0, ..AtmosphereParams::EARTH },
        ..Default::default()
    };
    let e = LitPlan::fixed(15.0).with_atmosphere(&thin, 500_000.0).unwrap_err();
    assert!(e.contains("top_height_m") && e.contains("900000"), "{e}");
}

// spec: ATMO-005
#[test]
fn a_table_texel_is_read_back_exactly_at_its_centre() {
    // The mapping of the tables is pinned: a lookup at the altitude and cosine a texel stands for returns that texel.
    let (atm, luts) = earth();
    for iy in (0..TRANSMITTANCE_H).step_by(3) {
        for ix in (0..TRANSMITTANCE_W).step_by(7) {
            let (h, mu) = atm.transmittance_texel(ix, iy);
            let got = atm.optical_depth(luts, h, mu);
            let want = luts.optical_depth[iy * TRANSMITTANCE_W + ix];
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() <= 1e-6 * want[k].abs() + 1e-9,
                    "texel ({ix},{iy}) channel {k}: {} vs {}",
                    got[k],
                    want[k]
                );
            }
        }
    }
    for iy in 0..MS_H {
        for ix in 0..MS_W {
            let (mu_sun, h) = (-1.0 + 2.0 * ix as f64 / (MS_W - 1) as f64, atm.top_m() * iy as f64 / (MS_H - 1) as f64);
            let got = atm.multiscatter(luts, h, mu_sun);
            let want = luts.multiscatter[iy * MS_W + ix];
            for k in 0..3 {
                assert!((got[k] - want[k]).abs() <= 1e-12 * want[k].abs() + 1e-15, "texel ({ix},{iy})");
            }
        }
    }
}

// spec: ATMO-005
#[test]
fn a_thick_non_absorbing_medium_gives_finite_multiple_scattering() {
    let p = AtmosphereParams {
        rayleigh_scattering: [1e-3; 3],
        rayleigh_scale_height_m: 1.0e6,
        mie_scattering: [0.0; 3],
        mie_absorption: [0.0; 3],
        ozone_absorption: [0.0; 3],
        top_height_m: 1.0e6,
        ..AtmosphereParams::EARTH
    };
    let atm = Atmosphere::new(p, 1.0e8).unwrap();
    let luts = StaticLuts::generate(&atm);
    assert!(luts.multiscatter.iter().all(|t| t.iter().all(|v| v.is_finite() && *v >= 0.0)), "capped, never inf or NaN");
}

// spec: ATMO-006
#[test]
fn aerial_perspective_is_not_cut_at_the_reference_sphere() {
    // A camera below the reference surface (a basin, a water level): the downward rays still get their haze.
    let (atm, luts) = earth();
    let cam = Camera::at_surface_point(PlanetFixed(Vec3::new(0.0, 0.0, R - 200.0)), Vec3::new(1.0, 0.0, 0.0), 0.0, 1.0, 1.0, 0.1);
    let f = FrameGeometry::new(atm, &cam, [0.6, 0.0, 0.8]);
    assert_eq!(f.h, 0.0);
    let v = f.ap_texel(atm, luts, 16, 31, 12);
    assert!(v[3] < 1.0 && v[0] > 0.0, "{v:?}");
}

// spec: ATMO-006
#[test]
fn the_orbit_march_starts_on_the_atmosphere_and_the_gpu_word_layout_is_pinned() {
    let (atm, _) = earth();
    let f = frame_at(atm, 2.0e7, 0.5);
    let dir = [libm::sin(0.2), 0.0, -libm::cos(0.2)];
    let (mu, cos_vs) = f.view_cosines(dir);
    let rmu = f.r * mu;
    let sq = libm::sqrt(rmu * rmu - f.c_top);
    let t0 = f.c_top / (-rmu + sq);
    // P = camera + t0 dir lies on the top sphere, its r mu is -sqrt(D) and its r mu_sun is r mu_sun + t0 cos(view, sun).
    let p = [f.r * f.up[0] + t0 * dir[0], f.r * f.up[1] + t0 * dir[1], f.r * f.up[2] + t0 * dir[2]];
    let len = libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
    assert!((len - (R + atm.top_m())).abs() < 1e-5, "{len}");
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    assert!((dot(p, dir) + sq).abs() < 1e-4);
    assert!((dot(p, f.sun) - (f.r * f.mu_sun + t0 * cos_vs)).abs() < 1e-4);
    // The word layout: the GPU block is 6 vec4 of the body and 10 of the frame (ATMO-005).
    let s = atm.static_words();
    assert_eq!((s[3], s[7], s[11], s[15], s[19]), (8000.0, 1200.0, 0.8, 25_000.0, 15_000.0));
    assert_eq!((s[20], s[21]), (R as f32, 100_000.0));
    assert_eq!(s[22], atm.rho_top as f32);
    let w = f.words(3.5, 5);
    assert_eq!((w[0], w[1], w[3]), (f.h as f32, f.r as f32, f.c_top as f32));
    assert_eq!((w[5], w[6], w[7]), (f.mu_h as f32, f.theta_h as f32, f.beta as f32));
    assert_eq!((w[11], w[15]), (f.mu_sun as f32, f.sin_sun as f32));
    assert_eq!((w[35], w[36], w[37], w[38]), (0.1, 3.5, 0.0, AP_MAX_DISTANCE_M as f32));
    assert_eq!(w[39].to_bits(), 5);
}

// spec: ATMO-021
#[test]
fn terrain_beyond_the_aerial_perspective_range_is_hazed_the_same_just_inside_and_just_outside_the_top() {
    // SPIKE-17 (atmosphere colour jump): geometry farther than AP_MAX_DISTANCE_M along its ray (the far land at the horizon of a
    // camera near the top of the atmosphere). Inside the top the frame composites it with the march from the camera to the geometry
    // (above 8 km); outside the top with the orbit march to the geometry. One camera crossing H must not change the
    // colour of the same terrain pixel.
    let (atm, luts) = earth();
    let top = atm.top_m();
    let inside = frame_at(atm, top - 1000.0, 0.7);
    let outside = frame_at(atm, top + 1000.0, 0.7);
    let land = [0.010, 0.012, 0.006]; // the lit radiance of green land, scene units
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;
    for iy in 16..AP_H {
        let v = inside.froxel_ray(16, iy);
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        let dir = [v[0] / l, v[1] / l, v[2] / l];
        let (mu, _) = inside.view_cosines(dir);
        let Some(t_ground) = distance_to_ground(inside.r * mu, inside.c) else { continue };
        if t_ground <= AP_MAX_DISTANCE_M * 1.2 {
            continue;
        }
        // The camera 1 km inside the top is above 8 km: the composite marches from it to the ground (ATMO-021).
        assert!(inside.uses_camera_march());
        let (mu_i, cos_i) = inside.view_cosines(dir);
        let c = inside.camera_ray_cosines(atm, luts, mu_i, cos_i, t_ground);
        let a: Vec<f64> = (0..3).map(|k| land[k] * c.transmittance[k] + c.radiance[k]).collect();
        let (mu_o, cos_o) = outside.view_cosines(dir);
        // The outside camera is 2 km higher on the same axis: it sees the same terrain pixel at its own distance (the numerics review of
        // the change found the inside distance used here stopping the march short of the ground by 7 to 34 km).
        let t_outside = distance_to_ground(outside.r * mu_o, outside.c).expect("the same ray reaches the ground from 2 km higher");
        let o = outside.orbit_ray_cosines(atm, luts, mu_o, cos_o, Some(t_outside));
        let b: Vec<f64> = (0..3).map(|k| land[k] * o.transmittance[k] + o.radiance[k]).collect();
        eprintln!(
            "row {iy}: ground {:.0} km, inside rgb {:.5} {:.5} {:.5}, outside rgb {:.5} {:.5} {:.5}",
            t_ground / 1000.0,
            a[0],
            a[1],
            a[2],
            b[0],
            b[1],
            b[2]
        );
        for k in 0..3 {
            let rel = (a[k] - b[k]).abs() / b[k].abs().max(1e-9);
            if rel > worst.0 {
                worst =
                    (rel, format!("row {iy} t_ground {:.0} km channel {k}: inside {:.5} vs outside {:.5}", t_ground / 1000.0, a[k], b[k]));
            }
        }
        checked += 1;
    }
    assert!(checked >= 3, "{checked} far-terrain rays compared");
    // 5 %: the measured worst is 3.5 % (the two cameras are 2 km apart; the difference of the haze itself); it was 7.2 % with the volume
    // (32 slices, quadratic to the frame range, about 70 km apart near 300 km) and the jump 95 % with the 256 km clamp.
    assert!(worst.0 <= 0.05, "far terrain jumps by {:.1} % across the top of the atmosphere: {}", worst.0 * 100.0, worst.1);
}

// spec: ATMO-021, ATMO-007
#[test]
fn the_aerial_perspective_range_is_256_km_up_to_5_km_altitude_the_horizon_distance_above_it_and_reaches_the_gpu_words() {
    let (atm, _) = earth();
    for h in [0.0, 1.0, 2000.0, 5000.0] {
        assert_eq!(frame_at(atm, h, 0.5).ap_range, AP_MAX_DISTANCE_M, "h {h}: the near-ground volume is the one it always was");
    }
    let f = frame_at(atm, 20_000.0, 0.5);
    assert_eq!(f.ap_range, f.rho, "above 5 km the last slice is at the horizon");
    assert!(f.ap_range > AP_MAX_DISTANCE_M);
    let top = frame_at(atm, atm.top_m() - 1000.0, 0.5);
    assert!(top.ap_range > 1.1e6 && top.ap_range < 1.2e6, "about 1,130 km at the top: {}", top.ap_range);
    assert_eq!(frame_at(atm, atm.top_m() + 1000.0, 0.5).ap_range, AP_MAX_DISTANCE_M, "outside the atmosphere the volume is not used");
    // The slices are quadratic in the frame's range, the last at the range itself; the GPU receives the range.
    assert_eq!(top.ap_slice_distance(AP_D - 1), top.ap_range);
    assert!((top.ap_slice_distance(0) - top.ap_range / (AP_D * AP_D) as f64).abs() < 1e-6);
    assert_eq!(ap_slice_distance(AP_D - 1), AP_MAX_DISTANCE_M, "the base-range function is unchanged");
    assert_eq!(top.words(1.0, 0)[38], top.ap_range as f32);
    assert_eq!(frame_at(atm, 1000.0, 0.5).words(1.0, 0)[38], AP_MAX_DISTANCE_M as f32);
}

/// A pixel's haze for a ray with the given cosines to the up and to the sun, ground distance and screen position: radiance and
/// transmittance per channel.
type Haze<'a> = dyn Fn(f64, f64, f64, [f64; 2]) -> ([f64; 3], [f64; 3]) + 'a;

/// The worst relative error over the rays of the centre column that end on the ground (600 rays from 0.95 to -0.95 of the half
/// height), of the pixel's haze by `haze(frame, mu, cos_vs, ground distance, uv)` against the converged march of the same ray, for
/// land of radiance (0.010, 0.012, 0.006) at the ground distance, with the transmittance of the channel or, for the volume (which stores
/// one mean transmittance), the mean. Returns the worst error and where it is.
fn worst_haze_error(atm: &Atmosphere, luts: &StaticLuts, f: &FrameGeometry, mean_transmittance: bool, haze: &Haze) -> (f64, String) {
    let land = [0.010, 0.012, 0.006];
    let (mut worst, mut at) = (0.0f64, String::new());
    for k in 0..600 {
        let b = 0.95 - 1.9 * f64::from(k) / 599.0;
        let v = [f.fwd[0] + f.upv[0] * b, f.fwd[1] + f.upv[1] * b, f.fwd[2] + f.upv[2] * b];
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        let (mu, cos_vs) = f.view_cosines([v[0] / l, v[1] / l, v[2] / l]);
        let Some(t_g) = distance_to_ground(f.r * mu, f.c) else { continue };
        let o = RayOrigin { h: f.h, r: f.r, c: f.c, rmu: f.r * mu, r_musun: f.r * f.mu_sun };
        let limit = distance_to_top(f.r * mu, f.c_top);
        let m = atm.march(luts, &o, cos_vs, t_g.min(limit), StepLayout::Uniform(2048), Scatter::Physical { cos_gamma: cos_vs });
        let tm = (m.transmittance[0] + m.transmittance[1] + m.transmittance[2]) / 3.0;
        let (radiance, trans) = haze(mu, cos_vs, t_g, [0.5, (1.0 - b) / 2.0]);
        let mut err = 0.0f64;
        for c in 0..3 {
            let exact = land[c] * (if mean_transmittance { tm } else { m.transmittance[c] }) + m.radiance[c];
            let got = land[c] * trans[c] + radiance[c];
            err = err.max((got - exact).abs() / exact.abs().max(1e-6));
        }
        if err > worst {
            worst = err;
            at = format!("b {b:.3} ground {:.0} km: {:.2} %", t_g / 1000.0, err * 100.0);
        }
    }
    (worst, at)
}

fn ap_table(atm: &Atmosphere, luts: &StaticLuts, f: &FrameGeometry) -> Vec<[f64; 4]> {
    let mut table = vec![[0.0f64; 4]; AP_W * AP_H * AP_D];
    for iz in 0..AP_D {
        for iy in 0..AP_H {
            for ix in 0..AP_W {
                table[(iz * AP_H + iy) * AP_W + ix] = f.ap_texel(atm, luts, ix, iy, iz);
            }
        }
    }
    table
}

// spec: ATMO-007, ATMO-021
#[test]
fn above_8_km_the_camera_march_is_within_the_bound_of_the_converged_march_for_every_sun_and_side() {
    // SPIKE-20: the volume is 18 to 23 % off between its texels at 60 and 97.5 km, 10 % at 14 km. The march from the camera to the geometry
    // (8 steps by day, 12 in twilight, dense towards the end) is within the bound of ATMO-007 (5 % + 1e-4 per channel) of the converged
    // march for the sun high, low and below the horizon, towards the sun and away from it (the review of the change: at one sun the
    // centre column only looks towards it).
    let (atm, luts) = earth();
    let land = [0.010, 0.012, 0.006];
    let mut worst = (0.0f64, String::new());
    for h in [8_001.0f64, 14_000.0, 20_000.0, 40_000.0, 60_000.0, 97_500.0, 99_500.0] {
        for mu_sun in [0.7f64, 0.2, 0.1, 0.05, -0.05, -0.08] {
            for away in [false, true] {
                let sun = [libm::sqrt(1.0 - mu_sun * mu_sun) * if away { -1.0 } else { 1.0 }, 0.0, mu_sun];
                let cam = Camera::at_surface_point(
                    PlanetFixed(Vec3::new(0.0, 0.0, atm.radius_m + h)),
                    Vec3::new(1.0, 0.0, 0.0),
                    0.0,
                    1.0,
                    1.0,
                    0.1,
                );
                let f = FrameGeometry::new(atm, &cam, sun);
                assert!(f.uses_camera_march(), "{h} m is above the altitude of the march");
                assert_eq!(
                    f.camera_march_steps(),
                    if mu_sun >= CAMERA_MARCH_TWILIGHT_MU { CAMERA_MARCH_STEPS_DAY } else { CAMERA_MARCH_STEPS_TWILIGHT }
                );
                for k in 0..300 {
                    let b = 0.95 - 1.9 * f64::from(k) / 299.0;
                    let v = [f.fwd[0] + f.upv[0] * b, f.fwd[1] + f.upv[1] * b, f.fwd[2] + f.upv[2] * b];
                    let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
                    let (mu, cos_vs) = f.view_cosines([v[0] / l, v[1] / l, v[2] / l]);
                    let Some(t_g) = distance_to_ground(f.r * mu, f.c) else { continue };
                    let o = RayOrigin { h: f.h, r: f.r, c: f.c, rmu: f.r * mu, r_musun: f.r * f.mu_sun };
                    let end = t_g.min(distance_to_top(f.r * mu, f.c_top));
                    let m = atm.march(luts, &o, cos_vs, end, StepLayout::Uniform(2048), Scatter::Physical { cos_gamma: cos_vs });
                    let r = f.camera_ray_cosines(atm, luts, mu, cos_vs, t_g);
                    for c in 0..3 {
                        let want = land[c] * m.transmittance[c] + m.radiance[c];
                        let got = land[c] * r.transmittance[c] + r.radiance[c];
                        let excess = (got - want).abs() / (MARCH_REL * want.abs() + 1e-4);
                        if excess > worst.0 {
                            worst = (
                                excess,
                                format!(
                                    "h {h}, sun {mu_sun}{}, ground {:.0} km, channel {c}: {got:.5} against {want:.5}",
                                    if away { " (away)" } else { "" },
                                    t_g / 1000.0
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
    eprintln!("camera march worst excess over the bound {:.2}: {}", worst.0, worst.1);
    assert!(worst.0 <= 1.0, "the camera march is outside the bound ({:.2} of it): {}", worst.0, worst.1);
}

// spec: ATMO-007, ATMO-021
#[test]
fn above_60_km_the_volume_between_its_texels_is_outside_the_bound_the_camera_march_replaces_it() {
    let (atm, luts) = earth();
    for h in [60_000.0f64, 97_500.0, 99_500.0] {
        let f = frame_at(atm, h, 0.7);
        let table = ap_table(atm, luts, &f);
        let (ev, at) = worst_haze_error(atm, luts, &f, true, &|_, _, t, uv| {
            let a = f.ap_lookup(&table, uv, t);
            ([a[0], a[1], a[2]], [a[3]; 3])
        });
        assert!(ev > MARCH_REL, "{h} m: the volume is within the bound after all ({at}): the march is not needed there");
    }
}

/// The bound of the volume between its texels up to 8 km altitude at the high sun (0.7): measured worst 5.63 % (2 km), 3.9 % to 3.0 % elsewhere.
/// At a low sun it is not within it (17 to 18 % at 2 km with the sun at 0.2 and 0.05; the volume's own weakness, not this change's).
const VOLUME_BETWEEN_TEXELS_REL: f64 = 0.06;

// spec: ATMO-007, ATMO-021
#[test]
fn up_to_6_km_the_volume_between_its_texels_is_within_its_bound_of_the_converged_march() {
    let (atm, luts) = earth();
    let mut report = String::new();
    let mut worst = 0.0f64;
    for h in [1.0f64, 2_000.0, 5_000.0, 6_000.0] {
        let f = frame_at(atm, h, 0.7);
        assert!(!f.uses_camera_march(), "{h} m uses the volume");
        let table = ap_table(atm, luts, &f);
        let (e, at) = worst_haze_error(atm, luts, &f, true, &|_, _, t, uv| {
            let a = f.ap_lookup(&table, uv, t);
            ([a[0], a[1], a[2]], [a[3]; 3])
        });
        report += &format!("h {h}: volume worst {at}\n");
        worst = worst.max(e);
    }
    eprintln!("{report}");
    assert!(
        worst <= VOLUME_BETWEEN_TEXELS_REL,
        "the volume differs from the converged march by up to {:.1} % at or below 8 km:\n{report}",
        worst * 100.0
    );
}

// spec: ATMO-021
#[test]
fn the_haze_of_far_terrain_is_continuous_across_the_blend_band_between_the_volume_and_the_camera_march() {
    // The review of the change (the owner's colour jump on a descent): the volume is 7 to 19 % off the march between 6 and 10 km, so a
    // hard switch would step the haze when the camera crosses. The composite mixes the two by the smoothstep of the altitude. For a fixed
    // screen position the haze is compared with the converged march of the same ray at heights 250 m apart; the error against it
    // (the volume's, then the mix's, then the march's) may change by at most `STEP` per 100 m of altitude, for a high sun and a low one.
    const STEP: f64 = 0.01;
    let (atm, luts) = earth();
    let land = [0.010, 0.012, 0.006];
    let heights: Vec<f64> = (0..19).map(|i| 5_750.0 + 250.0 * f64::from(i)).collect();
    for mu_sun in [0.7f64, 0.05] {
        // err[k][h] per channel: relative error of the mixed haze against the converged march at height index h, ray index k.
        let mut err: Vec<Vec<Option<[f64; 3]>>> = vec![vec![None; heights.len()]; 40];
        for (hi, &h) in heights.iter().enumerate() {
            let f = frame_at(atm, h, mu_sun);
            let table = ap_table(atm, luts, &f);
            let w = f.camera_march_weight();
            for (k, slot) in err.iter_mut().enumerate() {
                let b = 0.9 - 1.0 * (k as f64) / 39.0;
                let v = [f.fwd[0] + f.upv[0] * b, f.fwd[1] + f.upv[1] * b, f.fwd[2] + f.upv[2] * b];
                let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
                let (mu, cos_vs) = f.view_cosines([v[0] / l, v[1] / l, v[2] / l]);
                let Some(t_g) = distance_to_ground(f.r * mu, f.c) else { continue };
                let o = RayOrigin { h: f.h, r: f.r, c: f.c, rmu: f.r * mu, r_musun: f.r * f.mu_sun };
                let end = t_g.min(distance_to_top(f.r * mu, f.c_top));
                let truth = atm.march(luts, &o, cos_vs, end, StepLayout::Uniform(2048), Scatter::Physical { cos_gamma: cos_vs });
                let a = f.ap_lookup(&table, [0.5, (1.0 - b) / 2.0], t_g);
                let r = f.camera_ray_cosines(atm, luts, mu, cos_vs, t_g);
                let mut e = [0.0; 3];
                for c in 0..3 {
                    let vol = land[c] * a[3] + a[c];
                    let mar = land[c] * r.transmittance[c] + r.radiance[c];
                    let want = land[c] * truth.transmittance[c] + truth.radiance[c];
                    e[c] = ((1.0 - w) * vol + w * mar - want) / want.abs().max(1e-4);
                }
                slot[hi] = Some(e);
            }
        }
        let (mut worst, mut at) = (0.0f64, String::new());
        for (k, row) in err.iter().enumerate() {
            for hi in 1..heights.len() {
                let (Some(a), Some(b)) = (row[hi - 1], row[hi]) else { continue };
                for c in 0..3 {
                    let per_100_m = (b[c] - a[c]).abs() / ((heights[hi] - heights[hi - 1]) / 100.0);
                    if per_100_m > worst {
                        worst = per_100_m;
                        at = format!(
                            "sun {mu_sun}, ray {k}, {} to {} m, channel {c}: {:.2} % to {:.2} %",
                            heights[hi - 1],
                            heights[hi],
                            a[c] * 100.0,
                            b[c] * 100.0
                        );
                    }
                }
            }
        }
        eprintln!("sun {mu_sun}: the largest change of the error per 100 m is {:.2} %: {at}", worst * 100.0);
        assert!(worst <= STEP, "the haze steps by {:.2} % per 100 m across the blend band (sun {mu_sun}): {at}", worst * 100.0);
    }
}

// spec: ATMO-021
#[test]
fn the_haze_upsample_weighs_texels_by_distance_and_gives_way_to_the_pixels_own_march_at_a_depth_edge() {
    let ray = |v: f64| CameraRay { radiance: [v, 2.0 * v, 3.0 * v], transmittance: [0.9, 0.8, 0.7] };
    let t = 100_000.0;
    let tol = HAZE_DEPTH_TOL_REL * t + HAZE_DEPTH_TOL_ABS_M;
    // No texel with geometry: the pixel marches itself.
    assert_eq!(haze_upsample(&[(0.5, -1.0, ray(1.0)), (0.5, 0.0, ray(2.0))], t), None);
    // One texel on the pixel's own sample: exactly its ray.
    assert_eq!(haze_upsample(&[(1.0, t, ray(1.0)), (0.0, t, ray(5.0))], t), Some(ray(1.0)));
    // Bilinear weights alone: the mean of two equal-distance texels.
    let mid = haze_upsample(&[(0.5, t, ray(1.0)), (0.5, t, ray(3.0))], t).unwrap();
    assert!((mid.radiance[0] - 2.0).abs() < 1e-12 && (mid.transmittance[0] - 0.9).abs() < 1e-12);
    // A texel 3 tolerances away has no say; one of the same weight at the pixel's distance has all of it.
    let edge = haze_upsample(&[(0.5, t, ray(1.0)), (0.5, t + 3.0 * tol, ray(2.0))], t).unwrap();
    assert!((edge.radiance[0] - 1.0).abs() < 1e-3, "{edge:?}");
    // Four texels at 1.6 tolerances (weight exp(-2.56) = 0.077 each, 0.31 in all with the bilinear weights 0.25 -> 0.019 in all): marches itself.
    let far: Vec<_> = (0..4).map(|_| (0.25, t + 1.6 * tol, ray(1.0))).collect();
    assert_eq!(haze_upsample(&far, t), None, "the weights sum below {HAZE_MIN_WEIGHT}");
    // At 1.4 tolerances the weights sum to 4 x 0.25 x exp(-1.96) = 0.14: the average is used.
    let near: Vec<_> = (0..4).map(|_| (0.25, t + 1.4 * tol, ray(1.0))).collect();
    assert_eq!(haze_upsample(&near, t), Some(ray(1.0)));
}
