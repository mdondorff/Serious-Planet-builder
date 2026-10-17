//! What a tile counts in the per-frame upload budget (STRM-006, TEST-012): the size it has in the sink's format, not the vertex
//! size. A sink that writes compact tiles (the renderer's) must count 8,840 B a tile at cells 32, or the 1 MiB cap is wrong by 6x.

use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::managed::TileSink;
use planet_generators::compact::{compact_tile_field, slot_bytes, RenderTile};
use planet_generators::height::Reference;
use planet_generators::mesh::{tile_mesh_field, MeshVertex};
use planet_streaming::SlotRef;

const CELLS: u32 = 32;
const RADIUS_M: f64 = 6_371_000.0;

/// A sink that does not override `upload_bytes`: it counts what the trait says, as the renderer's sink does.
struct DefaultSink;

impl TileSink for DefaultSink {
    fn upload_at(&mut self, _tile: &RenderTile, _slot: SlotRef) -> Result<(), String> {
        Ok(())
    }
    fn forget(&mut self, _tile: TileId) {}
}

fn tile_id() -> TileId {
    TileId::new(planet_core::Face(0), 5, 3, 7).unwrap()
}

// spec: STRM-006, TEST-012
#[test]
fn a_compact_tile_counts_at_its_slot_size_and_a_vertex_fallback_tile_at_its_vertex_size() {
    let field = Reference(1);
    let compact = RenderTile::Compact(compact_tile_field(&field, &TangentWarp, tile_id(), CELLS, RADIUS_M));
    let mesh = tile_mesh_field(&field, &TangentWarp, tile_id(), CELLS, RADIUS_M);
    let vertex_bytes = mesh.vertices.len() as u64 * MeshVertex::GPU_STRIDE;
    let vertex = RenderTile::Vertex(mesh);
    let sink = DefaultSink;

    assert_eq!(sink.upload_bytes(&compact), slot_bytes(CELLS), "a compact tile counts at texels plus record");
    assert_eq!(sink.upload_bytes(&compact), 8_840, "8,840 B at cells 32");
    assert_eq!(sink.upload_bytes(&vertex), vertex_bytes, "a vertex tile counts at its vertex size");
    assert!(vertex_bytes > 5 * 8_840, "the two sizes differ by about 6x, which is what a wrong count would cost: {vertex_bytes}");
    // Whole tiles under the cap: 118 compact tiles of 8,840 B in 1 MiB (the learnings entry of the upload budget).
    assert_eq!((1u64 << 20) / sink.upload_bytes(&compact), 118);
}
