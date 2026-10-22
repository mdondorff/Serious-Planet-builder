//! Liquid kinds and the sea of a body as world data (WDEF-006, ADR 0041 points 1 and 3, owner's M5 constraint 3).
//!
//! A **liquid kind** is a row of a parameter table in the definition, not a code enum: an id and nine numbers, the nodes
//! `liquid.<k>.colour_r|g|b`, `emission_r|g|b`, `viscosity_pa_s`, `wave_amplitude_m` and `wave_length_m` (all floats). A world
//! that defines no `liquid.*` node has the Earth table of one liquid, id 0, water ([`LiquidTable::earth`]); a world that defines
//! any is the table it defines, and nothing is merged. So a definition that never mentions liquids hashes exactly as it did
//! before the table existed, and a definition with a table is a different definition with a different hash (WDEF-003).
//!
//! The **sea** of a body is three nodes, each optional with an Earth default: `body.<n>.sea.level_m` (the water level W of the
//! sea, 0), `body.<n>.sea.liquid` (the kind id, 0) and `body.<n>.sea.min_area_fraction` (the area threshold of the ocean
//! connectivity, a fraction of the body's surface, [`EARTH_OCEAN_MIN_AREA_FRACTION`], ADR 0041 point 2).

use crate::{BodyId, GenContext, MissingNode, NodeValue, WorldDefinition};
use std::collections::BTreeMap;

/// Identifies a liquid kind: the `<k>` of its `liquid.<k>.*` nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LiquidKindId(pub u32);

impl LiquidKindId {
    /// The Earth table's one liquid, water.
    pub const WATER: LiquidKindId = LiquidKindId(0);
}

/// The Earth default of the area threshold (ADR 0041 point 2): 1e6 km^2 of Earth's 4 pi (6371 km)^2, about 0.196 % of the
/// surface. Stated as a fraction of the body's surface so that other radii scale with it.
pub const EARTH_OCEAN_MIN_AREA_FRACTION: f64 = 1.0e6 / (4.0 * core::f64::consts::PI * 6371.0 * 6371.0);

/// The largest water level a sea may have, in metres from the reference surface: it reaches the shader as an `f32` height, so it is a
/// height, never a radius (CON-11).
pub const MAX_SEA_LEVEL_M: f64 = 1.0e5;

/// The water level of the sea by default: the reference surface.
pub const DEFAULT_SEA_LEVEL_M: f64 = 0.0;

/// One row of the liquid table. The class of every attribute (ADR 0035) is in the world-definition spec (WDEF-006): all nine
/// are world data (a key input where a generator reads them), none is a GPU twin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidKind {
    /// Base colour of the liquid in deep water, linear RGB in [0, 1].
    pub colour: [f64; 3],
    /// Emitted radiance in units of the scene's sun illuminance, linear RGB, each at least 0 (lava glows, water does not).
    pub emission: [f64; 3],
    /// Dynamic viscosity in Pa s, positive (water 1e-3, lava 1e2 to 1e5).
    pub viscosity_pa_s: f64,
    /// Wave height in m (half the crest-to-trough distance): 0 means no waves (a lava lake, a still pond).
    pub wave_amplitude_m: f64,
    /// Dominant wavelength in m, positive when there are waves.
    pub wave_length_m: f64,
}

/// The names of a liquid's nodes, in the order of [`LiquidKind::to_values`].
pub const LIQUID_FIELDS: [&str; 9] =
    ["colour_r", "colour_g", "colour_b", "emission_r", "emission_g", "emission_b", "viscosity_pa_s", "wave_amplitude_m", "wave_length_m"];

impl LiquidKind {
    /// Water at the Earth's surface: a dark blue-green deep colour, no emission, 1 mPa s, waves of 0.5 m every 60 m (a default
    /// the look file may override for presentation).
    pub const WATER: LiquidKind = LiquidKind {
        colour: [0.010, 0.060, 0.110],
        emission: [0.0, 0.0, 0.0],
        viscosity_pa_s: 1.0e-3,
        wave_amplitude_m: 0.5,
        wave_length_m: 60.0,
    };

    pub fn has_waves(&self) -> bool {
        self.wave_amplitude_m > 0.0
    }

    pub fn to_values(&self) -> [f64; 9] {
        [
            self.colour[0],
            self.colour[1],
            self.colour[2],
            self.emission[0],
            self.emission[1],
            self.emission[2],
            self.viscosity_pa_s,
            self.wave_amplitude_m,
            self.wave_length_m,
        ]
    }

    pub fn from_values(v: [f64; 9]) -> LiquidKind {
        LiquidKind {
            colour: [v[0], v[1], v[2]],
            emission: [v[3], v[4], v[5]],
            viscosity_pa_s: v[6],
            wave_amplitude_m: v[7],
            wave_length_m: v[8],
        }
    }

    /// Whether the numbers mean something: every one finite, the colour in [0, 1], emission and amplitude at least 0, the
    /// viscosity positive, the wavelength positive when there are waves. The error names the field.
    pub fn validate(&self) -> Result<(), String> {
        let v = self.to_values();
        for (name, x) in LIQUID_FIELDS.iter().zip(v) {
            if !x.is_finite() {
                return Err(format!("{name} is {x}, not a finite number"));
            }
        }
        for (name, x) in LIQUID_FIELDS[..3].iter().zip(self.colour) {
            if !(0.0..=1.0).contains(&x) {
                return Err(format!("{name} is {x}, a linear colour component from 0 to 1"));
            }
        }
        for (name, x) in LIQUID_FIELDS[3..6].iter().zip(self.emission) {
            if x < 0.0 {
                return Err(format!("{name} is {x}, an emitted radiance of at least 0"));
            }
        }
        if self.viscosity_pa_s <= 0.0 {
            return Err(format!("viscosity_pa_s is {}, a viscosity above 0", self.viscosity_pa_s));
        }
        if self.wave_amplitude_m < 0.0 {
            return Err(format!("wave_amplitude_m is {}, a height of at least 0 (0 means no waves)", self.wave_amplitude_m));
        }
        if self.wave_amplitude_m > 0.0 && self.wave_length_m <= 0.0 {
            return Err(format!("wave_length_m is {}, a length above 0 when there are waves", self.wave_length_m));
        }
        Ok(())
    }
}

/// The liquid kinds of a world by id.
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidTable {
    kinds: BTreeMap<LiquidKindId, LiquidKind>,
}

impl LiquidTable {
    /// The Earth table: one liquid, water, id 0.
    pub fn earth() -> LiquidTable {
        LiquidTable { kinds: BTreeMap::from([(LiquidKindId::WATER, LiquidKind::WATER)]) }
    }

    pub fn get(&self, id: LiquidKindId) -> Option<&LiquidKind> {
        self.kinds.get(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = LiquidKindId> + '_ {
        self.kinds.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

/// The sea of one body: where its water surface is, what it is made of and how big a low area must be to count as sea.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeaParams {
    pub level_m: f64,
    pub liquid: LiquidKindId,
    pub min_area_fraction: f64,
}

impl SeaParams {
    pub const EARTH: SeaParams =
        SeaParams { level_m: DEFAULT_SEA_LEVEL_M, liquid: LiquidKindId::WATER, min_area_fraction: EARTH_OCEAN_MIN_AREA_FRACTION };
}

pub fn liquid_path(kind: LiquidKindId, field: &str) -> String {
    format!("liquid.{}.{field}", kind.0)
}

pub fn body_sea_level_path(body: BodyId) -> String {
    format!("body.{}.sea.level_m", body.0)
}

pub fn body_sea_liquid_path(body: BodyId) -> String {
    format!("body.{}.sea.liquid", body.0)
}

pub fn body_sea_min_area_path(body: BodyId) -> String {
    format!("body.{}.sea.min_area_fraction", body.0)
}

/// The liquid id and field a path names (`liquid.<k>.<field>`), if it is one of the table's nodes.
pub(crate) fn liquid_node(path: &str) -> Option<(LiquidKindId, &str)> {
    let rest = path.strip_prefix("liquid.")?;
    let (k, field) = rest.split_once('.')?;
    // No leading zeros: `liquid.01` and `liquid.1` would be two paths for one kind.
    (!k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) && (k == "0" || !k.starts_with('0')) && LIQUID_FIELDS.contains(&field))
        .then(|| k.parse().ok().map(|k| (LiquidKindId(k), field)))
        .flatten()
}

/// The sea node of `body` that `path` is, if it is one: which of the three.
pub(crate) fn sea_node(path: &str) -> Option<&'static str> {
    let rest = path.strip_prefix("body.")?;
    let (index, leaf) = rest.split_once('.')?;
    if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    ["sea.level_m", "sea.liquid", "sea.min_area_fraction"].into_iter().find(|n| *n == leaf)
}

fn float(def: &WorldDefinition, path: &str) -> Result<Option<f64>, MissingNode> {
    match def.get(path) {
        None => Ok(None),
        Some(NodeValue::F64(v)) if v.is_finite() => Ok(Some(v)),
        Some(NodeValue::F64(_)) => Err(MissingNode(format!("{path} (not finite)"))),
        Some(NodeValue::U64(_)) => Err(MissingNode(format!("{path} (not a number)"))),
    }
}

impl WorldDefinition {
    /// Set liquid kind `id` (its nine nodes). A kind that [`LiquidKind::validate`] refuses is an error naming the field.
    pub fn set_liquid(&mut self, id: LiquidKindId, kind: &LiquidKind) -> Result<(), String> {
        kind.validate().map_err(|e| format!("liquid {}: {e}", id.0))?;
        for (name, v) in LIQUID_FIELDS.iter().zip(kind.to_values()) {
            self.set(&liquid_path(id, name), NodeValue::F64(v));
        }
        Ok(())
    }

    /// Set the sea of `body` (its three nodes).
    pub fn set_sea(&mut self, body: BodyId, sea: &SeaParams) {
        self.set(&body_sea_level_path(body), NodeValue::F64(sea.level_m));
        self.set(&body_sea_liquid_path(body), NodeValue::U64(u64::from(sea.liquid.0)));
        self.set(&body_sea_min_area_path(body), NodeValue::F64(sea.min_area_fraction));
    }

    fn liquid_ids(&self) -> Vec<LiquidKindId> {
        let mut ids: Vec<LiquidKindId> = self.nodes().filter_map(|(p, _)| liquid_node(p).map(|(k, _)| k)).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// The liquid table: the kinds the definition defines, or the Earth table when it defines none. A kind with a missing,
    /// wrongly typed or meaningless node is an error naming it.
    pub fn liquids(&self) -> Result<LiquidTable, MissingNode> {
        self.liquids_with(&mut |_, _| ())
    }

    fn liquids_with(&self, seen: &mut dyn FnMut(&str, NodeValue)) -> Result<LiquidTable, MissingNode> {
        let ids = self.liquid_ids();
        if ids.is_empty() {
            return Ok(LiquidTable::earth());
        }
        let mut kinds = BTreeMap::new();
        for id in ids {
            let mut v = [0.0; 9];
            for (slot, name) in v.iter_mut().zip(LIQUID_FIELDS) {
                let path = liquid_path(id, name);
                let value = float(self, &path)?.ok_or_else(|| MissingNode(path.clone()))?;
                seen(&path, NodeValue::F64(value));
                *slot = value;
            }
            let kind = LiquidKind::from_values(v);
            kind.validate().map_err(|e| MissingNode(format!("liquid.{} ({e})", id.0)))?;
            kinds.insert(id, kind);
        }
        Ok(LiquidTable { kinds })
    }

    /// The sea of `body`: the nodes it defines over the Earth defaults. A liquid id that is not an integer, a level that is not
    /// a finite number or a fraction outside [0, 1] is an error naming the node.
    pub fn sea(&self, body: BodyId) -> Result<SeaParams, MissingNode> {
        self.sea_with(body, &mut |_, _| ())
    }

    fn sea_with(&self, body: BodyId, seen: &mut dyn FnMut(&str, NodeValue)) -> Result<SeaParams, MissingNode> {
        let mut sea = SeaParams::EARTH;
        let path = body_sea_level_path(body);
        if let Some(v) = float(self, &path)? {
            if v.abs() > MAX_SEA_LEVEL_M {
                return Err(MissingNode(format!("{path} (a water level within {MAX_SEA_LEVEL_M} m of the reference surface, got {v})")));
            }
            seen(&path, NodeValue::F64(v));
            sea.level_m = v;
        }
        let path = body_sea_min_area_path(body);
        if let Some(v) = float(self, &path)? {
            if !(0.0..=1.0).contains(&v) {
                return Err(MissingNode(format!("{path} (a fraction of the surface from 0 to 1, got {v})")));
            }
            seen(&path, NodeValue::F64(v));
            sea.min_area_fraction = v;
        }
        let path = body_sea_liquid_path(body);
        match self.get(&path) {
            None => {}
            Some(NodeValue::U64(k)) => match u32::try_from(k) {
                Ok(k) => {
                    seen(&path, NodeValue::U64(k.into()));
                    sea.liquid = LiquidKindId(k);
                }
                Err(_) => return Err(MissingNode(format!("{path} (a kind id up to {})", u32::MAX))),
            },
            Some(NodeValue::F64(_)) => return Err(MissingNode(format!("{path} (not an integer)"))),
        }
        Ok(sea)
    }
}

/// The range of a body's relief scale (WDEF-017): no less than 1, because the coast band of the water code (about 64 m) is measured on the unscaled bake and covers a scaled field only upward.
pub const RELIEF_SCALE_RANGE: (f64, f64) = (1.0, 4.0);

pub fn body_relief_scale_path(body: BodyId) -> String {
    format!("body.{}.relief_scale", body.0)
}

impl WorldDefinition {
    /// Set the relief scale of `body` (WDEF-017).
    pub fn set_relief_scale(&mut self, body: BodyId, scale: f64) {
        self.set(&body_relief_scale_path(body), NodeValue::F64(scale));
    }

    /// The factor on the elevation of the global field of `body` (above and below the sea level): 1 when the definition sets none. A value
    /// that is not a number, or outside [`RELIEF_SCALE_RANGE`], is an error naming the node.
    pub fn relief_scale(&self, body: BodyId) -> Result<f64, MissingNode> {
        let path = body_relief_scale_path(body);
        match float(self, &path)? {
            None => Ok(1.0),
            Some(v) if (RELIEF_SCALE_RANGE.0..=RELIEF_SCALE_RANGE.1).contains(&v) => Ok(v),
            Some(v) => Err(MissingNode(format!("{path} (a factor from {} to {}, got {v})", RELIEF_SCALE_RANGE.0, RELIEF_SCALE_RANGE.1))),
        }
    }
}

impl GenContext<'_> {
    /// The relief scale of `body`, recording the node when the definition sets it (a world without it reads as before: its keys do not change).
    pub fn relief_scale(&self, body: BodyId) -> Result<f64, MissingNode> {
        let v = self.def.relief_scale(body)?;
        let path = body_relief_scale_path(body);
        if self.def.get(&path).is_some() {
            self.record(path.clone(), crate::ReadRecord::Node { path, value: NodeValue::F64(v) });
        }
        Ok(v)
    }

    /// The liquid table, recording every liquid node it read (none, when the world defines none and the Earth table applies:
    /// defining a table later then changes the read set).
    pub fn liquids(&self) -> Result<LiquidTable, MissingNode> {
        self.def.liquids_with(&mut |p, v| self.record(p.to_string(), crate::ReadRecord::Node { path: p.to_string(), value: v }))
    }

    /// The sea of `body`, recording the nodes the definition sets.
    pub fn sea(&self, body: BodyId) -> Result<SeaParams, MissingNode> {
        self.def.sea_with(body, &mut |p, v| self.record(p.to_string(), crate::ReadRecord::Node { path: p.to_string(), value: v }))
    }
}
