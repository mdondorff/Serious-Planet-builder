//! The kind and parameters of a stamp item (WDEF-010, GEN-026).
//!
//! A stamp is an authored item whose output is a height delta around its anchor. This module holds only what an item carries: the kind
//! and its parameters, canonical and hashable. The profiles are the generators' (`planet_generators::stamp`).

use crate::item_id::ItemHash;

/// The kinds of stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StampKind {
    /// `amount_m` metres at the centre, falling to 0 at the reach (signed).
    Hill = 1,
    /// A bowl of depth `amount_m` (at least 0) with a low rim.
    Crater = 2,
    /// Flatten toward `target_m`, an absolute height above the reference sphere.
    Plateau = 3,
}

impl StampKind {
    /// The kind spelled as an item file spells it (`kind = "hill"`), if it is one.
    pub fn from_name(name: &str) -> Option<StampKind> {
        [StampKind::Hill, StampKind::Crater, StampKind::Plateau].into_iter().find(|k| k.name() == name)
    }

    /// The name of the one parameter of the kind in an item file, with its unit: `height_m` (signed), `depth_m` (at least 0), `target_m`
    /// (an absolute height above the reference sphere). The radius of the stamp is the file's `reach_m`; the bound `max_delta_m` is the
    /// file's.
    pub fn param_key(self) -> &'static str {
        match self {
            StampKind::Hill => "height_m",
            StampKind::Crater => "depth_m",
            StampKind::Plateau => "target_m",
        }
    }

    /// The name in item files and messages.
    pub fn name(self) -> &'static str {
        match self {
            StampKind::Hill => "hill",
            StampKind::Crater => "crater",
            StampKind::Plateau => "plateau",
        }
    }
}

/// A stamp: its kind and three parameters in metres (`amount_m` for a hill or a crater, `target_m` for a plateau, in the first; the other
/// two are reserved and 0). Held as bits (`-0.0` read as `0.0`) so equality and hashes are exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stamp {
    pub kind: StampKind,
    params_bits: [u64; 3],
}

impl Stamp {
    /// A stamp of `kind` with `params`; `-0.0` is read as `0.0`.
    pub fn new(kind: StampKind, params: [f64; 3]) -> Stamp {
        Stamp { kind, params_bits: params.map(|p| if p == 0.0 { 0.0f64.to_bits() } else { p.to_bits() }) }
    }

    /// The stamp an item file's `kind` and `params` name: `Ok(None)` for a kind that is no stamp (the item has no height output of its own),
    /// `Err` for a stamp kind without its parameter ([`StampKind::param_key`]). Other keys are ignored: they take no part in the output
    /// and so none in the stamp's hash.
    pub fn from_file(kind: &str, params: &std::collections::BTreeMap<String, f64>) -> Result<Option<Stamp>, String> {
        let Some(kind) = StampKind::from_name(kind) else { return Ok(None) };
        let key = kind.param_key();
        match params.get(key) {
            Some(v) => Ok(Some(Stamp::new(kind, [*v, 0.0, 0.0]))),
            None => Err(format!("a {} needs the parameter '{key}'", kind.name())),
        }
    }

    /// The kind and parameters as an item file spells them: the inverse of [`Stamp::from_file`].
    pub fn to_file(&self) -> (&'static str, std::collections::BTreeMap<String, f64>) {
        (self.kind.name(), std::collections::BTreeMap::from([(self.kind.param_key().to_string(), self.params()[0])]))
    }

    /// The parameters.
    pub fn params(&self) -> [f64; 3] {
        self.params_bits.map(f64::from_bits)
    }

    /// The bits of the parameters.
    pub fn params_bits(&self) -> [u64; 3] {
        self.params_bits
    }

    /// The content hash of a stamp item: BLAKE3-128 under `planet-builder stamp v1` of the kind byte and the parameter bits. The content
    /// hash of a stamp is this, derived and never given, so the kind and the parameters cannot change without the hash (WDEF-010).
    pub fn content_hash(&self) -> ItemHash {
        let mut m = Vec::with_capacity(25);
        m.push(self.kind as u8);
        for b in self.params_bits {
            m.extend_from_slice(&b.to_le_bytes());
        }
        let full = blake3::derive_key("planet-builder stamp v1", &m);
        u128::from_be_bytes(full[..16].try_into().expect("16 of 32 bytes"))
    }
}
