//! A small render graph (report §8): passes declare the resources they read and write; compiling checks that every read
//! has a producer, drops passes nobody needs, and computes transient resource lifetimes so memory can be aliased.
//! GPU-free declaration and analysis; executors turn the compiled order into wgpu passes.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassDesc {
    pub name: &'static str,
    pub reads: Vec<&'static str>,
    pub writes: Vec<&'static str>,
    /// Passes with side effects (presenting, readback) are never culled.
    pub side_effect: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GraphError {
    MissingProducer { pass: &'static str, resource: &'static str },
    DuplicatePass(&'static str),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::MissingProducer { pass, resource } => {
                write!(f, "pass '{pass}' reads '{resource}', which no earlier pass writes and which is not an external resource")
            }
            GraphError::DuplicatePass(p) => write!(f, "pass '{p}' is declared twice"),
        }
    }
}

impl std::error::Error for GraphError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lifetime {
    pub resource: &'static str,
    /// Positions in the compiled order of the first write and the last read.
    pub first: usize,
    pub last: usize,
    /// Resources are only aliased with others of the same description (format, size class).
    pub class: &'static str,
    /// Bytes of the resource when it was described with a size (`describe_sized`), else 0.
    pub bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompiledGraph {
    /// Indices into the declared passes, in execution order.
    pub order: Vec<usize>,
    pub lifetimes: Vec<Lifetime>,
    /// Groups of transient resources that never live at the same time and can share memory.
    pub alias_groups: Vec<Vec<&'static str>>,
    pub culled: Vec<&'static str>,
    /// Persistent resources (declared with `persistent`) that a kept pass reads or writes: they live across frames and count
    /// in the frame-target share.
    pub persistent: Vec<(&'static str, u64)>,
}

impl CompiledGraph {
    /// Bytes of the transient resources as the renderer allocates them (no aliasing: each resource its own texture or buffer).
    pub fn transient_bytes(&self) -> u64 {
        self.lifetimes.iter().map(|l| l.bytes).sum()
    }

    /// Bytes the transient resources would need if equal-class resources with disjoint lifetimes shared memory (the largest
    /// member of each alias group): the saving available to a later aliasing executor.
    pub fn aliased_transient_bytes(&self) -> u64 {
        self.alias_groups
            .iter()
            .map(|g| g.iter().map(|r| self.lifetimes.iter().find(|l| l.resource == *r).map_or(0, |l| l.bytes)).max().unwrap_or(0))
            .sum()
    }

    /// Bytes of the persistent resources the kept passes use.
    pub fn persistent_bytes(&self) -> u64 {
        self.persistent.iter().map(|(_, b)| b).sum()
    }

    /// The frame-target share (ADR 0033 amendment, ADR 0042 point 5): transient plus persistent bytes of everything the compiled
    /// frame allocates besides terrain and the presentable target.
    pub fn frame_target_bytes(&self) -> u64 {
        self.transient_bytes() + self.persistent_bytes()
    }
}

#[derive(Default)]
pub struct RenderGraph {
    passes: Vec<PassDesc>,
    external: BTreeSet<&'static str>,
    classes: BTreeMap<&'static str, &'static str>,
    sizes: BTreeMap<&'static str, u64>,
    /// External resources the renderer keeps across frames, with their bytes.
    persistent: BTreeMap<&'static str, u64>,
}

impl RenderGraph {
    /// `external` resources exist before the graph runs (the swapchain image, persistent buffers).
    pub fn new(external: &[&'static str]) -> Self {
        Self { external: external.iter().copied().collect(), ..Default::default() }
    }

    /// Declare the memory class of a transient resource (for example `rgba8-1x`, `depth32-1x`).
    pub fn describe(&mut self, resource: &'static str, class: &'static str) -> &mut Self {
        self.classes.insert(resource, class);
        self
    }

    /// [`describe`](RenderGraph::describe) with the resource's size in bytes, so the compiled graph can count it.
    pub fn describe_sized(&mut self, resource: &'static str, class: &'static str, bytes: u64) -> &mut Self {
        self.classes.insert(resource, class);
        self.sizes.insert(resource, bytes);
        self
    }

    /// Declare an external resource that lives across frames (exposure history) and counts in the frame-target share when a
    /// kept pass uses it.
    pub fn persistent(&mut self, resource: &'static str, bytes: u64) -> &mut Self {
        self.external.insert(resource);
        self.persistent.insert(resource, bytes);
        self
    }

    /// The declared passes, in declaration order (the indices of [`CompiledGraph::order`]).
    pub fn passes(&self) -> &[PassDesc] {
        &self.passes
    }

    pub fn add_pass(&mut self, name: &'static str, reads: &[&'static str], writes: &[&'static str], side_effect: bool) -> &mut Self {
        self.passes.push(PassDesc { name, reads: reads.to_vec(), writes: writes.to_vec(), side_effect });
        self
    }

    pub fn compile(&self) -> Result<CompiledGraph, GraphError> {
        let mut names = BTreeSet::new();
        for p in &self.passes {
            if !names.insert(p.name) {
                return Err(GraphError::DuplicatePass(p.name));
            }
        }
        // Producers: the latest earlier pass writing each resource a pass reads.
        let mut producer: Vec<Vec<usize>> = vec![Vec::new(); self.passes.len()];
        for (i, p) in self.passes.iter().enumerate() {
            for r in &p.reads {
                match (0..i).rev().find(|&j| self.passes[j].writes.contains(r)) {
                    Some(j) => producer[i].push(j),
                    None if self.external.contains(r) => {}
                    None => return Err(GraphError::MissingProducer { pass: p.name, resource: r }),
                }
            }
        }
        // Keep passes with side effects and everything they depend on.
        let mut keep = vec![false; self.passes.len()];
        let mut stack: Vec<usize> = (0..self.passes.len()).filter(|&i| self.passes[i].side_effect).collect();
        while let Some(i) = stack.pop() {
            if !std::mem::replace(&mut keep[i], true) {
                stack.extend(producer[i].iter().copied());
            }
        }
        let order: Vec<usize> = (0..self.passes.len()).filter(|&i| keep[i]).collect();
        let culled = (0..self.passes.len()).filter(|&i| !keep[i]).map(|i| self.passes[i].name).collect();
        // Lifetimes of transient resources over the compiled order.
        let mut first: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut last: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (pos, &i) in order.iter().enumerate() {
            for r in &self.passes[i].writes {
                first.entry(r).or_insert(pos);
                last.entry(r).and_modify(|l| *l = (*l).max(pos)).or_insert(pos);
            }
            for r in &self.passes[i].reads {
                last.entry(r).and_modify(|l| *l = (*l).max(pos)).or_insert(pos);
            }
        }
        let lifetimes: Vec<Lifetime> = first
            .iter()
            .filter(|(r, _)| !self.external.contains(*r))
            .map(|(r, &f)| Lifetime {
                resource: r,
                first: f,
                last: last[r],
                class: self.classes.get(r).copied().unwrap_or("unclassified"),
                bytes: self.sizes.get(r).copied().unwrap_or(0),
            })
            .collect();
        // Greedy aliasing: a resource joins the first group of its class whose members all ended before it starts.
        let mut groups: Vec<(&'static str, Vec<&Lifetime>)> = Vec::new();
        let mut sorted: Vec<&Lifetime> = lifetimes.iter().collect();
        sorted.sort_by_key(|l| (l.first, l.resource));
        for l in sorted {
            if l.class == "unclassified" {
                groups.push((l.class, vec![l]));
                continue;
            }
            match groups.iter_mut().find(|(c, members)| *c == l.class && members.iter().all(|m| m.last < l.first)) {
                Some((_, members)) => members.push(l),
                None => groups.push((l.class, vec![l])),
            }
        }
        let alias_groups = groups.into_iter().map(|(_, m)| m.into_iter().map(|l| l.resource).collect()).collect();
        let persistent = self
            .persistent
            .iter()
            .filter(|(r, _)| order.iter().any(|&i| self.passes[i].reads.contains(*r) || self.passes[i].writes.contains(*r)))
            .map(|(r, &b)| (*r, b))
            .collect();
        Ok(CompiledGraph { order, lifetimes, alias_groups, culled, persistent })
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The lit path's frame shape (ADR 0042 points 1, 2 and 5, ADR 0038)
// ---------------------------------------------------------------------------------------------------------------------------

/// Bytes per pixel of the scene target: `Rgba16Float`.
pub const HDR_BYTES_PER_PIXEL: u64 = 8;
/// Bytes per pixel of the depth target: reversed-Z `Depth32Float` (CON-12).
pub const DEPTH_BYTES_PER_PIXEL: u64 = 4;
/// The luminance histogram: 64 `u32` bins.
pub const HISTOGRAM_BYTES: u64 = 64 * 4;
/// The exposure history: the adapted EV, the target EV, the factor to the pre-exposure and a spare word (four `f32`).
pub const EXPOSURE_STATE_BYTES: u64 = 16;

/// Frame targets may take this many bytes per pixel of the internal resolution plus [`FRAME_TARGET_FIXED_BYTES`] (the
/// frame-target share, an amendment to ADR 0033's reserved remainder; ADR 0042 point 5). The lit path uses 12 B per pixel today
/// (HDR 8, depth 4); the rest is room for the water pass's copies, the sky's targets and the atmosphere volumes.
pub const FRAME_TARGET_BYTES_PER_PIXEL: u64 = 40;
pub const FRAME_TARGET_FIXED_BYTES: u64 = 32 * 1024 * 1024;

/// The frame-target share at an internal resolution: a count-based cap (CON-20) asserted from the compiled graph.
pub fn frame_target_share_bytes(size: (u32, u32)) -> u64 {
    u64::from(size.0) * u64::from(size.1) * FRAME_TARGET_BYTES_PER_PIXEL + FRAME_TARGET_FIXED_BYTES
}

/// What the lit frame's passes depend on. The graph is compiled when the shape changes (a new size or exposure mode), not
/// every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LitShape {
    pub size: (u32, u32),
    /// Adaptive exposure runs the histogram; fixed exposure culls it.
    pub adaptive: bool,
}

/// The lit path as a graph: terrain into HDR colour and depth, the exposure histogram and adaptation, the tone map into the
/// presentable target. The atmosphere, water and sky passes of ADR 0042 point 2 join here as their changes land.
pub fn lit_frame_graph(shape: &LitShape) -> RenderGraph {
    let pixels = u64::from(shape.size.0) * u64::from(shape.size.1);
    let mut g = RenderGraph::new(&["presentable"]);
    g.describe_sized("hdr", "rgba16float-frame", pixels * HDR_BYTES_PER_PIXEL)
        .describe_sized("depth", "depth32-frame", pixels * DEPTH_BYTES_PER_PIXEL)
        .describe_sized("histogram", "histogram-64", HISTOGRAM_BYTES)
        .persistent("exposure-state", EXPOSURE_STATE_BYTES);
    g.add_pass("terrain", &[], &["hdr", "depth"], false);
    g.add_pass("exposure-histogram", &["hdr"], &["histogram"], false);
    // Fixed exposure reads no histogram, so the histogram pass above is culled by the compile, not by a branch in the executor.
    let adapt_reads: &[&'static str] = if shape.adaptive { &["histogram"] } else { &[] };
    g.add_pass("exposure-adapt", adapt_reads, &["exposure-state"], false);
    g.add_pass("tonemap", &["hdr", "exposure-state"], &["presentable"], true);
    g
}
