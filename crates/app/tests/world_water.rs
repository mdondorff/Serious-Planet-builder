//! Tier A/B: the water of a world (ADR 0041, ADR 0051 point 6): the sea of an in-memory bake answers the water field through the
//! per-tile query the plan uses, and the cost of that query per tile is measured (ad hoc, `--ignored --nocapture`).

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::WaterTileCache;
use planet_frame::{plan_terrain_threaded, scripted_views, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_world_def::WorldDefinition;

fn world(res: u32) -> WorldTerrain {
    WorldTerrain::open(WorldDefinition::single_planet(6_371_000.0, 5), res).expect("the world opens")
}

// spec: GEN-023, ATMO-012
#[test]
fn a_world_with_a_bake_has_the_sea_of_its_definition_and_the_per_tile_query_finds_it() {
    let w = world(128);
    assert!(w.has_water());
    let sea = w.definition().sea(planet_world_def::BodyId::PLANET).unwrap();
    // Sample the sphere: some directions are sea (level 0), some are not.
    let (mut wet, mut dry) = (0, 0);
    let res = w
        .with_water(|f| {
            for k in 0..400u32 {
                let u = |i: u64| planet_core::hash::unit_f64(planet_core::hash::hash_u64s(9, &[u64::from(k), i])) - 0.5;
                let d = Vec3::new(u(0), u(1), u(2)).normalized();
                match f.water_at(d).unwrap() {
                    Some(s) => {
                        assert_eq!(s.level_m, sea.level_m);
                        wet += 1;
                    }
                    None => dry += 1,
                }
            }
        })
        .unwrap();
    assert!(res.is_some());
    assert!(wet > 40 && dry > 40, "an ocean world: {wet} wet and {dry} dry of 400 directions");
}

/// The tiles of the scripted views over the world, planned the way the viewer plans them.
fn view_tiles(w: &WorldTerrain) -> Vec<TileId> {
    let field = w.field().unwrap();
    let lod =
        LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 22, max_height_m: 12_000.0, ..LodParams::earth_1080p() };
    let mut tiles = std::collections::BTreeSet::new();
    for (_, camera) in scripted_views(6_371_000.0, 16.0 / 9.0, 5) {
        let plan = plan_terrain_threaded(
            &TerrainRequest {
                camera,
                size: (2560, 1440),
                view: TerrainView::Level,
                lod: lod.clone(),
                face_mapping: "tangent-v1".into(),
                seed: 5,
            },
            field.as_ref(),
            &mut SelectionCache::new(),
            4,
        )
        .unwrap();
        tiles.extend(plan.nodes.iter().map(|n| n.tile));
    }
    tiles.into_iter().collect()
}

// spec: ATMO-012
#[test]
#[ignore = "ad hoc measurement of the per-tile water query (ADR 0051 point 6); run with --ignored --nocapture"]
fn the_cost_of_the_per_tile_water_query() {
    for res in [256u32, 1024] {
        let w = world(res);
        let tiles = view_tiles(&w);
        let mut cache = WaterTileCache::new();
        let started = std::time::Instant::now();
        let wet = w
            .with_water(|f| tiles.iter().filter(|t| cache.tile_water(f, &TangentWarp, **t, 6_371_000.0).unwrap().is_some()).count())
            .unwrap()
            .unwrap();
        let total = started.elapsed();
        println!(
            "bake {res}: {} tiles of the scripted views ({wet} wet), {} field queries, {:.1} ms in all: {:.1} us per tile, {:.2} us per query (cache recorded {:.1} ms)",
            tiles.len(),
            cache.queries,
            total.as_secs_f64() * 1e3,
            total.as_secs_f64() * 1e6 / tiles.len() as f64,
            total.as_secs_f64() * 1e6 / cache.queries as f64,
            cache.query_ns as f64 / 1e6
        );
        // A warm plan asks nothing.
        let again = std::time::Instant::now();
        w.with_water(|f| {
            tiles.iter().for_each(|t| {
                let _ = cache.tile_water(f, &TangentWarp, *t, 6_371_000.0).unwrap();
            })
        })
        .unwrap();
        println!("  warm: {:.3} ms for {} tiles", again.elapsed().as_secs_f64() * 1e3, tiles.len());
    }
}

// spec: TEST-024
#[test]
fn the_ocean_probe_reads_a_fixed_grid_and_a_world_without_a_store_has_nothing_to_check() {
    let w = world(128);
    let (read, sea) = w.ocean_probe().unwrap().expect("the bake has an ocean channel");
    assert_eq!(read, 288, "12 latitudes by 24 longitudes");
    assert!(sea > 20 && sea < 268, "an ocean world, neither all land nor all sea: {sea} of {read}");
    assert_eq!(w.ocean_probe().unwrap(), Some((read, sea)), "a fixed grid gives the same answer every time");
    let e = w.store_check(1, 32, 22).unwrap_err();
    assert!(e.contains("no tile pack"), "{e}");
}

// spec: TEST-024
#[test]
fn a_store_check_names_every_problem_of_a_store_and_passes_a_complete_one() {
    use planet::worldfield::StoreCheck;
    let good = StoreCheck {
        source: "store S, 16384 cells".into(),
        res: 16384,
        levels: 4,
        ocean: Some((288, 140)),
        compact: (2046, 2046),
        vertex: (0, 2046),
    };
    assert!(good.problems().is_empty(), "the vertex form is information only: {:?}", good.problems());
    let text = good.lines().join("\n");
    assert!(text.contains("ocean channel: present, 288 directions read, 140 sea and 148 land"), "{text}");
    assert!(text.contains("levels 0-4: compact form 2046 of 2046 present") && text.contains("vertex form 0 of 2046"), "{text}");
    assert!(
        text.contains(&format!("GLOBAL_VERSION {}", planet_generators::global::GLOBAL_VERSION))
            && text.contains(&format!("MESH_VERSION {}", planet_generators::mesh::MESH_VERSION)),
        "{text}"
    );
    let no_ocean = StoreCheck { ocean: None, ..good.clone() };
    assert!(no_ocean.problems()[0].contains("no ocean channel") && no_ocean.problems()[0].contains("generate --global-dir"));
    for (sea, word) in [(0usize, "land"), (288, "sea")] {
        let p = StoreCheck { ocean: Some((288, sea)), ..good.clone() }.problems();
        assert!(p.len() == 1 && p[0].contains(&format!("reads {word} on all 288")), "{p:?}");
    }
    let short = StoreCheck { compact: (2000, 2046), ..good.clone() }.problems();
    assert!(
        short.len() == 1 && short[0].contains("2000 of 2046") && short[0].contains("MESH_VERSION") && short[0].contains("--prefill-dir"),
        "{short:?}"
    );
    let both = StoreCheck { ocean: None, compact: (0, 2046), ..good }.problems();
    assert_eq!(both.len(), 2);
}

// spec: GEN-024
#[test]
fn an_in_memory_world_answers_the_tiles_at_the_block_level_from_a_summary_like_a_stored_one() {
    let w = world(128);
    let radius = 6_371_000.0;
    let (mut coarse, mut wet) = (0, 0);
    w.with_water(|f| {
        for face in 0..6u8 {
            let t = TileId::new(planet_core::Face(face), 0, 0, 0).unwrap();
            if let Some(a) = f.coarse_tile_water(&TangentWarp, t, radius).unwrap() {
                coarse += 1;
                wet += usize::from(a.is_some());
            }
            // A finer tile is not answered by the summary: the block of a 128-cell bake is the face.
            let child = t.children().unwrap()[0];
            assert!(f.coarse_tile_water(&TangentWarp, child, radius).unwrap().is_none());
        }
    })
    .unwrap();
    assert_eq!(coarse, 6);
    assert!(wet >= 1, "some face of the ocean world carries sea");
}
