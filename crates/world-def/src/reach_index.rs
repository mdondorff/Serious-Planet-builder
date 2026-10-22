//! The candidate index of the footprint query (WDEF-007, WDEF-014): items filed by reach class in cells sized for the class, so a query looks at
//! the few cells near its cap and not at every item of the 40 km cells round it (SPIKE-29: a height query in a cluster of 20,000 items cost
//! 2.4 ms, all of it the scan of the candidates).
//!
//! An item is filed in the class whose largest reach (`CLASS_REACH_M`) holds its reach, in the cell, of the class's level, that holds its
//! direction. The cell of a class is no narrower than twice its largest reach (a nominal edge of at least `2 reach`, a cell no narrower than half
//! of it), so a query's cap reaches the items of a class that can touch it from 2 rings of cells around the cap's centre, more for a larger
//! cap. A cap whose rings hold more cells than the class has entries to look at (a lookup costs about [`LOOKUP_COST`] entries) takes the whole class. The candidates are a superset of what the touch test accepts
//! ([`footprint::touches`]): each one is checked cheaply against the cap first (the same inequality on the chord, widened), and the caller
//! applies the exact test, so the result is the set the scan of every item gives.

use std::collections::BTreeMap;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};

use crate::cells::{self, with_margin, MAX_CELL_LEVEL};
use crate::footprint::{Cap, FOOTPRINT_ABS_MARGIN_RAD, FOOTPRINT_REL_MARGIN};
use crate::item_id::ItemId;

/// The largest reach of each class, metres: an item is in the first class whose largest reach it does not exceed. The last is the largest
/// reach an item may have ([`cells::CELL_EDGE_M`]).
pub const CLASS_REACH_M: [f64; 5] = [400.0, 1_600.0, 6_400.0, 25_600.0, cells::CELL_EDGE_M];

/// What a lookup of a cell costs against looking at one entry: the cells of a query are looked up only while that costs less than the entries of
/// the class.
pub const LOOKUP_COST: f64 = 16.0;

/// An item as the index holds it: its unit direction and its reach, for the cheap check.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    dir: Vec3,
    reach_m: f64,
}

/// The items of one reach class: the level of its cells, and the items by cell.
#[derive(Clone, Debug, PartialEq)]
struct Class {
    level: u8,
    max_reach_m: f64,
    cells: BTreeMap<u64, BTreeMap<ItemId, Entry>>,
    /// The entries of the class.
    count: usize,
}

/// The index of one body.
#[derive(Clone, Debug, PartialEq)]
pub struct ReachIndex {
    radius_m: f64,
    classes: Vec<Class>,
}

fn class_of(reach_m: f64) -> usize {
    CLASS_REACH_M.iter().position(|max| reach_m <= *max).unwrap_or(CLASS_REACH_M.len() - 1)
}

impl ReachIndex {
    /// An empty index for a body of `radius_m`.
    pub fn new(radius_m: f64) -> ReachIndex {
        let classes = CLASS_REACH_M
            .iter()
            .map(|&max_reach_m| {
                // The deepest level whose cells are at least twice the class's largest reach wide (nominally).
                let quarter_circle_m = core::f64::consts::FRAC_PI_2 * radius_m;
                let level =
                    libm::floor(libm::log2((quarter_circle_m / (2.0 * max_reach_m)).max(1.0))).clamp(0.0, f64::from(MAX_CELL_LEVEL)) as u8;
                Class { level, max_reach_m, cells: BTreeMap::new(), count: 0 }
            })
            .collect();
        ReachIndex { radius_m, classes }
    }

    fn cell(&self, class: usize, dir: Vec3) -> TileId {
        TileId::from_direction(&TangentWarp, dir, self.classes[class].level)
    }

    /// File an item (replacing one of the same id and place).
    pub fn insert(&mut self, id: ItemId, dir: Vec3, reach_m: f64) {
        let c = class_of(reach_m);
        let cell = self.cell(c, dir).raw();
        if self.classes[c].cells.entry(cell).or_default().insert(id, Entry { dir, reach_m }).is_none() {
            self.classes[c].count += 1;
        }
    }

    /// Take an item out, given the direction and reach it was filed with.
    pub fn remove(&mut self, id: ItemId, dir: Vec3, reach_m: f64) {
        let c = class_of(reach_m);
        let cell = self.cell(c, dir).raw();
        let class = &mut self.classes[c];
        if let Some(items) = class.cells.get_mut(&cell) {
            if items.remove(&id).is_some() {
                class.count -= 1;
            }
            if items.is_empty() {
                class.cells.remove(&cell);
            }
        }
    }

    /// How many items are filed.
    pub fn len(&self) -> usize {
        self.classes.iter().map(|c| c.count).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The candidates for `cap`: every item the touch test can accept and some it does not, in no order. `visited` (if given) is set to the number
    /// of entries looked at, before the cheap check.
    pub fn candidates(&self, cap: Cap, visited: Option<&mut usize>) -> Vec<ItemId> {
        // The chord test wants a unit centre (the angle of the touch test does not).
        let cap = Cap { centre: cap.centre.normalized(), ..cap };
        let mut out = Vec::new();
        let mut seen = 0usize;
        let whole_sphere = !(cap.angle.is_finite() && cap.angle < core::f64::consts::PI);
        for (k, class) in self.classes.iter().enumerate() {
            if class.cells.is_empty() {
                continue;
            }
            let nominal_edge_m = core::f64::consts::FRAC_PI_2 * self.radius_m / f64::from(1u32 << class.level);
            let reach_m = cap.angle * self.radius_m + class.max_reach_m;
            let rings = libm::ceil(reach_m / (0.5 * nominal_edge_m)) + 1.0;
            let mut take = |items: &BTreeMap<ItemId, Entry>| {
                for (id, e) in items {
                    seen += 1;
                    if may_touch(cap, e, self.radius_m) {
                        out.push(*id);
                    }
                }
            };
            let lookups = (2.0 * rings + 1.0) * (2.0 * rings + 1.0);
            if whole_sphere || !(rings.is_finite() && lookups * LOOKUP_COST <= class.count as f64) {
                class.cells.values().for_each(&mut take);
            } else {
                let centre = self.cell(k, cap.centre);
                for c in with_margin(&[centre], rings as u32) {
                    if let Some(items) = class.cells.get(&c.raw()) {
                        take(items);
                    }
                }
            }
        }
        if let Some(v) = visited {
            *v = seen;
        }
        out
    }
}

/// The inequality of [`footprint::touches`](crate::footprint::touches) on the chord instead of the angle, with room to spare, so that it never
/// rejects an item the touch test accepts (the margin is far larger than the rounding of either side).
fn may_touch(cap: Cap, e: &Entry, radius_m: f64) -> bool {
    let limit = (cap.angle + e.reach_m / radius_m) * (1.0 + FOOTPRINT_REL_MARGIN) + FOOTPRINT_ABS_MARGIN_RAD + 1e-9;
    if limit >= core::f64::consts::PI {
        return true;
    }
    let chord = 2.0 * libm::sin(0.5 * limit);
    let d = e.dir - cap.centre;
    d.dot(d) <= chord * chord
}
