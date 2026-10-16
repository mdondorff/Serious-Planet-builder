//! Tier B: the lit path's post chain against the f64 references of `planet_frame::look` (ADR 0038 point 4): the tone-map pass on a
//! ramp image within 1/255 per channel (REND-023), the GPU histogram against the CPU one and the adaptation against the CPU step
//! (REND-022), the false-colour bands, and the compiled-graph executor (REND-019). Synthetic scene targets are written as f16, and
//! the references read the same rounded values back.

use planet_frame::look::{self, ExposureMode, ExposureParams, LitPlan, ToneMapMode};
use planet_render::lit::{f16_bits_to_f32, f32_to_f16_bits};
use planet_render::{AdapterPolicy, GpuContext, LitPath};

const W: u32 = 64;
const H: u32 = 16;

fn ctx() -> GpuContext {
    GpuContext::new(AdapterPolicy::from_env()).expect("adapter")
}

fn half(v: f32) -> f32 {
    f16_bits_to_f32(f32_to_f16_bits(v))
}

/// An image of `w * h` pixels from a function of the pixel index, rounded to the f16 values the target holds.
fn image(w: u32, h: u32, f: impl Fn(u32) -> [f32; 3]) -> Vec<[f32; 4]> {
    (0..w * h)
        .map(|i| {
            let c = f(i);
            [half(c[0]), half(c[1]), half(c[2]), 1.0]
        })
        .collect()
}

fn rgb64(p: &[f32; 4]) -> [f64; 3] {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

// spec: REND-023
#[test]
fn the_tone_map_matches_the_f64_reference_within_one_step_on_a_ramp() {
    let ctx = ctx();
    let mut path = LitPath::new(&ctx, (W, H));
    // A log-spaced ramp from 1e-3 to 50 in red, the same reversed in green, a constant in blue: every row the same, so a
    // column reads as one input. Fixed exposure at the pre-exposure EV: the exposed value is the written one.
    let col = |i: u32| i % W;
    let img = image(W, H, |i| {
        let t = f64::from(col(i)) / f64::from(W - 1);
        let v = 1.0e-3 * libm::pow(50.0 / 1.0e-3, t) as f32;
        let r = 1.0e-3 * libm::pow(50.0 / 1.0e-3, 1.0 - t) as f32;
        [v, r, 0.18]
    });
    path.upload_hdr(&ctx, &img);
    for ev in [10.0f32, 13.5] {
        let out = path.tone_map_to_image(&ctx, &LitPlan::fixed(ev));
        let mut worst = 0i32;
        for (i, p) in img.iter().enumerate() {
            let want = look::tonemap_pixel(rgb64(p), f64::from(ev), f64::from(ev));
            for (c, w) in want.iter().enumerate() {
                let got = out.rgba[4 * i + c];
                let d = (i32::from(got) - i32::from(*w)).abs();
                worst = worst.max(d);
                assert!(d <= 1, "EV {ev}, column {}, channel {c}: GPU {got}, reference {w} (input {:?})", col(i as u32), p);
            }
            assert_eq!(out.rgba[4 * i + 3], 255);
        }
        println!("tone map EV {ev}: worst difference {worst}/255");
    }
}

// spec: REND-023
#[test]
fn the_tone_map_scales_by_the_exposure_difference_to_the_pre_exposure() {
    let ctx = ctx();
    let mut path = LitPath::new(&ctx, (W, H));
    let img = image(W, H, |i| {
        let v = 0.02 + 0.5 * (i % W) as f32 / W as f32;
        [v, v * 0.5, v * 0.25]
    });
    path.upload_hdr(&ctx, &img);
    // Adaptive, reset: the exposure lands on the band mean of the histogram. The tone map must then show the exposed value
    // written * 2^(pre - ev) for the EV the GPU reports.
    let plan = LitPlan { reset_exposure: true, ..LitPlan::adaptive(11.0, 0.016) };
    let out = path.tone_map_to_image(&ctx, &plan);
    let state = path.read_exposure(&ctx);
    let k = libm::exp2(f64::from(11.0 - state.ev100));
    assert!((f64::from(state.factor) - k).abs() < 1e-5 * k.max(1.0), "{state:?} vs {k}");
    for (i, p) in img.iter().enumerate() {
        let want = look::tonemap_pixel(rgb64(p), 11.0, f64::from(state.ev100));
        for (c, w) in want.iter().enumerate() {
            let d = (i32::from(out.rgba[4 * i + c]) - i32::from(*w)).abs();
            assert!(d <= 1, "pixel {i} channel {c}: GPU {}, reference {w} at EV {}", out.rgba[4 * i + c], state.ev100);
        }
    }
}

/// A scene whose pixel `i` has scene EV `evs[i % evs.len()]` when written at `pre`.
fn scene_at(evs: &[f64], pre: f64, n: u32) -> Vec<[f32; 4]> {
    image(n, 1, |i| {
        let ev = evs[i as usize % evs.len()];
        let y = (look::exposure_scale(pre) * libm::exp2(ev) / 8.0) as f32;
        [y, y, y]
    })
}

// spec: REND-022
#[test]
fn the_gpu_histogram_counts_what_the_cpu_reference_counts() {
    let ctx = ctx();
    let size = (W, W);
    let mut path = LitPath::new(&ctx, size);
    // A pre-exposure at which every bin's pixels fit in f16, the first bin's too.
    let pre = 6.0;
    // Bin centres of every bin, uneven counts (bin b gets b % 5 + 1 pixel slots), black pixels and out-of-range ones.
    let mut evs: Vec<f64> = Vec::new();
    for b in 0..look::HIST_BINS {
        for _ in 0..(b % 5 + 1) {
            evs.push(look::bin_center_ev(b));
        }
    }
    let n = W * W;
    let mut img = scene_at(&evs, pre, n);
    for (k, p) in img.iter_mut().enumerate() {
        match k % 97 {
            // Black: not counted.
            0 => *p = [0.0, 0.0, 0.0, 1.0],
            // Far above the last bin and far below the first: counted in the end bins.
            1 => *p = [half(60_000.0), half(60_000.0), half(60_000.0), 1.0],
            2 => *p = [half(1.0e-7), half(1.0e-7), half(1.0e-7), 1.0],
            _ => {}
        }
    }
    path.upload_hdr(&ctx, &img);
    let plan = LitPlan { reset_exposure: true, ..LitPlan::adaptive(pre as f32, 0.016) };
    let _ = path.tone_map_to_image(&ctx, &plan);
    let gpu = path.read_histogram(&ctx);
    // The band mean of this many-bin histogram, through the shader's 40-90 % weighting: the exposure target equals the CPU's.
    let exposure = path.read_exposure(&ctx);
    let want = look::target_ev(&gpu, &ExposureParams::default(), pre);
    assert!((f64::from(exposure.target_ev100) - want).abs() < 1e-4, "target {} vs {want}", exposure.target_ev100);
    let reference: Vec<[f64; 3]> = img.iter().map(rgb64).collect();
    let cpu = look::histogram_of(&reference, pre);
    let total: u32 = gpu.iter().sum();
    assert!(total > n / 2, "a histogram of {total} counts");
    assert!(gpu[0] > cpu_first_bin_of_centres(), "the far-below pixels reach the first bin: {}", gpu[0]);
    for b in 0..look::HIST_BINS {
        assert_eq!(gpu[b], cpu[b], "bin {b}: GPU {} CPU {} (GPU {gpu:?} CPU {cpu:?})", gpu[b], cpu[b]);
    }
    // A second frame clears the histogram first: the same image gives the same counts, not double.
    let _ = path.tone_map_to_image(&ctx, &plan);
    assert_eq!(path.read_histogram(&ctx), gpu, "the histogram is cleared every frame");
}

// spec: REND-022
#[test]
fn the_adaptation_follows_the_cpu_step_frame_by_frame() {
    let ctx = ctx();
    let pre = 12.0f32;
    let params = ExposureParams::default();
    let n = 32u32;
    // Scenes by scene EV: 8 (the first frame snaps to its band centre), then a much brighter one (up at 3 stops/s), then a
    // darker one (down at 1 stop/s), then one above the range maximum (target clamped to 16), then nothing (holds).
    let sequence: [(f64, f32); 8] = [(8.0, 0.1), (14.0, 0.25), (14.0, 0.25), (3.0, 0.5), (3.0, 0.5), (25.0, 0.1), (25.0, 0.1), (0.0, 0.1)];
    let scene = |scene_ev: f64| if scene_ev == 0.0 { vec![[0.0f32; 4]; n as usize] } else { scene_at(&[scene_ev], f64::from(pre), n) };
    let mut path = LitPath::new(&ctx, (n, 1));
    let mut ev = f64::NAN;
    for (frame, &(scene_ev, dt)) in sequence.iter().enumerate() {
        let img = scene(scene_ev);
        path.upload_hdr(&ctx, &img);
        let _ = path.tone_map_to_image(&ctx, &LitPlan::adaptive(pre, dt));
        let gpu = path.read_exposure(&ctx);
        // The CPU chain: the histogram of the frame, its target, one step (the first frame snaps).
        let reference: Vec<[f64; 3]> = img.iter().map(rgb64).collect();
        let hist = look::histogram_of(&reference, f64::from(pre));
        let current = if frame == 0 { f64::from(pre) } else { ev };
        let target = look::target_ev(&hist, &params, current);
        ev = look::adapt_ev(current, target, f64::from(dt), &params, frame == 0);
        assert!(
            (f64::from(gpu.ev100) - ev).abs() < 1e-4 && (f64::from(gpu.target_ev100) - target).abs() < 1e-4,
            "frame {frame} (scene EV {scene_ev}, dt {dt}): GPU ev {} target {}, CPU ev {ev} target {target}",
            gpu.ev100,
            gpu.target_ev100
        );
        let k = libm::exp2(f64::from(pre) - ev);
        assert!((f64::from(gpu.factor) - k).abs() < 1e-3 * k.max(1.0), "frame {frame}: factor {} vs {k}", gpu.factor);
    }
    // Determinism: the same plans from a fresh path give the same exposure to the bit.
    let mut again = LitPath::new(&ctx, (n, 1));
    let mut last = None;
    for &(scene_ev, dt) in &sequence {
        again.upload_hdr(&ctx, &scene(scene_ev));
        let _ = again.tone_map_to_image(&ctx, &LitPlan::adaptive(pre, dt));
        last = Some(again.read_exposure(&ctx));
    }
    let last = last.unwrap();
    assert!((f64::from(last.ev100) - ev).abs() < 1e-4);
    assert_eq!(last, path.read_exposure(&ctx), "two runs of one plan sequence agree exactly");
    assert!(last.ev100 <= params.max_ev100 + 1e-6);
}

// spec: REND-022
#[test]
fn fixed_exposure_ignores_the_scene_and_the_history() {
    let ctx = ctx();
    let mut path = LitPath::new(&ctx, (32, 1));
    for (scene_ev, dt) in [(2.0, 0.5), (18.0, 0.5)] {
        path.upload_hdr(&ctx, &scene_at(&[scene_ev], 13.0, 32));
        let _ = path.tone_map_to_image(&ctx, &LitPlan { dt_s: dt, ..LitPlan::fixed(13.0) });
        let s = path.read_exposure(&ctx);
        assert_eq!((s.ev100, s.target_ev100, s.factor), (13.0, 13.0, 1.0), "scene EV {scene_ev}");
    }
    assert_eq!(path.executed_passes(), ["terrain", "exposure-adapt", "tonemap"], "no histogram pass under fixed exposure");
}

// spec: REND-019
#[test]
fn the_executor_follows_the_compiled_graph_and_recompiles_only_when_the_shape_changes() {
    let ctx = ctx();
    let mut path = LitPath::new(&ctx, (32, 1));
    path.upload_hdr(&ctx, &scene_at(&[8.0], 12.0, 32));
    assert_eq!(path.graph_compiles(), 0, "nothing is compiled before the first frame");
    for _ in 0..3 {
        let _ = path.tone_map_to_image(&ctx, &LitPlan::fixed(12.0));
    }
    assert_eq!(path.graph_compiles(), 1, "three frames of one shape compile once");
    assert_eq!(path.executed_passes(), ["terrain", "exposure-adapt", "tonemap"]);
    let _ = path.tone_map_to_image(&ctx, &LitPlan::adaptive(12.0, 0.016));
    assert_eq!(path.graph_compiles(), 2, "a new exposure mode is a new shape");
    assert_eq!(path.executed_passes(), ["terrain", "exposure-histogram", "exposure-adapt", "tonemap"]);
    for _ in 0..3 {
        let _ = path.tone_map_to_image(&ctx, &LitPlan::adaptive(12.0, 0.016));
    }
    assert_eq!(path.graph_compiles(), 2);
    let _ = ExposureMode::Fixed { ev100: 1.0 };
}

// spec: REND-023
#[test]
fn the_false_colour_view_shows_the_ev_band_of_the_exposed_luminance() {
    let ctx = ctx();
    let n = look::EV_BAND_COUNT as u32 + 2;
    let mut path = LitPath::new(&ctx, (n, 1));
    // Pixel b is grey at 2^-(b + 0.5): the middle of band b; then one clipped (4.0) and one black.
    let img = image(n, 1, |i| {
        let v = match i as usize {
            b if b < look::EV_BAND_COUNT => libm::exp2(-(b as f64 + 0.5)) as f32,
            b if b == look::EV_BAND_COUNT => 4.0,
            _ => 0.0,
        };
        [v, v, v]
    });
    path.upload_hdr(&ctx, &img);
    let plan = LitPlan { tonemap: ToneMapMode::EvBands, ..LitPlan::fixed(10.0) };
    let out = path.tone_map_to_image(&ctx, &plan);
    let px = |i: usize| [out.rgba[4 * i], out.rgba[4 * i + 1], out.rgba[4 * i + 2]];
    for b in 0..look::EV_BAND_COUNT {
        assert_eq!(px(b), look::EV_BAND_COLORS[b], "band {b}");
    }
    assert_eq!(px(look::EV_BAND_COUNT), look::EV_BAND_COLORS[0], "clipped light is in band 0");
    assert_eq!(px(look::EV_BAND_COUNT + 1), [0, 0, 0], "black stays black");
}

/// Pixels at the centre of bin 0 in the histogram test's scene (it has one slot per `b % 5 + 1` of bin 0, so one per 97 more).
fn cpu_first_bin_of_centres() -> u32 {
    1
}
