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
}
