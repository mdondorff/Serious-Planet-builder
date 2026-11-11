//! The runtime's read of the channel graph (item 15 PR 8, STRM-085): the channels of an L tile, read through the commit the layer and the basin tables are read under, decoded once while they stay in a bounded
//! least-recently-used cache, and every failure an error that names the tile. The frame's river plan asks this for plain data (CON-03); nothing here draws.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use planet_core::Face;
use planet_generators::bake::k7::{Channel, DISC_RADIUS_MAX};

use crate::graph_feed::ChannelGraphFeed;
use crate::l_writer::Commit;
use crate::pack::TileStore;

type Built = (HashMap<(u8, u32, u32), Arc<Vec<Channel>>>, VecDeque<(u8, u32, u32)>);

/// The channels of the tiles of one commit.
pub struct RiverReader {
    feed: ChannelGraphFeed,
    capacity: usize,
    built: Mutex<Built>,
    decodes: std::sync::atomic::AtomicU64,
}

impl RiverReader {
    /// The reader of `commit` in `store`, with `capacity` tiles cached: an error when the commit holds no channel graph.
    pub fn new(store: Arc<TileStore>, commit: Commit, capacity: usize) -> Result<RiverReader, String> {
        Ok(RiverReader {
            feed: ChannelGraphFeed::new(store, commit)?,
            capacity: capacity.max(1),
            built: Mutex::new((HashMap::new(), VecDeque::new())),
            decodes: 0.into(),
        })
    }

    /// The identity of the commit this reader reads: equal to the layer's and the tables' (CON-22).
    pub fn commit_id(&self) -> u64 {
        self.feed.commit().id
    }

    /// Tiles decoded so far (a hit is not a decode; a tile that left the cache and comes back is).
    pub fn decodes(&self) -> u64 {
        self.decodes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Tiles held.
    pub fn len(&self) -> usize {
        self.built.lock().expect("built lock").0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The channels that can touch the core of tile `(ti, tj)` of `face`: the pieces of the blocks that meet the core widened by the widest trench (`DISC_RADIUS_MAX` cells), in the face's cell-index space.
    /// An error names the tile and the commit when a block the commit names is missing or damaged.
    pub fn tile_channels(&self, face: Face, ti: u32, tj: u32) -> Result<Arc<Vec<Channel>>, String> {
        let id = (face.0, ti, tj);
        {
            let mut g = self.built.lock().expect("built lock");
            if let Some(t) = g.0.get(&id).cloned() {
                g.1.retain(|x| *x != id);
                g.1.push_back(id);
                return Ok(t);
            }
        }
        let tile = self.feed.tile_cells();
        let reach = DISC_RADIUS_MAX;
        let channels = self
            .feed
            .try_channels_near(face, i64::from(ti) * tile - reach, i64::from(tj) * tile - reach, tile + 2 * reach)
            .map_err(|e| format!("river tile {}/{ti}/{tj}: {e}", face.0))?;
        let channels = Arc::new(channels);
        self.decodes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut g = self.built.lock().expect("built lock");
        if g.0.insert(id, channels.clone()).is_none() {
            g.1.push_back(id);
        }
        while g.0.len() > self.capacity {
            if let Some(old) = g.1.pop_front() {
                g.0.remove(&old);
            }
        }
        Ok(channels)
    }
}
