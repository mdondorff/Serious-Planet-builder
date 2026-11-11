//! The channel feed over a commit's graph blocks (item 15 PR 7, STRM-084; architect D51): `channels_near` reads the blocks a window touches through the commit, turns their pieces into K7 channels (points in
//! the face's local cell-index space, the bed level and `Q_w` dequantised: the burn and the drawing read the same values) and hands them to the writer. A face edge is the limit of a block's pieces: a
//! channel of a neighbouring face is not mapped into this face's plane.

use std::sync::Arc;

use planet_core::Face;
use planet_generators::bake::channel_graph::{bed_value, q_value};
use planet_generators::bake::k7::Channel;

use crate::basin_table::BasinSpec;
use crate::graph_store::{load_graph_block, GraphSpec};
use crate::l_writer::{ChannelFeed, Commit};
use crate::pack::TileStore;

/// The feed of a commit's channel graph.
pub struct ChannelGraphFeed {
    store: Arc<TileStore>,
    commit: Commit,
    spec: GraphSpec,
    /// Local cells to an H1 cell.
    ratio: i64,
}

impl ChannelGraphFeed {
    /// The feed of `commit` in `store`: an error when the commit holds no graph.
    pub fn new(store: Arc<TileStore>, commit: Commit) -> Result<ChannelGraphFeed, String> {
        if commit.graph_h1_cells == 0 {
            return Err(format!("commit {:016x} holds no channel graph", commit.id));
        }
        let s = commit.spec;
        let tiles_per_edge = s.cells / s.tile;
        let spec = GraphSpec {
            // The hydrology parameters are in the keys the commit names; a block is read by that key.
            basin: BasinSpec {
                hydro: planet_generators::bake::hydro::HydroParams::default(),
                tile_cells: s.tile,
                tiles_per_edge,
                base_id: s.base_id,
                inputs_id: s.inputs_id,
            },
            h1_cells: commit.graph_h1_cells,
        };
        let per_block = spec.h1_per_block();
        if per_block == 0 || !s.tile.is_multiple_of(per_block) {
            return Err(format!("{} local cells a tile are not a whole number of the {per_block} H1 cells of a block", s.tile));
        }
        Ok(ChannelGraphFeed { store, ratio: i64::from(s.tile / per_block), commit, spec })
    }
}

impl ChannelFeed for ChannelGraphFeed {
    fn q_is_discharge(&self) -> bool {
        // The writer refuses a graph in area units, so a commit's graph is a discharge.
        true
    }

    /// The pieces of the blocks that meet the window. A block the store lacks or that is damaged stops the bake with a message naming it: a commit is written after its records.
    fn channels_near(&self, face: Face, i0: i64, j0: i64, w: i64) -> Vec<Channel> {
        let tile = i64::from(self.commit.spec.tile);
        let n = i64::from(self.spec.basin.tiles_per_edge);
        let (b0i, b1i) = ((i0.div_euclid(tile)).clamp(0, n - 1), ((i0 + w - 1).div_euclid(tile)).clamp(0, n - 1));
        let (b0j, b1j) = ((j0.div_euclid(tile)).clamp(0, n - 1), ((j0 + w - 1).div_euclid(tile)).clamp(0, n - 1));
        let mut out = Vec::new();
        for bj in b0j..=b1j {
            for bi in b0i..=b1i {
                let Some(&key) = self.commit.graph.get(&(face.0, bi as u32, bj as u32)) else { continue };
                let block = load_graph_block(&self.store, &self.spec, face.0, bi as u32, bj as u32, key)
                    .unwrap_or_else(|e| panic!("{e}"))
                    .unwrap_or_else(|| {
                        panic!(
                            "commit {:016x} names the graph block {}/{bi}/{bj} (key {key}) but the store holds no record of it",
                            self.commit.id, face.0
                        )
                    });
                let half = (self.ratio - 1) as f64 / 2.0;
                for p in &block.pieces {
                    out.push(Channel {
                        points: p.coords().iter().map(|&(i, j)| ((self.ratio * i) as f64 + half, (self.ratio * j) as f64 + half)).collect(),
                        levels: p.bed.iter().map(|&b| bed_value(b)).collect(),
                        q: p.q.iter().map(|&q| q_value(q)).collect(),
                        q_unit: block.q_unit,
                    });
                }
            }
        }
        out
    }
}
