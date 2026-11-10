//! Preview level 1 (A3 11.1, AUTH-007 to AUTH-010, GEN-157): the H0 re-route over an edit, planet-wide with the base heights resident, and the trunk rivers and
//! watersheds as plain data for an overlay. A height or landform stamp edit changes the heights at the cell centres in the stamps' reach and the sea mask; a rainfall or temperature region edit
//! changes the discharge weight and keeps the routing. The preview is a non-authoritative field with an id of its own (CON-07), decided outside the renderer (CON-03), and never stored or keyed
//! as an authoritative field. The preview reads no stored field by itself: the caller hands the resident heights in, or builds the base from a field source ([`PreviewBase::from_field`]).

use super::h0::{GlobalGrid, GlobalRouting, NONE};
use crate::global::{read_direction, read_elevation_c1_cached, LatticeSource, ReadError, StencilCache};
use crate::runoff::{discharge_weight, runoff_mm};
use crate::stamp::apply_stamps;
use planet_core::cube::FaceMapping;
use planet_core::Vec3;
use planet_world_def::footprint::FootprintItem;

/// The version of the preview's definition, folded into every id.
pub const PREVIEW_VERSION: u64 = 1;

/// The radius of the planet the class-source base is sampled on, metres (the reference body).
pub const PLANET_RADIUS_M: f64 = 6_371_000.0;

/// The domain tag of a preview id: no authoritative key carries it (CON-07).
const PREVIEW_DOMAIN: u64 = 0x7072_6576_6965_7731; // "preview1"

/// A rainfall or temperature region: a cap of `reach_m` round `dir` where the annual rainfall is multiplied by up to `rain_factor` and the temperature raised by up to `temp_offset_c`, both fading smoothly to nothing at the rim.
#[derive(Clone, Debug, PartialEq)]
pub struct ClimateRegion {
    pub dir: Vec3,
    pub reach_m: f64,
    pub rain_factor: f64,
    pub temp_offset_c: f64,
}

/// The resident fields of the base, one value a cell: the height (metres above the reference sphere), the annual rainfall and the temperature.
#[derive(Clone, Debug)]
pub struct BaseFields {
    pub z0: Vec<f64>,
    pub p_mm: Vec<f32>,
    pub t_c: Vec<f32>,
}

/// An edit of the preview: stamps (in application order, the caller's) and climate regions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewEdit {
    pub stamps: Vec<FootprintItem>,
    pub climate: Vec<ClimateRegion>,
}

/// The resident base of a preview session, built once: the grid, the sea level, the base heights and climate per cell, and the routing and discharge of the base.
pub struct PreviewBase {
    grid: GlobalGrid,
    radius_m: f64,
    sea_level: f64,
    epsilon: f64,
    z0: Vec<f64>,
    p_mm: Vec<f32>,
    t_c: Vec<f32>,
    word: u64,
    routing0: GlobalRouting,
    discharge0: Vec<f64>,
}

/// The overlay of a preview, as data for the frame plan (A3 11.2): no solve is requested by drawing it.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    /// The cells whose discharge is at least the threshold, in id order.
    pub trunk: Vec<u32>,
    /// The watershed of every cell: the outlet it drains to.
    pub watershed: Vec<u32>,
    /// The cells whose receiver or watershed differs from the base's, in id order.
    pub changed: Vec<u32>,
}

/// The seconds of each phase of a preview (TEST-099).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Phases {
    pub stamps_s: f64,
    pub route_s: f64,
    pub discharge_s: f64,
    pub overlay_s: f64,
}

/// A level-1 preview.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub id: u64,
    pub routing: GlobalRouting,
    /// The discharge of every cell, in cubic-metres-a-year equivalents of the weights (area times weight, accumulated).
    pub discharge: Vec<f64>,
    pub overlay: Overlay,
}

fn mix(h: u64, v: u64) -> u64 {
    // splitmix64 finaliser over the running word and the value: integer only, the same bits everywhere (CON-14).
    let mut x = h ^ v.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_add(h << 6).wrapping_add(h >> 2);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl PreviewEdit {
    /// The edit's own validity: every climate region has a finite direction, a positive finite reach and finite factors.
    pub fn validate(&self) -> Result<(), String> {
        for (i, r) in self.climate.iter().enumerate() {
            let finite = [r.dir.x, r.dir.y, r.dir.z, r.reach_m, r.rain_factor, r.temp_offset_c].iter().all(|v| v.is_finite());
            if !finite || r.reach_m <= 0.0 || r.rain_factor < 0.0 {
                return Err(format!(
                    "climate region {i} is not valid: its numbers are finite, its reach positive and its rainfall factor not negative"
                ));
            }
        }
        Ok(())
    }

    /// The content words of the edit: the stamps' content hashes in order and the regions' bits.
    fn words(&self) -> Vec<u64> {
        let mut w = vec![self.stamps.len() as u64, self.climate.len() as u64];
        for s in &self.stamps {
            // The footprint hash of the item: layer, priority, content, anchor, reach and height bound (a stamp's own content hash holds none of the last three).
            let h = planet_world_def::footprint::item_hash128(s);
            w.extend([u64::from_le_bytes(h[..8].try_into().expect("8 bytes")), u64::from_le_bytes(h[8..].try_into().expect("8 bytes"))]);
        }
        for c in &self.climate {
            w.extend([
                c.dir.x.to_bits(),
                c.dir.y.to_bits(),
                c.dir.z.to_bits(),
                c.reach_m.to_bits(),
                c.rain_factor.to_bits(),
                c.temp_offset_c.to_bits(),
            ]);
        }
        w
    }
}

/// The accumulated discharge: every cell's area times its weight, passed down the receivers (donors in a fixed order, so the sums are the same bits on any thread count).
pub fn discharge(grid: &GlobalGrid, routing: &GlobalRouting, weight: impl Fn(usize) -> f64) -> Vec<f64> {
    let n = grid.len();
    let mut q: Vec<f64> = (0..n).map(|c| grid.area(c as u32) * weight(c)).collect();
    let mut donors = vec![0u32; n];
    for c in 0..n {
        if routing.receiver[c] != NONE {
            donors[routing.receiver[c] as usize] += 1;
        }
    }
    let mut stack: Vec<u32> = (0..n as u32).rev().filter(|&c| donors[c as usize] == 0).collect();
    while let Some(c) = stack.pop() {
        let r = routing.receiver[c as usize];
        if r == NONE {
            continue;
        }
        q[r as usize] += q[c as usize];
        donors[r as usize] -= 1;
        if donors[r as usize] == 0 {
            stack.push(r);
        }
    }
    q
}

fn fade(x: f64) -> f64 {
    // 1 at the centre, 0 at the rim, smooth (the smoothstep of the remaining distance).
    let u = (1.0 - x).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

impl PreviewBase {
    /// The base of a session: the `fields` of every cell, `word` the base's identity word (what the base is made from; it goes into every preview id), cells at or below `sea_level` the sea.
    pub fn new(grid: GlobalGrid, radius_m: f64, sea_level: f64, epsilon: f64, fields: BaseFields, word: u64) -> PreviewBase {
        let BaseFields { z0, p_mm, t_c } = fields;
        let n = grid.len();
        assert!(z0.len() == n && p_mm.len() == n && t_c.len() == n, "one height, one rainfall and one temperature per cell");
        let outlet: Vec<bool> = z0.iter().map(|z| *z <= sea_level).collect();
        let routing0 = grid.route(&z0, &outlet, epsilon);
        let w0 = |c: usize| discharge_weight(runoff_mm(f64::from(p_mm[c]), f64::from(t_c[c])));
        let discharge0 = discharge(&grid, &routing0, w0);
        PreviewBase { grid, radius_m, sea_level, epsilon, z0, p_mm, t_c, word, routing0, discharge0 }
    }

    /// The base of a session from the default class source (GEN-115): the height, rainfall and temperature of every H0 cell of `n` cells a face edge (`class_sample` at the cell centres, rows dealt to
    /// `threads` threads; each cell is its own sample, so the result does not depend on the thread count), the sea at 0 m. The base word is the seed's, so another seed is another preview id.
    pub fn from_class_source(n: usize, seed: u64, threads: usize) -> PreviewBase {
        let grid = GlobalGrid::new(n, PLANET_RADIUS_M);
        let inputs = crate::bake::inputs::BakeInputs::new(seed, threads);
        let len = grid.len();
        let (mut z0, mut p, mut t) = (vec![0f64; len], vec![0f32; len], vec![0f32; len]);
        let chunk = len.div_ceil(threads.max(1));
        std::thread::scope(|s| {
            for (k, ((zc, pc), tc)) in z0.chunks_mut(chunk).zip(p.chunks_mut(chunk)).zip(t.chunks_mut(chunk)).enumerate() {
                let (grid, inputs) = (&grid, &inputs);
                s.spawn(move || {
                    for (i, ((z, pp), tt)) in zc.iter_mut().zip(pc.iter_mut()).zip(tc.iter_mut()).enumerate() {
                        let sample = crate::global::class_sample(seed, grid.direction((k * chunk + i) as u32), inputs);
                        *z = f64::from(sample.elevation_m);
                        *pp = sample.precipitation_mm;
                        *tt = sample.temperature_c;
                    }
                });
            }
        });
        PreviewBase::new(grid, PLANET_RADIUS_M, 0.0, 1e-3, BaseFields { z0, p_mm: p, t_c: t }, PREVIEW_DOMAIN ^ seed)
    }

    /// The base of a session read from a field source, a stored bake or an in-memory grid (item 3, GEN-164): the height of every H0 cell of `n` cells a face edge is the C1 read of the
    /// source at the cell's centre ([`read_elevation_c1_cached`]), the rainfall and the temperature the bilinear read ([`read_direction`]); samples beyond a face edge come from the face
    /// it meets, so a cell on a cube edge or at a corner reads the same field from every side. A source that cannot answer is an error, never a placeholder. The base word is the caller's.
    pub fn from_field(n: usize, src: &dyn LatticeSource, map: &dyn FaceMapping, word: u64) -> Result<PreviewBase, ReadError> {
        if !(2..=26_000).contains(&n) {
            return Err(ReadError::Shape(format!("an H0 base needs between 2 and 26,000 cells a face edge, not {n}")));
        }
        if src.records_reads() {
            return Err(ReadError::Source(
                "a source that only records the cells asked for answers placeholders: no base is built from it".into(),
            ));
        }
        let grid = GlobalGrid::new(n, PLANET_RADIUS_M);
        let len = grid.len();
        let (mut z0, mut p, mut t) = (vec![0f64; len], vec![0f32; len], vec![0f32; len]);
        let mut cache = StencilCache::new();
        for id in 0..len {
            let d = grid.direction(id as u32);
            z0[id] = f64::from(read_elevation_c1_cached(src, map, d, &mut cache, 0)?);
            let s = read_direction(src, map, d)?;
            (p[id], t[id]) = (s.precipitation_mm, s.temperature_c);
        }
        if let Some(bad) = z0.iter().position(|v| !v.is_finite()) {
            return Err(ReadError::Source(format!("the field reads a height that is not a number at H0 cell {bad}")));
        }
        if z0.iter().all(|&v| v > 0.0) {
            return Err(ReadError::Shape("the field has no cell at or below the sea level: H0 needs an outlet".into()));
        }
        Ok(PreviewBase::new(grid, PLANET_RADIUS_M, 0.0, 1e-3, BaseFields { z0, p_mm: p, t_c: t }, word))
    }

    /// The base height of every cell, metres.
    pub fn heights(&self) -> &[f64] {
        &self.z0
    }

    /// The base annual rainfall of every cell, millimetres.
    pub fn rainfall(&self) -> &[f32] {
        &self.p_mm
    }

    /// The base temperature of every cell, degrees Celsius.
    pub fn temperature(&self) -> &[f32] {
        &self.t_c
    }

    pub fn grid(&self) -> &GlobalGrid {
        &self.grid
    }

    pub fn base_routing(&self) -> &GlobalRouting {
        &self.routing0
    }

    /// The id of the preview of `edit` on this base: a hash of the domain tag, the version, the base's word and the edit's content.
    pub fn id_of(&self, edit: &PreviewEdit) -> u64 {
        let mut h = mix(PREVIEW_DOMAIN, PREVIEW_VERSION);
        h = mix(h, self.word);
        for v in [self.radius_m.to_bits(), self.sea_level.to_bits(), self.epsilon.to_bits()] {
            h = mix(h, v);
        }
        for w in edit.words() {
            h = mix(h, w);
        }
        h
    }

    /// The heights of the base with the stamps of `edit` added at the cell centres: a cell outside every stamp's reach keeps the base's bits.
    pub fn edited_heights(&self, edit: &PreviewEdit) -> Vec<f64> {
        let mut z = self.z0.clone();
        if edit.stamps.is_empty() {
            return z;
        }
        // A cap per stamp (anchor, reach angle with a small margin): the cheap test before `apply_stamps`, which decides exactly.
        let caps: Vec<(Vec3, f64)> = edit
            .stamps
            .iter()
            .map(|s| {
                let a = Vec3::new(f64::from_bits(s.dir[0]), f64::from_bits(s.dir[1]), f64::from_bits(s.dir[2]));
                let angle = f64::from_bits(s.reach_bits) / self.radius_m * 1.001 + 1e-9;
                // A cosine with an absolute slack far above the rounding of a dot product of unit vectors (a few ulp of 1), so no cell the exact test would take is skipped; a reach of the whole sphere takes every cell.
                (a, if angle >= core::f64::consts::PI { -2.0 } else { libm::cos(angle) - 1e-12 })
            })
            .collect();
        for (c, h) in z.iter_mut().enumerate() {
            let d = self.grid.direction(c as u32);
            if caps.iter().any(|(a, cos)| a.x * d.x + a.y * d.y + a.z * d.z >= *cos) {
                *h = apply_stamps(&edit.stamps, self.radius_m, d, self.z0[c]);
            }
        }
        z
    }

    /// The discharge weight of every cell with the climate regions of `edit` applied.
    fn weights(&self, edit: &PreviewEdit) -> Vec<f64> {
        (0..self.z0.len())
            .map(|c| {
                let (mut p, mut t) = (f64::from(self.p_mm[c]), f64::from(self.t_c[c]));
                if !edit.climate.is_empty() {
                    let d = self.grid.direction(c as u32);
                    for r in &edit.climate {
                        let chord = Vec3::new(d.x - r.dir.x, d.y - r.dir.y, d.z - r.dir.z);
                        let angle =
                            2.0 * libm::asin((0.5 * libm::sqrt(chord.x * chord.x + chord.y * chord.y + chord.z * chord.z)).min(1.0));
                        let w = fade(self.radius_m * angle / r.reach_m);
                        if w > 0.0 {
                            p *= 1.0 + (r.rain_factor - 1.0) * w;
                            t += r.temp_offset_c * w;
                        }
                    }
                }
                discharge_weight(runoff_mm(p, t))
            })
            .collect()
    }

    /// The level-1 preview of `edit`: the H0 route over the edited heights (the base routing when no height changes), the discharge, and the overlay with the trunk threshold `trunk_discharge`. An edit that is not valid (a climate region with a non-finite or non-positive number, a stamp that makes a height not finite) or that leaves no sea is an error, never a panic: a drag drops it.
    pub fn preview(&self, edit: &PreviewEdit, trunk_discharge: f64) -> Result<Preview, String> {
        self.preview_timed(edit, trunk_discharge).map(|(p, _)| p)
    }

    /// [`PreviewBase::preview`] with the seconds of each phase (TEST-099): the stamp pass, the flood and routing, the discharge, the overlay. The clock reads nothing that enters the result.
    pub fn preview_timed(&self, edit: &PreviewEdit, trunk_discharge: f64) -> Result<(Preview, Phases), String> {
        edit.validate()?;
        let mut phases = Phases::default();
        let t = std::time::Instant::now();
        let heights_change = !edit.stamps.is_empty();
        let routing = if heights_change {
            let z = self.edited_heights(edit);
            phases.stamps_s = t.elapsed().as_secs_f64();
            if z.iter().any(|v| !v.is_finite()) {
                return Err("the stamps make a height that is not finite".into());
            }
            let outlet: Vec<bool> = z.iter().map(|v| *v <= self.sea_level).collect();
            if !outlet.iter().any(|o| *o) {
                return Err("the edit leaves no sea: a planet needs an outlet".into());
            }
            let top = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if top + self.epsilon <= top {
                return Err(format!(
                    "the highest height {top} m is beyond the flood epsilon {}: the route could not slope over a flat",
                    self.epsilon
                ));
            }
            let t = std::time::Instant::now();
            let r = self.grid.route(&z, &outlet, self.epsilon);
            phases.route_s = t.elapsed().as_secs_f64();
            r
        } else {
            self.routing0.clone()
        };
        let t = std::time::Instant::now();
        let w = self.weights(edit);
        let q = discharge(&self.grid, &routing, |c| w[c]);
        phases.discharge_s = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let trunk: Vec<u32> = (0..q.len() as u32).filter(|&c| q[c as usize] >= trunk_discharge).collect();
        let changed: Vec<u32> = (0..q.len() as u32)
            .filter(|&c| {
                routing.receiver[c as usize] != self.routing0.receiver[c as usize]
                    || routing.outlet_of[c as usize] != self.routing0.outlet_of[c as usize]
            })
            .collect();
        let watershed = routing.outlet_of.clone();
        phases.overlay_s = t.elapsed().as_secs_f64();
        Ok((Preview { id: self.id_of(edit), routing, discharge: q, overlay: Overlay { trunk, watershed, changed } }, phases))
    }

    /// The base's own discharge (the reference the climate preview is compared to).
    pub fn base_discharge(&self) -> &[f64] {
        &self.discharge0
    }
}

/// The latest-wins slot of a drag (AUTH-010, ADR 0072 point 3): `submit` gives a ticket, and only the newest ticket's result is accepted.
#[derive(Debug, Default)]
pub struct PreviewSlot {
    latest: u64,
}

impl PreviewSlot {
    pub fn submit(&mut self) -> u64 {
        self.latest += 1;
        self.latest
    }

    /// True when `ticket` is the newest submitted: its result is taken; an older ticket's result is dropped.
    pub fn accept(&self, ticket: u64) -> bool {
        ticket == self.latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_world_def::stamp::{Stamp, StampKind};

    const R: f64 = 6_371_000.0;

    /// A small planet: n cells a face edge, land where a smooth function of the direction is above 0.
    fn base(n: usize) -> PreviewBase {
        let grid = GlobalGrid::new(n, R);
        let len = grid.len();
        let z0: Vec<f64> = (0..len as u32)
            .map(|c| {
                let d = grid.direction(c);
                500.0 * d.x + 300.0 * d.y * d.z + 200.0 * libm::sin(3.0 * d.z) + 40.0 * d.x * d.y
            })
            .collect();
        let p: Vec<f32> = vec![800.0; len];
        let t: Vec<f32> = vec![12.0; len];
        PreviewBase::new(grid, R, 0.0, 1e-3, BaseFields { z0, p_mm: p, t_c: t }, 0x1234)
    }

    /// The great-circle distance in metres between two unit directions.
    fn arc(a: Vec3, b: Vec3) -> f64 {
        let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
        R * 2.0 * libm::asin((0.5 * libm::sqrt(dx * dx + dy * dy + dz * dz)).min(1.0))
    }

    fn hill_at(b: &PreviewBase, cell: u32, reach_m: f64, amount: f64) -> FootprintItem {
        let d = b.grid().direction(cell);
        FootprintItem {
            id: u128::from(cell),
            priority: 0,
            // One content hash for every hill: a moved or longer stamp must change the id through the anchor and the reach alone.
            content: 0xAB00,
            dir: [d.x.to_bits(), d.y.to_bits(), d.z.to_bits()],
            reach_bits: reach_m.to_bits(),
            max_delta_bits: amount.to_bits(),
            layer: "l".into(),
            stamp: Some(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0])),
        }
    }

    fn land_cell(b: &PreviewBase) -> u32 {
        (0..b.grid().len() as u32).find(|&c| b.z0[c as usize] > 150.0).expect("some land")
    }

    // spec: AUTH-007
    #[test]
    fn the_id_follows_the_base_and_the_edit_and_carries_a_domain_of_its_own() {
        let b = base(12);
        let c = land_cell(&b);
        let e1 = PreviewEdit { stamps: vec![hill_at(&b, c, 600_000.0, 800.0)], climate: vec![] };
        let e1_again = e1.clone();
        let e2 = PreviewEdit { stamps: vec![hill_at(&b, c, 600_000.0, 900.0)], climate: vec![] };
        let e3 = PreviewEdit {
            stamps: e1.stamps.clone(),
            climate: vec![ClimateRegion { dir: b.grid().direction(c), reach_m: 500_000.0, rain_factor: 2.0, temp_offset_c: 0.0 }],
        };
        let mut other = base(12);
        other.word = 0x9999;
        let ids = [b.id_of(&e1), b.id_of(&e1_again), b.id_of(&e2), b.id_of(&e3), other.id_of(&e1), b.id_of(&PreviewEdit::default())];
        assert_eq!(ids[0], ids[1], "the same base and edit, the same id");
        for i in [2, 3, 4, 5] {
            assert!(ids[i] != ids[0], "edit or base {i} changes the id");
        }
        let mut sorted = ids[2..].to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "the other ids differ from each other");
        // The same stamp content at another cell, with another reach, and with another height bound: each is another id.
        let c2 = (0..b.grid().len() as u32).find(|&k| k != c && b.z0[k as usize] > 150.0).unwrap();
        let moved = PreviewEdit { stamps: vec![hill_at(&b, c2, 600_000.0, 800.0)], climate: vec![] };
        let longer = PreviewEdit { stamps: vec![hill_at(&b, c, 700_000.0, 800.0)], climate: vec![] };
        assert!(b.id_of(&moved) != ids[0] && b.id_of(&longer) != ids[0] && b.id_of(&moved) != b.id_of(&longer));
        // The domain tag is mixed first: the id of the empty edit is not a function of the base word alone.
        assert!(ids[5] != mix(0, b.word) && ids[5] != b.word);
    }

    // spec: GEN-157
    #[test]
    fn the_edited_heights_keep_the_base_bits_outside_the_reach_and_the_reroute_equals_a_fresh_route() {
        let b = base(16);
        let c = land_cell(&b);
        let edit = PreviewEdit { stamps: vec![hill_at(&b, c, 900_000.0, 1500.0)], climate: vec![] };
        let z = b.edited_heights(&edit);
        let anchor = b.grid().direction(c);
        let mut changed = 0;
        for (i, h) in z.iter().enumerate() {
            let arc_m = arc(b.grid().direction(i as u32), anchor);
            if arc_m > 900_000.0 * 1.01 {
                assert_eq!(h.to_bits(), b.z0[i].to_bits(), "cell {i} is outside the reach");
            } else if *h != b.z0[i] {
                changed += 1;
            }
        }
        assert!(changed > 0, "the hill changes the heights inside its reach");
        // Not circular: every cell is what `apply_stamps` gives there, for stamps of a normal reach and of a metre or a few centimetres anchored on a cell centre.
        for (reach, amount) in [(900_000.0, 1500.0), (3.0, 40.0), (1.0, 40.0), (0.05, 40.0)] {
            let e = PreviewEdit { stamps: vec![hill_at(&b, c, reach, amount)], climate: vec![] };
            let zz = b.edited_heights(&e);
            for (k, h) in zz.iter().enumerate() {
                let want = apply_stamps(&e.stamps, R, b.grid().direction(k as u32), b.z0[k]);
                assert_eq!(h.to_bits(), want.to_bits(), "reach {reach} m, cell {k}");
            }
            assert!(zz[c as usize] > b.z0[c as usize], "the stamp's own cell is raised at reach {reach} m");
        }
        let p = b.preview(&edit, f64::INFINITY).unwrap();
        let outlet: Vec<bool> = z.iter().map(|v| *v <= 0.0).collect();
        let fresh = b.grid().route(&z, &outlet, 1e-3);
        assert_eq!(p.routing, fresh, "the preview's route is a route of the edited heights, bit for bit");
        assert_eq!(p.overlay.watershed, fresh.outlet_of);
    }

    // spec: AUTH-008
    #[test]
    fn a_hill_on_a_divide_changes_the_routing_near_it_and_a_rainfall_region_only_the_discharge() {
        let b = base(16);
        let c = land_cell(&b);
        let hill = PreviewEdit { stamps: vec![hill_at(&b, c, 700_000.0, 2500.0)], climate: vec![] };
        let p = b.preview(&hill, f64::INFINITY).unwrap();
        assert!(!p.overlay.changed.is_empty(), "the hill changes receivers or watersheds");
        let anchor = b.grid().direction(c);
        let far = p.overlay.changed.iter().filter(|&&k| arc(b.grid().direction(k), anchor) > 700_000.0 * 3.0);
        // Routing changes stay in the neighbourhood of the hill: not a planet-wide reshuffle (cells far from it may follow a changed outlet, so they are counted, not forbidden).
        assert!(far.count() < p.overlay.changed.len(), "most changes are near the hill");

        let rain = PreviewEdit {
            stamps: vec![],
            climate: vec![ClimateRegion { dir: b.grid().direction(c), reach_m: 2_000_000.0, rain_factor: 3.0, temp_offset_c: 0.0 }],
        };
        let q = b.preview(&rain, f64::INFINITY).unwrap();
        assert!(q.overlay.changed.is_empty(), "no height changed: the routing is the base's");
        assert_eq!(q.routing, *b.base_routing());
        let raised = (0..q.discharge.len()).filter(|&i| q.discharge[i] > b.base_discharge()[i] * 1.0000001).count();
        assert!(raised > 0, "more rain, more discharge below the region");
        assert!(
            (0..q.discharge.len()).all(|i| q.discharge[i] >= b.base_discharge()[i] * (1.0 - 1e-12) || b.z0[i] <= 0.0),
            "no cell on land loses discharge from more rain"
        );
    }

    // spec: AUTH-008
    #[test]
    fn the_trunk_cells_are_those_over_the_threshold_and_a_discharge_sums_the_weighted_areas_at_the_outlets() {
        let b = base(12);
        let p = b.preview(&PreviewEdit::default(), 0.0).unwrap();
        assert_eq!(p.overlay.trunk.len(), b.grid().len(), "a threshold of 0 takes every cell");
        let top = p.discharge.iter().cloned().fold(0.0, f64::max);
        let p2 = b.preview(&PreviewEdit::default(), top).unwrap();
        assert!(!p2.overlay.trunk.is_empty() && p2.overlay.trunk.len() < p.overlay.trunk.len());
        // Conservation: the sum of the discharge at the outlets is the sum of area times weight over every cell.
        let total: f64 = (0..b.grid().len()).map(|c| b.grid().area(c as u32) * discharge_weight(runoff_mm(800.0, 12.0))).sum();
        let at_outlets: f64 = (0..b.grid().len()).filter(|&c| p.routing.receiver[c] == NONE).map(|c| p.discharge[c]).sum();
        assert!((at_outlets - total).abs() <= 1e-9 * total, "{at_outlets} against {total}");
    }

    // spec: AUTH-008
    #[test]
    fn an_invalid_climate_region_and_an_edit_that_leaves_no_sea_are_errors_not_panics() {
        let b = base(12);
        let c = land_cell(&b);
        let region = |reach: f64, rain: f64| PreviewEdit {
            stamps: vec![],
            climate: vec![ClimateRegion { dir: b.grid().direction(c), reach_m: reach, rain_factor: rain, temp_offset_c: 0.0 }],
        };
        for (reach, rain) in [(0.0, 2.0), (-5.0, 2.0), (1e6, f64::NAN), (f64::INFINITY, 2.0), (1e6, -1.0)] {
            assert!(b.preview(&region(reach, rain), 0.0).is_err(), "reach {reach}, rain {rain}");
        }
        assert!(b.preview(&region(1e6, 2.0), 0.0).is_ok());
        // A plateau at 5,000 m over the whole sphere's reach lifts every cell above the sea: no outlet.
        let mut all = hill_at(&b, c, 40_000_000.0, 9000.0);
        all.stamp = Some(Stamp::new(StampKind::Plateau, [5000.0, 0.0, 0.0]));
        let e = b.preview(&PreviewEdit { stamps: vec![all], climate: vec![] }, 0.0);
        assert!(e.is_err(), "no sea: {:?}", e.map(|p| p.id));
    }

    // spec: TEST-099
    #[test]
    fn the_timed_preview_is_the_preview_and_times_only_the_phases_that_ran() {
        let b = base(12);
        let c = land_cell(&b);
        let hill = PreviewEdit { stamps: vec![hill_at(&b, c, 700_000.0, 2500.0)], climate: vec![] };
        let (p, ph) = b.preview_timed(&hill, 1.0e12).unwrap();
        assert_eq!(p, b.preview(&hill, 1.0e12).unwrap(), "the same result as the untimed call");
        assert!([ph.stamps_s, ph.route_s, ph.discharge_s, ph.overlay_s].iter().all(|v| v.is_finite() && *v >= 0.0));
        let rain = PreviewEdit {
            stamps: vec![],
            climate: vec![ClimateRegion { dir: b.grid().direction(c), reach_m: 1_000_000.0, rain_factor: 2.0, temp_offset_c: 0.0 }],
        };
        let (_, ph) = b.preview_timed(&rain, 1.0e12).unwrap();
        assert_eq!((ph.stamps_s, ph.route_s), (0.0, 0.0), "a climate edit runs no stamp pass and no route");
    }

    // spec: AUTH-010
    #[test]
    fn only_the_newest_ticket_of_a_drag_is_accepted() {
        let mut s = PreviewSlot::default();
        let (a, b, c) = (s.submit(), s.submit(), s.submit());
        assert!(!s.accept(b) && s.accept(c) && !s.accept(a), "arrival order second, third, first: only the third counts");
    }

    // spec: TEST-098
    #[test]
    fn the_preview_tests_build_grids_of_at_most_32_cells_a_face_edge() {
        let src = include_str!("preview.rs");
        let n: Vec<usize> = src.lines().filter_map(|l| l.trim().strip_prefix("let b = base(")?.strip_suffix(");")?.parse().ok()).collect();
        assert!(!n.is_empty() && n.iter().all(|&k| k <= 32), "{n:?}");
    }

    // spec: AUTH-009
    #[test]
    fn the_preview_module_names_no_graphics_crate() {
        let src = include_str!("preview.rs");
        let head = src.split("#[cfg(test)]").next().unwrap();
        for banned in ["wgpu", "planet_render", "winit"] {
            assert!(!head.contains(banned), "{banned} in a GPU-free module");
        }
    }
}
