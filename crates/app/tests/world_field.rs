//! GEN-018: the app and `test-render` take the terrain field from the world definition.

use planet::worldfield::{legacy_world, WorldTerrain};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference, IMPLEMENTATION_ID as REFERENCE_ID};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 16;

fn world(id: Option<&str>) -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(R, 5);
    if let Some(id) = id {
        d.set_height_field(id);
    }
    d
}

fn run_str(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn write_world(name: &str, extra: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("planet-world-field-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("world.toml");
    std::fs::write(&path, format!("schema_version = 1\n[body.0]\nradius_m = 6371000.0\n[world]\n{extra}seed = 5\n")).unwrap();
    path
}

// spec: GEN-018
#[test]
fn the_default_world_builds_the_eroded_field_and_a_named_world_its_own() {
    let default = WorldTerrain::open(world(None), RES).unwrap();
    assert_eq!(default.id(), "cpu-erosion-v1");
    assert_eq!(default.field().unwrap().implementation_id(), "cpu-erosion-v1");
    let named = WorldTerrain::open(world(Some("cpu-terrain-v1")), RES).unwrap();
    assert_eq!(named.id(), "cpu-terrain-v1");
    assert_eq!(named.field().unwrap().implementation_id(), "cpu-terrain-v1");
    assert!(named.describe().contains("cpu-terrain-v1") && named.describe().contains("16-cell bake"), "{}", named.describe());
}

// spec: GEN-018
#[test]
fn an_unknown_id_is_refused_with_the_known_ids() {
    let e = WorldTerrain::open(world(Some("cpu-terrain-v2")), RES).err().expect("refused");
    assert!(e.contains("cpu-terrain-v2"), "{e}");
    for id in ["cpu-erosion-v1", "cpu-terrain-v1", REFERENCE_ID] {
        assert!(e.contains(id), "{e}");
    }
}

// spec: GEN-018
#[test]
fn the_seed_only_world_names_the_reference_field_and_keys_like_it() {
    let w = WorldTerrain::open(legacy_world(7, R), RES).unwrap();
    let f = w.field().unwrap();
    assert_eq!(f.implementation_id(), REFERENCE_ID);
    let direct = Reference(7);
    let d = planet_core::Vec3::new(0.2, -0.6, 0.7).normalized();
    assert_eq!(f.height(d).to_bits(), direct.height(d).to_bits());
    assert_eq!(f.inputs_hash(), direct.inputs_hash(), "the plans and meshes of every golden keep their keys");
    assert_eq!(f.max_abs_height_m(), direct.max_abs_height_m());
}

// spec: GEN-018
#[test]
fn the_default_world_plans_a_view_with_its_eroded_field() {
    let w = WorldTerrain::open(world(None), 64).unwrap();
    let field = w.field().unwrap();
    let views = scripted_views_field(R, 4.0 / 3.0, field.as_ref());
    let (_, camera) = views[3];
    let lod = LodParams {
        cells: 16,
        viewport_h_px: 192.0,
        tau_px: 4.0,
        max_height_m: field.max_abs_height_m().max(9000.0),
        ..LodParams::earth_1080p()
    };
    let req = TerrainRequest {
        camera,
        size: (256, 192),
        view: TerrainView::Level,
        lod,
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    };
    let plan = plan_terrain_field(&req, field.as_ref()).unwrap();
    assert!(!plan.nodes.is_empty());
}

// spec: GEN-018
#[test]
fn test_render_refuses_an_unknown_id_before_any_gpu_work() {
    let path = write_world("unknown", "height_field = \"cpu-erosion-v9\"\n");
    let e = run_str(&["test-render", "--scene", "terrain", "--world", path.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("cpu-erosion-v9") && e.contains("cpu-erosion-v1") && e.contains("cpu-terrain-v1"), "{e}");
    // The unknown id is refused on every path that takes the world, the perf scene included.
    for extra in [vec!["--stream"], vec!["--bundle", "b.txt"], vec!["--perf-smoke"]] {
        let mut args = vec!["test-render", "--scene", "terrain", "--world", path.to_str().unwrap()];
        args.extend(extra.iter());
        let e = run_str(&args).unwrap_err();
        assert!(e.contains("cpu-erosion-v9"), "{args:?}: {e}");
    }
    // What a world-selected run cannot do is refused up front too.
    let ok = write_world("named", "height_field = \"cpu-terrain-v1\"\n");
    let ok = ok.to_str().unwrap();
    for (args, needle) in [
        (vec!["test-render", "--scene", "tiles", "--world", ok], "only applies to `generate`"),
        (vec!["test-render", "--scene", "terrain", "--global-res", "64"], "--global-res only applies with --world, or to --perf"),
        (vec!["test-render", "--scene", "terrain", "--world", ok, "--repro", "b.repro"], "takes the world from the bundle"),
    ] {
        let e = run_str(&args).unwrap_err();
        assert!(e.contains(needle), "{args:?}: {e}");
    }
}

// spec: GEN-018
#[test]
fn stream_bundle_and_perf_take_the_world_and_perf_without_one_is_the_default_world() {
    let w = write_world("paths", "");
    let w = w.to_str().unwrap();
    let parse = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        planet::parse_args(&args).unwrap()
    };
    for args in [
        vec!["test-render", "--scene", "terrain", "--world", w, "--stream"],
        vec!["test-render", "--scene", "terrain", "--world", w, "--bundle", "b.repro"],
        vec!["test-render", "--scene", "terrain", "--world", w, "--perf"],
        vec!["test-render", "--scene", "terrain", "--world", w, "--perf-smoke", "--global-res", "64"],
        // The perf scene without a world is the default world: --global-res sizes its bake.
        vec!["test-render", "--scene", "terrain", "--perf-smoke", "--global-res", "64"],
        vec!["test-render", "--scene", "terrain", "--perf", "--seed", "3"],
    ] {
        assert!(planet::worldfield::check_options(&parse(&args)).is_ok(), "{args:?}");
    }
    // The perf scene builds the eroded field without a world file; a view, stream or bundle without one stays the reference.
    let o = parse(&["test-render", "--scene", "terrain", "--perf-smoke", "--seed", "3", "--global-res", "16"]);
    let perf = WorldTerrain::of_perf_options(&o, R).unwrap();
    assert_eq!(perf.id(), "cpu-erosion-v1");
    assert!(perf.describe().contains("16-cell bake"), "{}", perf.describe());
    let view = WorldTerrain::of_options(&o, R).unwrap();
    assert_eq!(view.id(), REFERENCE_ID);
    // Only the world differs: the same seed gives the same definition apart from the field.
    assert_eq!(perf.definition().hash(), WorldDefinition::single_planet(R, 3).hash());
}

// spec: GEN-018
#[test]
fn world_option_checks_leave_the_global_bake_scene_alone_and_refuse_seed_with_world() {
    let w = write_world("checks", "");
    let w = w.to_str().unwrap();
    let parse = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        planet::parse_args(&args).unwrap()
    };
    for args in [
        vec!["test-render", "--perf", "--scene", "global-bake", "--global-res", "64"],
        vec!["test-render", "--perf", "--scene", "global-bake", "--global-res", "64", "--world", w],
    ] {
        assert!(planet::worldfield::check_options(&parse(&args)).is_ok(), "{args:?}");
    }
    let e = planet::worldfield::check_options(&parse(&["test-render", "--scene", "terrain", "--world", w, "--seed", "3"])).unwrap_err();
    assert!(e.contains("--world") && e.contains("--seed"), "{e}");
}

// spec: GEN-018
#[test]
fn the_seed_only_world_plans_with_the_hash_of_the_direct_reference_plan() {
    let w = WorldTerrain::open(legacy_world(7, R), RES).unwrap();
    let field = w.field().unwrap();
    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, field.as_ref())[5];
    let lod = LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 4.0, ..LodParams::earth_1080p() };
    let req =
        TerrainRequest { camera, size: (256, 192), view: TerrainView::Level, lod, face_mapping: TangentWarp.id().to_string(), seed: 7 };
    let via_world = plan_terrain_field(&req, field.as_ref()).unwrap();
    let direct = planet_frame::plan_terrain(&req).unwrap();
    assert_eq!(via_world.plan_hash(), direct.plan_hash());
    assert_eq!(field.read_records(), Reference(7).read_records());
}

// spec: GEN-018
#[test]
fn a_world_builds_its_fields_from_the_base_terrain() {
    use planet_core::{Face, TileId};
    use planet_generators::chunk::CHUNK_GRID_LEVEL;

    let terrain = WorldTerrain::open(world(Some(REFERENCE_ID)), RES).unwrap();
    let field = terrain.field().unwrap();
    assert_eq!(field.implementation_id(), REFERENCE_ID);
    let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let centre = chunk.center_direction(&TangentWarp);
    let base = Reference(5).height(centre);
    assert_eq!(field.height(centre).to_bits(), base.to_bits(), "the base");
}
