//! Tier A, BUILD-067 and TEST-150: the look sheet. A 256 x 192 chain window at global L cell (1000, 2000), H1 cells of 8 L cells, cut into crops; the crops' bytes and shade are the full
//! window's, the sheets' layout and scales are fixed, and a sheet over the per-sheet cap is refused and not written.
use std::path::{Path, PathBuf};

use planet_cache::look_sheet::{
    crop_box, parse_crops_json, read_store, sheet_px, write_sheets, Crop, CropWriter, DIFF_M, MAX_CROP, SHEET_LIMIT, STAGES,
};
use planet_cache::stage_sheet::{shade_at, Manifest, View, RAISE};

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
        run: vec!["commit 0123abcd".into(), "switch kh h5".into()],
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("look-sheet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The fixture surface: flat at 100 m west of x = 64 (a flat cell is the shade's mid grey), hills east of it, a step of `lift` m on the stage `lifted`.
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

/// Writes a crop store of the fixture: every stage of [`STAGES`] but `ks0` (a stage not dumped), `zs.f32` at H1 and a u16 stage field; the stage `lifted` is `lift` m higher.
fn store(root: &Path, label: &str, lifted: &str, lift: f32, f: &dyn Fn(usize, usize, f32) -> f32) -> Vec<(String, Vec<u8>)> {
    let w = CropWriter::new(root, W, H, K, &crops()).unwrap();
    let mut full = Vec::new();
    for (s, _) in STAGES.iter().filter(|(s, _)| *s != "ks0") {
        let l = if *s == lifted { lift } else { 0.0 };
        let b: Vec<u8> = (0..W * H).flat_map(|p| f(p % W, p / W, l).to_le_bytes()).collect();
        w.field(&format!("{s}.f32"), &b).unwrap();
        full.push((format!("{s}.f32"), b));
    }
    let zs: Vec<u8> = (0..(W / K) * (H / K)).flat_map(|p| (p as f32).to_le_bytes()).collect();
    w.field("zs.f32", &zs).unwrap();
    full.push(("zs.f32".into(), zs));
    let lower: Vec<u8> = (0..W * H).flat_map(|p| ((p % 977) as u16).to_le_bytes()).collect();
    w.field("stages/kv-lower.u16", &lower).unwrap();
    full.push(("stages/kv-lower.u16".into(), lower));
    w.manifest(&manifest(label).text()).unwrap();
    full
}

fn png_rgb(path: &Path) -> (u32, u32, Vec<u8>) {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgb, "{}", path.display());
    (info.width, info.height, buf[..info.buffer_size()].to_vec())
}

// spec: BUILD-067, TEST-150
#[test]
fn a_crop_holds_the_full_fields_bytes_and_the_full_windows_shade_over_its_cells() {
    let root = dir("bytes");
    let full = store(&root, "fixture", "", 0.0, &surface);
    let fm = manifest("fixture");
    for c in read_store(&root).unwrap() {
        let (bx, by, bw, bh) = crop_box(W, H, K, &c.crop).unwrap();
        assert!(bx % K == 0 && by % K == 0 && bw % K == 0 && bh % K == 0, "the box is aligned to the H1 grid: {:?}", (bx, by, bw, bh));
        assert_eq!((c.manifest.x0, c.manifest.y0, c.manifest.w, c.manifest.h), (1000 + bx as u64, 2000 + by as u64, bw, bh));
        assert_eq!(c.manifest.run, fm.run, "the crop keeps the run's chain lines");
        for (name, bytes) in &full {
            let (fw, scale, elem) = if name == "zs.f32" {
                (W / K, K, 4)
            } else if name.ends_with(".u16") {
                (W, 1, 2)
            } else {
                (W, 1, 4)
            };
            let cut: Vec<u8> = (by / scale..(by + bh) / scale)
                .flat_map(|y| bytes[(y * fw + bx / scale) * elem..(y * fw + (bx + bw) / scale) * elem].to_vec())
                .collect();
            assert_eq!(std::fs::read(c.dir.join(name)).unwrap(), cut, "crop {} field {name}", c.crop.name);
        }
        // the crop's shade is the full window's over the same cells, bit for bit, at both suns
        let z_full: Vec<f32> = full[0].1.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let z_crop = planet_cache::stage_sheet::read_f32(&c.dir, "base.f32", bw * bh).unwrap().unwrap();
        let fv = View { x: c.crop.x, y: c.crop.y, size_x: c.crop.size, size_y: c.crop.size, step: 1 };
        for az in [315.0, 135.0] {
            assert_eq!(shade_at(&z_crop, &c.manifest, &c.view, az), shade_at(&z_full, &fm, &fv, az), "crop {} at {az}", c.crop.name);
        }
    }
    let w = CropWriter::new(&dir("refuse"), W, H, K, &crops()).unwrap();
    assert!(w.field("odd.f32", &[0u8; 12]).unwrap_err().contains("odd.f32"), "a field of no known size is refused by name");
    assert!(CropWriter::new(&dir("out"), W, H, K, &[Crop { name: "far".into(), x: W - 10, y: 0, size: 64 }])
        .unwrap_err()
        .contains("not inside"));
    let _ = std::fs::remove_dir_all(&root);
}

// spec: BUILD-067, TEST-150
#[test]
fn the_crops_file_of_spike_143_reads_its_pairs_in_order_and_skips_its_numbers() {
    let c = parse_crops_json(
        r#"{ "size": 120, "river": [3160, 3640], "hill": [3280, 3520], "hill_x8": 1.134, "rim": [2219, 2555], "rim_depth_m": 247.8 }"#,
    )
    .unwrap();
    let names: Vec<&str> = c.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["river", "hill", "rim"]);
    assert_eq!((c[2].x, c[2].y, c[2].size), (2219, 2555, 120));
    assert!(parse_crops_json(r#"{"size": 400, "a": [1, 2]}"#).unwrap_err().contains(&format!("1 to {MAX_CROP}")));
    assert!(parse_crops_json(r#"{"a": [1, 2]}"#).unwrap_err().contains("no size"));
    assert!(parse_crops_json(r#"{"size": 64}"#).unwrap_err().contains("no crop"));
}

// spec: BUILD-067, TEST-150
#[test]
fn a_sheets_layout_and_scales_are_fixed_and_the_pair_shows_the_difference() {
    let (def, sw) = (dir("def"), dir("sw"));
    store(&def, "defaults", "", 0.0, &surface);
    store(&sw, "switch", "kv", 20.0, &surface);
    // one sheet an azimuth, a row a crop, at the fixed size; the same run gives the same bytes
    let out = dir("one");
    let a = write_sheets(&sw, None, &[315.0, 135.0], &out).unwrap();
    assert_eq!(a.sheets.len(), 2);
    let (pw, ph) = sheet_px(64, 3);
    for (p, size, bytes) in &a.sheets {
        assert_eq!(*size, (pw, ph), "{}", p.display());
        assert!(*bytes <= SHEET_LIMIT);
        let (w, h, _) = png_rgb(p);
        assert_eq!((w as usize, h as usize), (pw, ph));
    }
    let first = std::fs::read(&a.sheets[0].0).unwrap();
    write_sheets(&sw, None, &[315.0, 135.0], &out).unwrap();
    assert_eq!(std::fs::read(&a.sheets[0].0).unwrap(), first, "the same store gives the same sheet");
    // a flat cell is the shade's mid grey under both suns: the scale is fixed, not stretched per sheet
    let (_, _, flat_315) = png_rgb(&a.sheets[0].0);
    let (_, _, flat_135) = png_rgb(&a.sheets[1].0);
    let (pw, _) = sheet_px(64, 3);
    let at = |rgb: &[u8], x: usize, y: usize| [rgb[(y * pw + x) * 3], rgb[(y * pw + x) * 3 + 1], rgb[(y * pw + x) * 3 + 2]];
    // the first row's first panel (crop `flat`, stage B): its centre
    let (ox, oy) = first_panel(64, 0, 0);
    let mid = [(0.25f64 + 0.7 * 0.5), 0.24 + 0.68 * 0.5, 0.22 + 0.64 * 0.5].map(|v| (v * 255.0).round() as u8);
    assert_eq!(at(&flat_315, ox + 32, oy + 32), mid);
    assert_eq!(at(&flat_135, ox + 32, oy + 32), mid);
    // the pair: one sheet a crop and azimuth with three rows; the stage raised by 20 m (over DIFF_M) is full blue in the difference row; look.txt has its numbers
    let pair = dir("pair");
    let b = write_sheets(&sw, Some(&def), &[315.0], &pair).unwrap();
    assert_eq!(b.sheets.len(), 3);
    assert!(b.sheets.iter().all(|(_, s, _)| *s == sheet_px(64, 3)));
    let (_, _, hill) = png_rgb(&pair.join("hill-az315.png"));
    let kv = STAGES.iter().position(|(s, _)| *s == "kv").unwrap();
    let (ox, oy) = first_panel(64, kv, 2);
    const { assert!(DIFF_M < 20.0) };
    let full = RAISE.map(|v| (v * 255.0).round() as i32);
    let got = at(&hill, ox + 30, oy + 30);
    assert!(
        (0..3).all(|i| (i32::from(got[i]) - full[i]).abs() <= 1),
        "a raise past DIFF_M is the full raise colour: {got:?} against {full:?}"
    );
    let (ox, oy) = first_panel(64, 1, 2);
    let (_, _, same) = png_rgb(&pair.join("hill-az315.png"));
    let (ox1, oy1) = first_panel(64, 1, 1);
    assert_eq!(at(&same, ox + 30, oy + 30), at(&same, ox1 + 30, oy1 + 30), "an unchanged stage's difference panel is its switch panel");
    let numbers = std::fs::read_to_string(b.numbers.unwrap()).unwrap();
    assert!(numbers.contains("hill kv cells 4096 rms_m 20.000 max_abs_m 20.000"), "{numbers}");
    assert!(numbers.contains("hill s0 cells 4096 rms_m 0.000 max_abs_m 0.000"), "{numbers}");
    assert!(!numbers.contains(" ks0 "), "a stage not dumped has no numbers: {numbers}");
    // a pair needs the same crops
    let other = dir("other");
    let w = CropWriter::new(&other, W, H, K, &crops()[..2]).unwrap();
    w.manifest(&manifest("other").text()).unwrap();
    assert!(write_sheets(&sw, Some(&other), &[315.0], &dir("x")).unwrap_err().contains("other crops or windows"));
    // and the same crops of another window
    let moved = dir("moved");
    let w = CropWriter::new(&moved, W, H, K, &crops()).unwrap();
    w.manifest(&Manifest { x0: 5000, ..manifest("moved") }.text()).unwrap();
    assert!(write_sheets(&sw, Some(&moved), &[315.0], &dir("y")).unwrap_err().contains("same window"));
    let _ = std::fs::remove_dir_all(moved);
    for d in [def, sw, out, pair, other] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The top-left pixel of the panel of stage column `s` in row `r` (the sheet's fixed layout).
fn first_panel(size: usize, s: usize, r: usize) -> (usize, usize) {
    use planet_generators::preview::{TEXT_CELL_W, TEXT_ROW_H};
    let gap = 8;
    let x0 = gap + planet_cache::look_sheet::LABEL_CHARS * TEXT_CELL_W + gap;
    (x0 + s * (size + gap), 2 * TEXT_ROW_H + gap + TEXT_ROW_H + 4 + r * (size + gap))
}

// spec: BUILD-067, TEST-150
#[test]
fn a_sheet_over_the_per_sheet_cap_is_refused_naming_its_size_and_is_not_written() {
    // up to 256 m of hashed noise a cell, other in every stage: the shade is random grey, which does not compress
    let noise = |x: usize, y: usize, stage: u64| {
        // splitmix64 of the cell and the stage, so no two panels repeat
        let mut h = ((stage << 48) | ((y as u64) << 24) | x as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 31;
        100.0 + (h % 4096) as f32 / 16.0
    };
    let big = |name: &str, n: usize| {
        let d = dir(name);
        let cs: Vec<Crop> = (0..n).map(|i| Crop { name: format!("n{i}"), x: i * 16, y: 0, size: MAX_CROP }).collect();
        let w = CropWriter::new(&d, W, H, K, &cs).unwrap();
        for (k, (s, _)) in STAGES.iter().enumerate() {
            let b: Vec<u8> = (0..W * H).flat_map(|p| noise(p % W, p / W, k as u64).to_le_bytes()).collect();
            w.field(&format!("{s}.f32"), &b).unwrap();
        }
        w.manifest(&manifest(name).text()).unwrap();
        d
    };
    // a pair's three rows of the largest crops stay under the cap even on noise
    let three = big("noise3", 3);
    let ok = write_sheets(&three, None, &[315.0], &dir("noise3-out")).unwrap();
    assert!(ok.sheets[0].2 <= SHEET_LIMIT, "{} bytes", ok.sheets[0].2);
    // eight rows of noise do not: the sheet is refused, naming the limit, and not written
    let run = big("noise8", 8);
    let out = dir("noise8-out");
    let e = write_sheets(&run, None, &[315.0], &out).unwrap_err();
    assert!(e.contains("over the per-sheet limit") && e.contains(&SHEET_LIMIT.to_string()), "{e}");
    assert!(!out.join("az315.png").exists(), "a sheet over the cap is not written");
    let _ = std::fs::remove_dir_all(three);
    let _ = std::fs::remove_dir_all(dir("noise3-out"));
    let _ = std::fs::remove_dir_all(run);
    let _ = std::fs::remove_dir_all(out);
}
