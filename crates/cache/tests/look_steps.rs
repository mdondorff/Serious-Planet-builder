//! Tier A, BUILD-071 and TEST-158: the look sheet's step rows. TEST-150's 256 x 192 chain window and three crops, holding the stages `k5`, `k6` (the hills 5 m lower) and `k7`
//! (the hills 5 m higher again), written by `CropWriter::stage`; one sheet a crop and azimuth, a row a stage beside its step from the previous stage.
use std::path::{Path, PathBuf};

use planet_cache::look_sheet::{
    read_stages, read_store, step_sheet_px, write_steps, Crop, CropWriter, LABEL_CHARS, MAX_CROP, SHEET_LIMIT, STEP_M,
};
use planet_cache::stage_sheet::{Manifest, LOWER, RAISE};
use planet_generators::preview::{TEXT_CELL_W, TEXT_ROW_H};

const W: usize = 256;
const H: usize = 192;
const K: usize = 8;

fn manifest(label: &str) -> Manifest {
    Manifest {
        x0: 1000,
        y0: 2000,
        w: W,
        h: H,
        coarse: K,
        cell_m: 101.8,
        sea_m: 0.0,
        label: label.into(),
        run: vec!["commit 0123abcd".into()],
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("look-steps-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// TEST-150's surface: flat at 100 m west of x = 64, hills east of it, which `lift` raises.
fn surface(x: usize, y: usize, lift: f32) -> f32 {
    if x < 64 {
        100.0
    } else {
        let (fx, fy) = (x as f32, y as f32);
        100.0 + 80.0 * libm::sinf(fx / 17.0) * libm::cosf(fy / 23.0) + 0.5 * (fx - 64.0) + lift
    }
}

fn crops() -> Vec<Crop> {
    vec![
        Crop { name: "flat".into(), x: 0, y: 0, size: 64 },
        Crop { name: "hill".into(), x: 101, y: 37, size: 64 },
        Crop { name: "edge".into(), x: W - 64, y: H - 64, size: 64 },
    ]
}

fn field(lift: f32) -> Vec<f32> {
    (0..W * H).map(|p| surface(p % W, p / W, lift)).collect()
}

/// The three-stage store: K5, then K6 (the hills 5 m lower), then K7 (back up 5 m).
fn store(root: &Path) -> CropWriter {
    let mut w = CropWriter::new(root, W, H, K, &crops()).unwrap();
    w.stage("k5", "K5", &field(0.0)).unwrap();
    w.stage("k6", "K6", &field(-5.0)).unwrap();
    w.stage("k7", "K7", &field(0.0)).unwrap();
    w.manifest(&manifest("b-fixture").text()).unwrap();
    w
}

fn png_rgb(path: &Path) -> (usize, usize, Vec<u8>) {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgb, "{}", path.display());
    (info.width as usize, info.height as usize, buf[..info.buffer_size()].to_vec())
}

/// The top-left pixel of column `col` (0 the stage, 1 its step) in row `r`: the sheet's fixed layout.
fn panel(size: usize, col: usize, r: usize) -> (usize, usize) {
    let gap = 8;
    (gap + LABEL_CHARS * TEXT_CELL_W + gap + col * (size + gap), 2 * TEXT_ROW_H + gap + TEXT_ROW_H + 4 + r * (size + gap))
}

fn rgb_at(rgb: &[u8], w: usize, x: usize, y: usize) -> [i32; 3] {
    let i = (y * w + x) * 3;
    [i32::from(rgb[i]), i32::from(rgb[i + 1]), i32::from(rgb[i + 2])]
}

fn near(got: [i32; 3], want: [f64; 3]) -> bool {
    (0..3).all(|i| (got[i] - (want[i] * 255.0).round() as i32).abs() <= 1)
}

// spec: BUILD-071, TEST-158
#[test]
fn a_run_of_three_stages_gives_a_sheet_a_crop_and_azimuth_with_a_row_a_stage_and_signed_steps() {
    let root = dir("three");
    store(&root);
    let crops = read_store(&root).unwrap();
    let stages = read_stages(&root, &crops).unwrap();
    assert_eq!(
        stages,
        [("k5".into(), "K5".into()), ("k6".into(), "K6".into()), ("k7".into(), "K7".into())],
        "stages.txt keeps the order written"
    );
    assert!(std::fs::read_to_string(root.join("stages.txt")).unwrap().starts_with("look-stages 1\nstage k5 K5\n"));
    let out = dir("three-out");
    let w = write_steps(&root, &[315.0, 135.0], &out).unwrap();
    // one sheet a crop and azimuth, named <crop>-step-az<A>.png, of the fixed size of three rows
    let mut names: Vec<String> = w.sheets.iter().map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "edge-step-az135.png",
            "edge-step-az315.png",
            "flat-step-az135.png",
            "flat-step-az315.png",
            "hill-step-az135.png",
            "hill-step-az315.png"
        ]
    );
    let (pw, ph) = step_sheet_px(64, 3);
    for (p, size, bytes) in &w.sheets {
        assert_eq!(*size, (pw, ph), "{}", p.display());
        assert!(*bytes <= SHEET_LIMIT);
        let (iw, ih, _) = png_rgb(p);
        assert_eq!((iw, ih), (pw, ph), "{}", p.display());
    }
    // the hills: K6 lowers them by 5 m (past STEP_M: full red), K7 raises them by 5 m (full blue); the first row's step panel is grey
    const { assert!(STEP_M < 5.0) };
    let (_, _, hill) = png_rgb(&out.join("hill-step-az315.png"));
    let (x, y) = panel(64, 1, 1);
    assert!(near(rgb_at(&hill, pw, x + 30, y + 30), LOWER), "a lowering step is full red: {:?}", rgb_at(&hill, pw, x + 30, y + 30));
    let (x, y) = panel(64, 1, 2);
    assert!(near(rgb_at(&hill, pw, x + 30, y + 30), RAISE), "a raising step is full blue: {:?}", rgb_at(&hill, pw, x + 30, y + 30));
    let (x, y) = panel(64, 1, 0);
    assert!(near(rgb_at(&hill, pw, x + 32, y + 32), [0.35; 3]), "the first row has no step: grey");
    // the flat crop: no stage changes its cells, so its step panels are its stage panels, pixel for pixel
    let (_, _, flat) = png_rgb(&out.join("flat-step-az315.png"));
    for r in 1..3 {
        let ((sx, sy), (dx, dy)) = (panel(64, 0, r), panel(64, 1, r));
        for cy in 0..64 {
            for cx in 0..64 {
                assert_eq!(
                    rgb_at(&flat, pw, sx + cx, sy + cy),
                    rgb_at(&flat, pw, dx + cx, dy + cy),
                    "row {r} cell ({cx}, {cy}): an unchanged cell's step pixel is its stage pixel"
                );
            }
        }
    }
    // steps.txt: one line a crop and step, with the signed mean
    let numbers = std::fs::read_to_string(w.numbers.as_ref().unwrap()).unwrap();
    assert!(numbers.starts_with("look-steps 1\nrun b-fixture\n"), "{numbers}");
    assert!(numbers.contains("hill k6 k5 cells 4096 mean_m -5.000 rms_m 5.000 max_abs_m 5.000\n"), "{numbers}");
    assert!(numbers.contains("hill k7 k6 cells 4096 mean_m 5.000 rms_m 5.000 max_abs_m 5.000\n"), "{numbers}");
    assert!(numbers.contains("flat k6 k5 cells 4096 mean_m 0.000 rms_m 0.000 max_abs_m 0.000\n"), "{numbers}");
    assert_eq!(numbers.lines().count(), 2 + 3 * 2, "two steps for each of three crops: {numbers}");
    // the same store gives the same bytes
    let first = std::fs::read(&w.sheets[0].0).unwrap();
    write_steps(&root, &[315.0, 135.0], &out).unwrap();
    assert_eq!(std::fs::read(&w.sheets[0].0).unwrap(), first, "the same store gives the same sheet");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

// spec: BUILD-071, TEST-158
#[test]
fn a_store_without_a_stage_list_takes_the_chain_columns_it_holds() {
    let root = dir("chain");
    let w = CropWriter::new(&root, W, H, K, &crops()).unwrap();
    for s in ["final", "base", "s0"] {
        let b: Vec<u8> = field(0.0).iter().flat_map(|v| v.to_le_bytes()).collect();
        w.field(&format!("{s}.f32"), &b).unwrap();
    }
    w.manifest(&manifest("chain-run").text()).unwrap();
    let crops = read_store(&root).unwrap();
    let stages = read_stages(&root, &crops).unwrap();
    assert_eq!(
        stages,
        [("base".into(), "B".into()), ("s0".into(), "S0".into()), ("final".into(), "FINAL".into())],
        "BUILD-067's columns, in their order"
    );
    let out = dir("chain-out");
    let written = write_steps(&root, &[315.0], &out).unwrap();
    assert_eq!(written.sheets.len(), 3);
    assert!(written.sheets.iter().all(|(_, s, _)| *s == step_sheet_px(64, 3)));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

// spec: BUILD-071, TEST-158
#[test]
fn a_missing_stage_a_single_stage_a_wrong_size_and_a_repeated_name_are_refused() {
    // a listed stage that a crop does not hold, naming both
    let root = dir("missing");
    store(&root);
    std::fs::remove_file(root.join("hill").join("k6.f32")).unwrap();
    let e = write_steps(&root, &[315.0], &dir("missing-out")).unwrap_err();
    assert!(e.contains("k6") && e.contains("hill"), "{e}");
    // a store of one stage
    let one = dir("one");
    let mut w = CropWriter::new(&one, W, H, K, &crops()).unwrap();
    w.stage("k5", "K5", &field(0.0)).unwrap();
    w.manifest(&manifest("one").text()).unwrap();
    assert!(write_steps(&one, &[315.0], &dir("one-out")).unwrap_err().contains("fewer than two stages"));
    // a surface of another size, a name given twice, a label with a space, a name with a path
    let e = w.stage("k6", "K6", &[0.0; 10]).unwrap_err();
    assert!(e.contains("k6") && e.contains("10"), "{e}");
    assert!(w.stage("k5", "K5", &field(0.0)).unwrap_err().contains("twice"));
    assert!(w.stage("K5", "K5", &field(0.0)).unwrap_err().contains("twice"), "names differing only in case are one file on Windows");
    assert!(w.stage("k8", "K 8", &field(0.0)).unwrap_err().contains("whitespace"));
    assert!(w.stage("../k9", "K9", &field(0.0)).unwrap_err().contains("letters, digits"));
    // a list naming a stage twice
    let twice = dir("twice");
    store(&twice);
    std::fs::write(twice.join("stages.txt"), "look-stages 1\nstage k5 K5\nstage k6 K6\nstage k5 K5\n").unwrap();
    assert!(read_stages(&twice, &read_store(&twice).unwrap()).unwrap_err().contains("k5 listed twice"));
    // a new writer at a store's root removes the earlier run's list
    CropWriter::new(&twice, W, H, K, &crops()).unwrap();
    assert!(!twice.join("stages.txt").exists(), "a new writer leaves no stale stage list");
    for d in [root, one, twice] {
        let _ = std::fs::remove_dir_all(d);
    }
}

// spec: BUILD-071, TEST-158
#[test]
fn a_step_sheet_over_the_per_sheet_cap_is_refused_and_not_written() {
    // 48 stages of hashed noise on one crop of the largest side: the stage panels do not compress
    let noise = |p: usize, stage: u64| {
        let mut h = ((stage << 48) | p as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 31;
        100.0 + (h % 4096) as f32 / 16.0
    };
    let root = dir("cap");
    let mut w = CropWriter::new(&root, W, H, K, &[Crop { name: "n".into(), x: 16, y: 16, size: MAX_CROP }]).unwrap();
    for k in 0..48u64 {
        let z: Vec<f32> = (0..W * H).map(|p| noise(p, k)).collect();
        w.stage(&format!("s{k}"), &format!("S{k}"), &z).unwrap();
    }
    w.manifest(&manifest("cap").text()).unwrap();
    let out = dir("cap-out");
    let e = write_steps(&root, &[315.0], &out).unwrap_err();
    assert!(e.contains("over the per-sheet limit") && e.contains(&SHEET_LIMIT.to_string()), "{e}");
    assert!(!out.join("n-step-az315.png").exists(), "a sheet over the cap is not written");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
