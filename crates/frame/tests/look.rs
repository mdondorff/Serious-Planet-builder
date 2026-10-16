//! Tier A: the lit path's presentation maths in its GPU-free module (ADR 0038, ADR 0042 point 7): exposure units, the luminance
//! histogram and its percentile band, the adaptation step, the ACES fit and the sRGB encode, the pre-exposure factor, and the lit
//! plan's text form. Known answers are worked out by hand in the comments.

use planet_frame::look::*;
use planet_frame::{DebugView, TerrainView};

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a}, want {b} (tolerance {tol})");
}

// spec: REND-021
#[test]
fn exposure_units_have_known_answers() {
    // exposure = 1 / (1.2 * 2^EV): EV 0 is 1/1.2, and one stop halves it.
    close(exposure_scale(0.0), 1.0 / 1.2, 1e-15, "EV 0");
    close(exposure_scale(1.0), 0.5 / 1.2, 1e-15, "EV 1");
    close(exposure_scale(-2.0), 4.0 / 1.2, 1e-15, "EV -2");
    // EV = log2(8 L): L = 1/8 cd/m2 is EV 0, L = 1 is EV 3, L = 2^17 is EV 20 (the top of the histogram).
    close(luminance_ev(0.125), 0.0, 1e-15, "L = 1/8");
    close(luminance_ev(1.0), 3.0, 1e-15, "L = 1");
    close(luminance_ev(131_072.0), 20.0, 1e-12, "L = 2^17");
    // One scene unit is the sun's illuminance in lux; the pre-exposure factor folds the units and the exposure.
    close(pre_exposure_factor(0.0), SOLAR_ILLUMINANCE_LUX / 1.2, 1e-9, "factor at EV 0");
    close(pre_exposure_factor(15.0), 126_600.0 / (1.2 * 32_768.0), 1e-9, "factor at EV 15");
    // A sunlit white diffuse surface (1/pi units) exposed for a sunny day (EV 15) lands near the top of the display range:
    // 126600 / pi / (1.2 * 2^15) = 1.025.
    close(pre_exposure_factor(15.0) / core::f64::consts::PI, 1.0247, 1e-3, "sunlit white at EV 15");
}

// spec: REND-021
#[test]
fn bins_cover_ev_minus_10_to_20_in_64_equal_steps() {
    close(bin_width(), 30.0 / 64.0, 1e-15, "bin width");
    assert_eq!(bin_of_ev(-10.0), Some(0));
    assert_eq!(bin_of_ev(20.0 - 1e-9), Some(63));
    // Out of range lands in the end bins, not outside the table.
    assert_eq!(bin_of_ev(-30.0), Some(0));
    assert_eq!(bin_of_ev(45.0), Some(63));
    // EV 0 is (0 + 10) / (30/64) = 21.33 -> bin 21; its centre is -10 + 21.5 * 0.46875.
    assert_eq!(bin_of_ev(0.0), Some(21));
    close(bin_center_ev(21), -10.0 + 21.5 * 0.468_75, 1e-12, "centre of bin 21");
    // No light is not counted at all.
    assert_eq!(bin_of_ev(f64::NEG_INFINITY), None);
    assert_eq!(bin_of_ev(f64::INFINITY), Some(63), "too bright is the last bin");
    assert_eq!(bin_of_ev(f64::NAN), None);
    // The histogram of pixels: black pixels are skipped; pixels at the centre of bins land in them.
    let at = |ev: f64| {
        // A pixel whose scene EV is `ev` when written at pre-exposure `pre`: y = exposure_scale(pre) * 2^ev / 8.
        let pre = 12.0;
        let y = exposure_scale(pre) * libm::exp2(ev) / 8.0;
        ([y, y, y], pre)
    };
    let pre = at(0.0).1;
    let pixels = [at(bin_center_ev(10)).0, at(bin_center_ev(10)).0, at(bin_center_ev(40)).0, [0.0; 3]];
    let h = histogram_of(&pixels, pre);
    assert_eq!((h[10], h[40]), (2, 1));
    assert_eq!(h.iter().map(|&c| u64::from(c)).sum::<u64>(), 3, "the black pixel is not counted");
}

// spec: REND-021
#[test]
fn the_percentile_band_mean_has_known_answers() {
    let mut h = [0u32; HIST_BINS];
    assert_eq!(band_mean_ev(&h), None, "an empty histogram has no mean");
    // All pixels in one bin: the mean is that bin's centre, whatever the band.
    h[30] = 1000;
    close(band_mean_ev(&h).unwrap(), bin_center_ev(30), 1e-12, "one bin");
    // 100 pixels in bin 10, 100 in bin 50: the band [40 %, 90 %] of 200 is the counts 80..180: 20 pixels of bin 10, 80 of bin 50.
    let mut h = [0u32; HIST_BINS];
    h[10] = 100;
    h[50] = 100;
    let want = (20.0 * bin_center_ev(10) + 80.0 * bin_center_ev(50)) / 100.0;
    close(band_mean_ev(&h).unwrap(), want, 1e-12, "two bins");
    // Dark and bright outliers are outside the band: 40 % dark (bin 2), 50 % mid (bin 30), 10 % very bright (bin 60) -> only bin 30.
    let mut h = [0u32; HIST_BINS];
    h[2] = 40;
    h[30] = 50;
    h[60] = 10;
    close(band_mean_ev(&h).unwrap(), bin_center_ev(30), 1e-12, "outliers excluded");
}

// spec: REND-021
#[test]
fn the_target_is_clamped_to_the_range_and_held_without_light() {
    let params = ExposureParams::default();
    let mut h = [0u32; HIST_BINS];
    h[63] = 10; // EV near 20: above the range maximum of 16
    close(target_ev(&h, &params, 5.0), 16.0, 1e-12, "clamped above");
    h = [0; HIST_BINS];
    h[0] = 10; // EV near -10: below the minimum of -2
    close(target_ev(&h, &params, 5.0), -2.0, 1e-12, "clamped below");
    close(target_ev(&[0; HIST_BINS], &params, 7.25), 7.25, 0.0, "no counted pixel: hold the current exposure");
}

// spec: REND-021
#[test]
fn adaptation_is_linear_in_ev_with_separate_speeds_and_depends_on_dt_only() {
    let p = ExposureParams::default(); // 3 stops/s up (brighter scene), 1 stop/s down
    close(adapt_ev(10.0, 16.0, 0.5, &p, false), 11.5, 1e-12, "up at 3 EV/s for half a second");
    close(adapt_ev(10.0, 2.0, 0.5, &p, false), 9.5, 1e-12, "down at 1 EV/s for half a second");
    close(adapt_ev(10.0, 10.2, 0.5, &p, false), 10.2, 1e-12, "close targets are reached, never overshot");
    close(adapt_ev(10.0, 9.9, 5.0, &p, false), 9.9, 1e-12, "close target below");
    close(adapt_ev(10.0, 16.0, 0.0, &p, false), 10.0, 0.0, "no time, no change");
    close(adapt_ev(10.0, 16.0, -1.0, &p, false), 10.0, 0.0, "a negative step is no step");
    close(adapt_ev(10.0, 16.0, 0.0, &p, true), 16.0, 0.0, "reset snaps");
    // The step does not depend on how dt is split into frames until the target is reached: two half steps equal one whole step.
    let two = adapt_ev(adapt_ev(0.0, 100.0, 0.25, &p, false), 100.0, 0.25, &p, false);
    close(two, adapt_ev(0.0, 100.0, 0.5, &p, false), 1e-12, "dt split");
}

// spec: REND-021
#[test]
fn the_aces_fit_and_the_srgb_encode_have_known_answers() {
    // Narkowicz: f(x) = x (2.51 x + 0.03) / (x (2.43 x + 0.59) + 0.14).
    close(aces(0.0), 0.0, 0.0, "aces(0)");
    close(aces(1.0), 2.54 / 3.16, 1e-15, "aces(1)");
    close(aces(0.18), 0.18 * (2.51 * 0.18 + 0.03) / (0.18 * (2.43 * 0.18 + 0.59) + 0.14), 1e-15, "aces(0.18)");
    close(aces(1.0e6), 1.0, 0.0, "clamped at 1");
    close(aces(-3.0), 0.0, 0.0, "negative light is black");
    assert!(aces(0.5) > aces(0.25) && aces(0.25) > aces(0.1), "monotone");
    // sRGB: linear below 0.0031308, 1.055 l^(1/2.4) - 0.055 above; 0.5 -> 0.735357...
    close(srgb_encode(0.0), 0.0, 0.0, "srgb(0)");
    close(srgb_encode(1.0), 1.0, 1e-15, "srgb(1)");
    close(srgb_encode(0.002), 0.002 * 12.92, 1e-15, "toe");
    close(srgb_encode(0.5), 0.735_356_983, 1e-8, "srgb(0.5)");
    // The two curves agree at their joint (continuity within 1e-4).
    close(srgb_encode(0.003_130_8), srgb_encode(0.003_130_9), 1e-4, "joint");
    // The 8-bit pixel: the exposure shifts by 2^(pre - ev).
    let px = tonemap_pixel([0.5, 0.5, 0.5], 10.0, 11.0); // exposed value 0.25
    let want = (srgb_encode(aces(0.25)) * 255.0 + 0.5).floor() as u8;
    assert_eq!(px, [want; 3]);
    assert_eq!(tonemap_pixel([0.0; 3], 10.0, 10.0), [0, 0, 0]);
    assert_eq!(tonemap_pixel([1.0e6; 3], 10.0, 10.0), [255, 255, 255]);
}

// spec: REND-021
#[test]
fn the_false_colour_bands_are_whole_stops_below_saturation() {
    assert_eq!(ev_band(1.0), Some(0));
    assert_eq!(ev_band(8.0), Some(0), "clipped light is in the top band");
    assert_eq!(ev_band(0.5), Some(1));
    assert_eq!(ev_band(0.26), Some(1));
    assert_eq!(ev_band(0.25), Some(2));
    assert_eq!(ev_band(1.0e-9), Some(EV_BAND_COUNT - 1), "the darkest band collects everything below");
    assert_eq!(ev_band(0.0), None);
    assert_eq!(ev_band(f64::NAN), None);
}

// spec: REND-025
#[test]
fn written_values_are_pre_exposed_and_clamped() {
    // Written value: pre-exposed and clamped to 60,000, never negative.
    let w = written_value([1.0e9, 0.0, -1.0], 0.0);
    assert_eq!(w, [60_000.0, 0.0, 0.0]);
    close(written_value([0.1, 0.2, 0.3], 12.0)[1], 0.2 * pre_exposure_factor(12.0), 1e-12, "pre-exposed");
}

// spec: REND-024
#[test]
fn the_lit_plan_round_trips_through_its_text_form() {
    let mut fixed = LitPlan::fixed(14.5);
    fixed.sun_dir = [0.1, 0.2, 0.3];
    fixed.tonemap = ToneMapMode::EvBands;
    let mut adaptive = LitPlan::adaptive(11.0, 1.0 / 60.0);
    adaptive.reset_exposure = true;
    if let ExposureMode::Adaptive { params, .. } = &mut adaptive.exposure {
        params.speed_up = 2.5;
    }
    for plan in [fixed, adaptive] {
        let text = plan.to_text();
        let tokens: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(tokens[0], "lit");
        assert_eq!(LitPlan::from_tokens(&tokens[1..]).unwrap(), plan, "{text}");
    }
    assert!(LitPlan::from_tokens(&["fixed"]).is_err());
    assert!(LitPlan::from_tokens(&["warp", "0"]).is_err());
    assert_eq!(fixed.exposure.pre_ev100(), 14.5);
    assert_eq!(adaptive.exposure.pre_ev100(), 11.0);
    assert!(adaptive.exposure.is_adaptive() && !fixed.exposure.is_adaptive());
    assert_ne!(fixed.hash(), adaptive.hash());
    let sun = LitPlan::fixed(10.0).sun_dir;
    close(f64::from(sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]), 1.0, 1e-6, "the default sun is a unit vector");
}

// spec: REND-024
#[test]
fn a_debug_path_plan_keeps_its_text_and_a_lit_plan_adds_one_line() {
    let _ = (DebugView::Face, TerrainView::Relief);
    let camera = planet_frame::Camera::looking_at_centre(
        planet_core::PlanetFixed(planet_core::Vec3::new(0.0, 0.0, 7.0e6)),
        planet_core::Vec3::new(0.0, 1.0, 0.0),
        1.0,
        1.0,
        1.0,
    );
    let plan = planet_frame::plan_terrain(&planet_frame::TerrainRequest {
        camera,
        size: (32, 32),
        view: TerrainView::Relief,
        lod: planet_frame::LodParams {
            cells: 16,
            viewport_h_px: 96.0,
            tau_px: 2.5,
            max_level: planet_frame::production_finest_level(16),
            ..planet_frame::LodParams::earth_1080p()
        },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    })
    .unwrap();
    assert!(plan.lit.is_none(), "a plan is a debug-path plan unless it is made lit");
    let base = plan.to_text();
    assert!(!base.contains("\nlit "), "the debug path's text carries no lit line");
    let mut lit = plan.clone();
    lit.lit = Some(LitPlan::fixed(15.0));
    let text = lit.to_text();
    assert_eq!(text.lines().count(), base.lines().count() + 1);
    assert_eq!(planet_frame::TerrainPlan::from_text(&text).unwrap(), lit);
    assert_eq!(planet_frame::TerrainPlan::from_text(&base).unwrap(), plan);
    assert_ne!(lit.plan_hash(), plan.plan_hash());
}

// spec: REND-024
#[test]
fn a_lit_plan_with_unusable_values_is_refused_naming_the_value() {
    assert!(LitPlan::fixed(10.0).validate().is_ok());
    assert!(LitPlan::adaptive(10.0, 0.016).validate().is_ok());
    let bad = |f: &dyn Fn(&mut LitPlan)| {
        let mut p = LitPlan::adaptive(10.0, 0.016);
        f(&mut p);
        p.validate().unwrap_err()
    };
    assert!(bad(&|p| p.dt_s = f32::NAN).contains("dt"));
    assert!(bad(&|p| p.dt_s = -1.0).contains("negative"));
    assert!(bad(&|p| p.sun_dir = [0.0, 0.0, 0.0]).contains("sun"));
    assert!(bad(&|p| p.sun_dir = [2.0, 0.0, 0.0]).contains("length 2"));
    assert!(bad(&|p| if let ExposureMode::Adaptive { params, .. } = &mut p.exposure {
        params.min_ev100 = 20.0
    })
    .contains("reversed"));
    assert!(bad(&|p| if let ExposureMode::Adaptive { params, .. } = &mut p.exposure {
        params.speed_up = -1.0
    })
    .contains("speeds"));
    assert!(bad(&|p| if let ExposureMode::Adaptive { pre_ev100, .. } = &mut p.exposure {
        *pre_ev100 = f32::INFINITY
    })
    .contains("pre-exposure"));
}
