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
    /// The L tiles of the commit: a river is drawn only over them (REND-062; the graph's margin blocks lie over ground the commit did not carve).
    tiles: std::collections::BTreeSet<(u8, u32, u32)>,
    /// The lakes of the same commit: a river ends at their water (REND-065).
    lakes: Option<Arc<planet_cache::lake_read::CommitLakes>>,
}

impl ReaderRiverSource {
    pub fn new(reader: Arc<RiverReader>) -> ReaderRiverSource {
        let (cells, tile, cell_m) = reader.grid_dims();
        let tiles = reader.commit().tiles.keys().copied().collect();
        ReaderRiverSource { reader, grid: RiverGrid { cells, tile, cell_m }, tiles, lakes: None }
    }

    /// The same source whose rivers end at the water of `lakes` (the same commit's, CON-22).
    pub fn with_lakes(mut self, lakes: Arc<planet_cache::lake_read::CommitLakes>) -> ReaderRiverSource {
        self.lakes = Some(lakes);
        self
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

    fn has_tile(&self, face: Face, ti: u32, tj: u32) -> bool {
        self.tiles.contains(&(face.0, ti, tj))
    }

    fn lake_wet(&self, face: Face, i: i64, j: i64) -> Result<bool, String> {
        match &self.lakes {
            Some(l) => Ok(l.lake_cell(face, i, j)?.is_some()),
            None => Ok(false),
        }
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
            max_layers: planet_render::RIVER_SLOTS as usize,
        },
        nodes,
    )
}

/// What one frame's river lookup found (REND-064): the plan, the nodes whose coverage was not built yet (they draw no river this frame), the builds queued for the background and the cache's bytes.
#[derive(Clone, Debug)]
pub struct CoverageFrame {
    pub plan: planet_frame::river::RiverPlan,
    pub not_ready: u32,
    pub queued: u32,
    pub cache_bytes: usize,
}

/// The river coverage of the viewer and of the river scene (REND-064): the frame's river plan is a lookup in a byte-bounded cache of coverage by tile; the tiles that are missing are built OFF the frame path, as
/// `Prefetch` jobs of the terrain share, and a node whose coverage is not ready draws no river until it is. Without a share (a test, a tool) a missing tile is built at once, in the call.
pub struct RiverCoverage {
    src: Arc<dyn RiverSource + Send + Sync>,
    cache: Arc<std::sync::Mutex<planet_frame::river::RiverCache>>,
    share: Option<planet_streaming::jobs::Share>,
    inflight: Arc<std::sync::Mutex<std::collections::HashSet<planet_core::TileId>>>,
    failure: Arc<std::sync::Mutex<Option<String>>>,
    // Keeps the job system of a scene alive as long as the coverage is used.
    _system: Option<Arc<planet_streaming::jobs::JobSystem>>,
}

/// The bytes of coverage a run keeps (a node is 1,089 bytes: some 7,700 nodes).
pub const COVERAGE_CACHE_BYTES: usize = 8 << 20;

fn river_request<'a>(
    src: &'a (dyn RiverSource + Send + Sync),
    law: &'a planet_generators::bake::k7::WidthLaw,
    res: u32,
) -> planet_frame::river::RiverRequest<'a> {
    planet_frame::river::RiverRequest {
        source: src,
        law,
        res,
        min_samples: planet_frame::river::MIN_RIVER_SAMPLES,
        full_samples: planet_frame::river::FULL_RIVER_SAMPLES,
        max_layers: planet_render::RIVER_SLOTS as usize,
    }
}

impl RiverCoverage {
    pub fn new(
        src: Arc<ReaderRiverSource>,
        cache_bytes: usize,
        share: Option<planet_streaming::jobs::Share>,
        system: Option<Arc<planet_streaming::jobs::JobSystem>>,
    ) -> RiverCoverage {
        RiverCoverage::with_source(src, cache_bytes, share, system)
    }

    /// [`RiverCoverage::new`] over any river source (a test's).
    pub fn with_source(
        src: Arc<dyn RiverSource + Send + Sync>,
        cache_bytes: usize,
        share: Option<planet_streaming::jobs::Share>,
        system: Option<Arc<planet_streaming::jobs::JobSystem>>,
    ) -> RiverCoverage {
        RiverCoverage {
            src,
            cache: Arc::new(std::sync::Mutex::new(planet_frame::river::RiverCache::new(cache_bytes))),
            share,
            inflight: Arc::default(),
            failure: Arc::default(),
            _system: system,
        }
    }

    /// The source the coverage reads (the viewer compares it with the world's to notice a new commit).
    pub fn source(&self) -> &Arc<dyn RiverSource + Send + Sync> {
        &self.src
    }

    /// What the cache has done so far.
    pub fn counters(&self) -> planet_frame::river::CacheCounters {
        self.cache.lock().expect("river cache").counters()
    }

    /// Builds queued or running.
    pub fn pending(&self) -> usize {
        self.inflight.lock().expect("river inflight").len()
    }

    /// Waits until no build is pending (a scene's warm-up, a test), at most `limit`; false when it did not finish.
    pub fn wait_idle(&self, limit: std::time::Duration) -> bool {
        let started = std::time::Instant::now();
        while self.pending() > 0 {
            if started.elapsed() > limit {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        true
    }

    /// The river plan of a frame from the cache (REND-062, REND-064): missing tiles are queued for the background (or built here without a share). An error of a background build (a failed read, a panic) is returned by the next call; without a share, by the call that hit it.
    pub fn plan(&self, plan: &planet_frame::terrain::TerrainPlan) -> Result<CoverageFrame, String> {
        if let Some(e) = self.failure.lock().expect("river failure").take() {
            return Err(e);
        }
        let nodes: Vec<(u32, planet_core::TileId, f64)> = plan
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u32, n.tile, n.camera_relative_origin.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt()))
            .collect();
        self.plan_nodes(plan.cells, &nodes)
    }

    /// [`RiverCoverage::plan`] over `(index, tile, distance)` nodes of `res` cells an edge.
    pub fn plan_nodes(&self, res: u32, nodes: &[(u32, planet_core::TileId, f64)]) -> Result<CoverageFrame, String> {
        if let Some(e) = self.failure.lock().expect("river failure").take() {
            return Err(e);
        }
        let law = planet_generators::bake::k7::WidthLaw::STANDARD;
        let req = river_request(self.src.as_ref(), &law, res);
        let (looked, stamp, bytes) = {
            let mut cache = self.cache.lock().expect("river cache");
            let looked = planet_frame::river::plan_rivers_lookup(&req, nodes, &mut cache)?;
            let stamp = cache.adopt(&req)?;
            (looked, stamp, cache.bytes())
        };
        let mut queued = 0u32;
        for tile in &looked.missing {
            let fresh = self.inflight.lock().expect("river inflight").insert(*tile);
            if !fresh {
                continue;
            }
            queued += 1;
            let (src, cache, inflight, failure) = (self.src.clone(), self.cache.clone(), self.inflight.clone(), self.failure.clone());
            let tile = *tile;
            let job = move || {
                // The tile leaves `inflight` however the build ends (a panic of the build is caught below, and a job system swallows one).
                struct Done(Arc<std::sync::Mutex<std::collections::HashSet<planet_core::TileId>>>, planet_core::TileId);
                impl Drop for Done {
                    fn drop(&mut self) {
                        if let Ok(mut set) = self.0.lock() {
                            set.remove(&self.1);
                        }
                    }
                }
                let _done = Done(inflight, tile);
                let law = planet_generators::bake::k7::WidthLaw::STANDARD;
                let req = river_request(src.as_ref(), &law, res);
                let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| planet_frame::river::build_river_node(&req, tile)));
                match built {
                    Ok(Ok(v)) => cache.lock().expect("river cache").insert_stamped(stamp, tile, v),
                    Ok(Err(e)) => *failure.lock().expect("river failure") = Some(e),
                    Err(panic) => {
                        let why =
                            panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()));
                        *failure.lock().expect("river failure") = Some(format!(
                            "the river coverage of tile {tile:?} could not be built: the build panicked: {}",
                            why.unwrap_or_default()
                        ));
                    }
                }
            };
            match &self.share {
                Some(share) => share.spawn_class(planet_streaming::jobs::Class::Prefetch, Box::new(job)),
                None => job(),
            }
        }
        if self.share.is_none() && queued > 0 {
            // Built here: an error of a build is the error of this call, then the plan again from the filled cache.
            if let Some(e) = self.failure.lock().expect("river failure").take() {
                return Err(e);
            }
            let again = {
                let mut cache = self.cache.lock().expect("river cache");
                let again = planet_frame::river::plan_rivers_lookup(&req, nodes, &mut cache)?;
                (again, cache.bytes())
            };
            return Ok(CoverageFrame { plan: again.0.plan, not_ready: again.0.not_ready, queued, cache_bytes: again.1 });
        }
        Ok(CoverageFrame { plan: looked.plan, not_ready: looked.not_ready, queued, cache_bytes: bytes })
    }
}
