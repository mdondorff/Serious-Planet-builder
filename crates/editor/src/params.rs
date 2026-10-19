//! The parameters an edit shows in the inspector and the typed metric input (VIEW-017). The table is provisional until the stamp stage
//! fixes its parameter names; a parameter the table does not know is still shown and edited, without a range.

use planet_world_def::item::{ulid_text, Item};
use planet_world_def::ItemId;

use crate::lists::label_of;

/// The unit a parameter is shown and typed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Metres,
    Percent,
    Plain,
}

/// A parameter of a kind: its key, the range the inspector accepts, the step of a slider and the value of a new item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default: f64,
}

const fn m(key: &'static str, label: &'static str, min: f64, max: f64, step: f64, default: f64) -> ParamDef {
    ParamDef { key, label, unit: Unit::Metres, min, max, step, default }
}

const CRATER: [ParamDef; 3] = [
    m("radius_m", "Radius", 200.0, 10_000.0, 50.0, 2_000.0),
    m("depth_m", "Depth", 10.0, 800.0, 10.0, 250.0),
    m("rim_m", "Rim height", 0.0, 200.0, 5.0, 40.0),
];
const HILL: [ParamDef; 2] = [m("radius_m", "Radius", 50.0, 2_000.0, 10.0, 400.0), m("height_m", "Height", 5.0, 400.0, 5.0, 80.0)];
const PLATEAU: [ParamDef; 3] = [
    m("radius_m", "Radius", 1_000.0, 40_000.0, 500.0, 15_000.0),
    m("height_m", "Height", 20.0, 1_500.0, 10.0, 300.0),
    ParamDef { key: "softness_pct", label: "Edge softness", unit: Unit::Percent, min: 0.0, max: 100.0, step: 1.0, default: 40.0 },
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
    if v.abs() >= 1000.0 {
        format!("{} km", trimmed(v / 1000.0, 2))
    } else {
        format!("{} m", trimmed(v, 1))
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
    format!("{:.5}° {}", v.abs(), if v < 0.0 { neg } else { pos })
}

/// The inspector of `item`. `position` is `(place, count)` in its layer's apply order, `distance_m` from the camera.
pub fn inspector_of(item: &Item, position: (usize, usize), distance_m: f64) -> Inspector {
    let mut params: Vec<ParamRow> = defs_for(&item.kind)
        .iter()
        .filter_map(|d| {
            item.params.get(d.key).map(|v| ParamRow {
                key: d.key.to_string(),
                label: d.label.to_string(),
                unit: d.unit,
                value: *v,
                text: format_value(*v, d.unit),
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
