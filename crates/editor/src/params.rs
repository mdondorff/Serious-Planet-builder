//! The parameters an edit shows in the inspector and the typed metric input (VIEW-017), and where the stamp stage keeps them in the item
//! file (WDEF-010): the radius of a stamp is the file's `reach_m`, its one parameter is `height_m` (hill, signed), `depth_m` (crater) or
//! `target_m` (plateau, an absolute height), and a plateau's `max_delta_m` is how far it moves a point. A parameter the table does not
//! know is still shown and edited, without a range.

use planet_world_def::item::{ulid_text, Item, MAX_DELTA_LIMIT_M};
use planet_world_def::ItemId;
use std::collections::BTreeMap;

use crate::lists::label_of;

/// The unit a parameter is shown and typed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Metres,
    Percent,
    Plain,
}

/// Where a parameter of a stamp kind lives in the item file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stored {
    /// In `[params]` under its key.
    Param,
    /// As the file's `reach_m` (the radius of the stamp).
    Reach,
    /// As the file's `max_delta_m`.
    MaxDelta,
}

/// A parameter of a kind: its key, the range the inspector accepts, the step of a slider, the value of a new item and where it is stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default: f64,
    pub stored: Stored,
}

const fn p(key: &'static str, label: &'static str, min: f64, max: f64, step: f64, default: f64) -> ParamDef {
    ParamDef { key, label, unit: Unit::Metres, min, max, step, default, stored: Stored::Param }
}

const fn radius(min: f64, max: f64, step: f64, default: f64) -> ParamDef {
    ParamDef { key: "radius_m", label: "Radius", unit: Unit::Metres, min, max, step, default, stored: Stored::Reach }
}

const CRATER: [ParamDef; 2] = [radius(200.0, 10_000.0, 50.0, 2_000.0), p("depth_m", "Depth", 10.0, 800.0, 10.0, 250.0)];
const HILL: [ParamDef; 2] = [radius(50.0, 2_000.0, 10.0, 400.0), p("height_m", "Height", -400.0, 400.0, 5.0, 80.0)];
const PLATEAU: [ParamDef; 3] = [
    radius(1_000.0, 40_000.0, 500.0, 15_000.0),
    p("target_m", "Target height", -500.0, 9_000.0, 10.0, 300.0),
    ParamDef {
        key: "max_delta_m",
        label: "Flattening limit",
        unit: Unit::Metres,
        min: 10.0,
        max: 5_000.0,
        step: 10.0,
        default: 1_000.0,
        stored: Stored::MaxDelta,
    },
];

/// The kinds the stamp tool places, in the order of its palette.
pub const STAMP_KINDS: [&str; 3] = ["crater", "hill", "plateau"];

/// The parameters of a kind (empty for a kind the table does not know).
pub fn defs_for(kind: &str) -> &'static [ParamDef] {
    match kind {
        "crater" => &CRATER,
        "hill" => &HILL,
        "plateau" => &PLATEAU,
        _ => &[],
    }
}

/// Whether the table knows `kind` as a stamp: its radius is the file's `reach_m`.
pub fn is_stamp_kind(kind: &str) -> bool {
    defs_for(kind).iter().any(|d| d.stored == Stored::Reach)
}

/// The definition of the one parameter of a stamp kind that lives in `[params]` (`height_m`, `depth_m`, `target_m`).
pub fn amount_def(kind: &str) -> Option<&'static ParamDef> {
    defs_for(kind).iter().find(|d| d.stored == Stored::Param)
}

/// The value of a table parameter in an item, where it is stored.
pub fn param_value(item: &Item, d: &ParamDef) -> Option<f64> {
    match d.stored {
        Stored::Reach => Some(item.reach_m),
        Stored::MaxDelta => Some(item.max_delta_m),
        Stored::Param => item.params.get(d.key).copied(),
    }
}

/// Set a table parameter in an item, where it is stored.
pub fn set_param_value(item: &mut Item, d: &ParamDef, v: f64) {
    match d.stored {
        Stored::Reach => item.reach_m = v,
        Stored::MaxDelta => item.max_delta_m = v,
        Stored::Param => {
            item.params.insert(d.key.to_string(), v);
        }
    }
}

/// The bound on the change a stamp of `kind` with `params` declares: a hill's height and a crater's depth are exactly its bound (the
/// stamp stage checks it); a plateau's bound is its own parameter, so a new plateau gets the table's default. 0 for other kinds.
pub fn bound_for(kind: &str, params: &BTreeMap<String, f64>) -> f64 {
    let amount = |key: &str| params.get(key).copied().unwrap_or(0.0).abs();
    let b = match kind {
        "hill" => amount("height_m"),
        "crater" => amount("depth_m"),
        "plateau" => defs_for(kind).iter().find(|d| d.stored == Stored::MaxDelta).map_or(0.0, |d| d.default),
        _ => 0.0,
    };
    b.min(MAX_DELTA_LIMIT_M)
}

/// Keep the bound of a hill or a crater equal to its amount (a plateau's bound is its own parameter).
pub fn fit_stamp_bound(item: &mut Item) {
    if matches!(item.kind.as_str(), "hill" | "crater") {
        item.max_delta_m = bound_for(&item.kind, &item.params);
    }
}

/// The definition of the parameter `key` of `kind`, if the table has it.
pub fn def_of(kind: &str, key: &str) -> Option<&'static ParamDef> {
    defs_for(kind).iter().find(|d| d.key == key)
}

/// The title of a kind: its name with a capital.
pub fn kind_title(kind: &str) -> String {
    let mut c = kind.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

fn trimmed(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// A length for people: metres under a kilometre, kilometres above.
pub fn format_metres(v: f64) -> String {
    // Rounded to a tenth of a metre first, so 999.96 m reads 1 km and not 1000 m.
    let tenth = (v * 10.0).round() / 10.0;
    if tenth.abs() >= 1000.0 {
        format!("{} km", trimmed(v / 1000.0, 2))
    } else {
        format!("{} m", trimmed(if tenth == 0.0 { 0.0 } else { tenth }, 1))
    }
}

/// A value in its unit.
pub fn format_value(v: f64, unit: Unit) -> String {
    match unit {
        Unit::Metres => format_metres(v),
        Unit::Percent => format!("{} %", trimmed(v, 1)),
        Unit::Plain => trimmed(v, 3),
    }
}

/// Read what was typed for a parameter in `unit`: a number with an optional suffix (`m`, `km`, `%`). A bare number is in the unit's base
/// (metres, percent). Anything else, and anything not finite, is refused with the reason.
pub fn parse_value(text: &str, unit: Unit) -> Result<f64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("nothing was typed".to_string());
    }
    let split = t.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))).unwrap_or(t.len());
    let (num, suffix) = t.split_at(split);
    let n: f64 = num.trim().parse().map_err(|_| format!("'{t}' is not a number"))?;
    let scale = match (unit, suffix.trim().to_lowercase().as_str()) {
        (_, "") => 1.0,
        (Unit::Metres, "m") => 1.0,
        (Unit::Metres, "km") => 1000.0,
        (Unit::Percent, "%") => 1.0,
        (_, s) => return Err(format!("'{s}' is not a unit here")),
    };
    let v = n * scale;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("'{t}' is not a finite number"))
    }
}

/// A parameter as the inspector shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamRow {
    pub key: String,
    pub label: String,
    pub unit: Unit,
    pub value: f64,
    pub text: String,
    pub range: Option<(f64, f64)>,
    pub step: f64,
}

/// An edit as the inspector shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspector {
    pub id: ItemId,
    pub id_text: String,
    pub title: String,
    pub kind: String,
    pub layer: String,
    pub latitude: String,
    pub longitude: String,
    /// Distance from the camera, as text.
    pub distance: String,
    /// Place in the order of its layer, as text (`2 of 5 in its layer`).
    pub order: String,
    pub reach: String,
    pub params: Vec<ParamRow>,
}

fn degrees(v: f64, pos: char, neg: char) -> String {
    let text = format!("{:.5}", v.abs());
    // A value that rounds to zero has no sign.
    let side = if v < 0.0 && text.chars().any(|c| matches!(c, '1'..='9')) { neg } else { pos };
    format!("{text}° {side}")
}

/// The inspector of `item`. `position` is `(place, count)` in its layer's apply order, `distance_m` from the camera.
pub fn inspector_of(item: &Item, position: (usize, usize), distance_m: f64) -> Inspector {
    let mut params: Vec<ParamRow> = defs_for(&item.kind)
        .iter()
        .filter_map(|d| {
            param_value(item, d).map(|v| ParamRow {
                key: d.key.to_string(),
                label: d.label.to_string(),
                unit: d.unit,
                value: v,
                text: format_value(v, d.unit),
                range: Some((d.min, d.max)),
                step: d.step,
            })
        })
        .collect();
    for (k, v) in &item.params {
        if def_of(&item.kind, k).is_none() {
            let unit = if k.ends_with("_m") { Unit::Metres } else { Unit::Plain };
            params.push(ParamRow {
                key: k.clone(),
                label: k.clone(),
                unit,
                value: *v,
                text: format_value(*v, unit),
                range: None,
                step: 1.0,
            });
        }
    }
    Inspector {
        id: item.id,
        id_text: ulid_text(item.id),
        title: label_of(item),
        kind: kind_title(&item.kind),
        layer: item.layer.clone(),
        latitude: degrees(item.anchor.latitude_deg, 'N', 'S'),
        longitude: degrees(item.anchor.longitude_deg, 'E', 'W'),
        distance: format_metres(distance_m),
        order: format!("{} of {} in its layer", position.0, position.1),
        reach: format_metres(item.reach_m),
        params,
    }
}
