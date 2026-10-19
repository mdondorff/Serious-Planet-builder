//! The lists of edits (VIEW-016): the nearby list, the planet-wide search, the layers and the apply order. Plain data for the panels.

use planet_core::Vec3;
use planet_world_def::item::{ulid_text, Item};
use planet_world_def::ItemId;
use std::collections::BTreeSet;

/// One edit in a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: ItemId,
    pub label: String,
    pub kind: String,
    pub layer: String,
    /// Metres from the camera to the edit's anchor on the sphere.
    pub distance_m: f64,
    pub reach_m: f64,
}

/// The name of an edit for people: its name, else its kind and the last six characters of its id.
pub fn label_of(item: &Item) -> String {
    match item.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => {
            let t = ulid_text(item.id);
            format!("{} {}", item.kind, &t[t.len() - 6..])
        }
    }
}

/// Metres from the camera to the anchor of `item` on the sphere of `radius_m`.
pub fn distance_m(camera_position: Vec3, item: &Item, radius_m: f64) -> f64 {
    (item.direction() * radius_m - camera_position).length()
}

fn row_of(item: &Item, camera_position: Vec3, radius_m: f64) -> Row {
    Row {
        id: item.id,
        label: label_of(item),
        kind: item.kind.clone(),
        layer: item.layer.clone(),
        distance_m: distance_m(camera_position, item, radius_m),
        reach_m: item.reach_m,
    }
}

fn sorted(mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m).then(a.id.cmp(&b.id)));
    rows
}

/// The rows of the edits in `ids`, nearest to the camera first (ties by id). An id with no item is left out.
pub fn near_rows<'a>(items: impl IntoIterator<Item = &'a Item>, ids: &[ItemId], camera_position: Vec3, radius_m: f64) -> Vec<Row> {
    let keep: BTreeSet<ItemId> = ids.iter().copied().collect();
    sorted(items.into_iter().filter(|i| keep.contains(&i.id)).map(|i| row_of(i, camera_position, radius_m)).collect())
}

/// Whether `item` answers `query`: every word of it is part of the label, the kind, the layer or the id text, in any case. An empty query
/// matches everything. `kind` restricts to one kind.
pub fn matches(item: &Item, query: &str, kind: Option<&str>) -> bool {
    if kind.is_some_and(|k| k != item.kind) {
        return false;
    }
    let hay = format!("{} {} {} {}", label_of(item), item.kind, item.layer, ulid_text(item.id)).to_lowercase();
    query.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))
}

/// The edits that answer `query`, planet-wide (not only the visible ones), nearest to the camera first.
pub fn search_rows<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    query: &str,
    kind: Option<&str>,
    camera_position: Vec3,
    radius_m: f64,
) -> Vec<Row> {
    sorted(items.into_iter().filter(|i| matches(i, query, kind)).map(|i| row_of(i, camera_position, radius_m)).collect())
}

/// The edits in the order they apply: by priority, ties by id.
pub fn apply_order<'a>(items: impl IntoIterator<Item = &'a Item>, camera_position: Vec3, radius_m: f64) -> Vec<Row> {
    let mut v: Vec<&Item> = items.into_iter().collect();
    v.sort_by_key(|i| (i.priority, i.id));
    v.into_iter().map(|i| row_of(i, camera_position, radius_m)).collect()
}

/// The edits of one layer, in the order they apply.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerGroup {
    pub layer: String,
    pub rows: Vec<Row>,
}

/// The edits by layer: the layers by name, the rows of each in the order they apply.
pub fn layer_groups<'a>(items: impl IntoIterator<Item = &'a Item>, camera_position: Vec3, radius_m: f64) -> Vec<LayerGroup> {
    let all: Vec<&Item> = items.into_iter().collect();
    let layers: BTreeSet<&str> = all.iter().map(|i| i.layer.as_str()).collect();
    layers
        .into_iter()
        .map(|layer| LayerGroup {
            layer: layer.to_string(),
            rows: apply_order(all.iter().copied().filter(|i| i.layer == layer), camera_position, radius_m),
        })
        .collect()
}
