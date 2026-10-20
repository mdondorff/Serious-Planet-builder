//! The kind and parameters of a stamp item (WDEF-010, GEN-026, WDEF-011).
//!
//! A stamp is an authored item whose output is a height delta around its anchor. This module holds only what an item carries: the kind,
//! its parameters and, for a line, its points, canonical and hashable. The profiles are the generators' (`planet_generators::stamp`).

use crate::item_id::ItemHash;
use planet_core::Vec3;
use std::sync::Arc;

/// The most points a line or a region may have (WDEF-011).
pub const MAX_POINTS: usize = 64;

/// The kinds of stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StampKind {
    /// `amount_m` metres at the centre, falling to 0 at the reach (signed).
    Hill = 1,
    /// A bowl of depth `amount_m` (at least 0) with a low rim.
    Crater = 2,
    /// Flatten toward `target_m`, an absolute height above the reference sphere.
    Plateau = 3,
    /// An open polyline of 2 to 64 points raising the ground by up to `amount_m` out to `width_m` from the line (WDEF-011, GEN-028).
    Ridge = 4,
    /// An open polyline cutting a bed of depth `amount_m` with banks out to `width_m` from the line (WDEF-011, GEN-028).
    Channel = 5,
    /// A closed polygon of 3 to 64 points with a surface type and a mask mode (WDEF-012, GEN-029): no height of its own; in mode 2 it cuts the
    /// deltas of its layer inside it.
    Region = 6,
}

impl StampKind {
    /// Every kind.
    pub const ALL: [StampKind; 6] =
        [StampKind::Hill, StampKind::Crater, StampKind::Plateau, StampKind::Ridge, StampKind::Channel, StampKind::Region];

    /// The kind spelled as an item file spells it (`kind = "hill"`), if it is one.
    pub fn from_name(name: &str) -> Option<StampKind> {
        StampKind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The names of the parameters of the kind in an item file, in slot order: `height_m` (signed), `depth_m` (at least 0), `target_m`
    /// (an absolute height above the reference sphere); a line has `amount_m` (at least 0) and `width_m` (at least 1). The radius of a
    /// point stamp is the file's `reach_m`; the bound `max_delta_m` is the file's. The slots after the named ones are 0.
    pub fn param_keys(self) -> &'static [&'static str] {
        match self {
            StampKind::Hill => &["height_m"],
            StampKind::Crater => &["depth_m"],
            StampKind::Plateau => &["target_m"],
            StampKind::Ridge | StampKind::Channel => &["amount_m", "width_m"],
            StampKind::Region => &["surface_type", "mask_mode"],
        }
    }

    /// The name of the first (for a point stamp the only) parameter of the kind.
    pub fn param_key(self) -> &'static str {
        self.param_keys()[0]
    }

    /// The name in item files and messages.
    pub fn name(self) -> &'static str {
        match self {
            StampKind::Hill => "hill",
            StampKind::Crater => "crater",
            StampKind::Plateau => "plateau",
            StampKind::Ridge => "ridge",
            StampKind::Channel => "channel",
            StampKind::Region => "region",
        }
    }

    /// Whether the kind is a line: an open polyline with a points list.
    pub fn is_line(self) -> bool {
        matches!(self, StampKind::Ridge | StampKind::Channel)
    }

    /// Whether the kind is a region: a closed polygon with a surface type and a mask mode.
    pub fn is_region(self) -> bool {
        self == StampKind::Region
    }
}

/// A stamp: its kind, three parameters in metres (see [`StampKind::param_keys`]; the unnamed slots are 0) and, for a line, its points as
/// unit directions. Held as bits (`-0.0` read as `0.0`) so equality and hashes are exact.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stamp {
    pub kind: StampKind,
    params_bits: [u64; 3],
    /// Bits of the unit directions of the points, in order (shared and immutable); `None` for a stamp without points.
    points_bits: Option<Arc<[[u64; 3]]>>,
}

fn canonical_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0f64.to_bits()
    } else {
        v.to_bits()
    }
}

impl Stamp {
    /// A stamp of `kind` with `params` and no points; `-0.0` is read as `0.0`.
    pub fn new(kind: StampKind, params: [f64; 3]) -> Stamp {
        Stamp { kind, params_bits: params.map(canonical_bits), points_bits: None }
    }

    /// This stamp with the points `points` (unit directions, in order); an empty list is no points.
    pub fn with_points(mut self, points: &[Vec3]) -> Stamp {
        self.points_bits = (!points.is_empty()).then(|| points.iter().map(|p| [p.x, p.y, p.z].map(canonical_bits)).collect());
        self
    }

    /// The stamp an item file's `kind` and `params` name: `Ok(None)` for a kind that is no stamp (the item has no height output of its own),
    /// `Err` for a stamp kind without one of its parameters ([`StampKind::param_keys`]). Other keys are ignored: they take no part in the
    /// output and so none in the stamp's hash.
    pub fn from_file(kind: &str, params: &std::collections::BTreeMap<String, f64>) -> Result<Option<Stamp>, String> {
        let Some(kind) = StampKind::from_name(kind) else { return Ok(None) };
        let mut slots = [0.0f64; 3];
        for (slot, key) in kind.param_keys().iter().enumerate() {
            match params.get(*key) {
                Some(v) => slots[slot] = *v,
                None => return Err(format!("a {} needs the parameter '{key}'", kind.name())),
            }
        }
        Ok(Some(Stamp::new(kind, slots)))
    }

    /// The kind and parameters as an item file spells them: the inverse of [`Stamp::from_file`].
    pub fn to_file(&self) -> (&'static str, std::collections::BTreeMap<String, f64>) {
        let p = self.params();
        (self.kind.name(), self.kind.param_keys().iter().enumerate().map(|(slot, key)| ((*key).to_string(), p[slot])).collect())
    }

    /// The parameters.
    pub fn params(&self) -> [f64; 3] {
        self.params_bits.map(f64::from_bits)
    }

    /// The bits of the parameters.
    pub fn params_bits(&self) -> [u64; 3] {
        self.params_bits
    }

    /// The points as unit directions (empty for a stamp without points).
    pub fn points(&self) -> Vec<Vec3> {
        self.points_bits.as_ref().map_or_else(Vec::new, |p| {
            p.iter().map(|b| Vec3::new(f64::from_bits(b[0]), f64::from_bits(b[1]), f64::from_bits(b[2]))).collect()
        })
    }

    /// The bits of the points' directions.
    pub fn points_bits(&self) -> &[[u64; 3]] {
        self.points_bits.as_deref().unwrap_or(&[])
    }

    /// The content hash of a stamp item: BLAKE3-128 under `planet-builder stamp v1` of the kind byte, the parameter bits and, for a stamp
    /// with points, their count and bits in order. The content hash of a stamp is this, derived and never given, so the kind, the
    /// parameters and the points cannot change without the hash (WDEF-010, WDEF-011). A stamp without points hashes as it always did.
    pub fn content_hash(&self) -> ItemHash {
        let points = self.points_bits();
        let mut m = Vec::with_capacity(25 + if points.is_empty() { 0 } else { 4 + 24 * points.len() });
        m.push(self.kind as u8);
        for b in self.params_bits {
            m.extend_from_slice(&b.to_le_bytes());
        }
        if !points.is_empty() {
            m.extend_from_slice(&u32::try_from(points.len()).expect("at most 64 points").to_le_bytes());
            for p in points {
                for b in p {
                    m.extend_from_slice(&b.to_le_bytes());
                }
            }
        }
        let full = blake3::derive_key("planet-builder stamp v1", &m);
        u128::from_be_bytes(full[..16].try_into().expect("16 of 32 bytes"))
    }
}
