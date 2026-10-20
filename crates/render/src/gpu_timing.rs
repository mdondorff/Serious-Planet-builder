//! Per-pass GPU timestamps of a timed frame (REND-028; M5 item 8): one pair of timestamp queries per pass of the lit frame, resolved
//! with the frame and turned into milliseconds by a pure function. The terrain pass keeps queries 0 and 1 and the water pass 2 and 3
//! (what `FrameTiming::gpu_ms` and `water_gpu_ms` always read); the atmosphere's products, the composite and the post chain follow.
//!
//! The scene copy is a command on the encoder, not a pass, and cannot carry timestamps; its time is the gap between the end of the
//! terrain pass and the start of the water pass, which holds the copy and nothing else (the barriers around it included). The static
//! tables' upload (`atmosphere-luts`) is a queue write when the body's tables change and no GPU pass in a steady frame, so it has no
//! row; the harness reports it as not in the frame.

/// The passes that carry a pair of timestamps, in query order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPass {
    Terrain = 0,
    Water = 1,
    SkyView = 2,
    AerialPerspective = 3,
    SkyComposite = 4,
    ExposureHistogram = 5,
    ExposureAdapt = 6,
    Tonemap = 7,
}

impl GpuPass {
    pub const ALL: [GpuPass; 8] = [
        GpuPass::Terrain,
        GpuPass::Water,
        GpuPass::SkyView,
        GpuPass::AerialPerspective,
        GpuPass::SkyComposite,
        GpuPass::ExposureHistogram,
        GpuPass::ExposureAdapt,
        GpuPass::Tonemap,
    ];

    /// The pass's name in the render graph and in the perf logs (`gpu_<name>_ms`, dashes as underscores).
    pub fn name(self) -> &'static str {
        match self {
            GpuPass::Terrain => "terrain",
            GpuPass::Water => "water",
            GpuPass::SkyView => "sky-view",
            GpuPass::AerialPerspective => "aerial-perspective",
            GpuPass::SkyComposite => "sky-composite",
            GpuPass::ExposureHistogram => "exposure-histogram",
            GpuPass::ExposureAdapt => "exposure-adapt",
            GpuPass::Tonemap => "tonemap",
        }
    }

    /// Index of the pass's first query (the second is the next).
    pub fn first_query(self) -> u32 {
        2 * self as u32
    }
}

/// Queries in the set: two per pass.
pub const PASS_QUERIES: u32 = 2 * GpuPass::ALL.len() as u32;
/// Bytes between the resolved pairs of two passes: a resolve offset must be aligned to `QUERY_RESOLVE_BUFFER_ALIGNMENT` (256).
pub const RESOLVE_STRIDE: u64 = 256;
/// Bytes of the resolve and readback buffers: one stride per pass.
pub const PASS_QUERY_BYTES: u64 = RESOLVE_STRIDE * GpuPass::ALL.len() as u64;

/// The GPU time of each pass of one frame, milliseconds; `None` for a pass the frame did not run (or whose interval read zero: a
/// broken query is not a free pass).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassTimes {
    pub ms: [Option<f64>; 8],
    /// The scene copy before the water pass (gap between the end of the terrain pass and the start of the water pass).
    pub scene_copy_ms: Option<f64>,
    /// First timestamp to last of the passes that ran: the frame's GPU span including the gaps between passes.
    pub span_ms: Option<f64>,
}

impl PassTimes {
    pub fn get(&self, p: GpuPass) -> Option<f64> {
        self.ms[p as usize]
    }

    /// The times from the resolved ticks of the queries: `written` has bit `k` set when pass `k` wrote its pair, `period_ns` is the
    /// queue's timestamp period. Pure.
    pub fn from_ticks(ticks: &[u64], written: u16, period_ns: f64) -> PassTimes {
        let ms = |a: u64, b: u64| {
            let d = b.saturating_sub(a);
            (d > 0).then(|| d as f64 * period_ns / 1.0e6)
        };
        let mut out = PassTimes::default();
        let (mut first, mut last) = (u64::MAX, 0u64);
        for p in GpuPass::ALL {
            if written & (1 << p as u16) == 0 {
                continue;
            }
            let (a, b) = (ticks[p.first_query() as usize], ticks[p.first_query() as usize + 1]);
            out.ms[p as usize] = ms(a, b);
            first = first.min(a);
            last = last.max(b);
        }
        let has = |p: GpuPass| written & (1 << p as u16) != 0;
        if has(GpuPass::Terrain) && has(GpuPass::Water) {
            let end_of_terrain = ticks[GpuPass::Terrain.first_query() as usize + 1];
            let start_of_water = ticks[GpuPass::Water.first_query() as usize];
            out.scene_copy_ms = ms(end_of_terrain, start_of_water);
        }
        if written != 0 && last > first {
            out.span_ms = ms(first, last);
        }
        out
    }
}

/// The query set of a frame being recorded: hands out the timestamp writes of each pass and remembers which passes asked.
pub(crate) struct Stamps<'a> {
    set: &'a wgpu::QuerySet,
    written: std::cell::Cell<u16>,
}

impl<'a> Stamps<'a> {
    pub(crate) fn new(set: &'a wgpu::QuerySet) -> Self {
        Stamps { set, written: std::cell::Cell::new(0) }
    }

    fn mark(&self, p: GpuPass) {
        self.written.set(self.written.get() | (1 << p as u16));
    }

    pub(crate) fn render(&self, p: GpuPass) -> wgpu::RenderPassTimestampWrites<'a> {
        self.mark(p);
        wgpu::RenderPassTimestampWrites {
            query_set: self.set,
            beginning_of_pass_write_index: Some(p.first_query()),
            end_of_pass_write_index: Some(p.first_query() + 1),
        }
    }

    pub(crate) fn compute(&self, p: GpuPass) -> wgpu::ComputePassTimestampWrites<'a> {
        self.mark(p);
        wgpu::ComputePassTimestampWrites {
            query_set: self.set,
            beginning_of_pass_write_index: Some(p.first_query()),
            end_of_pass_write_index: Some(p.first_query() + 1),
        }
    }

    /// The beginning of `p` only (a compute pass that the render pass of `p` ends: the haze pass and the composite are one timed span).
    pub(crate) fn compute_begin(&self, p: GpuPass) -> wgpu::ComputePassTimestampWrites<'a> {
        self.mark(p);
        wgpu::ComputePassTimestampWrites {
            query_set: self.set,
            beginning_of_pass_write_index: Some(p.first_query()),
            end_of_pass_write_index: None,
        }
    }

    /// The end of `p` only (the render pass that ends a span a compute pass began).
    pub(crate) fn render_end(&self, p: GpuPass) -> wgpu::RenderPassTimestampWrites<'a> {
        self.mark(p);
        wgpu::RenderPassTimestampWrites {
            query_set: self.set,
            beginning_of_pass_write_index: None,
            end_of_pass_write_index: Some(p.first_query() + 1),
        }
    }

    /// Bit `k` set for each pass that took its writes.
    pub(crate) fn written(&self) -> u16 {
        self.written.get()
    }
}

/// The optional timestamp writes of a pass of a frame that may not be timed.
pub(crate) fn render_writes<'a>(s: Option<&Stamps<'a>>, p: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
    s.map(|s| s.render(p))
}

pub(crate) fn compute_begin_writes<'a>(s: Option<&Stamps<'a>>, p: GpuPass) -> Option<wgpu::ComputePassTimestampWrites<'a>> {
    s.map(|s| s.compute_begin(p))
}

pub(crate) fn render_end_writes<'a>(s: Option<&Stamps<'a>>, p: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
    s.map(|s| s.render_end(p))
}

pub(crate) fn compute_writes<'a>(s: Option<&Stamps<'a>>, p: GpuPass) -> Option<wgpu::ComputePassTimestampWrites<'a>> {
    s.map(|s| s.compute(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticks(pairs: &[(GpuPass, u64, u64)]) -> (Vec<u64>, u16) {
        let mut t = vec![0u64; PASS_QUERIES as usize];
        let mut w = 0u16;
        for (p, a, b) in pairs {
            t[p.first_query() as usize] = *a;
            t[p.first_query() as usize + 1] = *b;
            w |= 1 << *p as u16;
        }
        (t, w)
    }

    // spec: REND-028
    #[test]
    fn the_terrain_and_water_pass_keep_the_queries_the_frame_timing_always_read() {
        assert_eq!((GpuPass::Terrain.first_query(), GpuPass::Water.first_query()), (0, 2));
        assert_eq!(PASS_QUERIES, 16);
        let names: Vec<&str> = GpuPass::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&"sky-view") && names.contains(&"tonemap") && names.contains(&"exposure-adapt"));
    }

    // spec: REND-028
    #[test]
    fn pass_times_scale_by_the_period_derive_the_scene_copy_and_the_span_and_skip_what_did_not_run() {
        // 2 ns per tick: 1,000,000 ticks = 2 ms.
        let (t, w) = ticks(&[
            (GpuPass::SkyView, 100, 100_100),
            (GpuPass::AerialPerspective, 100_100, 150_100),
            (GpuPass::Terrain, 150_100, 1_150_100),
            (GpuPass::Water, 1_250_100, 1_750_100),
            (GpuPass::SkyComposite, 1_750_100, 2_250_100),
            (GpuPass::Tonemap, 2_250_100, 2_300_100),
        ]);
        let p = PassTimes::from_ticks(&t, w, 2.0);
        assert!((p.get(GpuPass::Terrain).unwrap() - 2.0).abs() < 1e-9);
        assert!((p.get(GpuPass::Water).unwrap() - 1.0).abs() < 1e-9);
        assert!((p.get(GpuPass::SkyView).unwrap() - 0.2).abs() < 1e-9);
        assert!((p.scene_copy_ms.unwrap() - 0.2).abs() < 1e-9, "the gap between terrain end and water start: {p:?}");
        assert!((p.span_ms.unwrap() - 4.6).abs() < 1e-9, "{p:?}");
        assert!(p.get(GpuPass::ExposureHistogram).is_none() && p.get(GpuPass::ExposureAdapt).is_none(), "not written: not reported");
        // A debug frame: the terrain pass alone, no copy.
        let (t, w) = ticks(&[(GpuPass::Terrain, 10, 510)]);
        let p = PassTimes::from_ticks(&t, w, 1.0);
        assert!(p.get(GpuPass::Terrain).is_some() && p.scene_copy_ms.is_none() && p.get(GpuPass::Water).is_none());
        assert_eq!(p.span_ms, p.get(GpuPass::Terrain));
        // Nothing written, or a zero interval: nothing, never 0 ms.
        assert_eq!(PassTimes::from_ticks(&[0; 16], 0, 1.0), PassTimes::default());
        let (t, w) = ticks(&[(GpuPass::Terrain, 5, 5)]);
        assert!(PassTimes::from_ticks(&t, w, 1.0).get(GpuPass::Terrain).is_none());
        // Water before the end of terrain (broken clocks) gives no copy time instead of a negative one.
        let (t, w) = ticks(&[(GpuPass::Terrain, 0, 100), (GpuPass::Water, 50, 200)]);
        assert!(PassTimes::from_ticks(&t, w, 1.0).scene_copy_ms.is_none());
    }
}
