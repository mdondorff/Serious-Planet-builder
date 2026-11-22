use planet_core::{PlanetFixed, Vec3, EARTH_RADIUS_M};
use planet_editor::bookmarks::{
    parse_jump_target, Bookmark, BookmarkShelf, CameraReadout, DEFAULT_JUMP_ALTITUDE_M, DEFAULT_JUMP_PITCH_DEG, MAX_NAME_CHARS,
};
use planet_editor::repro_shelf::{BundleKind, ReproCompatibility, ReproShelf, MAX_REPRO_ENTRIES};
use planet_editor::shape::tangent_frame;
use planet_frame::freecam::FreePose;
use planet_frame::pathlog::{PathLog, PERIOD_MS};
use planet_frame::plan::{DebugView, FramePlan, PlanRequest};
use planet_frame::terrain::{TerrainPlan, TerrainRequest, TerrainView};
use planet_frame::{BundleWorld, Camera, LodParams, ReproBundle, TerrainBundle};
use std::path::PathBuf;

fn unique_test_dir(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("planet_editor_{tag}_{}_{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test scratch dir");
    dir
}

fn synthetic_terrain_bundle(
    lat_deg: f64,
    lon_deg: f64,
    alt_m: f64,
    seed: u64,
    definition_hash: u64,
    open_commit: Option<&str>,
    open_site: Option<&str>,
) -> TerrainBundle {
    let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
    let (north, east) = tangent_frame(dir);
    let heading = (north * 0.6 + east * 0.8).normalized();
    let pose = FreePose::new(PlanetFixed(dir * (EARTH_RADIUS_M + alt_m)), heading, (-25.0_f64).to_radians());
    let cam = pose.camera(60.0_f64.to_radians(), 16.0 / 9.0, 0.1);
    let lod = LodParams {
        radius_m: EARTH_RADIUS_M,
        max_height_m: 9_000.0,
        fov_y_rad: cam.fov_y_rad,
        viewport_h_px: 1080.0,
        tau_px: 4.0,
        cells: 32,
        max_level: 22,
        min_level: 3,
        morph_band: 0.25,
        cull_horizon: true,
    };
    let request =
        TerrainRequest { camera: cam, size: (1920, 1080), view: TerrainView::Relief, lod, face_mapping: "tangent-v1".to_string(), seed };
    let plan = TerrainPlan {
        size: (1920, 1080),
        view: TerrainView::Relief,
        camera: cam,
        view_projection: cam.view_projection(),
        clear_color: [0, 0, 0, 255],
        height_range: [-500.0, 5000.0],
        cells: 32,
        shore_ramp_per_m: 0.0,
        nodes: Vec::new(),
        lit: None,
        water: None,
        rivers: None,
    };
    TerrainBundle {
        generator_version: 1,
        implementation_id: "test".to_string(),
        mesh_version: 1,
        definition_hash,
        adapter: "test-adapter".to_string(),
        commit: Some("deadbeef".to_string()),
        world: Some(BundleWorld {
            height_field: "test-field".to_string(),
            global_res: 256,
            definition_toml: "seed = 14\n".to_string(),
            items: Vec::new(),
            open_commit: open_commit.map(str::to_string),
            open_site: open_site.map(str::to_string),
        }),
        request,
        plan,
    }
}

fn synthetic_frame_bundle(seed: u64, definition_hash: u64) -> ReproBundle {
    let dir = Vec3::from_lat_lon(20.0_f64.to_radians(), 30.0_f64.to_radians());
    let cam_pos = PlanetFixed(dir * (EARTH_RADIUS_M + 50_000.0));
    let cam = Camera::looking_at_centre(cam_pos, Vec3::new(0.0, 0.0, 1.0), 1.0, 1.5, 0.1);
    ReproBundle {
        generator_version: 1,
        implementation_id: "test".to_string(),
        seed,
        res: 32,
        definition_hash,
        adapter: "test-adapter".to_string(),
        request: PlanRequest {
            tiles: Vec::new(),
            camera: cam_pos,
            size: (640, 480),
            view: DebugView::Height,
            radius_m: EARTH_RADIUS_M,
            face_mapping: "tangent-v1".to_string(),
        },
        plan: FramePlan {
            size: (640, 480),
            view: DebugView::Height,
            camera: cam.position,
            clear_color: [0, 0, 0, 255],
            height_range: [-100.0, 100.0],
            draws: Vec::new(),
        },
    }
}

// spec: VIEW-073
#[test]
fn a_camera_pose_round_trips_through_a_bookmark_and_toml_everywhere_including_poles() {
    let cases: [(&str, f64, f64, f64, f64, f64, f64); 5] = [
        ("North Pole", 90.0, 0.0, 2500.0, 45.0, -30.0, 120.0),
        ("South Pole", -90.0, 0.0, 1200.0, 215.4, 15.0, -40.0),
        ("Prime Equator", 0.0, 0.0, 0.5, 0.0, -85.0, 0.0),
        ("Antimeridian", -12.34567, 180.0, 45_000.0, 359.25, 42.0, 420.5),
        ("Alpine Valley", 47.2, 8.55, 800.0, 270.0, -12.5, -150.0),
    ];

    let mut shelf = BookmarkShelf::new();
    for (name, lat, lon, alt, yaw_deg, pitch_deg, ground_m) in cases {
        let dir = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
        let (north, east) = tangent_frame(dir);
        let y = yaw_deg.to_radians();
        let heading = (north * libm::cos(y) + east * libm::sin(y)).normalized();
        let orig = FreePose::new(PlanetFixed(dir * (EARTH_RADIUS_M + ground_m + alt)), heading, pitch_deg.to_radians());

        let readout = CameraReadout::from_pose(&orig, alt);
        assert!(readout.site_flag().starts_with("--site "));

        let id = shelf.add_from_pose(name, &orig, alt, Some("note".to_string())).expect("save pose");
        let parsed = BookmarkShelf::from_toml(&shelf.to_toml()).expect("round-trip toml");
        let b = parsed.get(id).expect("bookmark present");
        let restored = b.pose(EARTH_RADIUS_M, ground_m);

        let pos_err_m = (restored.position.0 - orig.position.0).length();
        assert!(pos_err_m < 1e-3, "{name}: position error {pos_err_m} m >= 1 mm");
        let head_err = (restored.heading - orig.heading).length();
        assert!(head_err < 1e-9, "{name}: heading error {head_err}");
        let pitch_err = (restored.pitch - orig.pitch).abs();
        assert!(pitch_err < 1e-9, "{name}: pitch error {pitch_err}");
    }

    // Blank name becomes automatic `Site <id>`.
    let mut auto = BookmarkShelf::new();
    let dir = Vec3::from_lat_lon(10.0_f64.to_radians(), 20.0_f64.to_radians());
    let (north, _) = tangent_frame(dir);
    let pose = FreePose::new(PlanetFixed(dir * (EARTH_RADIUS_M + 1000.0)), north, 0.0);
    let id = auto.add_from_pose("   ", &pose, 1000.0, None).unwrap();
    assert_eq!(auto.get(id).unwrap().name, format!("Site {id}"));
}

// spec: VIEW-073
#[test]
fn coordinate_jump_accepts_decimal_and_hemisphere_spellings_and_refuses_ambiguous_or_out_of_range_inputs() {
    let t1 = parse_jump_target("47.2, 8.55").unwrap();
    assert_eq!(t1.lat_deg, 47.2);
    assert_eq!(t1.lon_deg, 8.55);
    assert_eq!(t1.altitude_m, DEFAULT_JUMP_ALTITUDE_M);
    assert_eq!(t1.pitch_deg, DEFAULT_JUMP_PITCH_DEG);

    let t2 = parse_jump_target("--site -12.5, 179.9").unwrap();
    assert_eq!((t2.lat_deg, t2.lon_deg), (-12.5, 179.9));

    let t3 = parse_jump_target("--marker 12.5° S, 179.9° W, 3.2 km").unwrap();
    assert!((t3.lat_deg + 12.5).abs() < 1e-12);
    assert!((t3.lon_deg + 179.9).abs() < 1e-12);
    assert!((t3.altitude_m - 3200.0).abs() < 1e-12);

    let t_pole = parse_jump_target("90, 135, 500 m").unwrap();
    assert_eq!((t_pole.lat_deg, t_pole.lon_deg, t_pole.altitude_m), (90.0, 0.0, 500.0));

    let t_anti = parse_jump_target("0, -180").unwrap();
    assert_eq!(t_anti.lon_deg, 180.0);

    let t_wrap = parse_jump_target("0, 350").unwrap();
    assert!((t_wrap.lon_deg + 10.0).abs() < 1e-12);

    for bad in [
        "",
        "--site",
        "47.2",
        "1, 2, 3, 4",
        "91, 0",
        "0, 400",
        "-10 S, 20 E",
        "+10 N, 20 E",
        "10 E, 20 N",
        "10, 20, -5 m",
        "10, 20, 0.1 m",
        "10, 20, nan",
        "10, 20, 5 kg",
    ] {
        assert!(parse_jump_target(bad).is_err(), "expected refusal for '{bad}'");
    }
}

// spec: VIEW-073
#[test]
fn bookmark_shelf_persists_atomically_refuses_bad_toml_and_leaves_missing_files_empty() {
    let dir = unique_test_dir("bookmarks");
    let file = dir.join("nested").join("bookmarks.toml");

    let mut shelf = BookmarkShelf::load_file(&file).expect("missing file yields empty");
    assert!(shelf.is_empty());

    let dir_v = Vec3::from_lat_lon(35.0_f64.to_radians(), -110.0_f64.to_radians());
    let (north, _) = tangent_frame(dir_v);
    let pose = FreePose::new(PlanetFixed(dir_v * (EARTH_RADIUS_M + 2_000.0)), north, (-20.0_f64).to_radians());
    let id1 = shelf.add_from_pose("Canyon Rim", &pose, 2_000.0, Some("Upper bench".into())).unwrap();
    let id2 = shelf.add_from_pose("River Bend", &pose, 500.0, None).unwrap();

    shelf.rename(id2, "Delta Mouth").unwrap();
    assert!(shelf.remove(id1));
    shelf.save_file(&file).expect("atomic save");

    // No stray dot-tmp files left behind.
    let leftover_tmp = std::fs::read_dir(file.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
    assert!(!leftover_tmp, "temp file cleaned up after rename");

    let reloaded = BookmarkShelf::load_file(&file).expect("reload");
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded.get(id2).unwrap().name, "Delta Mouth");

    // Empty shelf still serializes with version = 1 and reloads clean.
    assert_eq!(BookmarkShelf::from_toml(&BookmarkShelf::new().to_toml()).unwrap().len(), 0);

    // Refusals: unknown key, wrong version, duplicate id, blank/too-long name.
    assert!(BookmarkShelf::from_toml("version = 1\nextra = 3\n").unwrap_err().contains("extra"));
    assert!(BookmarkShelf::from_toml("version = 9\n").unwrap_err().contains("version"));
    let dup = "version = 1\n\
        [[bookmark]]\nid = 1\nname = \"A\"\nlat_deg = 0.0\nlon_deg = 0.0\naltitude_m = 10.0\nyaw_deg = 0.0\npitch_deg = 0.0\n\
        [[bookmark]]\nid = 1\nname = \"B\"\nlat_deg = 1.0\nlon_deg = 1.0\naltitude_m = 10.0\nyaw_deg = 0.0\npitch_deg = 0.0\n";
    assert!(BookmarkShelf::from_toml(dup).unwrap_err().contains("duplicate"));

    assert!(shelf.rename(id2, "   ").is_err());
    let too_long = "x".repeat(MAX_NAME_CHARS + 1);
    assert!(shelf.rename(id2, &too_long).is_err());

    let bad_bookmark =
        Bookmark { id: 1, name: too_long, lat_deg: 0.0, lon_deg: 0.0, altitude_m: 100.0, yaw_deg: 0.0, pitch_deg: 0.0, notes: None };
    assert!(bad_bookmark.validate().is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

// spec: VIEW-074
#[test]
fn repro_shelf_scans_terrain_and_frame_bundles_newest_first_isolates_corrupt_files_and_builds_replay_commands() {
    let dir = unique_test_dir("repros");
    // Missing directory returns empty shelf.
    let missing = ReproShelf::scan_dir(&dir.join("absent")).expect("missing dir ok");
    assert!(missing.entries.is_empty() && missing.broken.is_empty());

    // frame-120: TerrainBundle v2 with open_commit, open_site, -path.txt, and .png.
    let b120 = synthetic_terrain_bundle(20.0, 30.0, 2500.0, 14, 0xAAAA_BBBB, Some("C:\\data\\commit 1"), Some("20,30"));
    std::fs::write(dir.join("frame-120.bundle"), b120.to_text()).unwrap();
    let mut path_log = PathLog::new();
    for i in 0..5u64 {
        path_log.record(i * PERIOD_MS, &b120.request.camera, 2500.0, 16.0);
    }
    std::fs::write(dir.join("frame-120-path.txt"), path_log.to_text()).unwrap();
    std::fs::write(dir.join("frame-120.png"), b"PNG-placeholder").unwrap();

    // frame-15: TerrainBundle without open_commit.
    let mut b15 = synthetic_terrain_bundle(-45.0, 120.0, 8000.0, 14, 0x1111_2222, None, None);
    b15.world = None;
    std::fs::write(dir.join("frame-15.bundle"), b15.to_text()).unwrap();

    // custom-a: ReproBundle v1 (`planet-repro 1`).
    let custom = synthetic_frame_bundle(99, 0x3333_4444);
    std::fs::write(dir.join("custom-a.bundle"), custom.to_text()).unwrap();

    // frame-999: corrupt bundle isolated without failing scan.
    std::fs::write(dir.join("frame-999.bundle"), "planet-terrain-repro 2\ngarbage\n---\nbad").unwrap();

    let shelf = ReproShelf::scan_dir(&dir).expect("scan_dir succeeds with broken file present");
    assert_eq!(shelf.broken.len(), 1);
    assert!(shelf.broken[0].path.ends_with("frame-999.bundle"));

    let stems: Vec<&str> = shelf.entries.iter().map(|e| e.stem.as_str()).collect();
    assert_eq!(stems, ["frame-120", "frame-15", "custom-a"]);

    let e120 = &shelf.entries[0];
    assert_eq!(e120.kind, BundleKind::TerrainV2);
    assert_eq!(e120.path_samples, Some(5));
    assert!(e120.png_path.is_some());
    assert_eq!(e120.open_commit.as_deref(), Some("C:\\data\\commit 1"));
    assert_eq!(e120.open_site.as_deref(), Some("20,30"));
    assert!(e120.replay_command().starts_with("cargo xtask repro "));
    assert!(e120.replay_command().contains("frame-120-path.txt"));
    assert!(e120.replay_command().ends_with(" --window"));
    assert!((e120.free_pose().position.0 - b120.request.camera.position.0).length() < 1e-6);

    assert_eq!(e120.compatibility(14, 0xAAAA_BBBB), ReproCompatibility::MatchesWorld);
    assert_eq!(e120.compatibility(14, 0x9999_9999), ReproCompatibility::DifferentDefinition);
    assert_eq!(e120.compatibility(7, 0xAAAA_BBBB), ReproCompatibility::DifferentSeed);

    let bm = e120.to_bookmark(7, 0.0);
    bm.validate().expect("converted bookmark valid");
    assert_eq!(bm.name, "frame-120");
    assert!((bm.pose(EARTH_RADIUS_M, 0.0).position.0 - b120.request.camera.position.0).length() < 1e-3);

    let e15 = &shelf.entries[1];
    assert!(!e15.replay_command().contains("--window"));
    assert!(e15.replay_command().ends_with("frame-15.bundle"));

    let e_custom = &shelf.entries[2];
    assert_eq!(e_custom.kind, BundleKind::FrameV1);

    // Truncation limit check when > MAX_REPRO_ENTRIES candidates exist.
    let big = unique_test_dir("repros_cap");
    for i in 0..=MAX_REPRO_ENTRIES {
        std::fs::write(big.join(format!("frame-{i}.bundle")), "corrupt").unwrap();
    }
    let capped = ReproShelf::scan_dir(&big).unwrap();
    assert_eq!(capped.broken.len(), MAX_REPRO_ENTRIES);
    assert_eq!(capped.truncated, 1);

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&big);
}
