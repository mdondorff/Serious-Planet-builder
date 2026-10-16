//! The look file (ADR 0042 point 6, ADR 0048 point 2; LOOK-001 to LOOK-004): presentation parameters per body, in a TOML file beside
//! the world definition and **outside** it. Nothing here touches `WorldDefinition`, its hash, a read set or a cache key: a look
//! change re-keys nothing and invalidates no authored content (CON-05, CON-09). The file has its own `schema_version` and tested
//! migrations (CON-10), like the definition's but independent of it.
//!
//! GPU-free (CON-03): schema, parse, validate, migrate, canonical write and the Earth defaults. The renderer never sees the file;
//! it sees the numbers the plan carries ([`LitPlan`](super::LitPlan)).
//!
//! Layout, schema version 1 (every table and key is optional; a missing one is the Earth default, so a body without an entry is
//! the Earth default as a whole):
//!
//! ```toml
//! schema_version = 1
//! kind = "look"
//!
//! [body.0.units]
//! solar_illuminance_lux = 126600.0
//!
//! [body.0.exposure]
//! min_ev100 = -2.0
//! max_ev100 = 16.0
//! speed_up = 3.0
//! speed_down = 1.0
//!
//! [body.0.histogram]
//! ev_min = -10.0
//! ev_max = 20.0
//! band_low = 0.4
//! band_high = 0.9
//!
//! [body.0.tone_map]
//! curve = "aces"
//! ```
//!
//! Bodies are addressed by id as in the world definition (`body.<n>`). Unknown tables and keys are errors that name them (a typo
//! must not silently fall back to a default). Extension points for the coming M5 items are new optional tables under `body.<n>`
//! (`albedo` and `sun` for ADR 0039, `atmosphere` for ADR 0040, `liquid_appearance` for ADR 0041): adding an optional table does
//! not change the layout of existing ones and so needs no migration; moving or renaming a key does and bumps the version.
//! What a generator reads (kind ids, liquid kinds, thresholds that shape the world) stays in the world definition, never here.

use super::{ExposureParams, HistogramParams, SceneUnits, ToneMapMode};
use planet_world_def::BodyId;
use std::collections::BTreeMap;
use std::fmt;

/// Schema version this build writes and reads without migration.
pub const SCHEMA_VERSION: u32 = 1;

const VERSION_KEY: &str = "schema_version";
const KIND_KEY: &str = "kind";
const KIND_VALUE: &str = "look";

/// The file name beside a world definition `world.toml`: `world.look.toml` (the stem, `.look.toml`).
pub fn sibling_file_name(definition_file_stem: &str) -> String {
    format!("{definition_file_stem}.look.toml")
}

/// Valid ranges (the declared bound of each attribute is stated in `openspec/specs/look`; these are the limits a value must stay in).
pub const SOLAR_ILLUMINANCE_RANGE_LUX: (f64, f64) = (1.0, 1.0e7);
pub const EXPOSURE_EV100_LIMITS: (f64, f64) = (-20.0, 40.0);
pub const ADAPTATION_SPEED_MAX: f64 = 100.0;
pub const HISTOGRAM_EV_LIMITS: (f64, f64) = (-50.0, 50.0);
pub const HISTOGRAM_MIN_WIDTH_STOPS: f64 = 1.0;
pub const HISTOGRAM_MIN_BAND_WIDTH: f64 = 0.01;
/// The darkest value the scene target may be asked to hold for one unit of radiance at the highest exposure: the smallest normal f16.
pub const MIN_WRITTEN_UNIT_VALUE: f64 = 6.103_515_625e-5;

/// The tone curve (the look's choice; the false-colour band view is a debug mode of the plan, not a look choice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneCurve {
    /// The ACES filmic fit (Narkowicz 2015) and the sRGB encode (ADR 0038 point 3).
    Aces,
}

impl ToneCurve {
    pub fn name(self) -> &'static str {
        match self {
            ToneCurve::Aces => "aces",
        }
    }

    pub fn from_name(s: &str) -> Option<ToneCurve> {
        match s {
            "aces" => Some(ToneCurve::Aces),
            _ => None,
        }
    }

    /// The tone-map mode of a plan that shows this curve.
    pub fn mode(self) -> ToneMapMode {
        match self {
            ToneCurve::Aces => ToneMapMode::Aces,
        }
    }
}

/// The presentation parameters of one body, all `f64` as read (rounded once to `f32` where a shader takes them). The default is
/// the physically based Earth look, identical to the values that were code constants before the look file existed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookParams {
    pub units: SceneUnits,
    /// Exposure range and speeds as read (the `f32` form is [`exposure_params`](LookParams::exposure_params)).
    pub exposure: ExposureLook,
    pub histogram: HistogramParams,
    pub tone_curve: ToneCurve,
}

/// The exposure range and speeds as the file holds them (`f64`, so the text round-trips bit for bit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureLook {
    pub min_ev100: f64,
    pub max_ev100: f64,
    pub speed_up: f64,
    pub speed_down: f64,
}

impl Default for ExposureLook {
    fn default() -> Self {
        let e = ExposureParams::default();
        Self {
            min_ev100: f64::from(e.min_ev100),
            max_ev100: f64::from(e.max_ev100),
            speed_up: f64::from(e.speed_up),
            speed_down: f64::from(e.speed_down),
        }
    }
}

impl Default for LookParams {
    fn default() -> Self {
        Self { units: SceneUnits::EARTH, exposure: ExposureLook::default(), histogram: HistogramParams::EARTH, tone_curve: ToneCurve::Aces }
    }
}

impl LookParams {
    /// The exposure range and speeds as the shader and the CPU reference take them (`f32`).
    pub fn exposure_params(&self) -> ExposureParams {
        let e = self.exposure;
        ExposureParams {
            min_ev100: e.min_ev100 as f32,
            max_ev100: e.max_ev100 as f32,
            speed_up: e.speed_up as f32,
            speed_down: e.speed_down as f32,
        }
    }

    /// Check every value against its valid range; the first violation names the key (relative to the body, e.g.
    /// `exposure.min_ev100`) and the value.
    pub fn validate(&self) -> Result<(), ParamError> {
        validate_units(&self.units)?;
        validate_exposure(&self.exposure)?;
        validate_histogram(&self.histogram)?;
        // One unit of radiance written at the highest exposure must stay a normal f16, or the scene target loses it and the
        // adaptation can no longer see light (the histogram counts only pixels above zero).
        let written = self.units.solar_illuminance_lux / (1.2 * libm::exp2(self.exposure.max_ev100));
        if written < MIN_WRITTEN_UNIT_VALUE {
            return Err(bad(
                "exposure.max_ev100",
                self.exposure.max_ev100,
                format!("with units.solar_illuminance_lux = {:?} one unit of radiance would be written as {written:e}, below the smallest normal f16", self.units.solar_illuminance_lux),
            ));
        }
        Ok(())
    }
}

/// A value outside its valid range: the key (relative to the body), the value as written and why it is refused.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamError {
    pub key: &'static str,
    pub value: String,
    pub why: String,
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` = {}: {}", self.key, self.value, self.why)
    }
}

impl std::error::Error for ParamError {}

fn bad(key: &'static str, value: impl fmt::Debug, why: impl Into<String>) -> ParamError {
    ParamError { key, value: format!("{value:?}"), why: why.into() }
}

fn check_range(key: &'static str, v: f64, lo: f64, hi: f64, what: &str) -> Result<(), ParamError> {
    if !v.is_finite() {
        return Err(bad(key, v, format!("{what} must be a finite number")));
    }
    if v < lo || v > hi {
        return Err(bad(key, v, format!("{what} must be between {lo:?} and {hi:?}")));
    }
    Ok(())
}

pub fn validate_units(u: &SceneUnits) -> Result<(), ParamError> {
    let (lo, hi) = SOLAR_ILLUMINANCE_RANGE_LUX;
    check_range("units.solar_illuminance_lux", u.solar_illuminance_lux, lo, hi, "the sun's illuminance in lux")
}

pub fn validate_exposure(e: &ExposureLook) -> Result<(), ParamError> {
    let (lo, hi) = EXPOSURE_EV100_LIMITS;
    check_range("exposure.min_ev100", e.min_ev100, lo, hi, "the lowest EV100")?;
    check_range("exposure.max_ev100", e.max_ev100, lo, hi, "the highest EV100")?;
    if e.min_ev100 > e.max_ev100 {
        return Err(bad("exposure.min_ev100", e.min_ev100, format!("must not exceed max_ev100 ({:?})", e.max_ev100)));
    }
    check_range("exposure.speed_up", e.speed_up, 0.0, ADAPTATION_SPEED_MAX, "the brightening speed in stops per second")?;
    check_range("exposure.speed_down", e.speed_down, 0.0, ADAPTATION_SPEED_MAX, "the darkening speed in stops per second")
}

pub fn validate_histogram(h: &HistogramParams) -> Result<(), ParamError> {
    let (lo, hi) = HISTOGRAM_EV_LIMITS;
    check_range("histogram.ev_min", h.ev_min, lo, hi, "the lowest EV of the histogram axis")?;
    check_range("histogram.ev_max", h.ev_max, lo, hi, "the highest EV of the histogram axis")?;
    if h.ev_max - h.ev_min < HISTOGRAM_MIN_WIDTH_STOPS {
        return Err(bad(
            "histogram.ev_max",
            h.ev_max,
            format!("the axis must span at least {HISTOGRAM_MIN_WIDTH_STOPS:?} stop above ev_min ({:?})", h.ev_min),
        ));
    }
    check_range("histogram.band_low", h.band_low, 0.0, 1.0, "the lower fraction of the band")?;
    check_range("histogram.band_high", h.band_high, 0.0, 1.0, "the upper fraction of the band")?;
    if h.band_high - h.band_low < HISTOGRAM_MIN_BAND_WIDTH {
        return Err(bad(
            "histogram.band_low",
            h.band_low,
            format!(
                "must be at least {HISTOGRAM_MIN_BAND_WIDTH:?} below band_high ({:?}), so the band survives rounding to f32",
                h.band_high
            ),
        ));
    }
    Ok(())
}

/// Why a look file could not be loaded. Every variant names what to fix.
#[derive(Clone, Debug, PartialEq)]
pub enum LookError {
    Syntax(String),
    MissingSchemaVersion,
    BadSchemaVersion(String),
    NewerSchema { found: u32, known: u32 },
    NoMigration { from: u32 },
    Migration { from: u32, message: String },
    NotALookFile { found: String },
    UnknownKey { key: String },
    BadBody { key: String },
    WrongType { key: String, expected: &'static str, found: &'static str },
    BadValue { key: String, value: String, why: String },
}

impl fmt::Display for LookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookError::Syntax(m) => write!(f, "not valid TOML: {m}"),
            LookError::MissingSchemaVersion => {
                write!(f, "missing top-level `{VERSION_KEY}`: add `{VERSION_KEY} = {SCHEMA_VERSION}`")
            }
            LookError::BadSchemaVersion(v) => write!(f, "`{VERSION_KEY}` must be a non-negative integer, found {v}"),
            LookError::NewerSchema { found, known } => write!(
                f,
                "look file schema version {found} is newer than this build understands ({known}): update the program, or load the file with the build that wrote it"
            ),
            LookError::NoMigration { from } => write!(
                f,
                "no migration from look schema version {from} to {}: load it with a build that still reads version {from}, or add the missing migration step",
                from + 1
            ),
            LookError::Migration { from, message } => write!(f, "migrating the look file from schema version {from} failed: {message}"),
            LookError::NotALookFile { found } => {
                write!(f, "not a look file: add `{KIND_KEY} = \"{KIND_VALUE}\"` (found {found}); a world definition is a different file")
            }
            LookError::UnknownKey { key } => write!(f, "`{key}`: unknown key or table in a look file of schema version {SCHEMA_VERSION}"),
            LookError::BadBody { key } => write!(f, "`{key}`: a body is addressed by its id, a non-negative integer without leading zeros (`body.0`)"),
            LookError::WrongType { key, expected, found } => write!(f, "`{key}` must be {expected}, found {found}"),
            LookError::BadValue { key, value, why } => write!(f, "`{key}` = {value}: {why}"),
        }
    }
}

impl std::error::Error for LookError {}

/// One step of the migration chain: rewrites a table of schema version `from` into the layout of `from + 1`. The engine sets
/// `schema_version` itself.
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut toml::Table) -> Result<(), String>,
}

/// Migrations from every older version to the current one (none yet: version 1 is the first).
pub const MIGRATIONS: &[Migration] = &[];

/// The look of every body that has an entry. A body without an entry has the Earth default.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LookFile {
    bodies: BTreeMap<BodyId, LookParams>,
}

impl LookFile {
    /// No entries: every body is the Earth default.
    pub fn empty() -> LookFile {
        LookFile::default()
    }

    /// The look of a body: its entry, or the Earth default when it has none.
    pub fn body(&self, id: BodyId) -> LookParams {
        self.bodies.get(&id).copied().unwrap_or_default()
    }

    /// Whether the file has an entry for the body.
    pub fn has_entry(&self, id: BodyId) -> bool {
        self.bodies.contains_key(&id)
    }

    /// The bodies with an entry, in id order.
    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &LookParams)> {
        self.bodies.iter().map(|(b, p)| (*b, p))
    }

    /// Give a body an entry; refuses values outside their valid ranges.
    pub fn set_body(&mut self, id: BodyId, params: LookParams) -> Result<(), ParamError> {
        params.validate()?;
        self.bodies.insert(id, params);
        Ok(())
    }
}

/// Load a look file from TOML text with the current schema and migrations.
pub fn parse_look(text: &str) -> Result<LookFile, LookError> {
    parse_with(text, SCHEMA_VERSION, MIGRATIONS)
}

/// [`parse_look`] with the current version and the migration chain as arguments, so the engine is testable with a chain longer than
/// the production one (the layout read after the chain is always the one of [`SCHEMA_VERSION`]).
pub fn parse_with(text: &str, current: u32, migrations: &[Migration]) -> Result<LookFile, LookError> {
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| LookError::Syntax(e.to_string()))?;
    let found = match table.get(VERSION_KEY) {
        None => return Err(LookError::MissingSchemaVersion),
        Some(toml::Value::Integer(v)) => u32::try_from(*v).map_err(|_| LookError::BadSchemaVersion(v.to_string()))?,
        Some(other) => return Err(LookError::BadSchemaVersion(format!("a {}", other.type_str()))),
    };
    if found > current {
        return Err(LookError::NewerSchema { found, known: current });
    }
    for (i, m) in migrations.iter().enumerate() {
        if migrations[..i].iter().any(|n| n.from == m.from) {
            return Err(LookError::Migration { from: m.from, message: "two migration steps start at this version".into() });
        }
    }
    let mut version = found;
    while version < current {
        let step = migrations.iter().find(|m| m.from == version).ok_or(LookError::NoMigration { from: version })?;
        (step.apply)(&mut table).map_err(|message| LookError::Migration { from: version, message })?;
        version += 1;
        table.insert(VERSION_KEY.to_string(), toml::Value::Integer(i64::from(version)));
    }
    table.remove(VERSION_KEY);
    match table.remove(KIND_KEY) {
        Some(toml::Value::String(s)) if s == KIND_VALUE => {}
        Some(other) => return Err(LookError::NotALookFile { found: describe(&other) }),
        None => return Err(LookError::NotALookFile { found: "no `kind` key".into() }),
    }
    let mut file = LookFile::empty();
    for (key, value) in table {
        if key != "body" {
            return Err(LookError::UnknownKey { key });
        }
        let toml::Value::Table(bodies) = value else {
            return Err(LookError::WrongType { key: "body".into(), expected: "a table of bodies", found: value.type_str() });
        };
        for (id_text, body_value) in bodies {
            let path = format!("body.{id_text}");
            let id: u32 =
                id_text.parse().ok().filter(|n: &u32| n.to_string() == id_text).ok_or(LookError::BadBody { key: path.clone() })?;
            let toml::Value::Table(t) = body_value else {
                return Err(LookError::WrongType { key: path, expected: "a table", found: body_value.type_str() });
            };
            let params = read_body(&path, t)?;
            file.bodies.insert(BodyId(id), params);
        }
    }
    Ok(file)
}

fn describe(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("`{KIND_KEY} = \"{s}\"`"),
        other => format!("`{KIND_KEY}` as a {}", other.type_str()),
    }
}

fn read_body(path: &str, mut t: toml::Table) -> Result<LookParams, LookError> {
    let mut p = LookParams::default();
    let mut section = |name: &str| -> Result<Option<toml::Table>, LookError> {
        match t.remove(name) {
            None => Ok(None),
            Some(toml::Value::Table(s)) => Ok(Some(s)),
            Some(other) => Err(LookError::WrongType { key: format!("{path}.{name}"), expected: "a table", found: other.type_str() }),
        }
    };
    let units = section("units")?;
    let exposure = section("exposure")?;
    let histogram = section("histogram")?;
    let tone_map = section("tone_map")?;
    if let Some(key) = t.keys().next() {
        return Err(LookError::UnknownKey { key: format!("{path}.{key}") });
    }
    if let Some(mut s) = units {
        let k = format!("{path}.units");
        take_f64(&k, &mut s, "solar_illuminance_lux", &mut p.units.solar_illuminance_lux)?;
        no_more(&k, &s)?;
    }
    if let Some(mut s) = exposure {
        let k = format!("{path}.exposure");
        take_f64(&k, &mut s, "min_ev100", &mut p.exposure.min_ev100)?;
        take_f64(&k, &mut s, "max_ev100", &mut p.exposure.max_ev100)?;
        take_f64(&k, &mut s, "speed_up", &mut p.exposure.speed_up)?;
        take_f64(&k, &mut s, "speed_down", &mut p.exposure.speed_down)?;
        no_more(&k, &s)?;
    }
    if let Some(mut s) = histogram {
        let k = format!("{path}.histogram");
        take_f64(&k, &mut s, "ev_min", &mut p.histogram.ev_min)?;
        take_f64(&k, &mut s, "ev_max", &mut p.histogram.ev_max)?;
        take_f64(&k, &mut s, "band_low", &mut p.histogram.band_low)?;
        take_f64(&k, &mut s, "band_high", &mut p.histogram.band_high)?;
        no_more(&k, &s)?;
    }
    if let Some(mut s) = tone_map {
        let k = format!("{path}.tone_map");
        match s.remove("curve") {
            None => {}
            Some(toml::Value::String(name)) => {
                p.tone_curve = ToneCurve::from_name(&name).ok_or(LookError::BadValue {
                    key: format!("{k}.curve"),
                    value: format!("{name:?}"),
                    why: "unknown tone curve (known: \"aces\")".into(),
                })?
            }
            Some(other) => return Err(LookError::WrongType { key: format!("{k}.curve"), expected: "a string", found: other.type_str() }),
        }
        no_more(&k, &s)?;
    }
    p.validate().map_err(|e| LookError::BadValue { key: format!("{path}.{}", e.key), value: e.value, why: e.why })?;
    Ok(p)
}

fn no_more(table_path: &str, t: &toml::Table) -> Result<(), LookError> {
    match t.keys().next() {
        Some(key) => Err(LookError::UnknownKey { key: format!("{table_path}.{key}") }),
        None => Ok(()),
    }
}

/// Read a number (a float, or an integer exact as a float) into `slot` when the key is present.
fn take_f64(table_path: &str, t: &mut toml::Table, key: &str, slot: &mut f64) -> Result<(), LookError> {
    let full = format!("{table_path}.{key}");
    match t.remove(key) {
        None => Ok(()),
        Some(toml::Value::Float(f)) => {
            *slot = f;
            Ok(())
        }
        Some(toml::Value::Integer(i)) => {
            if i.unsigned_abs() > 1 << 53 {
                return Err(LookError::BadValue {
                    key: full,
                    value: i.to_string(),
                    why: "an integer this large is not exact as a float: write it with a decimal point".into(),
                });
            }
            *slot = i as f64;
            Ok(())
        }
        Some(other) => Err(LookError::WrongType { key: full, expected: "a number", found: other.type_str() }),
    }
}

/// The canonical TOML text of a look file: version and kind first, bodies in id order, every key of every table written, floats in
/// Rust's shortest round-trip form. `parse_look(&to_toml(&f))` gives back a file equal to `f` bit for bit, and equal files give
/// equal text (also after a migration: a migrated file and the same content written in the current layout write the same text).
pub fn to_toml(file: &LookFile) -> String {
    let mut out = format!("{VERSION_KEY} = {SCHEMA_VERSION}\n{KIND_KEY} = \"{KIND_VALUE}\"\n");
    for (id, p) in file.bodies() {
        let b = id.0;
        let line = |k: &str, v: f64| format!("{k} = {v:?}\n");
        out.push_str(&format!("\n[body.{b}.units]\n{}", line("solar_illuminance_lux", p.units.solar_illuminance_lux)));
        out.push_str(&format!(
            "\n[body.{b}.exposure]\n{}{}{}{}",
            line("min_ev100", p.exposure.min_ev100),
            line("max_ev100", p.exposure.max_ev100),
            line("speed_up", p.exposure.speed_up),
            line("speed_down", p.exposure.speed_down)
        ));
        out.push_str(&format!(
            "\n[body.{b}.histogram]\n{}{}{}{}",
            line("ev_min", p.histogram.ev_min),
            line("ev_max", p.histogram.ev_max),
            line("band_low", p.histogram.band_low),
            line("band_high", p.histogram.band_high)
        ));
        out.push_str(&format!("\n[body.{b}.tone_map]\ncurve = \"{}\"\n", p.tone_curve.name()));
    }
    out
}
