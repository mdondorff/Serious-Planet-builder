//! Canonical TOML reading and writing of a [`WorldDefinition`] (WDEF-002, ADR 0007).
//!
//! The file carries its own `schema_version`, independent of any generator version (CON-10). Reading migrates older
//! versions step by step, then flattens the tables into nodes; the hash is taken from the parsed nodes, never from the
//! file bytes, so comments, key order, whitespace and formatting never change a key. Writing is canonical: the version
//! first, root values, then one table per path prefix, everything sorted, floats in Rust's shortest round-trip form.
//!
//! Supported values: non-negative integers (a `u64` beyond TOML's `i64` range, and only that, is written as a decimal
//! string and read back from one) and finite floats. Anything else is an error naming the path, never a silent default.
//!
//! Typing: a node's type is the type of its literal, except for the paths the program itself reads, whose type is fixed
//! (`world.seed` is an integer, `body.<n>.radius_m` a float): there an integer literal for a float is accepted when it
//! is exact and a float for an integer is an error, so `radius_m = 6371000` and `6371000.0` are one definition.

use std::collections::BTreeSet;
use std::fmt;

use crate::{is_height_field_path, valid_implementation_id, NodeValue, WorldDefinition, HEIGHT_FIELD_PATH};

/// Schema version this build writes and reads without migration.
/// History: 1 = the first layout; 2 = adds the liquid table (`liquid.<k>.*`) and the sea of a body (`body.<n>.sea.*`), WDEF-006.
pub const SCHEMA_VERSION: u32 = 2;

const VERSION_KEY: &str = "schema_version";

/// One step of the migration chain: rewrites a table of schema version `from` into the layout of `from + 1`. The
/// engine sets `schema_version` itself.
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut toml::Table) -> Result<(), String>,
}

/// Migrations from every older version to the current one. Version 1 to 2 changes no layout: version 2 only adds keys (the
/// liquid table and the sea of a body, both optional with Earth defaults), so a version 1 file means the same definition. The
/// step exists so that an older build says "newer schema version" instead of "unknown key" (the rule of ADR 0049 point 4).
pub const MIGRATIONS: &[Migration] = &[Migration { from: 1, apply: |_| Ok(()) }];

/// Why a definition file could not be loaded or written. Every variant names what to fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    Syntax(String),
    MissingSchemaVersion,
    BadSchemaVersion(String),
    NewerSchema { found: u32, known: u32 },
    NoMigration { from: u32 },
    Migration { from: u32, message: String },
    Unsupported { path: String, what: &'static str },
    NegativeInteger { path: String },
    NonFinite { path: String },
    BadPath { path: String, why: &'static str },
    Conflict { path: String, other: String },
    WrongType { path: String, expected: &'static str, found: &'static str },
    ItemsNotSupported,
    NotCanonical { path: String, hint: &'static str },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Syntax(m) => write!(f, "not valid TOML: {m}"),
            LoadError::MissingSchemaVersion => write!(f, "missing top-level `{VERSION_KEY}`: add `{VERSION_KEY} = {SCHEMA_VERSION}`"),
            LoadError::BadSchemaVersion(v) => write!(f, "`{VERSION_KEY}` must be a non-negative integer, found {v}"),
            LoadError::NewerSchema { found, known } => {
                write!(f, "schema version {found} is newer than this build understands ({known}): update the program")
            }
            LoadError::NoMigration { from } => write!(
                f,
                "no migration from schema version {from} to {}: load it with a build that still reads version {from}, or add the missing migration step",
                from + 1
            ),
            LoadError::Migration { from, message } => write!(f, "migrating from schema version {from} failed: {message}"),
            LoadError::Unsupported { path, what } => {
                write!(f, "`{path}`: a {what} is not a supported value (use a non-negative integer or a float)")
            }
            LoadError::NegativeInteger { path } => {
                write!(f, "`{path}`: negative integers are not supported (use a float for signed values)")
            }
            LoadError::NonFinite { path } => write!(f, "`{path}`: infinite and NaN values are not supported"),
            LoadError::BadPath { path, why } => write!(f, "`{path}`: {why}"),
            LoadError::ItemsNotSupported => write!(f, "the definition has authored items, which this file format cannot carry yet (M6): refusing to write a file that would load to a different definition"),
            LoadError::WrongType { path, expected, found } => write!(f, "`{path}` must be {expected}, found {found}"),
            LoadError::NotCanonical { path, hint } => write!(f, "`{path}`: {hint}"),
            LoadError::Conflict { path, other } => write!(f, "`{path}` is a value and `{other}` is below it: a path cannot be both"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load a definition from TOML text with the current schema and migrations.
pub fn parse_definition(text: &str) -> Result<WorldDefinition, LoadError> {
    parse_with(text, SCHEMA_VERSION, MIGRATIONS)
}

/// [`parse_definition`] with the current version and the migration chain as arguments, so the engine is testable with
/// a chain longer than the production one.
pub fn parse_with(text: &str, current: u32, migrations: &[Migration]) -> Result<WorldDefinition, LoadError> {
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| LoadError::Syntax(e.to_string()))?;
    let found = match table.get(VERSION_KEY) {
        None => return Err(LoadError::MissingSchemaVersion),
        Some(toml::Value::Integer(v)) => u32::try_from(*v).map_err(|_| LoadError::BadSchemaVersion(v.to_string()))?,
        Some(other) => return Err(LoadError::BadSchemaVersion(format!("a {}", other.type_str()))),
    };
    if found > current {
        return Err(LoadError::NewerSchema { found, known: current });
    }
    for (i, m) in migrations.iter().enumerate() {
        if migrations[..i].iter().any(|n| n.from == m.from) {
            return Err(LoadError::Migration { from: m.from, message: "two migration steps start at this version".into() });
        }
    }
    let mut version = found;
    while version < current {
        let step = migrations.iter().find(|m| m.from == version).ok_or(LoadError::NoMigration { from: version })?;
        (step.apply)(&mut table).map_err(|message| LoadError::Migration { from: version, message })?;
        version += 1;
        table.insert(VERSION_KEY.to_string(), toml::Value::Integer(i64::from(version)));
    }
    table.remove(VERSION_KEY);
    let mut def = WorldDefinition::empty();
    flatten("", &table, &mut def)?;
    Ok(def)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Integer,
    Float,
}

/// The fixed type of the paths the program reads (`GenContext`), if `path` is one of them.
fn expected_kind(path: &str) -> Option<Kind> {
    if path == WorldDefinition::SEED_PATH {
        return Some(Kind::Integer);
    }
    if crate::water::liquid_node(path).is_some() {
        return Some(Kind::Float);
    }
    match crate::water::sea_node(path) {
        Some("sea.liquid") => return Some(Kind::Integer),
        Some(_) => return Some(Kind::Float),
        None => {}
    }
    let rest = path.strip_prefix("body.")?;
    let (index, leaf) = rest.split_once('.')?;
    ((leaf == "radius_m" || leaf == "relief_scale") && !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
        .then_some(Kind::Float)
}

fn reserved(path: &str) -> bool {
    path == "derived" || path.starts_with("derived.") || path == VERSION_KEY || path.starts_with("schema_version.")
}

fn valid_segment(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn flatten(prefix: &str, table: &toml::Table, def: &mut WorldDefinition) -> Result<(), LoadError> {
    for (key, value) in table {
        let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
        if !valid_segment(key) {
            return Err(LoadError::BadPath { path, why: "keys may contain only letters, digits, `_` and `-`" });
        }
        if is_height_field_path(&path) {
            // The one string value: the implementation id of the world's height field.
            match value {
                toml::Value::String(s) if path == HEIGHT_FIELD_PATH && valid_implementation_id(s) => def.set_height_field(s),
                toml::Value::String(_) if path == HEIGHT_FIELD_PATH => {
                    return Err(LoadError::BadPath { path, why: "an implementation id is letters, digits, `_` and `-`, and not empty" })
                }
                toml::Value::Integer(_) | toml::Value::Float(_) | toml::Value::Boolean(_) if path == HEIGHT_FIELD_PATH => {
                    return Err(LoadError::WrongType { path, expected: "a string (an implementation id)", found: "a number or boolean" })
                }
                _ => return Err(LoadError::BadPath { path, why: "`world.height_field` is the id of the height field and holds a string" }),
            }
            continue;
        }
        match value {
            toml::Value::Table(t) => flatten(&path, t, def)?,
            toml::Value::Integer(i) => match expected_kind(&path) {
                Some(Kind::Float) => {
                    // An integer literal for a float node is the same number when it is exactly representable.
                    if i.unsigned_abs() > 1 << 53 {
                        return Err(LoadError::NotCanonical {
                            path,
                            hint: "an integer this large is not exact as a float: write it with a decimal point",
                        });
                    }
                    set(def, &path, NodeValue::F64(*i as f64))?;
                }
                _ => {
                    let v = u64::try_from(*i).map_err(|_| LoadError::NegativeInteger { path: path.clone() })?;
                    set(def, &path, NodeValue::U64(v))?;
                }
            },
            toml::Value::Float(f) => {
                if !f.is_finite() {
                    return Err(LoadError::NonFinite { path });
                }
                if expected_kind(&path) == Some(Kind::Integer) {
                    return Err(LoadError::WrongType { path, expected: "an integer", found: "a float" });
                }
                set(def, &path, NodeValue::F64(*f))?;
            }
            // A `u64` beyond TOML's integer range, and only that, travels as a canonical decimal string.
            toml::Value::String(s) if s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty() => match s.parse::<u64>() {
                Ok(v) if i64::try_from(v).is_err() && expected_kind(&path) != Some(Kind::Float) => set(def, &path, NodeValue::U64(v))?,
                Ok(_) if expected_kind(&path) == Some(Kind::Float) => {
                    return Err(LoadError::WrongType { path, expected: "a float", found: "a string" })
                }
                Ok(_) => {
                    return Err(LoadError::NotCanonical {
                        path,
                        hint: "a quoted number is only for values above 9223372036854775807: write it as a plain integer",
                    })
                }
                Err(_) => return Err(LoadError::Unsupported { path, what: "string" }),
            },
            toml::Value::String(_) => return Err(LoadError::Unsupported { path, what: "string" }),
            toml::Value::Boolean(_) => return Err(LoadError::Unsupported { path, what: "boolean" }),
            toml::Value::Datetime(_) => return Err(LoadError::Unsupported { path, what: "date or time" }),
            toml::Value::Array(_) => return Err(LoadError::Unsupported { path, what: "array" }),
        }
    }
    Ok(())
}

fn set(def: &mut WorldDefinition, path: &str, value: NodeValue) -> Result<(), LoadError> {
    if reserved(path) {
        return Err(LoadError::BadPath { path: path.to_string(), why: "this name is reserved (`derived`, `schema_version`)" });
    }
    def.set(path, value);
    Ok(())
}

/// The canonical TOML text of a definition. `parse_definition(&to_toml(&d)?)` gives back a definition equal to `d`
/// (bit for bit), and equal definitions give equal text.
pub fn to_toml(def: &WorldDefinition) -> Result<String, LoadError> {
    if def.has_items() {
        return Err(LoadError::ItemsNotSupported);
    }
    let paths: BTreeSet<&str> = def.nodes().map(|(p, _)| p).collect();
    let mut root = Vec::new();
    let mut tables: std::collections::BTreeMap<&str, Vec<String>> = std::collections::BTreeMap::new();
    for (path, value) in def.nodes() {
        if path.split('.').any(|s| !valid_segment(s)) {
            return Err(LoadError::BadPath { path: path.to_string(), why: "keys may contain only letters, digits, `_` and `-`" });
        }
        if reserved(path) {
            return Err(LoadError::BadPath { path: path.to_string(), why: "this name is reserved (`derived`, `schema_version`)" });
        }
        // A value and a table cannot share a path.
        let mut prefix = path;
        while let Some((head, _)) = prefix.rsplit_once('.') {
            if paths.contains(head) {
                return Err(LoadError::Conflict { path: head.to_string(), other: path.to_string() });
            }
            prefix = head;
        }
        let text = match value {
            NodeValue::U64(v) if i64::try_from(v).is_ok() => v.to_string(),
            NodeValue::U64(v) => format!("\"{v}\""),
            NodeValue::F64(v) if v.is_finite() => format!("{v:?}"),
            NodeValue::F64(_) => return Err(LoadError::NonFinite { path: path.to_string() }),
        };
        match path.rsplit_once('.') {
            None => root.push(format!("{path} = {text}")),
            Some((table, leaf)) => tables.entry(table).or_default().push(format!("{leaf} = {text}")),
        }
    }
    if let Some(id) = def.height_field() {
        // `valid_implementation_id` holds by construction, so the id needs no escaping. Sorted by key, as the nodes are.
        let lines = tables.entry("world").or_default();
        lines.push(format!("height_field = \"{id}\""));
        lines.sort_by(|a, b| a.split(" = ").next().cmp(&b.split(" = ").next()));
    }
    let mut out = format!("{VERSION_KEY} = {SCHEMA_VERSION}\n");
    for line in &root {
        out.push_str(line);
        out.push('\n');
    }
    for (table, lines) in &tables {
        out.push_str(&format!("\n[{table}]\n"));
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}
