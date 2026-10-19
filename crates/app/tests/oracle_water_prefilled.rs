//! Tier B: a pre-filled tile (read from the tile pack of a stored bake, STRM-021) leaves the same minimum in the `WaterBook` as a
//! generated one, and the plan cuts with it (ATMO-020). A small stored bake of 256 cells with the 384 tiles of level 3 pre-filled
//! (the render floor 3, LOD-011), software only.

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::look::file::LookParams;
use planet_frame::water::{plan_water, tile_above_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_cached, scripted_views, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

// spec: ATMO-020
#[test]
fn the_job_of_a_prefilled_tile_leaves_the_minimum_of_the_tile_it_read_and_the_plan_cuts_with_it() {
    let store = std::env::temp_dir().join(format!("planet-prefilled-min-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    let built =
        run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", "256", "--threads", "2", "--prefill-levels", "3"])
            .unwrap();
    assert!(built.contains("384 tiles, 384 written"), "{built}");
    let o = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options");
    let world = WorldTerrain::open_store(WorldDefinition::single_planet(R, o.seed), &store, 256).expect("the store opens").bounding_water();
    let field = world.field().unwrap();
    let (cells, max_level) = (32u32, 22u8);
    // An orbital plan: the nodes of the floor (level 3) and finer, the pre-filled tiles of level 3 among them.
    let lod = LodParams {
        cells,
        viewport_h_px: 1440.0,
        tau_px: 8.0,
        max_level,
        radius_m: R,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    // Every pre-filled tile (level 3, 384 tiles): wet ones leave their minimum, dry ones none.
    let (mut wet, mut dry) = (0, 0);
    for face in 0..6u8 {
        for level in 3..=3u8 {
            for x in 0..(1u32 << level) {
                for y in 0..(1u32 << level) {
                    let tile = TileId::new(planet_core::Face(face), level, x, y).unwrap();
                    let t = world.prefilled_tile(field.as_ref(), tile, cells, max_level).expect("the pack holds the tiles of level 3");
                    world.fill_water(tile, &t);
                    let book = world.water_book();
                    if book.get(tile).flatten().is_some() {
                        wet += 1;
                        assert_eq!(book.min_height(tile), Some(t.min_height_m()), "{tile:?}");
                        // The pack holds the tile the streamer generates, bit for bit.
                        let made = compact_tile_field(field.as_ref(), &TangentWarp, tile, cells, R);
                        assert_eq!(t, made, "{tile:?}");
                    } else {
                        dry += 1;
                        assert_eq!(book.min_height(tile), None, "{tile:?}");
                    }
                }
            }
        }
    }
    assert_eq!(wet + dry, 384);
    assert!(wet > 0, "no pre-filled tile carries water");
    let mut sel = SelectionCache::new();
    let mut prefilled = 0;
    let mut cut_by_minimum = 0;
    for index in [0usize, 1, 2, 3] {
        let (_, camera) = scripted_views(R, 16.0 / 9.0, 1)[index];
        let req = TerrainRequest {
            camera,
            size: (2560, 1440),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: field.seed(),
        };
        let plan = plan_terrain_cached(&req, field.as_ref(), &mut sel).expect("terrain plan");
        // The job of each node's tile, as MeshSource does for a pre-filled tile: read it from the pack, then fill the water.
        for n in &plan.nodes {
            let Some(t) = world.prefilled_tile(field.as_ref(), n.tile, cells, max_level) else { continue };
            prefilled += 1;
            world.fill_water(n.tile, &t);
            if let Some(m) = world.water_book().min_height(n.tile) {
                // The pack's tile is the generated tile bit for bit, so is its minimum.
                let made = compact_tile_field(field.as_ref(), &TangentWarp, n.tile, cells, R);
                assert_eq!(t, made, "{:?}: the pack holds the tile the streamer generates", n.tile);
                assert_eq!(m, made.min_height_m(), "{:?}: the book's minimum is the pre-filled tile's", n.tile);
            }
        }
        let table = world.definition().liquids().unwrap();
        let look = LookParams::default();
        let make = |cache: &mut WaterTileCache| {
            world
                .with_water(|f| {
                    plan_water(
                        &WaterRequest {
                            source: f,
                            map: &TangentWarp,
                            liquids: &table,
                            look: &look.water,
                            lod: &lod,
                            time_s: 1.0,
                            view: WaterView::Lit,
                            cut: Some(field.as_ref()),
                        },
                        cache,
                        &plan,
                    )
                })
                .unwrap()
                .unwrap()
                .unwrap()
        };
        let with = make(&mut WaterTileCache::with_book(world.water_book()));
        let mut cache = WaterTileCache::with_book(world.water_book());
        let again = make(&mut cache);
        assert_eq!(with, again);
        let loose = make(&mut WaterTileCache::new());
        // Every node the loose cut kept and the exact cut dropped is a tile whose stored minimum is above the level plus the rise.
        let rise =
            planet_frame::look::water::LiquidAppearance::new(0, table.get(planet_world_def::LiquidKindId::WATER).unwrap(), &look.water)
                .amplitude_sum();
        for l in &loose.nodes {
            let tile: TileId = plan.nodes[l.node as usize].tile;
            let kept = with.nodes.iter().any(|w| w.node == l.node);
            let expect_cut =
                world.water_book().min_height(tile).is_some_and(|m| tile_above_water(f64::from(m), f64::from(l.level_m), rise));
            assert_eq!(!kept, expect_cut, "{tile:?}: kept {kept}, stored minimum {:?}", world.water_book().min_height(tile));
            cut_by_minimum += u32::from(expect_cut);
        }
        assert_eq!(cache.cut_exact, cut_by_minimum_in(&loose, &with) as u64);
    }
    println!("{prefilled} pre-filled tiles read, {cut_by_minimum} nodes cut by their stored minimum");
    assert!(prefilled > 0, "the plans use no pre-filled tile: the pack path is not exercised");
    let _ = std::fs::remove_dir_all(&store);
}

fn cut_by_minimum_in(loose: &planet_frame::water::WaterPlan, with: &planet_frame::water::WaterPlan) -> usize {
    loose.nodes.len() - with.nodes.len()
}
