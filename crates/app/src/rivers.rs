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
    src: Arc<ReaderRiverSource>,
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
    src: &'a ReaderRiverSource,
    law: &'a planet_generators::bake::k7::WidthLaw,
    res: u32,
) -> planet_frame::river::RiverRequest<'a> {
    planet_frame::river::RiverRequest {
        source: src,
        law,
        res,
        min_samples: planet_frame::river::MIN_RIVER_SAMPLES,
        full_samples: planet_frame::river::FULL_RIVER_SAMPLES,
        max_layers: planet_render::RIVER_LAYERS as usize,
    }
}

impl RiverCoverage {
    pub fn new(
        src: Arc<ReaderRiverSource>,
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

    pub fn source(&self) -> &Arc<ReaderRiverSource> {
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

    /// The river plan of a frame from the cache (REND-062, REND-064): missing tiles are queued for the background (or built here without a share). An error from an earlier background build is returned once.
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
        let req = river_request(&self.src, &law, res);
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
                let law = planet_generators::bake::k7::WidthLaw::STANDARD;
                let req = river_request(&src, &law, res);
                match planet_frame::river::build_river_node(&req, tile) {
                    Ok(v) => cache.lock().expect("river cache").insert_stamped(stamp, tile, v),
                    Err(e) => *failure.lock().expect("river failure") = Some(e),
                }
                inflight.lock().expect("river inflight").remove(&tile);
            };
            match &self.share {
                Some(share) => share.spawn_class(planet_streaming::jobs::Class::Prefetch, Box::new(job)),
                None => job(),
            }
        }
        if self.share.is_none() && queued > 0 {
            // Built here: the plan again, now from the filled cache.
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
