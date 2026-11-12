//! The river source of the viewer (item 15 PR 8, STRM-086): the frame's river plan reads its channels through the runtime reader of the commit the layer is read under (STRM-085). The adapter holds the reader and the
//! grid the commit's channels live on; the frame crate knows neither the cache nor the commit.

use std::sync::Arc;

use planet_cache::river_read::RiverReader;
use planet_core::Face;
use planet_frame::river::{RiverGrid, RiverSource};
use planet_generators::bake::k7::Channel;

/// A [`RiverSource`] over the reader of a commit's channel graph.
pub struct ReaderRiverSource {
    reader: Arc<RiverReader>,
    grid: RiverGrid,
}

impl ReaderRiverSource {
    pub fn new(reader: Arc<RiverReader>) -> ReaderRiverSource {
        let (cells, tile, cell_m) = reader.grid_dims();
        ReaderRiverSource { reader, grid: RiverGrid { cells, tile, cell_m } }
    }

    /// The reader (its cache's bytes and decodes are the report of a flight).
    pub fn reader(&self) -> &RiverReader {
        &self.reader
    }
}

impl RiverSource for ReaderRiverSource {
    fn grid(&self) -> RiverGrid {
        self.grid
    }

    fn tile_channels(&self, face: Face, ti: u32, tj: u32) -> Result<Arc<Vec<Channel>>, String> {
        self.reader.tile_channels(face, ti, tj)
    }

    fn id(&self) -> u64 {
        self.reader.commit_id()
    }
}

/// The river plan of a frame (REND-062): every node of the terrain plan with its tile and the distance of its origin from the camera, the nearest
/// keeping a layer when the array is full. The width law is the bake's standard one.
pub fn plan_frame_rivers(
    src: &ReaderRiverSource,
    plan: &planet_frame::terrain::TerrainPlan,
) -> Result<planet_frame::river::RiverPlan, String> {
    let nodes: Vec<(u32, planet_core::TileId, f64)> = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32, n.tile, n.camera_relative_origin.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt()))
        .collect();
    plan_nodes_rivers(src, plan.cells, &nodes)
}

/// [`plan_frame_rivers`] over a list of `(index, tile, distance)` nodes of `res` cells an edge.
pub fn plan_nodes_rivers(
    src: &ReaderRiverSource,
    res: u32,
    nodes: &[(u32, planet_core::TileId, f64)],
) -> Result<planet_frame::river::RiverPlan, String> {
    use planet_frame::river::{plan_rivers, RiverRequest, FULL_RIVER_SAMPLES, MIN_RIVER_SAMPLES};
    let law = planet_generators::bake::k7::WidthLaw::STANDARD;
    plan_rivers(
        &RiverRequest {
            source: src,
            law: &law,
            res,
            min_samples: MIN_RIVER_SAMPLES,
            full_samples: FULL_RIVER_SAMPLES,
            max_layers: planet_render::RIVER_LAYERS as usize,
        },
        nodes,
    )
}
