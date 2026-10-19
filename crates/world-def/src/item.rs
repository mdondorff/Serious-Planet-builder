//! The authored item and its file (ADR 0066 point 6, ADR 0007): the parsed form, the ULID text of its id, the canonical TOML text
//! (fixed key order, one key per line, floats in shortest round-trip form), the loader that refuses what it does not know, and the
//! content hash (`planet-builder item hash v1`, BLAKE3-128 over the parsed form without the name and the notes).
//!
//! The hash and every key read the parsed form (WDEF-003), never the bytes, so comments, key order, spacing and merges change nothing.
//! `-0.0` is `0.0` everywhere: at load, in the hash, and in the text.

use crate::item_id::{ItemHash, ItemId};
use crate::{BodyId, ItemError, ItemSpec, WorldDefinition};
use planet_core::surface::{Geo, ReferenceSurface, Sphere};
use planet_core::{TileId, Vec3};
use std::collections::BTreeMap;
use std::fmt;

/// The version of the item file layout.
pub const ITEM_SCHEMA_VERSION: u32 = 1;

/// The blake3 context of the content hash.
pub const ITEM_HASH_CONTEXT: &str = "planet-builder item hash v1";

/// The most `max_delta_m` an item may declare, metres: no stage changes a height by more than this, and a sum of such bounds over a
/// tile's items (ADR 0066 point 5) stays finite.
pub const MAX_DELTA_LIMIT_M: f64 = 100_000.0;

/// The reference surface the sphere conversions use.
pub const DEFAULT_SURFACE: &str = crate::SPHERE_SURFACE_ID;

/// Where an item is anchored: geo-coordinates on the body's reference surface and a heading to true north (AUTH-001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    /// Height above the reference surface, metres.
    pub height_m: f64,
    /// Heading to true north, degrees.
    pub heading_deg: f64,
}

/// An authored item as its file holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub body: BodyId,
    /// The id of the reference surface the anchor is on.
    pub surface: String,
    pub layer: String,
    /// Application order within the layer: lower first, ties by id.
    pub priority: i32,
    /// The semantic type (`crater`, `hill`, ...).
    pub kind: String,
    pub anchor: Anchor,
    /// Arc distance in metres beyond which the item changes no output (ADR 0066 point 1).
    pub reach_m: f64,
    /// Bound in metres on the height change anywhere in the reach (ADR 0066 point 5).
    pub max_delta_m: f64,
    /// The kind's parameters, by name.
    pub params: BTreeMap<String, f64>,
    /// The hash of the procedural base the item was authored against (`WorldDefinition::base_hash`).
    pub base_hash: u64,
    pub generator_version: u32,
    pub stage_version: u32,
    /// Free text for people: in the file, never in the hash.
    pub name: Option<String>,
    pub notes: Option<String>,
}

/// Why an item could not be put into a definition.
#[derive(Clone, Debug, PartialEq)]
pub enum PutError {
    /// The item is not valid ([`Item::validate`]).
    Invalid(ItemFileError),
    /// The index refused it (a body without a radius, ...).
    Index(ItemError),
}

impl fmt::Display for PutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PutError::Invalid(e) => write!(f, "{e}"),
            PutError::Index(e) => write!(f, "item refused by the index: {e:?}"),
        }
    }
}

impl std::error::Error for PutError {}

/// The versions an item is checked against when it is loaded: what the build and the definition are now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveVersions {
    pub generator_version: u32,
    pub stage_version: u32,
    /// `WorldDefinition::base_hash` of the definition the item is loaded into.
    pub base_hash: u64,
}

/// An item authored against other versions than the active ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionMismatch {
    pub id: ItemId,
    pub differences: Vec<String>,
}

impl fmt::Display for VersionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "item {} was authored against {}; pin the definition to the item's generator version, freeze the region, or re-author the item",
            ulid_text(self.id),
            self.differences.join(" and ")
        )
    }
}

impl std::error::Error for VersionMismatch {}

/// Why an item or its file is refused: the key (or a short place) and the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemFileError {
    pub key: String,
    pub why: String,
}

impl fmt::Display for ItemFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "item file: `{}`: {}", self.key, self.why)
    }
}

impl std::error::Error for ItemFileError {}

fn err(key: &str, why: impl Into<String>) -> ItemFileError {
    ItemFileError { key: key.to_string(), why: why.into() }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The ULID text of an id

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The 26-character Crockford base32 text of an id, upper case: the first character holds the top 3 bits, so it is `0` to `7`, and the
/// text sorts as the numbers do.
pub fn ulid_text(id: ItemId) -> String {
    (0..26).map(|i| CROCKFORD[((id >> (5 * (25 - i))) & 31) as usize] as char).collect()
}

/// The id of a ULID text: exactly 26 characters of the upper-case Crockford alphabet, the first at most `7` (a larger one is above 128
/// bits). The canonical form only: a file name or an `id` that differs in case or in the look-alike letters is refused, so one id has
/// one name.
pub fn parse_ulid(text: &str) -> Result<ItemId, ItemFileError> {
    if text.len() != 26 {
        return Err(err("id", format!("a ULID has 26 characters, got {} ('{text}')", text.chars().count())));
    }
    let mut id: u128 = 0;
    for (i, c) in text.bytes().enumerate() {
        let Some(v) = CROCKFORD.iter().position(|a| *a == c) else {
            return Err(err("id", format!("'{}' is not an upper-case Crockford base32 character (ULID '{text}')", c as char)));
        };
        if i == 0 && v > 7 {
            return Err(err("id", format!("the first character of a ULID is at most 7 ('{text}')")));
        }
        id = (id << 5) | v as u128;
    }
    Ok(id)
}

/// A ULID: 48 bits of time in milliseconds, then 80 bits of entropy (the injected clock and entropy of the editor, never generation).
/// The time is taken modulo 2^48.
pub fn make_ulid(timestamp_ms: u64, entropy: [u8; 10]) -> ItemId {
    let mut e: u128 = 0;
    for b in entropy {
        e = (e << 8) | u128::from(b);
    }
    (u128::from(timestamp_ms & 0xFFFF_FFFF_FFFF) << 80) | e
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Validation and the content hash

fn canonical(v: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

fn bits(v: f64) -> u64 {
    canonical(v).to_bits()
}

fn name_ok(s: &str, extra: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || extra.contains(c))
}

impl Item {
    /// The item with `-0.0` made `0.0` in every number (the parsed form the hash and the text read).
    pub fn canonical(mut self) -> Item {
        self.anchor.latitude_deg = canonical(self.anchor.latitude_deg);
        self.anchor.longitude_deg = canonical(self.anchor.longitude_deg);
        self.anchor.height_m = canonical(self.anchor.height_m);
        self.anchor.heading_deg = canonical(self.anchor.heading_deg);
        self.reach_m = canonical(self.reach_m);
        self.max_delta_m = canonical(self.max_delta_m);
        for v in self.params.values_mut() {
            *v = canonical(*v);
        }
        self
    }

    /// Refuse an item the file cannot carry or the index cannot take: a number that is not finite, a latitude or longitude out of range,
    /// a name outside the allowed characters, a reach below 0 or over one cell edge (WDEF-007), a height bound below 0.
    pub fn validate(&self) -> Result<(), ItemFileError> {
        let finite = |key: &str, v: f64| if v.is_finite() { Ok(()) } else { Err(err(key, format!("{v} is not a finite number"))) };
        finite("anchor.latitude", self.anchor.latitude_deg)?;
        finite("anchor.longitude", self.anchor.longitude_deg)?;
        finite("anchor.height", self.anchor.height_m)?;
        finite("anchor.heading", self.anchor.heading_deg)?;
        finite("reach_m", self.reach_m)?;
        finite("max_delta_m", self.max_delta_m)?;
        if !(-90.0..=90.0).contains(&self.anchor.latitude_deg) {
            return Err(err("anchor.latitude", format!("{} is outside -90..90 degrees", self.anchor.latitude_deg)));
        }
        // A place has one spelling: longitude in (-180, 180] (-180 and 180 are the same meridian), 0 at a pole (every longitude is the same
        // point there), and a heading in [0, 360).
        if !(self.anchor.longitude_deg > -180.0 && self.anchor.longitude_deg <= 180.0) {
            return Err(err("anchor.longitude", format!("{} is outside (-180, 180] degrees", self.anchor.longitude_deg)));
        }
        if self.anchor.latitude_deg.abs() == 90.0 && self.anchor.longitude_deg != 0.0 {
            return Err(err(
                "anchor.longitude",
                format!("must be 0 at the pole (latitude {}), got {}", self.anchor.latitude_deg, self.anchor.longitude_deg),
            ));
        }
        if !(0.0..360.0).contains(&self.anchor.heading_deg) {
            return Err(err("anchor.heading", format!("{} is outside 0..360 degrees", self.anchor.heading_deg)));
        }
        if !(0.0..=crate::cells::CELL_EDGE_M).contains(&self.reach_m) {
            return Err(err("reach_m", format!("{} m is outside 0..{} m (one cell edge)", self.reach_m, crate::cells::CELL_EDGE_M)));
        }
        if !(0.0..=MAX_DELTA_LIMIT_M).contains(&self.max_delta_m) {
            return Err(err("max_delta_m", format!("{} m is outside 0..{MAX_DELTA_LIMIT_M} m", self.max_delta_m)));
        }
        // The sphere is the only reference surface (CON-15); a second one adds its id here and its conversion in `direction`.
        if self.surface != DEFAULT_SURFACE {
            return Err(err("surface", format!("'{}' is not a known reference surface (known: {DEFAULT_SURFACE})", self.surface)));
        }
        if !name_ok(&self.layer, "") {
            return Err(err("layer", format!("'{}' is not a layer name (letters, digits, `_`, `-`)", self.layer)));
        }
        if !name_ok(&self.kind, "") {
            return Err(err("kind", format!("'{}' is not a kind (letters, digits, `_`, `-`)", self.kind)));
        }
        for (k, v) in &self.params {
            if !name_ok(k, "") {
                return Err(err(&format!("params.{k}"), "a parameter name has letters, digits, `_` and `-` only"));
            }
            finite(&format!("params.{k}"), *v)?;
        }
        crate::stamp::Stamp::from_file(&self.kind, &self.params).map_err(|why| err("params", why))?;
        Ok(())
    }

    /// The content hash (`planet-builder item hash v1`): BLAKE3-128 over the parsed form, every field except the name and the notes, the
    /// numbers as bit patterns with `-0.0` read as `0.0`, the strings and the parameter names length-prefixed, the parameters in name order.
    /// The id is a field, so two items differing only in their id differ.
    pub fn hash(&self) -> ItemHash {
        let mut m: Vec<u8> = Vec::with_capacity(256);
        let text = |m: &mut Vec<u8>, s: &str| {
            m.extend_from_slice(&u32::try_from(s.len()).expect("a field under 4 GiB").to_le_bytes());
            m.extend_from_slice(s.as_bytes());
        };
        m.extend_from_slice(&ITEM_SCHEMA_VERSION.to_le_bytes());
        m.extend_from_slice(&self.id.to_be_bytes());
        m.extend_from_slice(&self.body.0.to_le_bytes());
        text(&mut m, &self.surface);
        text(&mut m, &self.layer);
        m.extend_from_slice(&self.priority.to_le_bytes());
        text(&mut m, &self.kind);
        for v in [self.anchor.latitude_deg, self.anchor.longitude_deg, self.anchor.height_m, self.anchor.heading_deg] {
            m.extend_from_slice(&bits(v).to_le_bytes());
        }
        m.extend_from_slice(&bits(self.reach_m).to_le_bytes());
        m.extend_from_slice(&bits(self.max_delta_m).to_le_bytes());
        m.extend_from_slice(&u32::try_from(self.params.len()).expect("few parameters").to_le_bytes());
        for (k, v) in &self.params {
            text(&mut m, k);
            m.extend_from_slice(&bits(*v).to_le_bytes());
        }
        m.extend_from_slice(&self.base_hash.to_le_bytes());
        m.extend_from_slice(&self.generator_version.to_le_bytes());
        m.extend_from_slice(&self.stage_version.to_le_bytes());
        let full = blake3::derive_key(ITEM_HASH_CONTEXT, &m);
        u128::from_be_bytes(full[..16].try_into().expect("16 of 32 bytes"))
    }

    /// The unit direction of the anchor (the sphere conversion, `libm`).
    pub fn direction(&self) -> Vec3 {
        // Through the reference surface interface (CON-15): the unit sphere, height 0.
        let g = Geo::new(canonical(self.anchor.latitude_deg), canonical(self.anchor.longitude_deg), 0.0);
        Sphere { radius_m: 1.0 }.geo_to_planet_fixed(g).0
    }

    /// The item as the index takes it: the layer, priority, anchor direction, content hash, reach and height bound.
    pub fn spec(&self) -> ItemSpec {
        let spec = ItemSpec {
            id: self.id,
            layer: self.layer.clone(),
            priority: self.priority,
            dir: self.direction(),
            content_hash: self.hash(),
            reach_m: canonical(self.reach_m),
            max_delta_m: canonical(self.max_delta_m),
            stamp: None,
        };
        // A stamp kind (WDEF-010): the stamp is set and the content hash is the one derived from kind and parameters, so an edit of a
        // field that changes no output (the name, the versions recorded) re-keys nothing. Any other kind has no height output of its own
        // and keeps the file hash.
        match crate::stamp::Stamp::from_file(&self.kind, &self.params) {
            Ok(Some(stamp)) => spec.with_stamp(stamp),
            _ => spec,
        }
    }

    /// Put the item into a definition (add, or replace the item of the same id), returning its cell. The item is validated first (what
    /// the file would refuse the index does not take either), so a place has one spelling and no NaN reaches the hash.
    pub fn put_into(&self, def: &mut WorldDefinition) -> Result<TileId, PutError> {
        self.validate().map_err(PutError::Invalid)?;
        def.put_item_spec(self.body, self.spec()).map_err(PutError::Index)
    }

    /// Check the versions the item records against the active ones (AUTH-002): the generator and stage versions and the hash of the base
    /// the item was authored against. A mismatch is an error naming both sides and the options.
    pub fn check_versions(&self, active: &ActiveVersions) -> Result<(), VersionMismatch> {
        let mut differences = Vec::new();
        if self.generator_version != active.generator_version {
            differences.push(format!("generator version {} (active {})", self.generator_version, active.generator_version));
        }
        if self.stage_version != active.stage_version {
            differences.push(format!("stage version {} (active {})", self.stage_version, active.stage_version));
        }
        if self.base_hash != active.base_hash {
            differences.push(format!("base hash {:016x} (active {:016x})", self.base_hash, active.base_hash));
        }
        if differences.is_empty() {
            Ok(())
        } else {
            Err(VersionMismatch { id: self.id, differences })
        }
    }

    // ----- the file

    /// The canonical text of the item file: `schema_version`, `id`, `body`, `surface`, `layer`, `priority`, `kind`, `reach_m`,
    /// `max_delta_m`, `base_hash`, `generator_version`, `stage_version`, the optional `name` and `notes`, then the tables `[anchor]`
    /// (`latitude`, `longitude`, `height`, `heading`) and `[params]` (by name). One key per line, floats in shortest round-trip form.
    pub fn to_toml(&self) -> Result<String, ItemFileError> {
        self.validate()?;
        let item = self.clone().canonical();
        let mut out = String::new();
        line(&mut out, "schema_version", ITEM_SCHEMA_VERSION.to_string());
        line(&mut out, "id", quote(&ulid_text(item.id)));
        line(&mut out, "body", item.body.0.to_string());
        line(&mut out, "surface", quote(&item.surface));
        line(&mut out, "layer", quote(&item.layer));
        line(&mut out, "priority", item.priority.to_string());
        line(&mut out, "kind", quote(&item.kind));
        line(&mut out, "reach_m", float(item.reach_m));
        line(&mut out, "max_delta_m", float(item.max_delta_m));
        line(&mut out, "base_hash", quote(&format!("{:016x}", item.base_hash)));
        line(&mut out, "generator_version", item.generator_version.to_string());
        line(&mut out, "stage_version", item.stage_version.to_string());
        if let Some(name) = &item.name {
            line(&mut out, "name", quote(name));
        }
        if let Some(notes) = &item.notes {
            line(&mut out, "notes", quote(notes));
        }
        out.push_str("\n[anchor]\n");
        line(&mut out, "latitude", float(item.anchor.latitude_deg));
        line(&mut out, "longitude", float(item.anchor.longitude_deg));
        line(&mut out, "height", float(item.anchor.height_m));
        line(&mut out, "heading", float(item.anchor.heading_deg));
        out.push_str("\n[params]\n");
        for (k, v) in &item.params {
            line(&mut out, k, float(*v));
        }
        Ok(out)
    }

    /// Parse an item file. Refuses a missing or newer `schema_version`, an unknown key (at the top, in `[anchor]` or anywhere else), a
    /// value of the wrong type, a number that is not finite, an id that is not the canonical ULID text, and what [`validate`](Item::validate)
    /// refuses. An exact integer literal is accepted for a float. `-0.0` is read as `0.0`.
    pub fn from_toml(text: &str) -> Result<Item, ItemFileError> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| err("(file)", format!("not TOML: {e}")))?;
        let version = match table.get("schema_version") {
            Some(toml::Value::Integer(v)) => *v,
            Some(_) => return Err(err("schema_version", "must be an integer")),
            None => return Err(err("schema_version", "is required")),
        };
        if version != i64::from(ITEM_SCHEMA_VERSION) {
            return Err(err(
                "schema_version",
                format!("is {version}; this build reads item files of version {ITEM_SCHEMA_VERSION} (a newer file needs a newer build)"),
            ));
        }
        const TOP: [&str; 14] = [
            "schema_version",
            "id",
            "body",
            "surface",
            "layer",
            "priority",
            "kind",
            "reach_m",
            "max_delta_m",
            "base_hash",
            "generator_version",
            "stage_version",
            "name",
            "notes",
        ];
        for key in table.keys() {
            if !TOP.contains(&key.as_str()) && key != "anchor" && key != "params" {
                return Err(err(key, "is not a key of an item file"));
            }
        }
        let string = |key: &str| -> Result<String, ItemFileError> {
            match table.get(key) {
                Some(toml::Value::String(s)) => Ok(s.clone()),
                Some(_) => Err(err(key, "must be a string")),
                None => Err(err(key, "is required")),
            }
        };
        let optional_string = |key: &str| -> Result<Option<String>, ItemFileError> {
            match table.get(key) {
                Some(toml::Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(err(key, "must be a string")),
                None => Ok(None),
            }
        };
        let integer = |key: &str, lo: i64, hi: i64| -> Result<i64, ItemFileError> {
            match table.get(key) {
                Some(toml::Value::Integer(v)) if (lo..=hi).contains(v) => Ok(*v),
                Some(toml::Value::Integer(v)) => Err(err(key, format!("{v} is outside {lo}..{hi}"))),
                Some(_) => Err(err(key, "must be an integer")),
                None => Err(err(key, "is required")),
            }
        };
        let number = |at: &toml::Table, prefix: &str, key: &str| -> Result<f64, ItemFileError> {
            let path = if prefix.is_empty() { key.to_string() } else { format!("{prefix}.{key}") };
            match at.get(key) {
                Some(toml::Value::Float(v)) if v.is_finite() => Ok(canonical(*v)),
                Some(toml::Value::Float(v)) => Err(err(&path, format!("{v} is not a finite number"))),
                Some(toml::Value::Integer(v)) if v.unsigned_abs() <= (1u64 << 53) => Ok(*v as f64),
                Some(toml::Value::Integer(v)) => {
                    Err(err(&path, format!("the integer {v} is not exact as a float; write it with a decimal point")))
                }
                Some(_) => Err(err(&path, "must be a number")),
                None => Err(err(&path, "is required")),
            }
        };
        let id = parse_ulid(&string("id")?)?;
        let anchor_table = match table.get("anchor") {
            Some(toml::Value::Table(t)) => t,
            Some(_) => return Err(err("anchor", "must be a table")),
            None => return Err(err("anchor", "is required")),
        };
        for key in anchor_table.keys() {
            if !["latitude", "longitude", "height", "heading"].contains(&key.as_str()) {
                return Err(err(&format!("anchor.{key}"), "is not a key of the anchor"));
            }
        }
        let anchor = Anchor {
            latitude_deg: number(anchor_table, "anchor", "latitude")?,
            longitude_deg: number(anchor_table, "anchor", "longitude")?,
            height_m: number(anchor_table, "anchor", "height")?,
            heading_deg: number(anchor_table, "anchor", "heading")?,
        };
        let mut params = BTreeMap::new();
        match table.get("params") {
            Some(toml::Value::Table(t)) => {
                for key in t.keys() {
                    params.insert(key.clone(), number(t, "params", key)?);
                }
            }
            Some(_) => return Err(err("params", "must be a table")),
            None => {}
        }
        let base_text = string("base_hash")?;
        if base_text.len() != 16 || !base_text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(err("base_hash", format!("must be 16 lower-case hex digits, got '{base_text}'")));
        }
        let item = Item {
            id,
            body: BodyId(u32::try_from(integer("body", 0, i64::from(u32::MAX))?).expect("range checked")),
            surface: string("surface")?,
            layer: string("layer")?,
            priority: i32::try_from(integer("priority", i64::from(i32::MIN), i64::from(i32::MAX))?).expect("range checked"),
            kind: string("kind")?,
            anchor,
            reach_m: number(&table, "", "reach_m")?,
            max_delta_m: number(&table, "", "max_delta_m")?,
            params,
            base_hash: u64::from_str_radix(&base_text, 16).expect("hex checked"),
            generator_version: u32::try_from(integer("generator_version", 0, i64::from(u32::MAX))?).expect("range checked"),
            stage_version: u32::try_from(integer("stage_version", 0, i64::from(u32::MAX))?).expect("range checked"),
            name: optional_string("name")?,
            notes: optional_string("notes")?,
        };
        item.validate()?;
        Ok(item)
    }
}

/// One `key = value` line.
fn line(out: &mut String, key: &str, value: String) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(&value);
    out.push('\n');
}

/// A TOML basic string: quotes, backslashes and control characters escaped.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The shortest text that reads back as the same float, with a decimal point or exponent so TOML types it as a float.
fn float(v: f64) -> String {
    format!("{:?}", canonical(v))
}
