//! GEN-034, GEN-035: the chunk scope, its key, the chunk grid with its apply level and the lattice of its cells.

use std::collections::BTreeMap;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::chunk::{apply_level, ChunkId, CHUNK_GRID_LEVEL, TILE_CELLS};
use planet_generators::stage::{scope_fits, FieldGraph, FieldSpec, OutputContext, Scope, Stage};
use planet_world_def::BodyId;

const R: f64 = 6_371_000.0;

fn spec(id: &'static str, stage: Stage) -> FieldSpec {
    FieldSpec { id, stage, reads: &[], version: 1, implementation_id: "cpu-test", halo_fraction: 0.0 }
}

fn ctx() -> OutputContext<'static> {
    OutputContext { body: BodyId::PLANET, surface_id: "sphere", inputs_hash: 99, face_mapping_id: "tangent-v1", resolution: 32 }
}

fn chunk(x: u32, y: u32, halo_m: f64, cap: u32) -> ChunkId {
    ChunkId::new(TileId::new(Face(2), CHUNK_GRID_LEVEL, x, y).unwrap(), halo_m, cap).unwrap()
}

fn key(g: &FieldGraph, scope: Scope) -> u64 {
    g.output_key("fine", scope, &ctx(), &BTreeMap::new())
}

// spec: GEN-034
#[test]
fn a_chunk_key_changes_with_the_grid_level_the_halo_and_the_step_cap_and_never_equals_a_tile_key() {
    let g = FieldGraph::new(&[spec("fine", Stage::Regional)]).unwrap();
    let base = key(&g, Scope::Chunk(chunk(3, 7, 400.0, 64)));
    assert_eq!(base, key(&g, Scope::Chunk(chunk(3, 7, 400.0, 64))), "the key is a pure function of the chunk");
    assert_ne!(base, key(&g, Scope::Chunk(chunk(3, 7, 401.0, 64))), "halo");
    assert_ne!(base, key(&g, Scope::Chunk(chunk(3, 7, 400.0, 65))), "step cap");
    assert_ne!(base, key(&g, Scope::Chunk(chunk(4, 7, 400.0, 64))), "chunk position");
    let coarser = ChunkId::new(TileId::new(Face(2), CHUNK_GRID_LEVEL - 1, 3, 7).unwrap(), 400.0, 64).unwrap();
    assert_ne!(base, key(&g, Scope::Chunk(coarser)), "grid level");
    let same_raw = TileId::new(Face(2), CHUNK_GRID_LEVEL, 3, 7).unwrap();
    assert_ne!(base, key(&g, Scope::Tile(same_raw)), "a tile with the chunk's own id");
    let neg = ChunkId::new(same_raw, -0.0, 64).unwrap();
    let pos = ChunkId::new(same_raw, 0.0, 64).unwrap();
    assert_eq!(key(&g, Scope::Chunk(neg)), key(&g, Scope::Chunk(pos)), "-0.0 is 0.0");
}

// spec: GEN-034
#[test]
fn a_chunk_scope_fits_regional_fields_only_and_a_bad_halo_or_cap_is_refused() {
    let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL, 0, 0).unwrap();
    let c = Scope::Chunk(ChunkId::new(tile, 400.0, 64).unwrap());
    assert!(scope_fits(Stage::Regional, c));
    assert!(!scope_fits(Stage::GlobalCoarse, c));
    assert!(!scope_fits(Stage::Local, c));
    for (halo, cap) in [(f64::NAN, 64), (f64::INFINITY, 64), (-1.0, 64), (400.0, 0)] {
        assert!(ChunkId::new(tile, halo, cap).is_none(), "halo {halo} cap {cap}");
    }
}

// spec: GEN-035
#[test]
fn chunk_ids_of_directions_at_face_edges_and_corners_are_tiles_of_the_grid_level_and_neighbours_differ() {
    let probes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(-1.0, -1.0, -1.0),
    ];
    let mut seen = Vec::new();
    for d in probes {
        let c = ChunkId::of_direction(&TangentWarp, d.normalized(), 400.0, 64).unwrap();
        assert_eq!(c.tile().level(), CHUNK_GRID_LEVEL);
        assert_eq!(c.tile(), TileId::from_direction(&TangentWarp, d.normalized(), CHUNK_GRID_LEVEL));
        seen.push(c);
    }
    seen.dedup();
    assert_eq!(seen.len(), probes.len(), "five well separated directions are five chunks");
}

// spec: GEN-035
#[test]
fn the_apply_level_is_the_coarsest_level_whose_vertex_spacing_is_at_most_twice_the_cell() {
    assert_eq!(TILE_CELLS, 32);
    assert_eq!(apply_level(8.0, R, TILE_CELLS), Some(15), "8 m cells: spacing 9.6 m at level 15, 19.1 m at 14");
    assert_eq!(apply_level(4.0, R, TILE_CELLS), Some(16), "4 m cells: 4.8 m at level 16, 9.6 m at 15");
    assert_eq!(apply_level(10.0, R, TILE_CELLS), Some(14), "10 m cells: spacing 19.1 m at level 14 is under 20 m");
    assert_eq!(apply_level(1e-9, R, TILE_CELLS), Some(planet_core::tile::MAX_LEVEL), "a finer cell than any level supports clamps");
    for (cell, radius, cells) in [(f64::NAN, R, 32), (0.0, R, 32), (-8.0, R, 32), (8.0, 0.0, 32), (8.0, f64::INFINITY, 32), (8.0, R, 0)] {
        assert_eq!(apply_level(cell, radius, cells), None, "cell {cell} radius {radius} cells {cells}");
    }
    // The apply level never gets coarser with a finer cell.
    let mut last = 0;
    for cell in [64.0, 32.0, 16.0, 8.0, 4.0, 2.0, 1.0] {
        let l = apply_level(cell, R, TILE_CELLS).unwrap();
        assert!(l >= last, "cell {cell}: level {l} after {last}");
        last = l;
    }
}

// spec: GEN-035
#[test]
fn the_cells_of_a_chunk_are_the_cells_of_its_tile_and_the_chunks_a_tile_reads_start_with_its_own() {
    use planet_generators::chunkfield::{chunks_read_by, Cell, ChunkLattice};
    let lat = ChunkLattice::new(16).unwrap();
    let per = lat.cells_per_chunk();
    for tile in [
        TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap(),
        TileId::new(Face(0), CHUNK_GRID_LEVEL, 2047, 5).unwrap(),
        TileId::new(Face(5), CHUNK_GRID_LEVEL, 0, 0).unwrap(),
    ] {
        for (i, j) in [(0, 0), (1, 0), (per - 1, per - 1), (per / 2, 3)] {
            let cell = Cell { face: tile.face().0, i: tile.x() * per + i, j: tile.y() * per + j };
            assert_eq!(TileId::from_direction(&TangentWarp, lat.direction(cell), CHUNK_GRID_LEVEL), tile, "cell ({i}, {j}) of {tile:?}");
            assert_eq!(lat.cell_at(lat.direction(cell)), cell, "a cell holds its own centre");
        }
        let own = (tile.face().0, tile.x(), tile.y());
        assert_eq!(chunks_read_by(&lat, CHUNK_GRID_LEVEL, 0, tile), vec![own], "a tile of the grid level reads its own chunk");
        let reads = chunks_read_by(&lat, CHUNK_GRID_LEVEL, i64::from(per), tile);
        assert_eq!(reads[0], own, "own chunk first");
        assert!(reads.len() > 1 && reads.windows(2).skip(1).all(|w| w[0] < w[1]), "the neighbours follow in a fixed order: {reads:?}");
    }
    // A tile coarser than the grid reads no chunk.
    let coarse = TileId::new(Face(2), CHUNK_GRID_LEVEL - 1, 5, 5).unwrap();
    assert!(chunks_read_by(&lat, CHUNK_GRID_LEVEL, 0, coarse).is_empty());
}
