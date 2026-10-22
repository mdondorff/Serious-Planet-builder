//! The editing session (VIEW-017): the selection, the hover, the tool, the drags and the typed values, over the E4 commands. Everything the
//! panels draw comes out of here as plain data, and everything the pointer and the keyboard do goes in through here, so the flows of the
//! editor are tested without a window.
//!
//! Each handle drag is one E4 command (`begin_edit` to `end_edit`), however many pointer moves it takes; a typed value, a placement and a
//! delete are one command each; undo and redo are the E4 ones.

use crate::brush::{self, BrushStroke};
use crate::commands::{Affected, EditError, Editor, NewItem};
use crate::health::Health;
use crate::lists::{self, LayerGroup, Row};
use crate::params::{self, Inspector, Unit};
use crate::polyline::{self, GeoPoint};
use crate::problems::{self, Issue, Problem};
use crate::relevance::{footprint_candidates, relevant_items, shown_from, Candidates, ShownLimits, ViewState, MIN_REACH_PX};
use crate::shape::{self, Handle, RADIUS_KEY, REACH_FACTOR};
use crate::slope;
use planet_core::{TileId, Vec3};
use planet_frame::pick::{pick_footprint, Footprint, SurfaceHit};
use planet_world_def::cells::CELL_EDGE_M;
use planet_world_def::item::{Anchor, Item, MAX_DELTA_LIMIT_M};
use planet_world_def::item_store::ItemStore;
use planet_world_def::stamp::BrushKind;
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
    /// Clicks on the planet add the points of a line of this kind (`ridge` or `channel`) to the draft; finishing puts the line.
    Line(String),
    /// Clicks on the planet add the points of a region outline to the draft; finishing puts the region.
    Region,
    /// Dragging over the planet paints a brush stroke of this kind (raise, lower, smooth, flatten); the release puts it (VIEW-035).
    Brush(BrushKind),
}

/// The brush the tool paints with: the radius and the strength in metres, typed by the user, and the height a flatten levels towards
/// (`None`: the terrain height under the start of the stroke).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushSettings {
    pub radius_m: f64,
    pub strength_m: f64,
    pub target_m: Option<f64>,
}

impl Default for BrushSettings {
    fn default() -> Self {
        BrushSettings { radius_m: brush::DEFAULT_RADIUS_M, strength_m: brush::DEFAULT_STRENGTH_M, target_m: None }
    }
}

/// A stroke being painted: the kind and numbers it started with (a change of the settings during the drag does not change it), the
/// height a flatten levels towards and the places the pointer passed.
#[derive(Clone, Debug, PartialEq)]
struct StrokeDraft {
    stroke: BrushStroke,
}

/// The most places one drag records; a longer drag ends there (the stroke still paints what was recorded).
pub const MAX_STROKE_SAMPLES: usize = 20_000;

/// What the status line says when a line or region tool is picked.
pub const DRAW_HINT: &str = "Click the planet to add points · Enter finishes · Backspace takes the last point back · Esc cancels";
/// How far from a line a click still picks it, as a share of the distance to the surface it hit (about 15 pixels of a typical view).
pub const LINE_PICK_FRACTION: f64 = 0.015;

/// The kinds the line tool draws, in the order of its palette.
pub const LINE_KINDS: [&str; 2] = ["ridge", "channel"];
/// The layer new regions go into.
pub const REGION_LAYER: &str = "regions";
/// What a new line starts with: the crest height or bed depth and the half-width, metres; the bound on its change.
pub const LINE_AMOUNT_M: f64 = 30.0;
pub const LINE_WIDTH_M: f64 = 200.0;
pub const LINE_BOUND_M: f64 = 60.0;
/// The most points a draft holds: a region is one item (64), a line is split into items when it is long.
pub const MAX_DRAFT_LINE_POINTS: usize = 640;
/// A point closer than this to the last one of a draft is a double click, not a new point, metres.
pub const MIN_POINT_SPACING_M: f64 = 0.5;

/// What the status line says when a brush tool is picked.
fn brush_hint(kind: BrushKind) -> String {
    let what = match kind {
        BrushKind::Raise => "raises the ground by up to the strength at the centre line",
        BrushKind::Lower => "lowers the ground by up to the strength at the centre line",
        BrushKind::Smooth => "moves the ground towards the mean of its surroundings, never past it",
        BrushKind::Flatten => "moves the ground towards the height under where the stroke starts, by up to the strength",
    };
    format!("Drag over the planet to paint: the {} brush {what} · one stroke is one undo · Esc cancels", kind.name())
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

fn group_delete(id: ItemId) -> crate::commands::GroupStep {
    crate::commands::GroupStep::Delete(id)
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
    hover_rank: Option<(usize, usize)>,
    hidden: BTreeSet<ItemId>,
    tool: Tool,
    status: String,
    drag: Option<Handle>,
    /// The points a line or region tool has collected, in order.
    draft: Vec<GeoPoint>,
    /// While a point handle is held: the points of the line as they are now, the editor not yet told (one command at the release).
    drag_points: Option<Vec<GeoPoint>>,
    /// The brush's radius, strength and target (the tool's settings).
    brush: BrushSettings,
    /// The stroke being painted, while the button is down.
    stroke: Option<StrokeDraft>,
    /// The brush items this session painted into: a new stroke joins one of these when it fits (ADR 0070: not an item someone else's
    /// session changed last).
    brush_open: Vec<ItemId>,
    /// The slope the plan can draw (`LodParams::drawable_slope`), from which the brush guard takes its limit (VIEW-043).
    drawable_slope: f64,
    /// The edit-health lists (steep, stacked, mergeable brushes), kept up to date from the editor's change log: a change re-checks only the
    /// items whose footprint it meets (VIEW-048). A `RefCell`: the session is for one thread (it is not `Sync`).
    health: std::cell::RefCell<Option<Health>>,
    /// The footprint find of the last [`Session::shown`], for the definition revision and the tiles and layers it was made for: a frame that
    /// changes neither (the camera moved, nothing was edited) reuses it (VIEW-051).
    shown_find: std::cell::RefCell<Option<ShownFind>>,
}

/// The footprint find kept by [`Session::shown`]: the definition revision and the key of the tiles and layers it was made for.
struct ShownFind {
    revision: u64,
    key: u64,
    candidates: Candidates,
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
            hover_rank: None,
            hidden: BTreeSet::new(),
            tool: Tool::Select,
            status: String::new(),
            drag: None,
            draft: Vec::new(),
            drag_points: None,
            brush: BrushSettings::default(),
            stroke: None,
            brush_open: Vec::new(),
            drawable_slope: planet_frame::LodParams::earth_1080p().drawable_slope(),
            health: std::cell::RefCell::new(None),
            shown_find: std::cell::RefCell::new(None),
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

    /// Switch the tool; a draft of a line or region that is not finished is dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        if tool != self.tool {
            self.draft.clear();
            self.drop_point_drag();
            self.stroke = None;
            if matches!(tool, Tool::Line(_) | Tool::Region) {
                self.status = DRAW_HINT.to_string();
            }
            if let Tool::Brush(kind) = tool {
                self.status = brush_hint(kind);
            }
        }
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
        self.drop_point_drag();
        self.selection = None;
    }

    /// Close a held point drag without telling the editor (the points belonged to a selection that is going away or to a state that
    /// history is about to change). A height, move or radius drag is an open edit in the editor, which refuses history itself.
    fn drop_point_drag(&mut self) {
        if matches!(self.drag, Some(Handle::Point(_))) {
            self.drag = None;
            self.drag_points = None;
        }
    }

    pub fn set_hover(&mut self, id: Option<ItemId>) {
        self.hover = id;
        if id.is_none() {
            self.hover_rank = None;
        }
    }

    /// Where the hovered edit stands among the edits under the cursor, as (its place, how many): a click selects it (VIEW-041). `None` when only
    /// one edit is under the cursor.
    pub fn hover_rank(&self) -> Option<(usize, usize)> {
        self.hover_rank
    }

    pub fn set_hover_rank(&mut self, rank: Option<(usize, usize)>) {
        self.hover_rank = rank;
    }

    pub fn is_hidden(&self, id: ItemId) -> bool {
        self.hidden.contains(&id)
    }

    /// Show or hide an edit in the views (not in the world): the layer list's checkbox.
    /// A line or a brush drag split into linked items is one row of the layer list, so the whole group is shown or hidden.
    pub fn toggle_hidden(&mut self, id: ItemId) {
        let members = self.editor.line_items(id).unwrap_or_else(|_| vec![id]);
        let hide = !self.hidden.contains(&id);
        for m in members {
            if hide {
                self.hidden.insert(m);
            } else {
                self.hidden.remove(&m);
            }
        }
    }

    /// The layers of the items, by name.
    fn layers(&self) -> Vec<String> {
        self.editor.items().map(|i| i.layer.clone()).collect::<BTreeSet<_>>().into_iter().collect()
    }

    /// The edits the camera shows (VIEW-015): the footprint find over the visible `tiles`, the screen size of the reach, and the selected
    /// edit always. Hidden edits are left out, except the selected one.
    pub fn shown(&self, tiles: &[TileId], view: &ViewState) -> Result<Vec<ItemId>, MissingNode> {
        use std::hash::{Hash, Hasher};
        let layers = self.layers();
        let def = self.editor.definition();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (tiles, &layers).hash(&mut hasher);
        let (revision, key) = (def.revision(), hasher.finish());
        let mut cache = self.shown_find.borrow_mut();
        if !matches!(cache.as_ref(), Some(f) if f.revision == revision && f.key == key) {
            let layer_refs: Vec<&str> = layers.iter().map(String::as_str).collect();
            *cache = Some(ShownFind { revision, key, candidates: footprint_candidates(def, self.body, &layer_refs, tiles)? });
        }
        let found = cache.as_ref().expect("just set");
        let mut ids = shown_from(&found.candidates, def, self.body, view, ShownLimits::default(), self.selection);
        ids.retain(|id| !self.hidden.contains(id) || Some(*id) == self.selection);
        Ok(ids)
    }

    /// The edits whose footprint overlaps one of `tiles` (the footprint find of [`shown`](Session::shown), without the selected edit and the hidden
    /// ones left out): the edits over the tiles the plan holds back, for the problems list (VIEW-047).
    pub fn items_over(&self, tiles: &[TileId], view: &ViewState) -> Result<Vec<ItemId>, MissingNode> {
        let layers = self.layers();
        let layer_refs: Vec<&str> = layers.iter().map(String::as_str).collect();
        relevant_items(self.editor.definition(), self.body, &layer_refs, tiles, view, MIN_REACH_PX, None)
    }

    /// The item under a surface hit among the `shown` ones: the nearest centre in units of its own outline radius (`pick_footprint`).
    pub fn pick(&self, hit: &SurfaceHit, shown: &[ItemId]) -> Option<ItemId> {
        // Stamps first, by their outline; then lines by the distance to the line, then regions (inside, or near the edge) (VIEW-031).
        self.pick_stamp(hit, shown).or_else(|| self.pick_outline(hit, shown))
    }

    /// Every edit under a surface hit among the `shown` ones, for clicking through them (VIEW-041): the stamps whose outline holds the hit, the
    /// smallest footprint first (then the lower id), then the line or region `pick` would take.
    pub fn pick_all(&self, hit: &SurfaceHit, shown: &[ItemId]) -> Vec<ItemId> {
        let mut stamps: Vec<(f64, ItemId)> = shown
            .iter()
            .filter_map(|id| {
                let it = self.editor.item(*id).filter(|i| i.points.is_empty() && i.kind != brush::BRUSH_KIND)?;
                let radius = shape::outline_radius_m(it);
                let d = planet_frame::pick::surface_distance_m(hit.dir, it.direction(), self.radius_m);
                (radius.is_finite() && d <= radius).then_some((radius, *id))
            })
            .collect();
        stamps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<ItemId> = stamps.into_iter().map(|(_, id)| id).collect();
        out.extend(self.pick_outline(hit, shown).filter(|id| !out.contains(id)));
        out
    }

    /// The line or region whose outline a click is on: a line within its half-width or [`LINE_PICK_FRACTION`] of the distance, the
    /// nearest first; else a region the click is inside or near the edge of. Ties go to the lower id.
    fn pick_outline(&self, hit: &SurfaceHit, shown: &[ItemId]) -> Option<ItemId> {
        let tolerance = LINE_PICK_FRACTION * hit.distance_m.max(1.0);
        let mut line: Option<(f64, ItemId)> = None;
        let mut region: Option<(f64, ItemId)> = None;
        for id in shown {
            let Some(item) = self.editor.item(*id).filter(|i| !i.points.is_empty()) else { continue };
            let points = self.stored_points(*id);
            let closed = item.kind == "region";
            let Some(near) = polyline::nearest_segment(&points, closed, hit.dir.normalized(), self.radius_m) else { continue };
            if closed {
                let inside = polyline::contains(&points, hit.dir.normalized());
                let d = if inside { 0.0 } else { near.distance_m };
                if (inside || d <= tolerance) && region.is_none_or(|(best, _)| d < best) {
                    region = Some((d, *id));
                }
            } else {
                let reach = item.params.get("width_m").copied().unwrap_or(0.0).max(tolerance);
                if near.distance_m <= reach && line.is_none_or(|(best, _)| near.distance_m < best) {
                    line = Some((near.distance_m, *id));
                }
            }
        }
        line.or(region).map(|(_, id)| id)
    }

    fn pick_stamp(&self, hit: &SurfaceHit, shown: &[ItemId]) -> Option<ItemId> {
        let ids: Vec<ItemId> = shown
            .iter()
            .copied()
            .filter(|id| self.editor.item(*id).is_some_and(|i| i.points.is_empty() && i.kind != brush::BRUSH_KIND))
            .collect();
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
        self.flagged(lists::near_rows(self.editor.items(), shown, camera_position, self.radius_m))
    }

    /// `rows` with the warning of the brush guard set on those over the limit (VIEW-043).
    fn flagged(&self, mut rows: Vec<Row>) -> Vec<Row> {
        let health = self.health_lists();
        if !health.steep().is_empty() {
            for row in &mut rows {
                row.steep = health.is_steep(row.id, self.editor.item(row.id).and_then(|i| i.group));
            }
        }
        rows
    }

    /// The edit-health lists, true for the items as they are now (the items that may have changed are checked here, once per change).
    fn health_lists(&self) -> std::cell::RefMut<'_, Health> {
        let mut h = self.health();
        h.settle_pending();
        h
    }

    /// The index of the edit-health lists, brought up to date with the editor: all the items the first time (or when the change log no longer
    /// reaches back, or the drawable slope moved), the items changed since otherwise. The answers are not found until asked for.
    fn health(&self) -> std::cell::RefMut<'_, Health> {
        let mut cell = self.health.borrow_mut();
        let seq = self.editor.change_seq();
        let h = cell.get_or_insert_with(|| Health::new(self.radius_m, self.drawable_slope));
        if h.seq() != seq || !h.is_built() {
            match self.editor.changes_since(h.seq()).filter(|_| h.is_built()) {
                Some(changed) => h.apply(seq, changed, |id| self.editor.item(id)),
                None => h.rebuild(seq, self.editor.items()),
            }
        }
        std::cell::RefMut::map(cell, |c| c.as_mut().expect("just set"))
    }

    /// How many items the last refresh of the edit-health lists checked (the work the last change cost).
    pub fn health_checked(&self) -> usize {
        let h = self.health_lists();
        h.last_checked()
    }

    /// The search rows (VIEW-016): planet-wide, nearest to the camera first.
    pub fn search(&self, query: &str, kind: Option<&str>, camera_position: Vec3) -> Vec<Row> {
        self.flagged(lists::search_rows(self.editor.items(), query, kind, camera_position, self.radius_m))
    }

    pub fn layer_groups(&self, camera_position: Vec3) -> Vec<LayerGroup> {
        let mut groups = lists::layer_groups(self.editor.items(), camera_position, self.radius_m);
        for group in &mut groups {
            group.rows = self.flagged(std::mem::take(&mut group.rows));
        }
        groups
    }

    /// The slope the plan can draw, which the brush guard limits the stack of strokes by (VIEW-043).
    pub fn drawable_slope(&self) -> f64 {
        self.drawable_slope
    }

    /// Set it from the LOD parameters of the window (`LodParams::drawable_slope`).
    pub fn set_drawable_slope(&mut self, slope: f64) {
        self.drawable_slope = slope;
        *self.health.borrow_mut() = None;
    }

    /// The brush items whose stacked gradient is over the limit of the guard, "too steep to draw fully" (VIEW-043). Kept up to date as the
    /// editor changes (VIEW-048).
    pub fn steep_items(&self) -> BTreeSet<ItemId> {
        self.health_lists().steep().clone()
    }

    /// The brush items listed as stacked with the depth of their stack (VIEW-047). Kept up to date as the editor changes (VIEW-048).
    pub fn stacked_items(&self) -> std::collections::BTreeMap<ItemId, usize> {
        self.health_lists().stacked().clone()
    }

    /// The problems list (VIEW-047): every edit too steep to draw fully, stacked, or near tiles the plan holds back (`capped`, the edits the
    /// window found over the capped tiles of the last plan). The steepest and deepest first, then by name.
    pub fn problems(&self, capped: &[ItemId], camera_position: Vec3) -> Vec<Problem> {
        let (steep, stacked) = {
            let h = self.health_lists();
            (h.steep().clone(), h.stacked().clone())
        };
        let mut ids: BTreeSet<ItemId> = steep.iter().copied().chain(stacked.keys().copied()).collect();
        ids.extend(capped.iter().copied().filter(|id| self.editor.item(*id).is_some()));
        let ids: Vec<ItemId> = ids.into_iter().collect();
        // The listed items only are looked up (not every item scanned).
        let mut rows =
            self.flagged(lists::near_rows(ids.iter().filter_map(|id| self.editor.item(*id)), &ids, camera_position, self.radius_m));
        let mut out: Vec<Problem> = rows
            .drain(..)
            .map(|row| {
                let item = self.editor.item(row.id);
                let mut issues = Vec::new();
                if steep.contains(&row.id) {
                    issues.push(Issue::TooSteep);
                }
                if let Some(depth) = stacked.get(&row.id) {
                    issues.push(Issue::Stacked { depth: *depth, strokes: row.strokes });
                }
                if capped.contains(&row.id) {
                    issues.push(Issue::TilesCapped);
                }
                let strokes: &[planet_world_def::item::ItemStroke] = item.map_or(&[], |i| &i.strokes);
                let can_reduce = strokes
                    .iter()
                    .any(|s| matches!(s.kind, planet_world_def::stamp::BrushKind::Raise | planet_world_def::stamp::BrushKind::Lower))
                    && issues.iter().any(|i| matches!(i, Issue::TooSteep | Issue::Stacked { .. }));
                let can_merge = self.is_mergeable(row.id, strokes);
                Problem { row, issues, can_reduce, can_merge }
            })
            .collect();
        out.sort_by(|a, b| {
            let rank = |p: &Problem| {
                p.issues
                    .iter()
                    .map(|i| match i {
                        Issue::TooSteep => 0,
                        Issue::Stacked { .. } => 1,
                        Issue::TilesCapped => 2,
                    })
                    .min()
            };
            rank(a).cmp(&rank(b)).then(a.row.distance_m.total_cmp(&b.row.distance_m)).then(a.row.id.cmp(&b.row.id))
        });
        out
    }

    /// Whether `strokes` (of item `id`) have a stack to merge, remembered until the item changes.
    fn is_mergeable(&self, id: ItemId, strokes: &[planet_world_def::item::ItemStroke]) -> bool {
        let mut h = self.health();
        *h.mergeable.entry(id).or_insert_with(|| problems::merge_stack(strokes, self.radius_m).is_some())
    }

    /// Reduce the brush item `id` (VIEW-047): the strengths of its raises and lowers are cut by the largest factor that makes it no longer too
    /// steep ([`problems::fit_factor`]); a stacked item that is not too steep is cut by [`problems::REDUCE_STACKED`]. One command. Returns the factor.
    pub fn reduce_item(&mut self, id: ItemId) -> Result<(f64, Vec<Affected>), SessionError> {
        let item = self.editor.item(id).ok_or_else(|| SessionError::Refused("no such edit".into()))?;
        if item.kind != brush::BRUSH_KIND {
            return Err(SessionError::Refused(format!("a {} has no strokes to reduce", item.kind)));
        }
        let limit = slope::gradient_limit(self.drawable_slope);
        let items: Vec<planet_world_def::item::Item> = self.editor.items().cloned().collect();
        let factor = match problems::fit_factor(&items, id, limit, self.radius_m) {
            Some(f) if f < 1.0 => f,
            Some(_) => problems::REDUCE_STACKED,
            None => {
                return Err(SessionError::Refused(
                    "the other edits make this ground too steep even with a hundredth of its strokes: reduce or delete them".into(),
                ))
            }
        };
        let affected = self
            .editor
            .change(id, |it| {
                it.strokes = problems::scaled(it, factor);
                it.max_delta_m = it.strokes.iter().map(|s| s.strength_m).sum::<f64>().max(1.0e-6);
            })
            .map_err(SessionError::Edit)?;
        self.status = format!("strengths reduced to {:.0} %", factor * 100.0);
        Ok((factor, affected))
    }

    /// Merge the deepest stack of strokes of one kind in the brush item `id` into one stroke (VIEW-047, [`problems::merge_stack`]). One command.
    /// Returns how many strokes were merged.
    pub fn merge_item(&mut self, id: ItemId) -> Result<(usize, Vec<Affected>), SessionError> {
        let item = self.editor.item(id).ok_or_else(|| SessionError::Refused("no such edit".into()))?;
        if item.kind != brush::BRUSH_KIND {
            return Err(SessionError::Refused(format!("a {} has no strokes to merge", item.kind)));
        }
        let Some((strokes, merged)) = problems::merge_stack(&item.strokes, self.radius_m) else {
            return Err(SessionError::Refused("no two strokes of one kind overlap".into()));
        };
        let affected = self.editor.change(id, |it| it.strokes = strokes).map_err(SessionError::Edit)?;
        self.status = format!("{merged} strokes merged into one");
        Ok((merged, affected))
    }

    /// Every edit in the order they apply.
    pub fn chain(&self, camera_position: Vec3) -> Vec<Row> {
        self.flagged(lists::apply_order(self.editor.items(), camera_position, self.radius_m))
    }

    /// The inspector of the selected edit.
    pub fn inspector(&self, camera_position: Vec3) -> Option<Inspector> {
        let it = self.selected_item()?;
        let mut same: Vec<(i32, ItemId)> = self.editor.items().filter(|i| i.layer == it.layer).map(|i| (i.priority, i.id)).collect();
        same.sort();
        let place = same.iter().position(|(_, id)| *id == it.id).map_or(1, |p| p + 1);
        let mut inspector = params::inspector_of(it, (place, same.len()), lists::distance_m(camera_position, it, self.radius_m));
        inspector.region = params::region_rows(it, &self.layers());
        if it.kind == brush::BRUSH_KIND && self.health_lists().is_steep(it.id, it.group) {
            inspector.warning = Some(slope::WARNING.to_string());
        }
        if inspector.region.is_some() {
            // The surface type and the mask have their own controls; a typed number would bypass them (and print the refusal raw).
            inspector.params.retain(|p| p.key != "surface_type" && p.key != "mask_mode");
        }
        Some(inspector)
    }

    /// The handles of the selected edit with their places in `PlanetFixed` (`ground_m` above the sphere at the edit).
    pub fn handles(&self, ground_m: f64) -> Vec<(Handle, Vec3)> {
        let Some(it) = self.selected_item() else { return Vec::new() };
        // A line or a region has one handle per point of the whole line, following a drag in progress.
        if !it.points.is_empty() {
            let r = self.radius_m + ground_m;
            return self.drawn_points(it.id).iter().enumerate().map(|(i, p)| (Handle::Point(i), p.dir() * r)).collect();
        }
        shape::handles_of(it).into_iter().map(|h| (h, shape::handle_point(it, h, self.radius_m, ground_m))).collect()
    }

    fn named(&self, id: ItemId) -> String {
        self.editor.item(id).map_or_else(|| "edit".to_string(), lists::label_of)
    }

    /// Place a stamp of `kind` at the unit direction `dir`: selected afterwards, the tool back to select. The parameters are the kind's
    /// defaults.
    pub fn place(&mut self, kind: &str, dir: Vec3) -> Result<(ItemId, Vec<Affected>), SessionError> {
        let defs = params::defs_for(kind);
        if defs.is_empty() || LINE_KINDS.contains(&kind) {
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
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        };
        let (id, affected) = self.editor.put(new)?;
        self.selection = Some(id);
        self.tool = Tool::Select;
        self.status = format!("{} placed", self.named(id));
        Ok((id, affected))
    }

    /// Delete the selected edit; a line or a brush drag split into linked items goes whole, as one command.
    pub fn delete_selected(&mut self) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        self.drop_point_drag();
        let name = self.named(id);
        let members = self.editor.line_items(id).map_err(SessionError::Edit)?;
        let affected = if members.len() > 1 {
            self.editor.group(members.into_iter().map(group_delete).collect())?.1
        } else {
            self.editor.delete(id)?
        };
        self.selection = None;
        // Any part of a line that went with it, not only the selected one.
        self.hover = self.hover.filter(|h| self.editor.item(*h).is_some());
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
        // The amount and the width of a line belong to all its parts (and the width to their reach): one command for the line (VIEW-033).
        if (key == "amount_m" || key == "width_m") && LINE_KINDS.contains(&self.editor.item(id).map_or("", |i| i.kind.as_str())) {
            let (amount, width) = if key == "amount_m" { (Some(v), None) } else { (None, Some(v)) };
            let (_, affected) = self.editor.set_line_params(id, amount, width)?;
            self.status = format!("{} {} set to {}", self.named(id), key, params::format_value(v, unit));
            return Ok(affected);
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

    /// Set the surface type of the selected region (an index of [`planet_world_def::region::SURFACE_TYPES`]): one command.
    pub fn set_surface_type(&mut self, index: usize) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        if self.editor.item(id).is_none_or(|i| i.kind != "region") {
            return refused("the selected edit is not a region");
        }
        let Some(kind) = planet_world_def::region::SURFACE_TYPES.get(index) else {
            return refused(format!("there is no surface type {index} (0 to {})", planet_world_def::region::SURFACE_TYPES.len() - 1));
        };
        let affected = self.editor.change(id, |it| {
            it.params.insert("surface_type".to_string(), index as f64);
        })?;
        self.status = format!("{} is {}", self.named(id), kind.name);
        Ok(affected)
    }

    /// Make the selected region clip `layer` inside its outline (`inside`; the mask mode 2 and the layer name) or only label (mode 0, no
    /// layer): one change, so the item is never in between (the world definition refuses a clipping region without a layer and a labelling
    /// one with a layer). The layer may be any other layer name.
    pub fn set_mask(&mut self, inside: bool, layer: Option<&str>) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let Some(item) = self.editor.item(id).filter(|i| i.kind == "region") else {
            return refused("the selected edit is not a region");
        };
        if inside {
            let Some(layer) = layer.map(str::trim).filter(|l| !l.is_empty()) else {
                return refused("choose the layer the region clips");
            };
            if layer == item.layer {
                return refused("a region cannot clip its own layer");
            }
            let layer = layer.to_string();
            let affected = self.editor.change(id, |it| {
                it.params.insert("mask_mode".to_string(), 2.0);
                it.mask_layer = Some(layer);
            })?;
            self.status = format!("{} removes the layer's edits inside its outline", self.named(id));
            Ok(affected)
        } else {
            let affected = self.editor.change(id, |it| {
                it.params.insert("mask_mode".to_string(), 0.0);
                it.mask_layer = None;
            })?;
            self.status = format!("{} only labels", self.named(id));
            Ok(affected)
        }
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
        if let Handle::Point(i) = handle {
            // The points are held here until the release: the editor hears of the whole drag as one command (VIEW-029).
            let points = self.stored_points(id);
            if i >= points.len() {
                return refused(format!("this edit has no point {i}"));
            }
            self.drag = Some(handle);
            self.drag_points = Some(points);
            return Ok(());
        }
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
            Some(Handle::Point(i)) => {
                let (lat, lon) = shape::lat_lon_of(dir);
                if let Some(p) = self.drag_points.as_mut().and_then(|v| v.get_mut(i)) {
                    *p = GeoPoint::new(lat, lon);
                }
                Ok(Vec::new())
            }
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
        if let Handle::Point(_) = handle {
            let points = self.drag_points.take().unwrap_or_default();
            let Some(id) = self.selection else { return refused("nothing is selected") };
            let affected = self.set_points(id, &points)?;
            self.status = format!("{} point moved", self.named(self.selection.unwrap_or(id)));
            return Ok(affected);
        }
        let affected = self.editor.end_edit()?;
        self.status = format!(
            "{} {}",
            self.selection.map_or_else(|| "edit".to_string(), |id| self.named(id)),
            match handle {
                Handle::Move => "moved",
                Handle::Radius => "resized",
                Handle::Height => "raised",
                Handle::Point(_) => "point moved",
            }
        );
        Ok(affected)
    }

    /// The drag was abandoned (Esc, focus lost): the edit goes back and nothing is recorded.
    pub fn cancel_drag(&mut self) -> Result<Vec<Affected>, SessionError> {
        let Some(handle) = self.drag.take() else { return refused("no handle is held") };
        if let Handle::Point(_) = handle {
            // Nothing was told to the editor: dropping the held points is all there is to take back.
            self.drag_points = None;
            self.status = "drag cancelled".to_string();
            return Ok(Vec::new());
        }
        let affected = self.editor.cancel_edit()?;
        self.status = "drag cancelled".to_string();
        Ok(affected)
    }

    // ----- lines and regions (VIEW-029)

    /// The points of a line (all its parts, joined) or a region as the editor holds them; empty for an item without points.
    fn stored_points(&self, id: ItemId) -> Vec<GeoPoint> {
        let Some(item) = self.editor.item(id) else { return Vec::new() };
        if item.points.is_empty() {
            return Vec::new();
        }
        if item.kind == "region" {
            item.points.iter().map(|(la, lo)| GeoPoint::new(*la, *lo)).collect()
        } else {
            self.editor.line_points(id).unwrap_or_default()
        }
    }

    /// The points of `id` as they are drawn: while one of its point handles is held, the held points; else the stored ones.
    pub fn drawn_points(&self, id: ItemId) -> Vec<GeoPoint> {
        match (&self.drag_points, self.selection) {
            (Some(held), Some(sel)) if sel == id => held.clone(),
            _ => self.stored_points(id),
        }
    }

    /// The points the line or region tool has collected so far.
    pub fn draft(&self) -> &[GeoPoint] {
        &self.draft
    }

    /// Give the line or region `id` these points: one command. The selection stays on a part that is still there, else goes to the first.
    fn set_points(&mut self, id: ItemId, points: &[GeoPoint]) -> Result<Vec<Affected>, SessionError> {
        let Some(item) = self.editor.item(id) else { return refused("the edit is gone") };
        if item.points.is_empty() {
            return refused("this edit has no points");
        }
        let members = if item.kind == "region" { vec![id] } else { self.editor.line_items(id)? };
        let affected =
            if item.kind == "region" { self.editor.set_region_points(id, points)? } else { self.editor.set_line_points(id, points)?.1 };
        if self.selection.is_some_and(|s| self.editor.item(s).is_none()) {
            self.selection = members.first().copied().filter(|m| self.editor.item(*m).is_some());
        }
        Ok(affected)
    }

    /// A click of the line or region tool at the unit direction `dir`: the point joins the draft. Returns the number of points now. A
    /// point within [`MIN_POINT_SPACING_M`] of the last one, and a point beyond what the tool holds, is refused.
    pub fn add_draft_point(&mut self, dir: Vec3) -> Result<usize, SessionError> {
        let limit = match self.tool {
            Tool::Line(_) => MAX_DRAFT_LINE_POINTS,
            Tool::Region => polyline::MAX_POINTS,
            _ => return refused("no line or region tool is on"),
        };
        if !is_direction(dir) {
            return refused("that is not a place on the planet");
        }
        let (lat, lon) = shape::lat_lon_of(dir);
        let point = GeoPoint::new(lat, lon);
        if let Some(last) = self.draft.last() {
            if polyline::arc_m(last.dir(), point.dir(), self.radius_m) < MIN_POINT_SPACING_M {
                return refused("that is the last point again");
            }
        }
        if self.draft.len() >= limit {
            return refused(format!("at most {limit} points; finish it"));
        }
        self.draft.push(point);
        self.status = format!("{} point(s); Enter finishes, Esc cancels", self.draft.len());
        Ok(self.draft.len())
    }

    /// Take back the last point of the draft.
    pub fn undo_draft_point(&mut self) -> Result<usize, SessionError> {
        if self.draft.pop().is_none() {
            return refused("no point to take back");
        }
        self.status = format!("{} point(s)", self.draft.len());
        Ok(self.draft.len())
    }

    /// Drop the draft; the tool stays on.
    pub fn cancel_draft(&mut self) {
        if !self.draft.is_empty() {
            self.status = "draft dropped".to_string();
        }
        self.draft.clear();
    }

    /// Put the line or region of the draft: one command (a line longer than one item reaches is linked parts, still one command). It is
    /// selected afterwards and the tool goes back to select. A refusal (too few points, a region too large or crossing itself) leaves the
    /// draft as it is, so the user can take points back and finish again.
    pub fn finish_draft(&mut self) -> Result<(ItemId, Vec<Affected>), SessionError> {
        let (kind, layer, params, bound, least) = match &self.tool {
            Tool::Line(kind) if LINE_KINDS.contains(&kind.as_str()) => (
                kind.clone(),
                STAMP_LAYER,
                BTreeMap::from([("amount_m".to_string(), LINE_AMOUNT_M), ("width_m".to_string(), LINE_WIDTH_M)]),
                LINE_BOUND_M,
                polyline::MIN_LINE_POINTS,
            ),
            Tool::Line(kind) => return refused(format!("'{kind}' is not a kind of line")),
            Tool::Region => (
                "region".to_string(),
                REGION_LAYER,
                BTreeMap::from([("surface_type".to_string(), 0.0), ("mask_mode".to_string(), 0.0)]),
                0.0,
                polyline::MIN_REGION_POINTS,
            ),
            _ => return refused("no line or region tool is on"),
        };
        if self.draft.len() < least {
            return refused(format!("a {kind} needs at least {least} points, there are {}", self.draft.len()));
        }
        // A line counts once, however many items it is split into.
        let count = self.editor.items().filter(|i| i.kind == kind && i.part.is_none_or(|p| p == 0)).count() + 1;
        let first = self.draft[0];
        let template = NewItem {
            body: self.body,
            surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
            layer: layer.to_string(),
            priority: 0,
            kind: kind.clone(),
            anchor: Anchor { latitude_deg: first.lat_deg, longitude_deg: first.lon_deg, height_m: 0.0, heading_deg: 0.0 },
            reach_m: 1.0,
            max_delta_m: bound,
            params,
            name: Some(format!("{} {count}", params::kind_title(&kind))),
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        };
        let draft = self.draft.clone();
        let (id, affected) = if kind == "region" {
            self.editor.put_region(template, &draft)?
        } else {
            let (ids, affected) = self.editor.put_line(template, &draft)?;
            (ids[0], affected)
        };
        self.draft.clear();
        self.selection = Some(id);
        self.tool = Tool::Select;
        self.status = format!("{} placed", self.named(id));
        Ok((id, affected))
    }

    // ----- the brush (VIEW-035)

    pub fn brush_settings(&self) -> BrushSettings {
        self.brush
    }

    /// Set the brush radius in metres, within [`brush::RADIUS_RANGE_M`]. A stroke being painted keeps the radius it started with.
    pub fn set_brush_radius(&mut self, radius_m: f64) -> Result<(), SessionError> {
        let (lo, hi) = brush::RADIUS_RANGE_M;
        if !(radius_m.is_finite() && (lo..=hi).contains(&radius_m)) {
            return refused(format!("The brush radius must be between {lo} m and {} km", hi / 1000.0));
        }
        self.brush.radius_m = radius_m;
        Ok(())
    }

    /// Set the strength of a stroke in metres, within [`brush::STRENGTH_RANGE_M`]: the largest height change at the stroke's centre line.
    pub fn set_brush_strength(&mut self, strength_m: f64) -> Result<(), SessionError> {
        let (lo, hi) = brush::STRENGTH_RANGE_M;
        if !(strength_m.is_finite() && (lo..=hi).contains(&strength_m)) {
            return refused(format!("The brush strength must be between {lo} m and {hi} m"));
        }
        self.brush.strength_m = strength_m;
        Ok(())
    }

    /// Set the height a flatten levels towards (metres above the reference surface), or `None` for the terrain height under the start of
    /// each stroke.
    pub fn set_brush_target(&mut self, target_m: Option<f64>) -> Result<(), SessionError> {
        if target_m.is_some_and(|t| !t.is_finite()) {
            return refused("The flatten height must be a finite number");
        }
        self.brush.target_m = target_m;
        Ok(())
    }

    /// The stroke being painted, if any: its kind and numbers as it started with them and the places of the pointer so far (for the overlay
    /// and the transient preview).
    pub fn stroke_in_progress(&self) -> Option<&BrushStroke> {
        self.stroke.as_ref().map(|d| &d.stroke)
    }

    /// The button went down with a brush tool on: a stroke starts at `dir`, whose terrain is `ground_m` above the reference surface (a
    /// flatten without a typed height levels towards it). Nothing happens, and the reason is returned, when no brush tool is on or a
    /// stroke is already being painted.
    pub fn begin_stroke(&mut self, dir: Vec3, ground_m: f64) -> Result<(), SessionError> {
        let Tool::Brush(kind) = self.tool else { return refused("no brush tool is on") };
        if self.stroke.is_some() {
            return refused("a stroke is being painted");
        }
        if self.drag.is_some() {
            return refused("a handle is held");
        }
        if !is_direction(dir) {
            return refused("that is not a place on the planet");
        }
        let target_m = (kind == BrushKind::Flatten).then(|| self.brush.target_m.unwrap_or(ground_m));
        if target_m.is_some_and(|t| !t.is_finite()) {
            return refused("the terrain height here is not a number");
        }
        let at = shape::lat_lon_of(dir);
        self.stroke = Some(StrokeDraft {
            stroke: BrushStroke {
                kind,
                radius_m: self.brush.radius_m,
                strength_m: self.brush.strength_m,
                target_m,
                points: vec![GeoPoint::new(at.0, at.1)],
            },
        });
        Ok(())
    }

    /// The pointer moved over the planet while the button is down: `dir` is added to the stroke when it is further than
    /// [`MIN_POINT_SPACING_M`] from the last place. Without a stroke it does nothing.
    pub fn extend_stroke(&mut self, dir: Vec3) {
        let radius = self.radius_m;
        let Some(draft) = self.stroke.as_mut() else { return };
        if !is_direction(dir) || draft.stroke.points.len() >= MAX_STROKE_SAMPLES {
            return;
        }
        let at = shape::lat_lon_of(dir);
        let p = GeoPoint::new(at.0, at.1);
        let last = draft.stroke.points.last().expect("a stroke has a place");
        if polyline::arc_m(last.dir(), p.dir(), radius) >= MIN_POINT_SPACING_M {
            draft.stroke.points.push(p);
        }
    }

    /// The button went up: the stroke is put as one command (it joins a brush item of this session or starts one, see
    /// [`Editor::add_stroke`]). Returns the places to refresh; `Ok` with none when no stroke was being painted.
    pub fn end_stroke(&mut self) -> Result<Vec<Affected>, SessionError> {
        let Some(mut draft) = self.stroke.take() else { return Ok(Vec::new()) };
        // The slope guard (VIEW-043): a raise or lower is cut to the strength the stack of strokes it overlaps leaves room for.
        let mut clamped = None;
        let painted = draft.stroke.strength_m;
        {
            // Only the strokes of the brush items whose footprint meets the circle round the new stroke can overlap it (VIEW-048).
            let near = {
                let points: Vec<Vec3> = draft.stroke.points.iter().map(|g| g.dir()).collect();
                let centre = points.first().copied().unwrap_or(Vec3::new(1.0, 0.0, 0.0));
                let spread = points.iter().map(|d| polyline::arc_m(centre, *d, self.radius_m)).fold(0.0, f64::max);
                self.health().items_near(centre, spread + draft.stroke.radius_m)
            };
            let existing: Vec<&planet_world_def::item::ItemStroke> =
                near.iter().filter_map(|id| self.editor.item(*id)).flat_map(|i| i.strokes.iter()).collect();
            let allowed = slope::allowed_strength(&draft.stroke, &existing, self.radius_m, slope::gradient_limit(self.drawable_slope));
            if allowed < painted {
                if allowed < brush::STRENGTH_RANGE_M.0 {
                    self.status = format!(
                        "{} stroke not painted: the ground there is already too steep to draw ({painted} m would add to a stack that has no room left)",
                        draft.stroke.kind.name()
                    );
                    return Ok(Vec::new());
                }
                clamped = Some(allowed);
                draft.stroke.strength_m = allowed;
            }
        }
        let count = self.editor.items().filter(|i| i.kind == brush::BRUSH_KIND).count() + 1;
        let template = NewItem {
            body: self.body,
            surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
            layer: brush::BRUSH_LAYER.to_string(),
            priority: 0,
            kind: brush::BRUSH_KIND.to_string(),
            anchor: Anchor { latitude_deg: 0.0, longitude_deg: 0.0, height_m: 0.0, heading_deg: 0.0 },
            reach_m: 1.0,
            max_delta_m: 0.0,
            params: BTreeMap::new(),
            name: Some(format!("Brush {count}")),
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        };
        let (touched, affected) = self.editor.add_stroke(template, &draft.stroke, &self.brush_open)?;
        for id in touched {
            if !self.brush_open.contains(&id) {
                self.brush_open.push(id);
            }
        }
        let what = draft.stroke.kind.name();
        self.status = match clamped {
            Some(allowed) => {
                format!("{what} stroke clamped to {allowed:.2} m: the ground would be too steep to draw ({painted} m was asked)")
            }
            None => format!("{what} stroke painted; undo takes it back"),
        };
        Ok(affected)
    }

    /// The stroke is abandoned (Esc, the window lost the focus): nothing is put.
    pub fn cancel_stroke(&mut self) {
        self.stroke = None;
    }

    /// How far from the outline of `id` a click still counts as on it, metres: its half-width (a line) or 1.5 % of `distance_m`, the larger
    /// (what [`pick`](Session::pick) uses), at least a metre.
    pub fn outline_tolerance_m(&self, id: ItemId, distance_m: f64) -> f64 {
        let width = self.editor.item(id).and_then(|i| i.params.get("width_m").copied()).unwrap_or(0.0);
        width.max(LINE_PICK_FRACTION * distance_m).max(1.0)
    }

    /// A click on the outline of the selected line or region at `dir`, within `max_arc_m` metres of it: a point is added there, one
    /// command. Returns the index of the new point.
    pub fn insert_point_at(&mut self, dir: Vec3, max_arc_m: f64) -> Result<(usize, Vec<Affected>), SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let points = self.stored_points(id);
        if points.is_empty() {
            return refused("the selected edit has no points");
        }
        if !is_direction(dir) {
            return refused("that is not a place on the planet");
        }
        let closed = self.editor.item(id).is_some_and(|i| i.kind == "region");
        let Some(hit) = polyline::nearest_segment(&points, closed, dir.normalized(), self.radius_m) else {
            return refused("the selected edit has no outline");
        };
        if hit.distance_m > max_arc_m {
            return refused("that is not on the outline");
        }
        let (lat, lon) = (hit.foot.lat_deg, hit.foot.lon_deg);
        let foot = hit.foot.dir();
        if points.iter().any(|p| polyline::arc_m(p.dir(), foot, self.radius_m) < MIN_POINT_SPACING_M) {
            return refused("there is a point here already");
        }
        let new = polyline::insert_point(&points, hit.segment, GeoPoint::new(lat, lon)).map_err(EditError::from)?;
        let affected = self.set_points(id, &new)?;
        self.status = "point added".to_string();
        Ok((hit.segment + 1, affected))
    }

    /// Remove point `index` of the selected line or region: one command; refused when fewer than the kind needs would be left.
    pub fn remove_point(&mut self, index: usize) -> Result<Vec<Affected>, SessionError> {
        let Some(id) = self.selection else { return refused("nothing is selected") };
        let points = self.stored_points(id);
        if points.is_empty() {
            return refused("the selected edit has no points");
        }
        let least =
            if self.editor.item(id).is_some_and(|i| i.kind == "region") { polyline::MIN_REGION_POINTS } else { polyline::MIN_LINE_POINTS };
        let new = polyline::remove_point(&points, index, least).map_err(EditError::from)?;
        let affected = self.set_points(id, &new)?;
        self.status = "point removed".to_string();
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
        self.drop_point_drag();
        let r = self.editor.undo()?;
        if r.is_some() {
            self.after_history("undone");
        }
        Ok(r)
    }

    /// Do again the last command taken back; `None` when there is none.
    pub fn redo(&mut self) -> Result<Option<Vec<Affected>>, SessionError> {
        self.drop_point_drag();
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
