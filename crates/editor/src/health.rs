//! The edit-health lists kept up to date as the items change (VIEW-048, M7 edit-health-scale): which brushes are too steep to draw fully
//! (VIEW-043) and which are stacked (VIEW-047), without reading every stroke of every item at each change (SPIKE-29: the full scan was the
//! first thing to break, at about 100 items).
//!
//! A brush item's answer depends only on the strokes of the items whose footprint (a circle round the item's strokes) meets its own. The items
//! sit in a grid of cells by latitude and longitude; a change to item X re-checks X and the items whose footprint meets X's old or new footprint,
//! and no others. The result is equal to the full scans `slope::steep_items` and `problems::stacked_items` (the sums are taken in the same order,
//! over the same terms); a test holds the two together.

use std::collections::{BTreeMap, BTreeSet};

use planet_core::Vec3;
use planet_world_def::item::Item;
use planet_world_def::ItemId;

use crate::brush::BRUSH_KIND;
use crate::polyline;
use crate::problems::{STACK_WARN, STROKES_WARN};
use crate::slope::{self, Poly};

/// Degrees of latitude and longitude in one cell of the grid.
const CELL_DEG: f64 = 0.02;
/// An item whose footprint covers more cells than this is kept apart and met by every query.
const MAX_CELLS: usize = 400;

/// A stroke with the circle round its points that bounds where it can matter.
struct Prep {
    poly: Poly,
    centre: Vec3,
    reach_m: f64,
}

/// A brush item as the lists need it.
struct Entry {
    strokes: Vec<Prep>,
    /// The footprint: a circle round all the strokes.
    centre: Vec3,
    reach_m: f64,
    /// The cells it is filed in (empty when it is in the `wide` set).
    cells: Vec<(i32, i32)>,
    group: Option<ItemId>,
    stroke_count: usize,
}

/// The incremental lists. Built from the items once, then fed the ids that changed.
pub struct Health {
    /// The position in the editor's change log this state is true for.
    seq: u64,
    radius_m: f64,
    limit: f64,
    entries: BTreeMap<ItemId, Entry>,
    grid: BTreeMap<(i32, i32), Vec<ItemId>>,
    wide: BTreeSet<ItemId>,
    groups: BTreeMap<ItemId, BTreeSet<ItemId>>,
    steep: BTreeSet<ItemId>,
    stacked: BTreeMap<ItemId, usize>,
    /// Whether a stack of strokes can be merged, found for an item when asked (it depends on the item alone) and dropped when it changes.
    pub(crate) mergeable: BTreeMap<ItemId, bool>,
    /// How many items the last refresh checked (the work a change costs; tests count it instead of timing).
    last_checked: usize,
    built: bool,
    /// The items whose answer may have changed and is not found yet (found when a list is asked for: the guard of a stroke needs only the index).
    pending: BTreeSet<ItemId>,
}

fn cell_of(lat_deg: f64, lon_deg: f64) -> (i32, i32) {
    ((lat_deg / CELL_DEG).floor() as i32, (lon_deg / CELL_DEG).floor() as i32)
}

fn lat_lon_deg(d: Vec3) -> (f64, f64) {
    (libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees(), libm::atan2(d.y, d.x).to_degrees())
}

impl Health {
    pub fn new(radius_m: f64, drawable_slope: f64) -> Health {
        Health {
            seq: 0,
            radius_m,
            limit: slope::gradient_limit(drawable_slope),
            entries: BTreeMap::new(),
            grid: BTreeMap::new(),
            wide: BTreeSet::new(),
            groups: BTreeMap::new(),
            steep: BTreeSet::new(),
            stacked: BTreeMap::new(),
            mergeable: BTreeMap::new(),
            last_checked: 0,
            built: false,
            pending: BTreeSet::new(),
        }
    }

    /// The position in the change log the lists are true for.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn is_built(&self) -> bool {
        self.built
    }

    pub fn last_checked(&self) -> usize {
        self.last_checked
    }

    pub fn steep(&self) -> &BTreeSet<ItemId> {
        &self.steep
    }

    pub fn stacked(&self) -> &BTreeMap<ItemId, usize> {
        &self.stacked
    }

    /// Whether the item `id` with group `group`, or any item of its group, is over the limit.
    pub fn is_steep(&self, id: ItemId, group: Option<ItemId>) -> bool {
        match group {
            Some(g) => self.groups.get(&g).is_some_and(|m| m.iter().any(|i| self.steep.contains(i))) || self.steep.contains(&id),
            None => self.steep.contains(&id),
        }
    }

    fn entry_of(&self, item: &Item) -> Entry {
        let preps: Vec<Prep> = item
            .strokes
            .iter()
            .map(Poly::of)
            .filter(|p| !p.geo.is_empty())
            .map(|poly| {
                let centre = poly.geo[0].dir();
                let spread = poly.geo.iter().map(|g| polyline::arc_m(centre, g.dir(), self.radius_m)).fold(0.0, f64::max);
                Prep { reach_m: spread + poly.radius_m, centre, poly }
            })
            .collect();
        let (centre, reach_m) = match preps.first() {
            Some(first) => {
                let c = first.centre;
                let reach = preps.iter().map(|p| polyline::arc_m(c, p.centre, self.radius_m) + p.reach_m).fold(0.0, f64::max);
                (c, reach)
            }
            None => (Vec3::new(1.0, 0.0, 0.0), 0.0),
        };
        Entry { strokes: preps, centre, reach_m, cells: Vec::new(), group: item.group, stroke_count: item.strokes.len() }
    }

    /// The cells a circle can touch, or `None` when there are more than `MAX_CELLS`.
    fn cells_of(&self, centre: Vec3, reach_m: f64) -> Option<Vec<(i32, i32)>> {
        let (lat, lon) = lat_lon_deg(centre);
        let d = (reach_m / self.radius_m).to_degrees() + CELL_DEG;
        let (lat0, lat1) = (lat - d, lat + d);
        let (r0, r1) = (cell_of(lat0, 0.0).0, cell_of(lat1, 0.0).0);
        let sin_d = libm::sin(d.to_radians());
        let cos_lat = libm::cos(lat.to_radians());
        // The half-width in longitude of a circle of angular radius d at latitude lat is asin(sin d / cos lat) (all of them when it holds a pole).
        if lat0 <= -89.0 || lat1 >= 89.0 || d >= 90.0 || sin_d >= cos_lat {
            return None;
        }
        let dlon = libm::asin(sin_d / cos_lat).to_degrees();
        if dlon >= 180.0 {
            return None;
        }
        let (c0, c1) = (cell_of(0.0, lon - dlon).1, cell_of(0.0, lon + dlon).1);
        let n_lon = (c1 - c0 + 1) as i64;
        if (r1 - r0 + 1) as i64 * n_lon > MAX_CELLS as i64 {
            return None;
        }
        let wrap = (360.0 / CELL_DEG).round() as i32;
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                out.push((r, c.rem_euclid(wrap)));
            }
        }
        out.sort_unstable();
        out.dedup();
        Some(out)
    }

    fn add(&mut self, id: ItemId, item: &Item) {
        let mut e = self.entry_of(item);
        match self.cells_of(e.centre, e.reach_m) {
            Some(cells) if !e.strokes.is_empty() => {
                for c in &cells {
                    self.grid.entry(*c).or_default().push(id);
                }
                e.cells = cells;
            }
            _ => {
                self.wide.insert(id);
            }
        }
        if let Some(g) = e.group {
            self.groups.entry(g).or_default().insert(id);
        }
        self.entries.insert(id, e);
    }

    fn remove(&mut self, id: ItemId) -> Option<(Vec3, f64)> {
        let e = self.entries.remove(&id)?;
        for c in &e.cells {
            if let Some(v) = self.grid.get_mut(c) {
                v.retain(|i| *i != id);
                if v.is_empty() {
                    self.grid.remove(c);
                }
            }
        }
        self.wide.remove(&id);
        if let Some(g) = e.group {
            if let Some(m) = self.groups.get_mut(&g) {
                m.remove(&id);
                if m.is_empty() {
                    self.groups.remove(&g);
                }
            }
        }
        self.steep.remove(&id);
        self.stacked.remove(&id);
        self.mergeable.remove(&id);
        Some((e.centre, e.reach_m))
    }

    /// The brush items whose footprint meets the circle, in id order (the order the full scans add in): the only items whose strokes can reach
    /// within `reach_m` of `centre`.
    pub fn items_near(&self, centre: Vec3, reach_m: f64) -> Vec<ItemId> {
        self.near(centre, reach_m)
    }

    fn near(&self, centre: Vec3, reach_m: f64) -> Vec<ItemId> {
        let mut out: BTreeSet<ItemId> = self.wide.iter().copied().collect();
        match self.cells_of(centre, reach_m) {
            Some(cells) => {
                for c in cells {
                    if let Some(v) = self.grid.get(&c) {
                        out.extend(v.iter().copied());
                    }
                }
            }
            None => out.extend(self.entries.keys().copied()),
        }
        out.into_iter()
            .filter(|id| {
                let e = &self.entries[id];
                polyline::arc_m(centre, e.centre, self.radius_m) < reach_m + e.reach_m
            })
            .collect()
    }

    /// Whether item `id` is over the limit and the depth of its stack, read from the strokes of the items near it.
    fn evaluate(&self, id: ItemId) -> (bool, usize) {
        let e = &self.entries[&id];
        let near = self.near(e.centre, e.reach_m);
        let (mut steep, mut depth) = (false, 0usize);
        for mine in &e.strokes {
            for g in &mine.poly.geo {
                let dir = g.dir();
                let (mut sum, mut count) = (0.0, 0usize);
                for other in &near {
                    for p in &self.entries[other].strokes {
                        if polyline::arc_m(dir, p.centre, self.radius_m) < p.reach_m
                            && p.poly.distance(dir, self.radius_m) < p.poly.radius_m
                        {
                            count += 1;
                            if p.poly.gradient > 0.0 {
                                sum += p.poly.gradient;
                            }
                        }
                    }
                }
                if mine.poly.gradient > 0.0 && sum > self.limit + 1e-9 {
                    steep = true;
                }
                depth = depth.max(count);
            }
        }
        (steep, depth)
    }

    fn settle(&mut self, id: ItemId) {
        let (steep, depth) = self.evaluate(id);
        let strokes = self.entries[&id].stroke_count;
        if steep {
            self.steep.insert(id);
        } else {
            self.steep.remove(&id);
        }
        if depth >= STACK_WARN || strokes >= STROKES_WARN {
            self.stacked.insert(id, depth);
        } else {
            self.stacked.remove(&id);
        }
        self.last_checked += 1;
    }

    /// Start again from all the items, true for position `seq` of the change log.
    pub fn rebuild<'a>(&mut self, seq: u64, items: impl IntoIterator<Item = &'a Item>) {
        let slope_limit = self.limit;
        *self = Health { limit: slope_limit, ..Health::new(self.radius_m, 0.0) };
        for item in items.into_iter().filter(|i| i.kind == BRUSH_KIND) {
            self.add(item.id, item);
        }
        self.pending = self.entries.keys().copied().collect();
        self.seq = seq;
        self.built = true;
    }

    /// Bring the index up to position `seq` of the change log, given the ids changed since the position they were true for and the items now.
    pub fn apply<'a>(&mut self, seq: u64, changed: &[ItemId], item: impl Fn(ItemId) -> Option<&'a Item>) {
        let changed: BTreeSet<ItemId> = changed.iter().copied().collect();
        let mut circles: Vec<(Vec3, f64)> = Vec::new();
        for id in &changed {
            circles.extend(self.remove(*id));
        }
        let mut added = Vec::new();
        for id in &changed {
            if let Some(it) = item(*id).filter(|i| i.kind == BRUSH_KIND) {
                self.add(*id, it);
                let e = &self.entries[id];
                circles.push((e.centre, e.reach_m));
                added.push(*id);
            }
        }
        let mut affected: BTreeSet<ItemId> = added.into_iter().collect();
        for (c, r) in circles {
            affected.extend(self.near(c, r));
        }
        self.pending.extend(affected);
        self.seq = seq;
    }

    /// Find the answers of the items that may have changed since the last time (the lists are true after this).
    pub fn settle_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.last_checked = 0;
        for id in std::mem::take(&mut self.pending) {
            if self.entries.contains_key(&id) {
                self.settle(id);
            }
        }
    }
}
