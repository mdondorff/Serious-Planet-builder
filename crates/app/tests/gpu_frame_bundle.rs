//! Tier C: the repro bundle of a viewer frame (F9, TEST-008): a frame with a stand-in node, a lit part and a water part is written,
//! and `test-render --repro` rebuilds the world from the bundle, draws the recorded plan and reports the drawn nodes beside the
//! plan of the request alone.

use planet::skeleton::frame_bundle;
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pipeline::DrawKind;
use planet_frame::water::{plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, DrawnFrame, LodParams, TerrainRequest, TerrainView};
use planet_testkit::read_png;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

// spec: TEST-008, VIEW-003
#[test]
fn a_frame_bundle_replays_the_drawn_plan_with_its_lit_part_and_its_water() {
    let world = WorldTerrain::open(WorldDefinition::single_planet(R, 5), 64).unwrap().bounding_water();
    let field = world.field().unwrap();
    let lod = LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 4.0, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    // A scripted view that shows sea: the first whose plan carries water nodes and a node finer than the floor.
    let mut found = None;
    for (name, camera) in planet_frame::scripted_views_field(R, 4.0 / 3.0, field.as_ref()) {
        let request = TerrainRequest {
            camera,
            size: (256, 192),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 5,
        };
        let plan = plan_terrain_field(&request, field.as_ref()).unwrap();
        let water = world
            .with_water(|f| {
                plan_water(
                    &WaterRequest {
                        source: f,
                        map: &TangentWarp,
                        liquids: &table,
                        look: &look.water,
                        lod: &lod,
                        time_s: 2.0,
                        view: WaterView::Lit,
                        cut: None,
                    },
                    &mut WaterTileCache::new(),
                    &plan,
                )
            })
            .unwrap()
            .unwrap()
            .unwrap();
        if !water.nodes.is_empty() && plan.nodes.iter().any(|n| n.level >= 4) {
            found = Some((name, request, plan, water));
            break;
        }
    }
    let (name, request, mut plan, water) = found.expect("one of the scripted views looks at the sea");
    let water_nodes = water.nodes.len();
    // The frame as the pipelined loop drew it: a node standing in for children that were not resident.
    let i = plan.nodes.iter().position(|n| n.level >= 4).expect("a node below the floor level has a parent at the floor or finer");
    let parent = plan.nodes[i].tile.parent().unwrap();
    plan.nodes[i].tile = parent;
    plan.nodes[i].level = parent.level();
    plan.nodes[i].morph_start_m = 1.0e30;
    plan.nodes[i].morph_end_m = 1.0e30;
    plan.nodes[i].mesh_key = planet_generators::compact::compact_tile_field(field.as_ref(), &TangentWarp, parent, 16, R).key();
    plan.lit = Some(LitPlan::fixed(14.0));
    plan.water = Some(water);
    let bundle =
        frame_bundle("test".into(), &world, true, request, plan, DrawnFrame { kind: DrawKind::Stale, substituted: 1, blank: 0 }).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/frame-bundle-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (path, png) = (dir.join("frame-1.txt"), dir.join("replay.png"));
    std::fs::write(&path, bundle.to_text()).unwrap();
    let log = run(&["test-render", "--scene", "terrain", "--repro", path.to_str().unwrap(), "--out", png.to_str().unwrap()]).unwrap();
    assert!(log.contains("replay: frame drawn Stale") && log.contains("1 substituted"), "{name}: {log}");
    assert!(log.contains(&format!("{water_nodes} water nodes")), "{name}: the replay draws the recorded water: {log}");
    let image = read_png(&png).unwrap();
    assert_eq!((image.width, image.height), (256, 192));
    let first = image.pixel(0, 0);
    let varied = (0..image.height).any(|y| (0..image.width).any(|x| image.pixel(x, y) != first));
    assert!(varied, "the replayed frame is not one flat colour");
    // A bundle with its plan edited is refused by its hash.
    let text = bundle.to_text();
    let tampered = text.replacen(&format!("node {:016x}", parent.raw()), "node 0000000000000000", 1);
    std::fs::write(&path, tampered).unwrap();
    let e = run(&["test-render", "--scene", "terrain", "--repro", path.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("corrupt") || e.contains("invalid tile"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
