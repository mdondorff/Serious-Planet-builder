//! Tier A, BUILD-061's cross-section sheet (TEST-141): a 64 x 64 chain directory at global L cell (1000, 2000) of face 3, cells of 100 m, holding a V valley along y at x = 32
//! (10 m a cell up each side), so the valley's depth and widths are known.
use std::path::{Path, PathBuf};

use planet_cache::stage_sheet::Manifest;
use planet_cache::transect_sheet::*;

const N: usize = 64;

fn manifest(face_line: bool) -> Manifest {
    let run = if face_line { vec!["switch window 3,125,132,250,257,7".to_string()] } else { Vec::new() };
    Manifest { x0: 1000, y0: 2000, w: N, h: N, coarse: 8, cell_m: 100.0, sea_m: 0.0, label: "test window".into(), run }
}

fn dir(name: &str, face_line: bool) -> PathBuf {
    let d = std::env::temp_dir().join(format!("transect-sheet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("chain.txt"), manifest(face_line).text()).unwrap();
    d
}

fn f32s(d: &Path, name: &str, f: &dyn Fn(usize, usize) -> f32) {
    let b: Vec<u8> = (0..N * N).flat_map(|p| f(p % N, p / N).to_le_bytes()).collect();
    std::fs::write(d.join(name), b).unwrap();
}

fn v_valley(x: usize, _y: usize) -> f32 {
    10.0 * (x as f32 - 32.0).abs()
}

/// T1 across the valley at y = 32 (x 4..60), T2 across a flat part (it reads the same field in the flat test), T9 off the window, T8 on another face.
const TRANSECTS: &str = "window,id,face,seed,gx0,gy0,gx1,gy1,river_gx,river_gy,area_km2,area_scope,relief_m,half_km
t,T1,3,7,1004,2032,1060,2032,1032,2032,100.0,window,300.0,2.8
t,T9,3,7,900,2032,960,2032,930,2032,100.0,window,300.0,3.0
t,T8,2,7,1004,2032,1060,2032,1032,2032,100.0,window,300.0,2.8
";

const DEM: &str = "# a comment line
window,id,tile,relief_m,area_km2,k,d_m,z_rel_m
t,T1,N00E000,40.0,90,0,-200.0,40.0
t,T1,N00E000,40.0,90,1,-100.0,20.0
t,T1,N00E000,40.0,90,2,0.0,0.0
t,T1,N00E000,40.0,90,3,100.0,5.0
t,T1,N00E000,40.0,90,4,200.0,30.0
";

// spec: BUILD-061, TEST-141
#[test]
fn a_v_valley_has_its_depth_and_widths() {
    let m = manifest(true);
    let all = parse_transects(TRANSECTS).unwrap();
    let inside = in_window(&all, &m, 3);
    assert_eq!(inside.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["T1"], "only T1 lies inside face 3's window");
    let z: Vec<f32> = (0..N * N).map(|p| v_valley(p % N, p / N)).collect();
    let p = profile(&z, &m, inside[0]);
    assert_eq!(p.len(), 113, "56 L cells at 0.5 a sample: 113 samples");
    assert!((p[0].0 + 2800.0).abs() < 1e-9 && (p[112].0 - 2800.0).abs() < 1e-9);
    let v = valley(&p).unwrap();
    assert!(v.river_m.abs() < 1e-9 && v.river_d.abs() < 1e-9, "the river point is the valley floor at d = 0: {v:?}");
    assert!((v.depth_m - 280.0).abs() < 1e-6, "depth: the lower side's top (x 4 and 60, 280 m) less the river: {v:?}");
    let closed = |w: Width, want: f64| matches!(w, Width::Closed(x) if (x - want).abs() < 1e-6);
    assert!(closed(v.widths[0], 200.0), "10 m above the river: 1 cell each side: {v:?}");
    assert!(closed(v.widths[1], 1000.0), "50 m above the river: 5 cells each side: {v:?}");
    assert_eq!(stale(inside[0], Some(&v)), Some(false), "280 m is over 10 % of the stored 300 m");
}

// spec: BUILD-061, TEST-141
#[test]
fn an_open_side_a_flat_line_and_the_glo30_profile() {
    // one side rises 10 m a cell, the other stays flat: the 10 m width is open on that side
    let half: Vec<(f64, f64)> = (0..21).map(|k| ((k as f64 - 10.0) * 50.0, if k >= 10 { (k as f64 - 10.0) * 5.0 } else { 0.0 })).collect();
    let v = valley(&half).unwrap();
    assert!(matches!(v.widths[0], Width::Open(_)), "the flat side never rises 10 m: {v:?}");
    assert_eq!(v.depth_m, 0.0, "the flat side's top is the river level");
    let t = &parse_transects(TRANSECTS).unwrap()[0];
    assert_eq!(stale(t, Some(&v)), Some(true), "a depth of 0 m on a transect of 300 m is stale");
    assert_eq!(stale(t, None), None, "no S0, no verdict");
    let dem = parse_dem(DEM).unwrap();
    let d = &dem[&("t".to_string(), "T1".to_string())];
    assert_eq!((d.tile.as_str(), d.samples.len()), ("N00E000", 5));
    let dv = valley(&d.samples).unwrap();
    assert!((dv.depth_m - 30.0).abs() < 1e-9, "the lower side's top (30 m on the right) less the river: {dv:?}");
    // 10 m is crossed halfway from 0 to 20 m (d = -50) and a fifth of the way from 5 to 30 m (d = 120)
    assert!(matches!(dv.widths[0], Width::Closed(w) if (w - 170.0).abs() < 1e-9), "{dv:?}");
}

// spec: BUILD-061, TEST-141
#[test]
fn the_sheet_and_table_are_written_and_misfits_are_refused() {
    let d = dir("ok", true);
    f32s(&d, "s0.f32", &v_valley);
    f32s(&d, "final.f32", &|x, y| v_valley(x, y) - if x == 32 { 5.0 } else { 0.0 });
    let all = parse_transects(TRANSECTS).unwrap();
    let dem = parse_dem(DEM).unwrap();
    let out = d.join("sheet.png");
    let (n, w, h) = render(&d, None, &out, &all, &dem).unwrap();
    assert_eq!(n, 1);
    assert!(w > 400 && h > 200 && out.exists());
    let table = std::fs::read_to_string(out.with_extension("txt")).unwrap();
    let rows: Vec<&str> = table.lines().filter(|l| l.starts_with("TRANSECT")).collect();
    assert_eq!(rows.len(), 3, "{table}");
    assert_eq!(rows[0], "TRANSECT test-window t T1 s0 0.00 280.00 200 1000");
    assert!(rows[1].starts_with("TRANSECT test-window t T1 final -5.00 "), "the final surface's river is 5 m lower: {}", rows[1]);
    assert!(rows[2].starts_with("TRANSECT test-window t T1 glo30:N00E000 0.00 30.00 "), "{}", rows[2]);
    // the default output path keeps letters, digits and '-'
    assert!(default_out(Path::new("r"), &manifest(true)).ends_with("target/review/stages/transects/test-window.png"));

    // a flat S0 makes T1 stale, in the table
    let flat = dir("flat", true);
    f32s(&flat, "s0.f32", &|_, _| 50.0);
    render(&flat, None, &flat.join("s.png"), &all, &dem).unwrap();
    let t = std::fs::read_to_string(flat.join("s.txt")).unwrap();
    assert!(t.lines().any(|l| l.starts_with("TRANSECT test-window t T1 s0 ") && l.ends_with(" stale")), "{t}");
    assert!(t.lines().any(|l| l == "TRANSECT test-window t T1 final missing"), "no final.f32: missing, not refused: {t}");

    // refusals: no face line and no --face; no transect inside; a file of the wrong size
    let noface = dir("noface", false);
    assert!(render(&noface, None, &noface.join("s.png"), &all, &dem).unwrap_err().contains("--face F"));
    assert!(render(&noface, Some(1), &noface.join("s.png"), &all, &dem).unwrap_err().contains("no transect lies inside face 1"));
    std::fs::write(noface.join("s0.f32"), [0u8; 12]).unwrap();
    let e = render(&noface, Some(3), &noface.join("s.png"), &all, &dem).unwrap_err();
    assert!(e.contains("s0.f32") && e.contains("12 bytes"), "{e}");
    // a malformed csv names its line
    assert!(parse_transects("window,id\nt,T1\n").unwrap_err().contains("no column 'face'"));
    assert!(parse_dem("window,id,tile,relief_m,area_km2,k,d_m,z_rel_m\nt,T1,X,1,1,0,a,0\n").unwrap_err().contains("'a' is not a number"));
    for p in [d, flat, noface] {
        let _ = std::fs::remove_dir_all(p);
    }
}
