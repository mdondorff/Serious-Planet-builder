//! The residency manager (STRM-006, STRM-007, ADR 0031 point 4, ADR 0033): it turns the scheduler's wants into what the
//! share can hold and drives the [`Streamer`] with it.
//!
//! [`degrade`] is the pure part. When the residency set R plus the draw set D with its ancestors (and the roots) need more
//! slots than the pool holds, the surroundings get coarser, never the view:
//! 1. tiles outside R and prefetch tiles go first (the streamer's eviction order, not a change of the wants);
//! 2. the leaves of R that are not pinned are replaced by their ancestors, sibling groups merged, farthest first and, among
//!    equally far, finest first. A group is merged only when none of its tiles is pinned (D, an ancestor of D, a root), so an
//!    ancestor never overlaps a drawn tile;
//! 3. last resort, a declared exception to LOD-008's pixel guarantee: when D with its ancestors alone exceed the pool, D itself
//!    is replaced by ancestors by the same order. This is a budget failure and is reported with the numbers.
//!
//! The result is a pure function of `(R, D, capacity)`: the same inputs in any order give the same sets.

use crate::layer::{normalized, parents_listed};
use crate::slots::VERTEX_POOL_MAX_LEVEL;
use crate::{priority_key, Resolved, Streamer, Wants};
use planet_core::{Face, TileId};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// What [`degrade`] did, with the numbers a failure report needs (CON-20: counts, not bytes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DegradationReport {
    /// Slots the pool holds (the capacity the sets were fitted to).
    pub capacity: usize,
    /// Slots R needed with the pinned tiles added (R_full).
    pub r_full: usize,
    /// Slots the pinned tiles (D with its ancestors, the roots) need.
    pub pinned: usize,
    /// Slots R_effective needs after the degradation.
    pub r_effective: usize,
    /// Sibling groups replaced by their parent.
    pub merged_groups: usize,
    /// Tiles removed from R by the merging.
    pub dropped: usize,
    /// Draw-set tiles replaced by ancestors: the declared LOD-008 exception. Empty unless the pinned tiles exceed the pool.
    pub draw_replaced: Vec<TileId>,
    /// The pinned tiles (or even the roots alone) exceed the capacity: a budget failure, reported with `pinned` and `capacity`.
    pub budget_failure: bool,
}

impl DegradationReport {
    pub fn degraded(&self) -> bool {
        self.dropped > 0
    }
}

/// R_effective and the draw set to ask for.
#[derive(Clone, Debug, PartialEq)]
pub struct Degraded {
    /// R_effective: ascending tile-id order, closed under parents, with each tile's distance.
    pub residency: Vec<(TileId, f64)>,
    /// The draw set: unchanged unless the exception applied, then the replaced tiles are their ancestors.
    pub draw: Vec<(TileId, f64)>,
    pub report: DegradationReport,
}

fn roots() -> impl Iterator<Item = TileId> {
    Face::ALL.into_iter().map(|f| TileId::new(f, 0, 0, 0).expect("root"))
}

/// Merge sibling groups of `set` until at most `capacity` counted tiles remain or nothing mergeable is left. `pinned` tiles
/// are never removed. Returns `(groups merged, tiles removed)`.
fn merge(set: &mut BTreeMap<TileId, f64>, pinned: &BTreeSet<TileId>, counted: &dyn Fn(TileId) -> bool, capacity: usize) -> (usize, usize) {
    let mut count = set.keys().filter(|t| counted(**t)).count();
    let mut kids: BTreeMap<TileId, Vec<TileId>> = BTreeMap::new();
    for &t in set.keys() {
        if let Some(p) = t.parent() {
            kids.entry(p).or_default().push(t);
        }
    }
    // A group (the children of `p` present in the set) is mergeable when all are leaves and none is pinned. Its key:
    // the nearest member's distance (farthest groups first), then the level (finest first), then the tile id.
    let group_key = |p: TileId, kids: &BTreeMap<TileId, Vec<TileId>>, set: &BTreeMap<TileId, f64>| {
        let ks = kids.get(&p)?;
        if ks.is_empty() || ks.iter().any(|c| pinned.contains(c) || kids.get(c).is_some_and(|k| !k.is_empty())) {
            return None;
        }
        let near = ks.iter().map(|c| set[c]).fold(f64::INFINITY, f64::min);
        Some((priority_key(near), ks[0].level(), Reverse(p)))
    };
    let mut heap: BinaryHeap<(u64, u8, Reverse<TileId>)> = BinaryHeap::new();
    for &p in kids.keys() {
        if let Some(k) = group_key(p, &kids, set) {
            heap.push(k);
        }
    }
    let (mut groups, mut dropped) = (0, 0);
    while count > capacity {
        let Some((_, _, Reverse(p))) = heap.pop() else { break };
        if group_key(p, &kids, set).is_none() {
            continue;
        }
        for c in kids.remove(&p).unwrap_or_default() {
            set.remove(&c);
            dropped += 1;
            count -= usize::from(counted(c));
        }
        groups += 1;
        if let Some(q) = p.parent() {
            if let Some(k) = group_key(q, &kids, set) {
                heap.push(k);
            }
        }
    }
    // Leftovers: a group with a pinned member cannot be merged (the pinned tile stays), but its other leaves are not needed
    // for the view either. They go one by one, by the same order, while the capacity is exceeded.
    if count > capacity {
        let mut leaves: BinaryHeap<(u64, u8, Reverse<TileId>)> = BinaryHeap::new();
        let leaf_key = |t: TileId, kids: &BTreeMap<TileId, Vec<TileId>>, set: &BTreeMap<TileId, f64>| {
            (!pinned.contains(&t) && counted(t) && kids.get(&t).is_none_or(|k| k.is_empty()) && set.contains_key(&t))
                .then(|| (priority_key(set[&t]), t.level(), Reverse(t)))
        };
        for &t in set.keys() {
            if let Some(k) = leaf_key(t, &kids, set) {
                leaves.push(k);
            }
        }
        while count > capacity {
            let Some((_, _, Reverse(t))) = leaves.pop() else { break };
            if leaf_key(t, &kids, set).is_none() {
                continue;
            }
            set.remove(&t);
            dropped += 1;
            count -= 1;
            if let Some(p) = t.parent() {
                if let Some(ks) = kids.get_mut(&p) {
                    ks.retain(|c| *c != t);
                }
                if let Some(k) = leaf_key(p, &kids, set) {
                    leaves.push(k);
                }
            }
        }
    }
    (groups, dropped)
}

/// Fit R to `capacity` slots (see the module docs). `draw` and `residency` carry `(tile, priority)` as in [`Wants`]; only the
/// residency priorities (distances) are read; a draw tile outside R (ADR 0031 keeps D inside R) or a root R lacks is taken at distance 0. `vertex_split`: tiles of level 0 to [`VERTEX_POOL_MAX_LEVEL`] live in the small
/// vertex pool and do not count against `capacity` (nor are they ever merged away).
pub fn degrade(draw: &[(TileId, f64)], residency: &[(TileId, f64)], capacity: usize, vertex_split: bool) -> Degraded {
    let counted = move |t: TileId| !vertex_split || t.level() > VERTEX_POOL_MAX_LEVEL;
    // The set: R, the draw set with all ancestors, the roots. Distance of a tile: the smallest given (ancestors of R are in R).
    let mut set: BTreeMap<TileId, f64> = BTreeMap::new();
    for &(t, d) in residency {
        let e = set.entry(t).or_insert(d);
        *e = e.min(d);
    }
    let mut pinned: BTreeSet<TileId> = roots().collect();
    for &(t, _) in draw {
        let mut x = Some(t);
        while let Some(a) = x {
            pinned.insert(a);
            x = a.parent();
        }
    }
    for &t in &pinned {
        set.entry(t).or_insert(0.0);
    }
    // Every ancestor of every member is a member (R is closed under parents; a caller's list may not be).
    let all: Vec<TileId> = set.keys().copied().collect();
    for t in all {
        let d = set[&t];
        let mut x = t.parent();
        while let Some(a) = x {
            if set.contains_key(&a) {
                break;
            }
            set.insert(a, d);
            x = a.parent();
        }
    }
    let count = |s: &BTreeMap<TileId, f64>| s.keys().filter(|t| counted(**t)).count();
    let pinned_count = pinned.iter().filter(|t| counted(**t)).count();
    let mut report = DegradationReport { capacity, r_full: count(&set), pinned: pinned_count, ..Default::default() };
    let mut draw_out = draw.to_vec();
    if report.r_full > capacity {
        // Tiles that never leave: the pinned ones and the ones that live in the vertex pool.
        let mut keep: BTreeSet<TileId> = pinned.clone();
        keep.extend(set.keys().copied().filter(|t| !counted(*t)));
        let (g, d) = merge(&mut set, &keep, &counted, capacity);
        report.merged_groups += g;
        report.dropped += d;
        if count(&set) > capacity {
            // Last resort (ADR 0031 point 4c): the draw set with its ancestors alone exceed the pool.
            report.budget_failure = true;
            let mut keep: BTreeSet<TileId> = roots().collect();
            keep.extend(set.keys().copied().filter(|t| !counted(*t)));
            let (g, d) = merge(&mut set, &keep, &counted, capacity);
            report.merged_groups += g;
            report.dropped += d;
            report.draw_replaced =
                draw.iter().map(|&(t, _)| t).filter(|t| !set.contains_key(t)).collect::<BTreeSet<_>>().into_iter().collect();
            let mut out: BTreeMap<TileId, f64> = BTreeMap::new();
            for &(t, p) in draw {
                let mut x = Some(t);
                while let Some(a) = x {
                    if set.contains_key(&a) {
                        let e = out.entry(a).or_insert(p);
                        *e = e.min(p);
                        break;
                    }
                    x = a.parent();
                }
            }
            draw_out = out.into_iter().collect();
        }
    }
    report.r_effective = count(&set);
    Degraded { residency: set.into_iter().collect(), draw: draw_out, report }
}

/// What one managed frame answered.
#[derive(Clone, Debug)]
pub struct ManagerFrame {
    /// For each tile of the caller's draw set, the resident tile to show.
    pub resolved: Vec<Resolved>,
    pub report: DegradationReport,
}

/// The residency manager: capacity in slots, degradation, eviction and requests in one place, on the injected clock, I/O and
/// spawner. It owns the [`Streamer`] (and through it the slot pools); the renderer asks it for a tile's slot.
///
/// Two ways to drive it, one per manager: [`begin_frame`](ResidencyManager::begin_frame) takes all the wants every frame (tests,
/// the skeleton); [`set_draw`](ResidencyManager::set_draw), [`set_residency`](ResidencyManager::set_residency),
/// [`set_prefetch`](ResidencyManager::set_prefetch) and [`step`](ResidencyManager::step) take only the lists that changed
/// (STRM-019). Both give the requests, cancellations and evictions of the same wants.
pub struct ResidencyManager<T: Send + Sync + 'static> {
    streamer: Streamer<T>,
    last: DegradationReport,
    /// The lists as handed to [`set_draw`](ResidencyManager::set_draw) and [`set_residency`](ResidencyManager::set_residency);
    /// fitted again only when one of them changed.
    raw_draw: Vec<(TileId, f64)>,
    raw_residency: Vec<(TileId, f64)>,
    draw_changed: bool,
    residency_changed: bool,
    /// Sum over the draw set of its tiles' level + 1 (at most the tiles of its closure) and whether R lists the parent of every
    /// tile it lists: what the pool's bound needs.
    draw_levels: usize,
    residency_closed: bool,
    /// The last fit went through [`degrade`]: the streamer then holds the fitted lists, not the raw ones.
    fitted: bool,
}

impl<T: Send + Sync + 'static> ResidencyManager<T> {
    pub fn new(streamer: Streamer<T>) -> Self {
        Self {
            streamer,
            last: DegradationReport::default(),
            raw_draw: Vec::new(),
            raw_residency: Vec::new(),
            draw_changed: false,
            residency_changed: false,
            draw_levels: 0,
            residency_closed: true,
            fitted: false,
        }
    }

    pub fn streamer(&self) -> &Streamer<T> {
        &self.streamer
    }

    pub fn streamer_mut(&mut self) -> &mut Streamer<T> {
        &mut self.streamer
    }

    pub fn last_report(&self) -> &DegradationReport {
        &self.last
    }

    /// One frame: fit the wants to the pool ([`degrade`]), hand them to the streamer, answer what to show for the caller's
    /// draw set (a draw tile replaced by the exception still resolves, to its nearest resident ancestor).
    pub fn begin_frame(&mut self, wants: &Wants) -> ManagerFrame {
        let fitted = degrade(&wants.draw, &wants.residency, self.streamer.main_capacity(), self.streamer.has_vertex_pool());
        let effective = Wants { draw: fitted.draw, residency: fitted.residency, prefetch: wants.prefetch.clone() };
        self.streamer.begin_frame_wants(&effective);
        self.last = fitted.report.clone();
        let resolved = wants.draw.iter().map(|&(t, _)| self.streamer.resolve(t)).collect();
        ManagerFrame { resolved, report: fitted.report }
    }

    /// The draw set for the next [`step`](ResidencyManager::step) and after, until replaced.
    pub fn set_draw(&mut self, draw: &[(TileId, f64)]) {
        self.raw_draw = draw.to_vec();
        self.draw_levels = draw.iter().map(|&(t, _)| usize::from(t.level()) + 1).sum();
        self.draw_changed = true;
    }

    /// The residency set for the next [`step`](ResidencyManager::step) and after, until replaced.
    pub fn set_residency(&mut self, residency: &[(TileId, f64)]) {
        self.raw_residency = normalized(residency).into_owned();
        self.residency_closed = parents_listed(&self.raw_residency, false);
        self.residency_changed = true;
    }

    /// The prefetch set for the next [`step`](ResidencyManager::step) and after, until replaced. Prefetch is not fitted
    /// (it takes free slots only).
    pub fn set_prefetch(&mut self, prefetch: &[(TileId, f64)]) {
        self.streamer.set_prefetch(prefetch);
    }

    /// One frame on the wants as last handed over. The wants are fitted again only if the draw or residency set changed since the
    /// last step: when the tiles the sets can occupy (the listed tiles with their ancestors) cannot exceed the pool, they are
    /// handed to the streamer as they are (nothing would be degraded) and only the list that changed is handed over; otherwise
    /// [`degrade`] fits them. The six roots are not requested here when no list names them: they are resident from
    /// [`Streamer::load_roots_blocking`] and never evicted. The work of a step that follows no change is that of a streamer frame with nothing to do (STRM-019).
    pub fn step(&mut self) -> &DegradationReport {
        let (draw_changed, residency_changed) = (std::mem::take(&mut self.draw_changed), std::mem::take(&mut self.residency_changed));
        if draw_changed || residency_changed {
            let capacity = self.streamer.main_capacity();
            // The most the two sets can occupy: R and the closure of D (at most one tile per level above each) and the roots.
            // R's own ancestors count only if R is not closed under parents.
            let bound = if self.residency_closed { self.raw_residency.len() + self.draw_levels + 6 } else { usize::MAX };
            if bound <= capacity {
                if self.fitted {
                    // The streamer holds the fitted lists: both are handed over.
                    self.streamer.set_draw(&self.raw_draw);
                    self.streamer.set_residency(&self.raw_residency);
                } else {
                    if draw_changed {
                        self.streamer.set_draw(&self.raw_draw);
                    }
                    if residency_changed {
                        self.streamer.set_residency(&self.raw_residency);
                    }
                }
                self.fitted = false;
            } else {
                self.fitted = true;
                let fitted = degrade(&self.raw_draw, &self.raw_residency, capacity, self.streamer.has_vertex_pool());
                self.streamer.set_draw(&fitted.draw);
                self.streamer.set_residency(&fitted.residency);
                self.last = fitted.report;
            }
        }
        self.streamer.step();
        if !self.fitted {
            let (r_full, pinned) = self.streamer.closure_numbers();
            self.last =
                DegradationReport { capacity: self.streamer.main_capacity(), r_full, pinned, r_effective: r_full, ..Default::default() };
        }
        &self.last
    }
}
