//! Tier A, BUILD-060's chain sheet: a 32 x 32 chain directory at global L cell (100, 200), H1 cells of 8 L cells, read at one L cell per pixel.
use std::path::{Path, PathBuf};

use planet_cache::stage_sheet::{compose, panels, render, shade, trib_min_m2, Manifest, View, CHANGE_M, COLUMNS, LOWER, TRIB, TRUNK};
use planet_generators::bake::river_graph::*;

const N: usize = 32;

fn manifest() -> Manifest {
    Manifest { x0: 100, y0: 200, w: N, h: N, coarse: 8, cell_m: 100.0, sea_m: 0.0, label: "test window".into() }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("stage-sheet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("stages")).unwrap();
    std::fs::write(d.join("chain.txt"), manifest().text()).unwrap();
    d
}

fn f32s(d: &Path, name: &str, f: &dyn Fn(usize, usize) -> f32, n: usize) {
    let b: Vec<u8> = (0..n * n).flat_map(|p| f(p % n, p / n).to_le_bytes()).collect();
    std::fs::write(d.join(name), b).unwrap();
}

fn graph() -> Graph {
    let gid = |x: u64, y: u64| (y << 32) | x;
    let node = |kind: NodeKind, x: u64, y: u64| {
        let anchor = gid(x, y);
        Node { id: node_id(1, kind, Level::L, anchor), kind, level: Level::L, anchor, parent: None, owner: Domain::Landmass, marker: None }
    };
    let reach = |up: &Node, down: &Node, cells: &[(u64, u64)], parent: Option<u64>, area: f64| Reach {
        id: reach_id(1, Level::L, up.id, down.id),
        level: Level::L,
        up: up.id,
        down: down.id,
        vertices: cells.iter().map(|&(x, y)| gid(x, y)).collect(),
        parent,
        owner: Domain::Landmass,
        attrs: Attrs {
            a_geo: vec![area; cells.len()],
            a_ext: vec![0.0; cells.len()],
            a_run: vec![1.0; cells.len()],
            a_run_no_ext: vec![1.0; cells.len()],
            q: vec![0.0; cells.len()],
            q_w: vec![0.0; cells.len()],
            strahler: vec![1; cells.len()],
            bed: vec![0; cells.len()],
        },
        exceptions: Vec::new(),
    };
    let (a, b, c) = (node(NodeKind::Source, 110, 210), node(NodeKind::OutletSea, 120, 210), node(NodeKind::Source, 105, 225));
    // the drawn tributary drains 1 km2; the small one 0.1 km2, under the 25 pixels (0.25 km2) of a 100 m pixel
    let d = node(NodeKind::Source, 125, 225);
    let trunk = reach(&a, &b, &[(110, 210), (115, 210), (120, 210)], Some(7), 1.0e7);
    let trib = reach(&c, &a, &[(105, 225), (110, 210)], None, 1.0e6);
    let small = reach(&d, &b, &[(125, 225), (120, 210)], None, 1.0e5);
    let mut g = Graph { body: 1, nodes: vec![a, b, c, d], reaches: vec![trunk, trib, small], basins: Vec::new() };
    g.sort();
    g
}

// spec: BUILD-060
#[test]
fn the_manifest_round_trips_and_refuses_a_wrong_version_or_size() {
    let m = manifest();
    assert_eq!(Manifest::parse(&m.text()).unwrap(), m);
    assert!(Manifest::parse("stage-chain 2\n").unwrap_err().contains("stage-chain 1"));
    assert!(Manifest::parse("stage-chain 1\nwindow 0 0 30 32\ncoarse 8\ncell_m 1\n").unwrap_err().contains("multiple of coarse 8"));
    assert!(Manifest::parse("stage-chain 1\nwindow 0 0 32 32\ncoarse 8\ncell_m 1\ncolour red\n").unwrap_err().contains("unknown line"));
    assert!(Manifest::parse("stage-chain 1\nwindow 0 0 32 32\ncoarse 8\n").unwrap_err().contains("cell_m"));
}

// spec: BUILD-060
#[test]
fn a_flat_surface_is_mid_grey_under_the_fixed_shade_and_a_slope_facing_the_sun_is_lighter() {
    let m = manifest();
    let v = View::whole(&m, 1);
    let flat = vec![10f32; N * N];
    let (g, mean) = shade(&flat, &m, &v);
    assert!(g.iter().all(|&x| (x - 0.5).abs() < 1e-12), "flat is 0.5 at any height");
    assert!((mean[0] - 10.0).abs() < 1e-12);
    // the sun is north-west: a surface rising to the south-east faces it
    let facing: Vec<f32> = (0..N * N).map(|p| ((p % N) + (p / N)) as f32 * 10.0).collect();
    let away: Vec<f32> = facing.iter().map(|z| -z).collect();
    let (gf, _) = shade(&facing, &m, &v);
    let (ga, _) = shade(&away, &m, &v);
    assert!(gf[N * 16 + 16] > 0.6 && ga[N * 16 + 16] < 0.4, "facing {} away {}", gf[N * 16 + 16], ga[N * 16 + 16]);
}

// spec: BUILD-060
#[test]
fn the_sheet_has_twelve_labelled_panels_draws_the_graph_and_colours_the_change_on_a_fixed_scale() {
    let d = dir("full");
    let m = manifest();
    for name in ["base", "s0", "kh", "ks", "kv"] {
        f32s(&d, &format!("{name}.f32"), &|_, _| 100.0, N);
    }
    // the final surface: 50 m lower than S0 left of x = 16, unchanged right of it
    f32s(&d, "final.f32", &|x, _| if x < 16 { 50.0 } else { 100.0 }, N);
    f32s(&d, "zs.f32", &|_, _| 100.0, N / 8);
    let mut riv = vec![0u8; 16];
    riv[5] = 1; // H1 cell (1, 1): L cells 8..16
    std::fs::write(d.join("h1-rivers.u8"), riv).unwrap();
    std::fs::write(d.join("graph.txt"), graph().canonical()).unwrap();
    let v = View::whole(&m, 1);
    let p = panels(&d, &m, &v).unwrap();
    assert_eq!(p.len(), 12);
    assert!(p.iter().all(|p| p.rgb.is_some()), "every panel has its data");
    let at = |k: usize, x: usize, y: usize| p[k].rgb.as_ref().unwrap()[y * N + x];
    assert_eq!(at(2, 12, 12), TRUNK, "panel 3 draws the H1 river cell");
    assert_eq!(at(6, 15, 10), TRUNK, "panel 7 draws the trunk (a reach with a parent)");
    assert_ne!(at(6, 5, 25), TRIB, "panel 7 draws no tributary");
    assert_eq!(at(7, 5, 25), TRIB, "panel 8 draws the tributary");
    assert_eq!(trib_min_m2(&m, &v), 250_000.0);
    assert_ne!(at(7, 25, 25), TRIB, "a tributary draining less than 25 pixels is not drawn");
    assert_eq!(at(10, 15, 10), TRUNK, "panel 11 draws every reach");
    let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-12);
    assert!(near(at(11, 4, 4), LOWER), "a lowering of CHANGE_M ({CHANGE_M} m) is full red: {:?}", at(11, 4, 4));
    assert!(near(at(11, 24, 4), [0.55; 3]), "no change is the shade only: {:?}", at(11, 24, 4));
    let img = compose(&m, &v, &p);
    assert_eq!(img.width as usize, 8 + COLUMNS * (N + 8));
    let out = d.join("sheet.png");
    assert_eq!(render(&d, None, 1, &out).unwrap(), (img.width, img.height));
    assert!(out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

// spec: BUILD-060
#[test]
fn a_missing_file_leaves_its_panel_empty_and_a_wrong_size_is_refused_naming_the_file() {
    let d = dir("partial");
    let m = manifest();
    f32s(&d, "s0.f32", &|_, _| 1.0, N);
    let p = panels(&d, &m, &View::whole(&m, 2)).unwrap();
    assert_eq!(p.iter().filter(|p| p.rgb.is_some()).count(), 1, "only S0 is there");
    assert!(p[3].label.starts_with("4 S0") && p[3].rgb.is_some());
    f32s(&d, "kh.f32", &|_, _| 1.0, N - 1);
    let e = panels(&d, &m, &View::whole(&m, 2)).err().unwrap();
    assert!(e.contains("kh.f32") && e.contains(&(4 * (N - 1) * (N - 1)).to_string()) && e.contains(&(4 * N * N).to_string()), "{e}");
    let e = render(&d, Some(View { x: 8, y: 0, size_x: N, size_y: N, step: 1 }), 1, &d.join("x.png")).unwrap_err();
    assert!(e.contains("not inside the window"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
