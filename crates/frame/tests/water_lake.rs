//! Tier A, REND-065: the lake masks of the water plan. A node's mask is its samples read at their nearest cell of the commit's lake grid; a lake node is drawn only with its mask (none of
//! whose samples is the lake's: no water), the sea keeps none; the masks survive the plan's text form and a mask index the plan does not hold is refused.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_frame::look::water::WaterLook;
use planet_frame::water::{
    empty_water_plan, node_lake_mask, plan_water, WaterMask, WaterPlan, WaterRequest, WaterSource, WaterTileCache, WaterView,
};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::global::ReadError;
use planet_generators::lake_water::{lake_body, LakeCell, LakeMaskSource};
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_world_def::{LiquidKindId, LiquidTable};

const R: f64 = 6_371_000.0;

/// A lake grid of `cells` cells a face edge whose lake (face 2, index 3) covers the cells `i < wet_below` of face 2 (every face when `everywhere`).
struct Lakes {
    cells: usize,
    wet_below: i64,
    everywhere: bool,
}

impl LakeMaskSource for Lakes {
    fn cells(&self) -> usize {
        self.cells
    }
    fn lake_at(&self, face: Face, i: i64, j: i64) -> Result<Option<LakeCell>, ReadError> {
        let n = self.cells as i64;
        let inside = (0..n).contains(&i) && (0..n).contains(&j);
        let wet = inside && (self.everywhere || (face == Face(2) && i < self.wet_below));
        Ok(wet.then_some(LakeCell { face: Face(2), index: 3, level_m: 100.0 }))
    }
}

// spec: REND-065
#[test]
fn a_nodes_mask_reads_each_sample_at_its_nearest_cell_of_the_lake_grid() {
    let lakes = Lakes { cells: 64, wet_below: 32, everywhere: false };
    let body = lake_body(Face(2), 3);
    // Level 1, tile (0, 0) of face 2 spans cells 0..32: with 8 cells a node its samples lie at x = -0.5 + 4a; a = 0 lies on the face's edge and reads the border cell 0 (wet), a = 8 (x = 31.5)
    // rounds to cell 32 (dry).
    let t = TileId::new(Face(2), 1, 0, 0).unwrap();
    let m = node_lake_mask(&lakes, t, 8, body).unwrap().expect("the tile holds the lake");
    assert_eq!(m.len(), 81);
    for b in 0..9 {
        for a in 0..9 {
            assert_eq!(m[b * 9 + a], if a < 8 { 255 } else { 0 }, "sample ({a}, {b})");
        }
    }
    // The other half of the face holds no sample of the lake; another lake's body reads dry everywhere.
    assert_eq!(node_lake_mask(&lakes, TileId::new(Face(2), 1, 1, 0).unwrap(), 8, body).unwrap(), None);
    assert_eq!(node_lake_mask(&lakes, t, 8, lake_body(Face(2), 4)).unwrap(), None);
    assert_eq!(node_lake_mask(&lakes, TileId::new(Face(1), 1, 0, 0).unwrap(), 8, body).unwrap(), None);
}

/// The lake (face 2, index 3) everywhere, or the sea everywhere.
struct Everywhere(WaterBodyId);

impl WaterSource for Everywhere {
    fn water_at(&self, _dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok(Some(WaterSample { level_m: 100.0, body: self.0, kind: LiquidKindId::WATER }))
    }
}

fn plan(src: &dyn WaterSource, lakes: Option<&dyn LakeMaskSource>) -> WaterPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[0];
    let lod = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let terrain = plan_terrain(&TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: "tangent-v1".into(),
        seed: 1,
    })
    .unwrap();
    let (table, look) = (LiquidTable::earth(), WaterLook::EARTH);
    let req = WaterRequest {
        source: src,
        map: &TangentWarp,
        liquids: &table,
        look: &look,
        lod: &lod,
        time_s: 1.0,
        view: WaterView::Lit,
        cut: None,
        lakes,
    };
    let p = plan_water(&req, &mut WaterTileCache::new(), &terrain).unwrap();
    p.validate().unwrap();
    p
}

// spec: REND-065
#[test]
fn a_lake_node_carries_its_mask_a_dry_one_is_dropped_and_the_sea_keeps_none() {
    let lake = Everywhere(lake_body(Face(2), 3));
    let wet = Lakes { cells: 1 << 14, wet_below: 0, everywhere: true };
    let dry = Lakes { cells: 1 << 14, wet_below: 0, everywhere: false };
    let p = plan(&lake, Some(&wet));
    assert!(!p.nodes.is_empty());
    assert!(p.nodes.iter().all(|n| n.mask.is_some()), "every lake node has a mask");
    assert_eq!(p.masks.len(), p.nodes.len());
    assert!(p.masks.iter().all(|m| m.size == 17 && m.codes.iter().all(|&c| c == 255)));
    assert!(plan(&lake, Some(&dry)).nodes.is_empty(), "a lake node none of whose samples is the lake's carries no water");
    let sea = plan(&Everywhere(WaterBodyId::SEA), Some(&wet));
    assert!(!sea.nodes.is_empty() && sea.nodes.iter().all(|n| n.mask.is_none()) && sea.masks.is_empty(), "the sea keeps no mask");
    // Without lake masks a lake node is drawn as before (wherever W - h_morph > 0).
    let old = plan(&lake, None);
    assert!(old.nodes.iter().all(|n| n.mask.is_none()) && old.nodes.len() == p.nodes.len());
}

// spec: REND-065
#[test]
fn the_masks_survive_the_text_form_and_a_missing_mask_is_refused() {
    let lake = Everywhere(lake_body(Face(2), 3));
    let wet = Lakes { cells: 1 << 14, wet_below: 0, everywhere: true };
    let mut p = plan(&lake, Some(&wet));
    p.masks[0].codes[5] = 0;
    p.mask_dropped = 2;
    let text = p.to_text();
    let lines: Vec<Vec<&str>> = text.lines().map(|l| l.split_whitespace().collect()).collect();
    assert_eq!(WaterPlan::from_lines(&lines).unwrap(), p);
    // A plan without masks writes the old text (no wmask line, no mask field).
    let mut e = empty_water_plan(&WaterLook::EARTH);
    assert!(!e.to_text().contains("mask"));
    e.masks.push(WaterMask { size: 3, codes: vec![0; 8] });
    assert!(e.validate().unwrap_err().contains("lake mask of size 3"));
    let mut q = p.clone();
    q.masks.clear();
    assert!(q.validate().unwrap_err().contains("names lake mask"));
}

// spec: REND-065
#[test]
fn one_cache_planned_at_two_node_sizes_keeps_a_mask_per_size() {
    let lake = Everywhere(lake_body(Face(2), 3));
    let wet = Lakes { cells: 1 << 14, wet_below: 0, everywhere: true };
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[0];
    let (table, look) = (LiquidTable::earth(), WaterLook::EARTH);
    let mut cache = WaterTileCache::new();
    for cells in [16u32, 8] {
        let lod = LodParams { cells, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
        let terrain = plan_terrain(&TerrainRequest {
            camera,
            size: (128, 96),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        let req = WaterRequest {
            source: &lake,
            map: &TangentWarp,
            liquids: &table,
            look: &look,
            lod: &lod,
            time_s: 1.0,
            view: WaterView::Lit,
            cut: None,
            lakes: Some(&wet),
        };
        let p = plan_water(&req, &mut cache, &terrain).unwrap();
        p.validate().unwrap_or_else(|e| panic!("cells {cells}: {e}"));
        assert!(p.masks.iter().all(|m| m.size == cells + 1));
    }
}
