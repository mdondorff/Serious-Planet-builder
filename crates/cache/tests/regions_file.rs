//! The regions file (WDEF-019) and the options of `bake-region` (BUILD-078): the format, its refusals, the shared box rule against `RegionSpec::check`, the paths and the plan.

use std::path::{Path, PathBuf};

use planet_cache::regions_file::*;

fn entry(name: &str, face: u8, b: (u32, u32, u32, u32)) -> RegionEntry {
    RegionEntry { name: name.into(), face, tiles_per_edge: 128, i0: b.0, j0: b.1, i1: b.2, j1: b.3 }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

// spec: WDEF-019
#[test]
fn the_file_round_trips_and_reads_the_owners_format() {
    let text = "format_version = 1\n\n[[region]]\nname = \"alps\"\nface = 1\ntiles_per_edge = 128\ni0 = 40\nj0 = 50\ni1 = 43\nj1 = 52\n";
    let f = RegionsFile::parse(text).unwrap();
    assert_eq!(f.regions, vec![entry("alps", 1, (40, 50, 43, 52))]);
    assert_eq!(f.to_toml(), text);
    let two = RegionsFile { regions: vec![entry("a", 0, (3, 3, 4, 4)), entry("b-2_x", 5, (120, 120, 124, 123))] };
    assert_eq!(RegionsFile::parse(&two.to_toml()).unwrap(), two);
    assert_eq!(RegionsFile::parse("format_version = 1\n").unwrap(), RegionsFile::default());
}

// spec: WDEF-019
#[test]
fn the_file_refuses_what_it_does_not_define_naming_it() {
    let ok = "[[region]]\nname = \"a\"\nface = 1\ntiles_per_edge = 128\ni0 = 4\nj0 = 4\ni1 = 5\nj1 = 5\n";
    for (text, want) in [
        (ok.to_string(), "no format_version"),
        (format!("format_version = 2\n{ok}"), "format_version 2"),
        (format!("format_version = \"1\"\n{ok}"), "not an integer"),
        (format!("format_version = 1\nextra = 3\n{ok}"), "unknown key 'extra'"),
        (format!("format_version = 1\n{}", ok.replace("j1 = 5", "j1 = 5\nstatus = \"ready\"")), "unknown key 'status'"),
        (format!("format_version = 1\n{}", ok.replace("i0 = 4\n", "")), "i0 is missing"),
        (format!("format_version = 1\n{}", ok.replace("i0 = 4", "i0 = -1")), "i0 = -1"),
        (format!("format_version = 1\n{}", ok.replace("face = 1", "face = 1.5")), "face is a float"),
        (format!("format_version = 1\n{}", ok.replace("\"a\"", "\"Big Name\"")), "'Big Name'"),
        (format!("format_version = 1\n{ok}\n{ok}"), "region 'a' twice"),
        ("format_version = 1\nregion = 3\n".to_string(), "not an array"),
        ("format_version = = 1".to_string(), "the regions file"),
    ] {
        let e = RegionsFile::parse(&text).unwrap_err();
        assert!(e.contains(want), "{want:?} not in {e:?}");
    }
}

// spec: WDEF-019
#[test]
fn a_missing_file_is_empty_and_a_write_replaces_the_file_whole() {
    let dir = std::env::temp_dir().join(format!("planet-wdef019-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = default_path(&dir.join("world"));
    assert_eq!(path, dir.join("world").join("regions.toml"));
    assert_eq!(RegionsFile::read(&path).unwrap(), RegionsFile::default());
    let f = RegionsFile { regions: vec![entry("alps", 1, (40, 50, 43, 52))] };
    f.write(&path).unwrap();
    assert_eq!(RegionsFile::read(&path).unwrap(), f);
    let g = RegionsFile { regions: vec![entry("andes", 2, (60, 60, 61, 61))] };
    g.write(&path).unwrap();
    assert_eq!(RegionsFile::read(&path).unwrap(), g);
    let left: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 1, "no temporary file is left: {left:?}");
    assert_eq!(commit_dir(&path, "alps"), dir.join("world").join("regions").join("alps"));
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: WDEF-019
#[test]
fn every_box_the_shared_rule_accepts_passes_region_spec_check_and_its_neighbours_outside_do_not() {
    let l = RegionLayout::PRODUCTION;
    assert_eq!(l.tiles_per_edge(), 128);
    let (lo, hi) = l.allowed_tiles();
    assert_eq!((lo, hi), (3, 124), "the margin of {SOLVE_MARGIN_CELLS} regional cells in tiles of 96");
    let mut accepted = 0;
    for face in 0..6u8 {
        for w in MIN_TILES..=MAX_TILES {
            for h in MIN_TILES..=MAX_TILES {
                for (i0, j0) in [(lo, lo), (hi + 1 - w, hi + 1 - h), (lo, hi + 1 - h), (60, 61)] {
                    let (i1, j1) = (i0 + w - 1, j0 + h - 1);
                    check_box(&l, face, 128, i0, j0, i1, j1).unwrap_or_else(|e| panic!("({i0}, {j0}) {w}x{h}: {e}"));
                    let e = entry("r", face, (i0, j0, i1, j1));
                    e.check(&l).unwrap();
                    e.region_spec(&l, 14, 6_371_000.0, 4, 9).check().unwrap();
                    accepted += 1;
                    // One tile further out on any side is refused, and so is RegionSpec::check where the margin leaves the face.
                    for (a, b, c, d) in
                        [(i0 - 1, j0, i1 - 1, j1), (i0, j0 - 1, i1, j1 - 1), (i0 + 1, j0, i1 + 1, j1), (i0, j0 + 1, i1, j1 + 1)]
                    {
                        if a < lo || b < lo || c > hi || d > hi {
                            let err = check_box(&l, face, 128, a, b, c, d).unwrap_err();
                            assert!(err.contains("solve margin"), "{err}");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(accepted, 6 * 9 * 4);
    // The tile just outside the band also fails RegionSpec::check itself: the band is the spec's, not stricter by a tile.
    let outside = entry("r", 0, (lo - 1, lo, lo, lo + 1)).region_spec(&l, 14, 6_371_000.0, 4, 9);
    assert!(outside.check().is_err(), "tile {} is outside the margin", lo - 1);
    for (b, want) in [((10, 10, 10, 11), "1 x 2"), ((10, 10, 14, 11), "5 x 2"), ((11, 10, 10, 11), "empty")] {
        let err = check_box(&l, 0, 128, b.0, b.1, b.2, b.3).unwrap_err();
        assert!(err.contains(want), "{want:?} not in {err}");
    }
    assert!(check_box(&l, 6, 128, 10, 10, 11, 11).unwrap_err().contains("face 6"));
    assert!(check_box(&l, 0, 64, 10, 10, 11, 11).unwrap_err().contains("128 a face edge"));
    assert!(entry("A", 0, (10, 10, 11, 11)).check(&l).unwrap_err().contains("'A'"));
}

// spec: BUILD-078
#[test]
fn the_options_take_the_world_folder_or_a_seed_with_a_file() {
    let a = BakeArgs::parse(&s(&["--world", "w/def.toml", "--items", "w/items", "--name", "alps", "--force", "--threads", "3"])).unwrap();
    assert_eq!(a.regions, Path::new("w").join("regions.toml"));
    assert_eq!((a.items.clone(), a.name.as_deref(), a.force, a.threads), (Some(PathBuf::from("w/items")), Some("alps"), true, 3));
    assert!(a.open_command(Path::new("w/regions/alps")).contains("editor --world w/def.toml --items w/items --commit w/regions/alps"));
    let b = BakeArgs::parse(&s(&["--world", "w/def.toml", "--regions", "x.toml"])).unwrap();
    assert_eq!(b.regions, PathBuf::from("x.toml"));
    let c = BakeArgs::parse(&s(&["--seed", "14", "--regions", "r.toml"])).unwrap();
    assert_eq!((c.seed, c.world.as_ref()), (Some(14), None));
    assert!(c.open_command(Path::new("regions/a")).contains("editor --seed 14 --commit regions/a"));
    for (args, want) in [
        (vec!["--seed", "14"], "--seed needs --regions"),
        (vec!["--seed", "14", "--world", "w.toml"], "exclude each other"),
        (vec![], "give --world F or --seed N"),
        (vec!["--world"], "--world needs"),
        (vec!["--world", "w.toml", "--bogus"], "unknown option '--bogus'"),
    ] {
        let e = BakeArgs::parse(&s(&args)).unwrap_err();
        assert!(e.contains(want) && e.contains("usage"), "{want:?} not in {e}");
    }
}

// spec: BUILD-078
#[test]
fn the_plan_checks_every_box_before_any_bake_and_skips_ready_regions_unless_forced() {
    let dir = std::env::temp_dir().join(format!("planet-build078-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let args = BakeArgs::parse(&s(&["--seed", "14", "--regions", dir.join("regions.toml").to_str().unwrap()])).unwrap();
    let l = RegionLayout::PRODUCTION;
    let f = RegionsFile { regions: vec![entry("a", 1, (40, 40, 41, 41)), entry("b", 1, (50, 50, 53, 53))] };
    let plan = bake_plan(&f, &args, &l, 7).unwrap();
    assert_eq!(
        plan.iter().map(|(e, s, b)| (e.name.as_str(), *s, *b)).collect::<Vec<_>>(),
        [("a", RegionState::Unbaked, true), ("b", RegionState::Unbaked, true)]
    );
    let one = BakeArgs { name: Some("b".into()), ..args.clone() };
    assert_eq!(bake_plan(&f, &one, &l, 7).unwrap().len(), 1);
    assert!(bake_plan(&f, &BakeArgs { name: Some("c".into()), ..args.clone() }, &l, 7).unwrap_err().contains("no region 'c'"));
    assert!(bake_plan(&RegionsFile::default(), &args, &l, 7).unwrap_err().contains("holds no region"));
    let bad = RegionsFile { regions: vec![entry("a", 1, (40, 40, 41, 41)), entry("edge", 1, (0, 40, 1, 41))] };
    assert!(
        bake_plan(&bad, &args, &l, 7).unwrap_err().contains("region 'edge'"),
        "a bad box anywhere stops the plan before the first bake"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
