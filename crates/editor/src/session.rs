//! The editing session (VIEW-017): the selection, the hover, the tool, the drags and the typed values, over the E4 commands. Everything the
//! panels draw comes out of here as plain data, and everything the pointer and the keyboard do goes in through here, so the flows of the
//! editor are tested without a window.
//!
//! Each handle drag is one E4 command (`begin_edit` to `end_edit`), however many pointer moves it takes; a typed value, a placement and a
//! delete are one command each; undo and redo are the E4 ones.

use crate::commands::{Affected, EditError, Editor, NewItem};
use crate::lists::{self, LayerGroup, Row};
use crate::params::{self, Inspector, Unit};
use crate::relevance::{relevant_items, ViewState, MIN_REACH_PX};
use crate::shape::{self, Handle, RADIUS_KEY, REACH_FACTOR};
use planet_core::{TileId, Vec3};
use planet_frame::pick::{pick_footprint, Footprint, SurfaceHit};
use planet_world_def::cells::CELL_EDGE_M;
use planet_world_def::item::{Anchor, Item, MAX_DELTA_LIMIT_M};
use planet_world_def::item_store::ItemStore;
use planet_world_def::{BodyId, ItemId, MissingNode};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The layer new stamps go into.
pub const STAMP_LAYER: &str = "stamps";

/// What the pointer does on the planet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Hover, select, drag handles.
    Select,
    /// A click on the planet places a stamp of this kind.
    Stamp(String),
}

/// Why a session action did not happen. Nothing has changed.
#[derive(Debug)]
pub enum SessionError {
    /// The command layer refused.
    Edit(EditError),
    /// The action itself was refused (nothing selected, a value out of range, an unknown kind), with the reason for the status line.
    Refused(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Edit(e) => write!(f, "{e}"),
            SessionError::Refused(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<EditError> for SessionError {
    fn from(e: EditError) -> Self {
        SessionError::Edit(e)
    }
}

fn refused<T>(why: impl Into<String>) -> Result<T, SessionError> {
    Err(SessionError::Refused(why.into()))
}

/// The reach and the bound on the change that hold a parameter set, both only growing: the reach holds the outline with its margin (at
/// most one cell edge) and the bound holds the largest of the metric parameters that are not the radius (at most the limit).
fn bounds(params: &BTreeMap<String, f64>, reach_m: f64, max_delta_m: f64) -> (f64, f64) {
    let want = match params.get(RADIUS_KEY) {
        Some(r) if r.is_finite() && *r > 0.0 => (r * REACH_FACTOR).min(CELL_EDGE_M),
        _ => 0.0,
    };
    let largest =
        params.iter().filter(|(k, _)| k.as_str() != RADIUS_KEY && k.ends_with("_m")).map(|(_, v)| v.abs()).fold(0.0_f64, f64::max);
    (reach_m.max(want).min(CELL_EDGE_M), max_delta_m.max(largest).min(MAX_DELTA_LIMIT_M))
}

/// Whether `dir` can be a direction: finite and not zero.
fn is_direction(dir: Vec3) -> bool {
    dir.x.is_finite() && dir.y.is_finite() && dir.z.is_finite() && dir.length() > 0.0
}

/// Keep the bounds of an item true to its parameters: a stamp's bound is its amount (a plateau's is its own parameter), another kind's
/// reach holds its outline and its bound its largest metric parameter.
fn fit_bounds(it: &mut Item) {
    if params::is_stamp_kind(&it.kind) {
        params::fit_stamp_bound(it);
    } else {
        (it.reach_m, it.max_delta_m) = bounds(&it.params, it.reach_m, it.max_delta_m);
    }
}

/// The editing session over a body.
pub struct Session<S: ItemStore> {
    editor: Editor<S>,
    body: BodyId,
    radius_m: f64,
    selection: Option<ItemId>,
    hover: Option<ItemId>,
    hidden: BTreeSet<ItemId>,
    tool: Tool,
    status: String,
    drag: Option<Handle>,
}

impl<S: ItemStore> Session<S> {
    /// A session over `editor` for `body`, whose radius is `radius_m`.
    pub fn new(editor: Editor<S>, body: BodyId, radius_m: f64) -> Session<S> {
        Session {
            editor,
            body,
            radius_m,
            selection: None,
            hover: None,
            hidden: BTreeSet::new(),
            tool: Tool::Select,
            status: String::new(),
            drag: None,
        }
    }

    pub fn editor(&self) -> &Editor<S> {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor<S> {
        &mut self.editor
    }

    pub fn radius_m(&self) -> f64 {
        self.radius_m
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = text.into();
    }

    pub fn tool(&self) -> &Tool {
        &self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn selected(&self) -> Option<ItemId> {
        self.selection
    }

    pub fn hovered(&self) -> Option<ItemId> {
        self.hover
    }

    pub fn dragging(&self) -> Option<Handle> {
        self.drag
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.editor.item(id)
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.selection.and_then(|id| self.editor.item(id))
    }

    /// Select an edit. While a handle is held the selection does not change.
    pub fn select(&mut self, id: ItemId) {
        if self.drag.is_none() && self.editor.item(id).is_some() {
            self.selection = Some(id);
        }
    }

    pub fn deselect(&mut self) {
        self.selection = None;
    }

    pub fn set_hover(&mut self, id: Option<ItemId>) {
        self.hover = id;
    }

    pub fn is_hidden(&self, id: ItemId) -> bool {
        self.hidden.contains(&id)
    }

    /// Show or hide an edit in the views (not in the world): the layer list's checkbox.
    pub fn toggle_hidden(&mut self, id: ItemId) {
        if !self.hidden.remove(&id) {
            self.hidden.insert(id);
        }
    }

    /// The layers of the items, by name.
    fn layers(&self) -> Vec<String> {
        self.editor.items().map(|i| i.layer.clone()).collect::<BTreeSet<_>>().into_iter().collect()
    }

    /// The edits the camera shows (VIEW-015): the footprint find over the visible `tiles`, the screen size of the reach, and the selected
    /// edit always. Hidden edits are left out, except the selected one.
    pub fn shown(&self, tiles: &[TileId], view: &ViewState) -> Result<Vec<ItemId>, MissingNode> {
        let layers = self.layers();
        let layer_refs: Vec<&str> = layers.iter().map(String::as_str).collect();
        let mut ids = relevant_items(self.editor.definition(), self.body, &layer_refs, tiles, view, MIN_REACH_PX, self.selection)?;
        ids.retain(|id| !self.hidden.contains(id) || Some(*id) == self.selection);
        Ok(ids)
    }

    /// The item under a surface hit among the `shown` ones: the nearest centre in units of its own outline radius (`pick_footprint`).
    pub fn pick(&self, hit: &SurfaceHit, shown: &[ItemId]) -> Option<ItemId> {
        let ids: Vec<ItemId> = shown.iter().copied().filter(|id| self.editor.item(*id).is_some()).collect();
        let footprints: Vec<Footprint> = ids
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let it = self.editor.item(*id)?;
                Some(Footprint { id: i as u64, dir: it.direction(), radius_m: shape::outline_radius_m(it) })
            })
            .collect();
        pick_footprint(hit, &footprints, self.radius_m).map(|i| ids[i as usize])
    }

    /// The rows of the nearby list: the `shown` edits, nearest first.
    pub fn near(&self, shown: &[ItemId], camera_position: Vec3) -> Vec<Row> {
        lists::near_rows(self.editor.items(), shown, camera_position, self.radius_m)
    }

    /// The search rows (VIEW-016): planet-wide, nearest to the camera first.
    pub fn search(&self, query: &str, kind: Option<&str>, camera_position: Vec3) -> Vec<Row> {
        lists::search_rows(self.editor.items(), query, kind, camera_position, self.radius_m)
    }

    pub fn layer_groups(&self, camera_position: Vec3) -> Vec<LayerGroup> {
        lists::layer_groups(self.editor.items(), camera_position, self.radius_m)
    }

    /// Every edit in the order they apply.
    pub fn chain(&self, camera_position: Vec3) -> Vec<Row> {
        lists::apply_order(self.editor.items(), camera_position, self.radius_m)
    }

    /// The inspector of the selected edit.
    pub fn inspector(&self, camera_position: Vec3) -> Option<Inspector> {
        let it = self.selected_item()?;
        let mut same: Vec<(i32, ItemId)> = self.editor.items().filter(|i| i.layer == it.layer).map(|i| (i.priority, i.id)).collect();
        same.sort();
        let place = same.iter().position(|(_, id)| *id == it.id).map_or(1, |p| p + 1);
        Some(params::inspector_of(it, (place, same.len()), lists::distance_m(camera_position, it, self.radius_m)))
    }

    /// The handles of the selected edit with their places in `PlanetFixed` (`ground_m` above the sphere at the edit).
    pub fn handles(&self, ground_m: f64) -> Vec<(Handle, Vec3)> {
        let Some(it) = self.selected_item() else { return Vec::new() };
        shape::handles_of(it).into_iter().map(|h| (h, shape::handle_point(it, h, self.radius_m, ground_m))).collect()
    }

    fn named(&self, id: ItemId) -> String {
        self.editor.item(id).map_or_else(|| "edit".to_string(), lists::label_of)
    }

    /// Place a stamp of `kind` at the unit direction `dir`: selected afterwards, the tool back to select. The parameters are the kind's
    /// defaults.
    pub fn place(&mut self, kind: &str, dir: Vec3) -> Result<(ItemId, Vec<Affected>), SessionError> {
        let defs = params::defs_for(kind);
        if defs.is_empty() {
            return refused(format!("'{kind}' is not a kind of stamp"));
        }
        if !is_direction(dir) {
            return refused("that is not a place on the planet");
        }
        let (lat, lon) = shape::lat_lon_of(dir);
        // A stamp keeps its radius in `reach_m`, its amount in `[params]` and (a plateau) its limit in `max_delta_m`.
        let params: BTreeMap<String, f64> =
            defs.iter().filter(|d| d.stored == params::Stored::Param).map(|d| (d.key.to_string(), d.default)).collect();
        let count = self.editor.items().filter(|i| i.kind == kind).count() + 1;
        let reach_m = defs.iter().find(|d| d.stored == params::Stored::Reach).map_or(0.0, |d| d.default);
        let max_delta_m = params::bound_for(kind, &params);
        let new = NewItem {
            body: self.body,
            surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
            layer: STAMP_LAYER.to_string(),
            priority: 0,
            kind: kind.to_string(),
            anchor: Anchor { latitude_deg: lat, longitude_deg: lon, height_m: 0.0, heading_deg: 0.0 },
            reach_m,
            max_delta_m,
            params,
            name: Some(format!("{} {count}", params::kind_title(kind))),
            notes: None,
        };
        let (id, affected) = self.editor.put(new)?;
        self.selection = Some(id);
        self.tool = Tool::Select;
        self.status = format!("{} placed", self.named(id));
        Ok((id, affected))
    }

    /// Delete the selected edit.
    pub fn delete_selected(&mut self) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let name = self.named(id);
        let affected = self.editor.delete(id)?;
        self.selection = None;
        self.hover = self.hover.filter(|h| *h != id);
        self.status = format!("{name} deleted; undo brings it back");
        Ok(affected)
    }

    /// Set a parameter of the selected edit from what was typed (`2.4 km`, `450`, `40 %`): one command. A value out of the kind's range,
    /// or not a number, changes nothing.
    pub fn set_param_text(&mut self, key: &str, text: &str) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let Some(item) = self.editor.item(id) else { return refused("the selected edit is gone") };
        let def = params::def_of(&item.kind, key);
        let unit = match def {
            Some(d) => d.unit,
            None if item.params.contains_key(key) => {
                if key.ends_with("_m") {
                    Unit::Metres
                } else {
                    Unit::Plain
                }
            }
            None => return refused(format!("'{key}' is not a parameter of this edit")),
        };
        let v = params::parse_value(text, unit).map_err(|why| SessionError::Refused(format!("{key}: {why}")))?;
        if let Some(d) = def {
            if !(d.min..=d.max).contains(&v) {
                return refused(format!(
                    "{} must be between {} and {}",
                    d.label,
                    params::format_value(d.min, d.unit),
                    params::format_value(d.max, d.unit)
                ));
            }
        } else if unit == Unit::Metres {
            // No range for this parameter: the limits of the item file hold (an outline within one cell edge, a change within the bound).
            if key == RADIUS_KEY && !(v > 0.0 && v <= CELL_EDGE_M) {
                return refused(format!("{key} must be between 0 and {}", params::format_metres(CELL_EDGE_M)));
            }
            if key != RADIUS_KEY && v.abs() > MAX_DELTA_LIMIT_M {
                return refused(format!("{key} must be within {}", params::format_metres(MAX_DELTA_LIMIT_M)));
            }
        }
        let affected = self.editor.change(id, |it| {
            match def {
                Some(d) => params::set_param_value(it, d, v),
                None => {
                    it.params.insert(key.to_string(), v);
                }
            }
            fit_bounds(it);
        })?;
        self.status = format!("{} {} set to {}", self.named(id), key, params::format_value(v, unit));
        Ok(affected)
    }

    /// Rename the selected edit: one command.
    pub fn rename_selected(&mut self, name: &str) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let name = name.trim().to_string();
        let affected = self.editor.change(id, |it| it.name = if name.is_empty() { None } else { Some(name) })?;
        self.status = "renamed".to_string();
        Ok(affected)
    }

    /// Start dragging a handle of the selected edit. The drag is one command until [`end_drag`](Session::end_drag).
    pub fn begin_drag(&mut self, handle: Handle) -> Result<(), SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let Some(item) = self.editor.item(id) else { return refused("the selected edit is gone") };
        if !shape::handles_of(item).contains(&handle) {
            return refused(format!("this edit has no {handle:?} handle"));
        }
        self.editor.begin_edit(id)?;
        self.drag = Some(handle);
        Ok(())
    }

    /// The pointer moved over the surface at the unit direction `dir` while a position or radius handle is held.
    pub fn drag_surface(&mut self, dir: Vec3) -> Result<Vec<Affected>, SessionError> {
        if !is_direction(dir) {
            return refused("that is not a place on the planet");
        }
        let radius_m = self.radius_m;
        match self.drag {
            Some(Handle::Move) => Ok(self.editor.edit(|it| *it = shape::moved(it, dir))?),
            Some(Handle::Radius) => Ok(self.editor.edit(|it| *it = shape::resized(it, dir, radius_m))?),
            Some(Handle::Height) => refused("the height handle follows the pointer's height, not the surface"),
            None => refused("no handle is held"),
        }
    }

    /// The pointer moved by `delta_m` metres of height while the height handle is held.
    pub fn drag_height(&mut self, delta_m: f64) -> Result<Vec<Affected>, SessionError> {
        match self.drag {
            Some(Handle::Height) => Ok(self.editor.edit(|it| *it = shape::raised(it, delta_m))?),
            Some(_) => refused("only the height handle follows the pointer's height"),
            None => refused("no handle is held"),
        }
    }

    /// The button went up: the drag becomes one command.
    pub fn end_drag(&mut self) -> Result<Vec<Affected>, SessionError> {
        let Some(handle) = self.drag.take() else { return refused("no handle is held") };
        let affected = self.editor.end_edit()?;
        self.status = format!(
            "{} {}",
            self.selection.map_or_else(|| "edit".to_string(), |id| self.named(id)),
            match handle {
                Handle::Move => "moved",
                Handle::Radius => "resized",
                Handle::Height => "raised",
            }
        );
        Ok(affected)
    }

    /// The drag was abandoned (Esc, focus lost): the edit goes back and nothing is recorded.
    pub fn cancel_drag(&mut self) -> Result<Vec<Affected>, SessionError> {
        if self.drag.take().is_none() {
            return refused("no handle is held");
        }
        let affected = self.editor.cancel_edit()?;
        self.status = "drag cancelled".to_string();
        Ok(affected)
    }

    fn after_history(&mut self, what: &str) {
        if self.selection.is_some_and(|id| self.editor.item(id).is_none()) {
            self.selection = None;
        }
        if self.hover.is_some_and(|id| self.editor.item(id).is_none()) {
            self.hover = None;
        }
        self.status = what.to_string();
    }

    /// Take back the last command; `None` when there is none.
    pub fn undo(&mut self) -> Result<Option<Vec<Affected>>, SessionError> {
        let r = self.editor.undo()?;
        if r.is_some() {
            self.after_history("undone");
        }
        Ok(r)
    }

    /// Do again the last command taken back; `None` when there is none.
    pub fn redo(&mut self) -> Result<Option<Vec<Affected>>, SessionError> {
        let r = self.editor.redo()?;
        if r.is_some() {
            self.after_history("redone");
        }
        Ok(r)
    }
}

/// The metres a vertical pointer move of `dy_px` pixels (down is positive on the screen) changes the height by at `point`: dragging up
/// raises. A helper for the window, kept here so the sign is tested once.
pub fn height_delta_m(dy_px: f64, metres_per_pixel: f64) -> f64 {
    -dy_px * metres_per_pixel
}
