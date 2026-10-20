//! The editor mode of the window (VIEW-022): the session over the item store, the pointer and the keys turned into session calls, the
//! panels' data each frame, and the camera flight to an edit. The window (`window.rs`) is glue around this: it hands over the camera, the
//! cursor and the events, and draws what comes back. Nothing here names a window or a GPU type, so the flows are tested without either.

use crate::panels::{Event, PanelData, PanelState};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_editor::commands::{Affected, Editor};
use planet_editor::ids::IdMaker;
use planet_editor::interaction::{Interaction, PointerView, Pressed};
use planet_editor::layout::Layout;
use planet_editor::lists::{LayerGroup, Row};
use planet_editor::overlay::{self, OverlayItem};
use planet_editor::params::{kind_title, Inspector, STAMP_KINDS};
use planet_editor::relevance::ViewState;
use planet_editor::session::{Session, SessionError, Tool};
use planet_editor::shape::{self, Handle};
use planet_frame::freecam::FreePose;
use planet_frame::globe::{Marker, Viewport};
use planet_frame::pick::{cursor_ray, pick_surface, PickLimits, SurfaceHit};
use planet_frame::preview::{preview_delta, PreviewStamp};
use planet_frame::Camera;
use planet_generators::height::HeightField;
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::item::Item;
use planet_world_def::item_store::{ItemStore, LooseFileStore, MemoryStore, StoreError};
use planet_world_def::{BodyId, ItemId, WorldDefinition};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The first id of the markers the edits have on the globe overview; the markers of `--marker` are numbered from 1 and the smoke run's
/// own is 1000.
pub const EDIT_MARKER_BASE: u64 = 1_000_000;

/// The stage version new edits are authored against (the stamp stage of the generators takes it from the item).
pub const EDIT_STAGE_VERSION: u32 = 1;

/// Where the items go: memory (lost with the window) or one file each in a directory.
pub enum EditStore {
    Memory(MemoryStore),
    Files(LooseFileStore),
}

impl ItemStore for EditStore {
    fn put(&mut self, item: &Item) -> Result<(), StoreError> {
        match self {
            EditStore::Memory(s) => s.put(item),
            EditStore::Files(s) => s.put(item),
        }
    }

    fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
        match self {
            EditStore::Memory(s) => s.delete(body, id),
            EditStore::Files(s) => s.delete(body, id),
        }
    }

    fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
        match self {
            EditStore::Memory(s) => s.get(body, id),
            EditStore::Files(s) => s.get(body, id),
        }
    }

    fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
        match self {
            EditStore::Memory(s) => s.load_all(body),
            EditStore::Files(s) => s.load_all(body),
        }
    }
}

/// What the editor mode did in the run (the smoke summary reports it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditStats {
    pub frames: u64,
    pub placed: u64,
    pub selected: u64,
    pub dragged: u64,
    pub typed: u64,
    pub deleted: u64,
    pub undone: u64,
    pub redone: u64,
    pub flights: u64,
    pub refused: u64,
    /// The most edits the camera showed in one frame.
    pub shown_max: usize,
    /// The resident tiles the footprint find sent to refresh, summed over the commands (the terrain follows once the definition is live).
    pub refresh_tiles: u64,
    /// The times the definition was handed to the generator.
    pub published: u64,
    /// The previews set with at least one stamp (VIEW-023).
    pub previews: u64,
    /// The most nodes a frame drew previewed.
    pub previewed_max: usize,
}

/// A key the editor mode takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    /// A tool by number: 1 select, 2 and on the stamp kinds in order.
    Tool(usize),
    Undo,
    Redo,
    Delete,
    /// Esc: abandon a drag, else leave the stamp tool, else deselect.
    Escape,
}

/// The lists the panels show this frame.
#[derive(Default)]
struct Lists {
    near: Vec<Row>,
    found: Vec<Row>,
    groups: Vec<LayerGroup>,
    chain: Vec<Row>,
    hidden: Vec<ItemId>,
    inspector: Option<Inspector>,
}

/// The cursor in render pixels and whether it is over a panel.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub cursor: [f64; 2],
    pub over_ui: bool,
}

/// The editor mode.
pub struct EditRuntime {
    pub session: Session<EditStore>,
    pub interaction: Interaction,
    pub panels: PanelState,
    pub layout: Layout,
    /// The mode is on: the panels and the outlines show and the pointer edits.
    pub on: bool,
    pub stats: EditStats,
    saved: String,
    radius_m: f64,
    shown: Vec<ItemId>,
    overlay: Vec<OverlayItem>,
    handles: Vec<(Handle, [f64; 2])>,
    lists: Lists,
    camera_position: Vec3,
    /// An edit the camera is to fly to (set by a double click or Fly to; the window takes it).
    fly: Option<ItemId>,
    /// The places the commands of this frame changed, for the footprint find of the tiles (`take_affected`).
    affected: Vec<Affected>,
    /// The revision of the editor's definition the generator was last given (zero: the one the window opened with).
    published: u64,
    /// The preview of the edit in flight (VIEW-023).
    feed: PreviewFeed,
    /// The next refresh is every resident tile (the definition changed with no command: items loaded at the start).
    refresh_all: bool,
    /// The edits the last [`globe_markers`](EditRuntime::globe_markers) made markers of, by marker number minus [`EDIT_MARKER_BASE`].
    marker_ids: Vec<ItemId>,
}

/// The footprint items of the whole body, layer by layer in stack order (the order the stamp stages are stacked in, ADR 0007): what the
/// preview compares. Only stamps that differ count, so the whole body gives the same list as a neighbourhood of the edit.
fn body_items(def: &WorldDefinition) -> Vec<FootprintItem> {
    let all = Cap { centre: Vec3::new(1.0, 0.0, 0.0), angle: core::f64::consts::PI };
    def.stamp_layers(BodyId::PLANET)
        .iter()
        .flat_map(|layer| def.footprint_items_cap(BodyId::PLANET, layer, all).unwrap_or_default())
        .collect()
}

/// What the preview of an edit in flight needs (VIEW-023): the definition the generator was last given, and while an edit is in flight the
/// items of the oldest definition any resident tile was built from and the current items.
struct PreviewFeed {
    handed_over: Arc<WorldDefinition>,
    oldest: Option<Vec<FootprintItem>>,
    current: Vec<FootprintItem>,
}

impl PreviewFeed {
    fn new(handed_over: Arc<WorldDefinition>) -> PreviewFeed {
        PreviewFeed { handed_over, oldest: None, current: Vec::new() }
    }
}

/// The frame is exact with nothing substituted and the loop has nothing in flight: no unfinished tile job, no busy plan worker, no pending
/// upload (every refreshed tile is in; the end of the preview of VIEW-023 and of the edit-latency scene).
pub fn exact_and_idle(managed: &planet_frame::managed::ManagedLoop, frame: &planet_frame::managed::ManagedFrame) -> bool {
    frame.pipeline.draw.kind == planet_frame::pipeline::DrawKind::Exact
        && frame.pipeline.draw.substituted == 0
        && managed.manager().streamer().unfinished_jobs() == 0
        && !managed.pipeline().worker().busy()
        && managed.pending().is_empty()
}

fn open_store(dir: Option<&Path>) -> (EditStore, String) {
    match dir {
        Some(d) => (EditStore::Files(LooseFileStore::new(d)), format!("saved in {}", d.display())),
        None => (EditStore::Memory(MemoryStore::new()), "not saved (start with --items DIR)".to_string()),
    }
}

impl EditRuntime {
    /// The editor mode over a copy of the world's definition. With a directory the items in it are loaded and every command writes its file.
    pub fn new(def: WorldDefinition, radius_m: f64, items: Option<&PathBuf>, layout: Layout) -> Result<EditRuntime, String> {
        EditRuntime::with_ids(def, radius_m, items, layout, IdMaker::system())
    }

    /// [`new`](EditRuntime::new) with the clock and entropy of the ids given (tests).
    pub fn with_ids(
        def: WorldDefinition,
        radius_m: f64,
        items: Option<&PathBuf>,
        layout: Layout,
        ids: IdMaker,
    ) -> Result<EditRuntime, String> {
        let (store, saved) = open_store(items.map(PathBuf::as_path));
        let mut editor = Editor::new(def, store, ids, planet_generators::GENERATOR_VERSION, EDIT_STAGE_VERSION);
        // What the generator has is the definition as it was given, before the items of the directory are loaded into the editor's copy.
        let published = editor.revision();
        let feed = PreviewFeed::new(Arc::new(editor.definition().clone()));
        let loaded = editor.load(BodyId::PLANET).map_err(|e| format!("loading the items: {e}"))?;
        let mut session = Session::new(editor, BodyId::PLANET, radius_m);
        session.set_status(if loaded > 0 { format!("{loaded} edits loaded") } else { String::new() });
        Ok(EditRuntime {
            interaction: Interaction::new(layout.handle_hit_px),
            panels: PanelState::new(&layout),
            session,
            layout,
            on: false,
            stats: EditStats::default(),
            saved,
            radius_m,
            shown: Vec::new(),
            overlay: Vec::new(),
            handles: Vec::new(),
            lists: Lists::default(),
            camera_position: Vec3::new(radius_m, 0.0, 0.0),
            fly: None,
            affected: Vec::new(),
            published,
            feed,
            refresh_all: false,
            marker_ids: Vec::new(),
        })
    }

    pub fn edit_count(&self) -> usize {
        self.session.editor().items().count()
    }

    /// The edit mode on or off. Leaving it abandons a drag.
    pub fn toggle(&mut self) {
        self.on = !self.on;
        if !self.on {
            // The edit of an abandoned drag goes back, and its places are still to refresh.
            self.cancel_drag();
            self.session.set_hover(None);
        }
    }

    fn note(&mut self, r: Result<(), SessionError>) {
        if let Err(e) = r {
            self.stats.refused += 1;
            self.session.set_status(e.to_string());
        }
    }

    /// The places the commands since the last call changed.
    pub fn take_affected(&mut self) -> Vec<Affected> {
        std::mem::take(&mut self.affected)
    }

    /// Draw the outlines, names and handles over the planet and the panels; returns what the panels asked for. `scale` is window pixels per
    /// render pixel on each axis (the overlay is projected in render pixels).
    pub fn draw(&mut self, ui: &mut egui::Ui, view_status: &str, scale: [f64; 2]) -> Vec<Event> {
        let ppp = f64::from(ui.ctx().pixels_per_point());
        crate::panels::draw_overlay(ui, &self.overlay, |p| egui::pos2((p[0] * scale[0] / ppp) as f32, (p[1] * scale[1] / ppp) as f32));
        // The panels read the lists and write their own state: take the state out for the call.
        let mut panels = std::mem::replace(&mut self.panels, PanelState::new(&self.layout));
        let events = crate::panels::show(ui, &mut panels, &self.panel_data(view_status));
        self.panels = panels;
        events
    }

    /// The definition to hand to the generator when it differs from the one it was last given (once per change): the editor's copy as a
    /// shared snapshot (STRM-028). The commands that made the change have named the places to refresh; a change with no command (the items
    /// loaded at the start) refreshes every resident tile.
    pub fn take_changed_definition(&mut self) -> Option<std::sync::Arc<WorldDefinition>> {
        let revision = self.session.editor().revision();
        if revision == self.published {
            return None;
        }
        self.published = revision;
        self.refresh_all |= self.affected.is_empty();
        self.stats.published += 1;
        Some(std::sync::Arc::new(self.session.editor().definition().clone()))
    }

    /// The generator took `def` (VIEW-023): the preview of the edit in flight now runs from the oldest state a resident tile shows to `def`.
    /// The first hand-over since the preview ended records the items of the definition it replaces; the next ones keep them, so a drag in
    /// many steps is one delta from its first state.
    pub fn preview_begin(&mut self, def: &Arc<WorldDefinition>) {
        let before = std::mem::replace(&mut self.feed.handed_over, Arc::clone(def));
        if self.feed.oldest.is_none() {
            self.feed.oldest = Some(body_items(&before));
        }
        self.feed.current = body_items(def);
    }

    /// The stamps to give `ManagedLoop::set_preview` for the camera at `camera` (planet-fixed); empty when no edit is in flight. Counted.
    pub fn preview_stamps(&mut self, camera: Vec3) -> Vec<PreviewStamp> {
        let Some(oldest) = &self.feed.oldest else { return Vec::new() };
        let stamps = preview_delta(oldest, &self.feed.current, camera).stamps;
        self.stats.previews += u64::from(!stamps.is_empty());
        stamps
    }

    /// True while an edit is in flight: the preview is set and the exact tiles are not all in.
    pub fn preview_active(&self) -> bool {
        self.feed.oldest.is_some()
    }

    /// The exact tiles are in: the preview ends and the oldest state is forgotten.
    pub fn preview_end(&mut self) {
        self.feed.oldest = None;
    }

    /// The tiles of `resident` the commands since the last call changed (the footprint find, `planet_editor::feed`), counted; every
    /// resident tile after a change with no command. The window gives them to `ManagedLoop::edit` after it has handed over the definition.
    pub fn refresh_tiles(&mut self, resident: &[TileId]) -> Vec<TileId> {
        let affected = self.take_affected();
        if std::mem::take(&mut self.refresh_all) {
            self.stats.refresh_tiles += resident.len() as u64;
            return resident.to_vec();
        }
        if affected.is_empty() {
            return Vec::new();
        }
        let tiles = planet_editor::feed::edited_tiles(&affected, BodyId::PLANET, resident, self.radius_m, &planet_core::cube::TangentWarp);
        self.stats.refresh_tiles += tiles.len() as u64;
        tiles
    }

    /// The markers of the edits the camera shows (VIEW-015: the footprint find over the `wanted` tiles and the size of the reach on the
    /// screen, the selected edit always) for the globe overview, numbered from [`EDIT_MARKER_BASE`], sized by the outline radius. The edits
    /// do not have to be in the editor mode for this.
    pub fn globe_markers(&mut self, camera: &Camera, viewport: Viewport, wanted: &[TileId]) -> Vec<Marker> {
        let view = self.view(camera, viewport);
        let shown = self.session.shown(wanted, &view).unwrap_or_default();
        self.marker_ids = shown.iter().copied().filter(|id| self.session.item(*id).is_some()).collect();
        self.marker_ids
            .iter()
            .enumerate()
            .filter_map(|(k, id)| {
                let it = self.session.item(*id)?;
                Some(Marker { id: EDIT_MARKER_BASE + k as u64, dir: it.direction(), radius_m: shape::outline_radius_m(it).max(1.0) })
            })
            .collect()
    }

    /// The labels of the markers of the last [`globe_markers`](EditRuntime::globe_markers): the names of the edits.
    pub fn marker_labels(&self) -> HashMap<u64, String> {
        self.marker_ids
            .iter()
            .enumerate()
            .filter_map(|(k, id)| Some((EDIT_MARKER_BASE + k as u64, planet_editor::lists::label_of(self.session.item(*id)?))))
            .collect()
    }

    /// A click on the marker `marker`: the edit it stands for is selected. `false` for a marker that is not an edit's.
    pub fn select_marker(&mut self, marker: u64) -> bool {
        let Some(id) = marker.checked_sub(EDIT_MARKER_BASE).and_then(|k| self.marker_ids.get(k as usize)).copied() else { return false };
        self.session.select(id);
        self.stats.selected += 1;
        true
    }

    /// The edit the camera is to fly to, once.
    pub fn take_fly(&mut self) -> Option<ItemId> {
        self.fly.take()
    }

    fn view(&self, camera: &Camera, viewport: Viewport) -> ViewState {
        ViewState { camera: *camera, viewport, radius_m: self.radius_m }
    }

    /// The surface under a cursor (render pixels).
    pub fn hit_at(
        camera: &Camera,
        viewport: Viewport,
        cursor: [f64; 2],
        limits: &PickLimits,
        field: &dyn HeightField,
    ) -> Option<SurfaceHit> {
        let ray = cursor_ray(camera, cursor[0], cursor[1], viewport.width_px, viewport.height_px);
        pick_surface(&ray, limits, &|d| field.height(d))
    }

    /// The frame: which edits the camera shows (the footprint find over the `wanted` tiles), their outlines and handles on the screen,
    /// the hover, and the lists the panels show. `over_ui` is whether the cursor is over a panel (no hover then).
    pub fn update(
        &mut self,
        camera: &Camera,
        viewport: Viewport,
        wanted: &[TileId],
        field: &dyn HeightField,
        limits: &PickLimits,
        pointer: Pointer,
    ) {
        let Pointer { cursor, over_ui } = pointer;
        if !self.on {
            return;
        }
        self.stats.frames += 1;
        self.camera_position = camera.position.0;
        let view = self.view(camera, viewport);
        self.shown = match self.session.shown(wanted, &view) {
            Ok(ids) => ids,
            Err(e) => {
                self.session.set_status(format!("the edits in view could not be found: {e}"));
                Vec::new()
            }
        };
        self.stats.shown_max = self.stats.shown_max.max(self.shown.len());
        self.overlay = overlay::build(&self.session, &self.shown, &view, &|d| field.height(d));
        self.handles = overlay::handle_pixels(&self.overlay);
        if !over_ui {
            let hit = Self::hit_at(camera, viewport, cursor, limits, field);
            let mpp = self.height_mpp(camera, viewport);
            let v = PointerView { cursor, hit, shown: &self.shown, handles: &self.handles, metres_per_pixel: mpp };
            self.interaction.hover(&mut self.session, &v);
        } else if self.session.dragging().is_none() {
            self.session.set_hover(None);
        }
        // Hover and selection show in the outlines of this frame's overlay.
        for o in &mut self.overlay {
            o.hovered = self.session.hovered() == Some(o.id);
        }
        let cam = self.camera_position;
        let s = &self.session;
        let q = self.panels.query.trim();
        let kind = self.panels.kind.as_deref();
        self.lists = Lists {
            near: s.near(&self.shown, cam),
            found: if q.is_empty() && kind.is_none() { Vec::new() } else { s.search(q, kind, cam) },
            groups: s.layer_groups(cam),
            chain: s.chain(cam),
            hidden: s.editor().items().filter(|i| s.is_hidden(i.id)).map(|i| i.id).collect(),
            inspector: s.inspector(cam),
        };
    }

    /// Metres one pixel spans at the selected edit's height handle.
    fn height_mpp(&self, camera: &Camera, viewport: Viewport) -> f64 {
        let at = self.session.handles(0.0).into_iter().find(|(h, _)| *h == Handle::Height).map(|(_, p)| p);
        at.map_or(1.0, |p| shape::metres_per_pixel(camera, viewport.height_px, p))
    }

    /// The overlay to draw this frame.
    pub fn overlay(&self) -> &[OverlayItem] {
        &self.overlay
    }

    /// The data the panels show this frame.
    pub fn panel_data<'a>(&'a self, view_status: &'a str) -> PanelData<'a> {
        let s = &self.session;
        PanelData {
            layout: &self.layout,
            tool: s.tool(),
            selected: s.selected(),
            near: &self.lists.near,
            found: &self.lists.found,
            groups: &self.lists.groups,
            chain: &self.lists.chain,
            hidden: &self.lists.hidden,
            inspector: self.lists.inspector.as_ref(),
            can_undo: s.editor().can_undo(),
            can_redo: s.editor().can_redo(),
            undo_depth: s.editor().undo_depth(),
            status: s.status(),
            edit_count: self.edit_count(),
            saved: &self.saved,
            view_status,
        }
    }

    /// The primary button went down in the world at `cursor`.
    pub fn press(&mut self, camera: &Camera, viewport: Viewport, cursor: [f64; 2], limits: &PickLimits, field: &dyn HeightField) {
        if !self.on {
            return;
        }
        let hit = Self::hit_at(camera, viewport, cursor, limits, field);
        // The view borrows the lists and the interaction needs the session mutably: it works on copies.
        let (shown, handles, mpp) = (self.shown.clone(), self.handles.clone(), self.height_mpp(camera, viewport));
        let v = PointerView { cursor, hit, shown: &shown, handles: &handles, metres_per_pixel: mpp };
        match self.interaction.press(&mut self.session, &v) {
            Ok((Pressed::Placed(_), affected)) => {
                self.stats.placed += 1;
                self.affected.extend(affected);
            }
            Ok((Pressed::Selected(_), _)) => self.stats.selected += 1,
            Ok(_) => {}
            Err(e) => self.note(Err(e)),
        }
    }

    /// The pointer moved with the primary button down.
    pub fn moved(&mut self, camera: &Camera, viewport: Viewport, cursor: [f64; 2], limits: &PickLimits, field: &dyn HeightField) {
        if !self.on || self.session.dragging().is_none() {
            return;
        }
        let hit = Self::hit_at(camera, viewport, cursor, limits, field);
        let (shown, handles, mpp) = (self.shown.clone(), self.handles.clone(), self.height_mpp(camera, viewport));
        let v = PointerView { cursor, hit, shown: &shown, handles: &handles, metres_per_pixel: mpp };
        match self.interaction.moved(&mut self.session, &v) {
            Ok(a) => self.affected.extend(a),
            Err(e) => self.note(Err(e)),
        }
    }

    /// The primary button went up.
    pub fn release(&mut self) {
        match self.interaction.release(&mut self.session) {
            Ok(Some(a)) => {
                self.stats.dragged += 1;
                self.affected.extend(a);
            }
            Ok(None) => {}
            Err(e) => self.note(Err(e)),
        }
    }

    /// The window lost the focus or a drag was abandoned.
    pub fn cancel_drag(&mut self) {
        match self.interaction.cancel(&mut self.session) {
            Ok(Some(a)) => self.affected.extend(a),
            Ok(None) => {}
            Err(e) => self.note(Err(e)),
        }
    }

    /// A key of the editor mode.
    pub fn key(&mut self, k: EditKey) {
        if !self.on {
            return;
        }
        match k {
            EditKey::Tool(n) => self.apply(vec![Event::Tool(tool_number(n))]),
            EditKey::Undo => self.apply(vec![Event::Undo]),
            EditKey::Redo => self.apply(vec![Event::Redo]),
            EditKey::Delete => self.apply(vec![Event::Delete]),
            EditKey::Escape => {
                if self.session.dragging().is_some() {
                    self.cancel_drag();
                } else if self.session.tool() != &Tool::Select {
                    self.session.set_tool(Tool::Select);
                } else {
                    self.session.deselect();
                }
            }
        }
    }

    /// What the panels asked for.
    pub fn apply(&mut self, events: Vec<Event>) {
        for e in events {
            match e {
                Event::Tool(t) => self.session.set_tool(t),
                Event::Select(id) => {
                    self.session.select(id);
                    self.stats.selected += 1;
                }
                Event::FlyTo(id) => {
                    self.session.select(id);
                    self.fly = Some(id);
                }
                Event::Deselect => self.session.deselect(),
                Event::Undo => match self.session.undo() {
                    Ok(Some(a)) => {
                        self.stats.undone += 1;
                        self.affected.extend(a);
                    }
                    Ok(None) => self.session.set_status("nothing to undo"),
                    Err(e) => self.note(Err(e)),
                },
                Event::Redo => match self.session.redo() {
                    Ok(Some(a)) => {
                        self.stats.redone += 1;
                        self.affected.extend(a);
                    }
                    Ok(None) => self.session.set_status("nothing to redo"),
                    Err(e) => self.note(Err(e)),
                },
                Event::Delete => match self.session.delete_selected() {
                    Ok(a) => {
                        self.stats.deleted += 1;
                        self.affected.extend(a);
                    }
                    Err(e) => self.note(Err(e)),
                },
                Event::Param { key, text } => match self.session.set_param_text(&key, &text) {
                    Ok(a) => {
                        self.stats.typed += 1;
                        self.affected.extend(a);
                    }
                    Err(e) => self.note(Err(e)),
                },
                Event::Rename(name) => match self.session.rename_selected(&name) {
                    Ok(a) => self.affected.extend(a),
                    Err(e) => self.note(Err(e)),
                },
                Event::ToggleHidden(id) => self.session.toggle_hidden(id),
            }
        }
    }

    /// The camera pose that looks at an edit from the south, `2.5` outline radii away and `1.2` up (at least 150 m and 80 m), pitched
    /// down at it. `ground_m` is the height of the ground at a direction.
    pub fn fly_pose(&self, id: ItemId, ground_m: &dyn Fn(Vec3) -> f64) -> Option<FreePose> {
        let it = self.session.item(id)?;
        let dir = it.direction();
        let radius = shape::outline_radius_m(it);
        let (back, up) = ((2.5 * radius).max(150.0), (1.2 * radius).max(80.0));
        let from = shape::point_at(dir, 180.0, back, self.radius_m);
        let position = from * (self.radius_m + ground_m(dir).max(ground_m(from)) + up);
        let north = shape::tangent_frame(from).0;
        Some(FreePose::new(PlanetFixed(position), north, -libm::atan2(up, back)))
    }
}

/// The keys of the editor mode, for the usage text and the log.
pub const EDIT_KEYS_HELP: &str = "editor mode: Tab on/off (the panels, the outlines of the edits the camera shows, the planet is for editing: the pointer is free), \
     1 select, 2 crater, 3 hill, 4 plateau then click the planet to place it, click an edit to select it (double-click in a list to fly there), drag its handles \
     (circle: move, square: radius, light circle: height) or type values in the inspector, Ctrl+Z undo, Ctrl+Y or Ctrl+Shift+Z redo, Delete deletes, Esc steps back \
     (drag, tool, selection) and does not quit; hold the right mouse button to look around, WASD/QE move";

/// The first frame of the smoke run's editing script.
pub const SMOKE_FIRST_FRAME: u64 = 60;
/// The frames a smoke run with the editing script drives.
pub const SMOKE_EDIT_FRAMES: u64 = 90;

impl EditRuntime {
    /// A pixel on the vertical line through the middle of the view where the mouse ray meets the surface, from the lower part up.
    fn surface_pixel(camera: &Camera, viewport: Viewport, limits: &PickLimits, field: &dyn HeightField) -> Option<[f64; 2]> {
        (1..=9)
            .rev()
            .map(|k| [viewport.width_px / 2.0, viewport.height_px * f64::from(k) / 10.0])
            .find(|c| Self::hit_at(camera, viewport, *c, limits, field).is_some())
    }

    /// One frame of the smoke run's editing script, driven through the same calls as the pointer and the keys: the mode on, a hill placed
    /// where the view meets the surface, its move handle dragged, a value typed, undo, redo, delete, undo. Frames other than the script's
    /// do nothing.
    pub fn smoke_step(
        &mut self,
        frame: u64,
        camera: &Camera,
        viewport: Viewport,
        limits: &PickLimits,
        field: &dyn HeightField,
    ) -> Result<(), String> {
        match frame {
            f if f == SMOKE_FIRST_FRAME => {
                if !self.on {
                    self.toggle();
                }
                self.key(EditKey::Tool(3));
            }
            f if f == SMOKE_FIRST_FRAME + 2 => {
                let c = Self::surface_pixel(camera, viewport, limits, field).ok_or("edit smoke: no pixel of the view meets the surface")?;
                self.press(camera, viewport, c, limits, field);
                self.release();
                if self.stats.placed == 0 {
                    return Err(format!("edit smoke: the click at {c:?} placed nothing ({})", self.session.status()));
                }
            }
            f if f == SMOKE_FIRST_FRAME + 4 => {
                let h = self
                    .handles
                    .iter()
                    .find(|(k, _)| *k == Handle::Move)
                    .map(|(_, p)| *p)
                    .ok_or("edit smoke: the placed stamp has no move handle on the screen")?;
                self.press(camera, viewport, h, limits, field);
                self.moved(camera, viewport, [h[0] + 12.0, h[1]], limits, field);
            }
            f if f == SMOKE_FIRST_FRAME + 6 => self.release(),
            f if f == SMOKE_FIRST_FRAME + 8 => self.apply(vec![Event::Param { key: "radius_m".to_string(), text: "300".to_string() }]),
            f if f == SMOKE_FIRST_FRAME + 10 => self.key(EditKey::Undo),
            f if f == SMOKE_FIRST_FRAME + 12 => self.key(EditKey::Redo),
            f if f == SMOKE_FIRST_FRAME + 14 => self.key(EditKey::Delete),
            f if f == SMOKE_FIRST_FRAME + 16 => self.key(EditKey::Undo),
            _ => {}
        }
        Ok(())
    }

    /// The verdict of the editing script: everything it drove happened and nothing was refused.
    pub fn smoke_verdict(&self) -> Result<(), String> {
        let s = &self.stats;
        let ok = s.placed >= 1
            && s.dragged >= 1
            && s.typed >= 1
            && s.undone >= 2
            && s.redone >= 1
            && s.deleted >= 1
            && s.refused == 0
            && s.shown_max >= 1
            && s.published >= 1;
        if ok && self.edit_count() == 1 {
            Ok(())
        } else {
            Err(format!("edit smoke: the script did not complete: {} (one edit expected, {} there)", self.summary(), self.edit_count()))
        }
    }

    /// The counts of the run, one line.
    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "{} frames in the editor mode, {} placed, {} selected, {} dragged, {} typed, {} deleted, {} undone, {} redone, {} flights, {} refused, up to {} edits shown, {} tiles sent to refresh, {} definitions handed to the generator, {} previews set, up to {} nodes previewed",
            s.frames, s.placed, s.selected, s.dragged, s.typed, s.deleted, s.undone, s.redone, s.flights, s.refused, s.shown_max, s.refresh_tiles, s.published, s.previews, s.previewed_max
        )
    }
}

/// The tool a number key picks: 1 select, then the stamp kinds in the order of the rail.
pub fn tool_number(n: usize) -> Tool {
    match n {
        0 | 1 => Tool::Select,
        n => STAMP_KINDS.get(n - 2).map_or(Tool::Select, |k| Tool::Stamp((*k).to_string())),
    }
}

/// The status line of the editor mode in the window.
pub fn mode_status(e: &EditRuntime) -> String {
    match e.session.tool() {
        Tool::Select => "Select".to_string(),
        Tool::Stamp(k) => format!("Place {}", kind_title(k)),
        Tool::Line(k) => format!("Draw {}", kind_title(k)),
        Tool::Region => "Draw region".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::cube::TangentWarp;
    use planet_world_def::cells::cell_of;

    const R: f64 = 6_371_000.0;
    const VIEWPORT: Viewport = Viewport { width_px: 1000.0, height_px: 1000.0 };

    /// A bare sphere.
    struct Flat;

    impl HeightField for Flat {
        fn height(&self, _dir: Vec3) -> f64 {
            0.0
        }
        fn seed(&self) -> u64 {
            1
        }
        fn implementation_id(&self) -> &'static str {
            "flat-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            0.0
        }
    }

    fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
    }

    fn above(dir: Vec3, altitude_m: f64) -> Camera {
        Camera::at_surface_point(
            PlanetFixed(dir * (R + altitude_m)),
            Vec3::new(0.0, 0.0, 1.0),
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            1.0,
            1.0,
        )
    }

    fn runtime(items: Option<&PathBuf>) -> EditRuntime {
        let ids = IdMaker::new(Box::new(planet_editor::ids::FixedClock(1_760_000_000_000)), Box::new(planet_editor::ids::SeededEntropy(5)));
        EditRuntime::with_ids(WorldDefinition::single_planet(R, 7), R, items, Layout::default(), ids).unwrap()
    }

    const CENTRE: [f64; 2] = [500.0, 500.0];

    fn limits() -> PickLimits {
        PickLimits::new(R, 9_000.0)
    }

    fn frame(e: &mut EditRuntime, cam: &Camera, over_ui: bool) {
        let tiles = [TileId::from_direction(&TangentWarp, cam.position.0.normalized(), 8), cell_of(R, cam.position.0.normalized())];
        e.update(cam, VIEWPORT, &tiles, &Flat, &limits(), Pointer { cursor: CENTRE, over_ui });
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-editwin-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // spec: VIEW-022
    #[test]
    fn nothing_happens_until_the_mode_is_on_and_leaving_it_abandons_a_drag() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.key(EditKey::Tool(2));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        frame(&mut e, &cam, false);
        assert_eq!((e.edit_count(), e.stats, e.overlay().len()), (0, EditStats::default(), 0), "off: nothing is placed, counted or drawn");
        e.toggle();
        assert!(e.on);
        e.key(EditKey::Tool(2));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        frame(&mut e, &cam, false);
        e.press(&cam, VIEWPORT, e.handles[0].1, &limits(), &Flat);
        assert!(e.session.dragging().is_some(), "the move handle of the new stamp is held");
        e.moved(&cam, VIEWPORT, [CENTRE[0] + 30.0, CENTRE[1]], &limits(), &Flat);
        let before = e.session.editor().hash();
        e.toggle();
        assert!(!e.on && e.session.dragging().is_none(), "leaving the mode abandons the drag");
        assert_ne!(e.session.editor().hash(), before, "and the edit goes back");
        assert_eq!(e.session.editor().undo_depth(), 1, "only the placement is recorded");
        assert!(!e.take_affected().is_empty(), "the places of the abandoned drag are still to refresh");
    }

    // spec: VIEW-022
    #[test]
    fn a_stamp_is_placed_dragged_typed_undone_redone_and_deleted_by_the_pointer_and_the_keys() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        frame(&mut e, &cam, false);
        assert!(e.panel_data("").near.is_empty());
        // Place a hill with the stamp tool at the centre of the view.
        e.key(EditKey::Tool(3));
        assert_eq!(e.session.tool(), &Tool::Stamp("hill".into()));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let placed = e.take_affected();
        assert_eq!((e.stats.placed, e.edit_count(), placed.len()), (1, 1, 1));
        let id = e.session.selected().expect("the new stamp is selected");
        assert!(e.session.item(id).unwrap().direction().dot(at) > 1.0 - 1e-9, "placed under the cursor");
        frame(&mut e, &cam, false);
        assert_eq!(e.overlay().len(), 1);
        let d = e.panel_data("view");
        assert_eq!(
            (d.edit_count, d.near.len(), d.inspector.is_some(), d.saved, d.view_status),
            (1, 1, true, "not saved (start with --items DIR)", "view")
        );
        assert_eq!(mode_status(&e), "Select");
        // Drag the move handle 60 pixels right: one command.
        let depth = e.session.editor().undo_depth();
        let handle = e.handles.iter().find(|(h, _)| *h == Handle::Move).unwrap().1;
        e.press(&cam, VIEWPORT, handle, &limits(), &Flat);
        assert_eq!(e.session.dragging(), Some(Handle::Move));
        let before = e.session.item(id).unwrap().direction();
        for k in 1..=6 {
            e.moved(&cam, VIEWPORT, [handle[0] + 10.0 * f64::from(k), handle[1]], &limits(), &Flat);
        }
        e.release();
        assert_eq!((e.stats.dragged, e.session.editor().undo_depth()), (1, depth + 1));
        let after = e.session.item(id).unwrap().direction();
        let moved_m = R * libm::atan2(before.cross(after).length(), before.dot(after));
        let want = 60.0 * 3_000.0 * 2.0 * libm::tan(std::f64::consts::FRAC_PI_6) / 1000.0;
        assert!((moved_m - want).abs() < 0.03 * want, "60 px at 3 km up is {want} m, moved {moved_m}");
        assert!(!e.take_affected().is_empty());
        // A typed value, a refused one, and the status that says why.
        e.apply(vec![Event::Param { key: "radius_m".into(), text: "1.2 km".into() }]);
        assert_eq!((e.stats.typed, e.session.item(id).unwrap().reach_m), (1, 1_200.0));
        e.apply(vec![Event::Param { key: "radius_m".into(), text: "9 km".into() }]);
        assert_eq!(e.stats.refused, 1);
        assert!(e.session.status().contains("Radius must be between"), "{}", e.session.status());
        // Undo, redo, delete through the keys; the places to refresh come out.
        let _ = e.take_affected();
        e.key(EditKey::Undo);
        e.key(EditKey::Redo);
        e.key(EditKey::Delete);
        assert_eq!((e.stats.undone, e.stats.redone, e.stats.deleted, e.edit_count()), (1, 1, 1, 0));
        assert_eq!(e.take_affected().len(), 2 + 2 + 1, "undo and redo of a change have two places each, a delete one");
        e.key(EditKey::Undo);
        e.key(EditKey::Undo);
        e.key(EditKey::Undo);
        e.key(EditKey::Undo);
        e.key(EditKey::Undo);
        assert_eq!(e.edit_count(), 0, "everything undone");
        e.key(EditKey::Undo);
        assert!(e.session.status().contains("nothing to undo"));
    }

    // spec: VIEW-022
    #[test]
    fn a_press_selects_the_edit_under_it_and_escape_steps_back_one_thing_at_a_time() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(2));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let id = e.session.selected().unwrap();
        e.session.deselect();
        frame(&mut e, &cam, false);
        assert_eq!(e.session.hovered(), Some(id), "the cursor is over it");
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        assert_eq!((e.session.selected(), e.stats.selected), (Some(id), 1));
        e.release();
        // Over a panel there is no hover.
        frame(&mut e, &cam, true);
        assert_eq!(e.session.hovered(), None);
        frame(&mut e, &cam, false);
        // Esc: a drag first, then the tool, then the selection.
        let h = e.handles[0].1;
        e.press(&cam, VIEWPORT, h, &limits(), &Flat);
        e.key(EditKey::Escape);
        assert!(e.session.dragging().is_none() && e.session.selected().is_some());
        e.key(EditKey::Tool(2));
        e.key(EditKey::Escape);
        assert_eq!((e.session.tool().clone(), e.session.selected()), (Tool::Select, Some(id)));
        e.key(EditKey::Escape);
        assert_eq!(e.session.selected(), None);
        assert_eq!(tool_number(0), Tool::Select);
        assert_eq!(tool_number(1), Tool::Select);
        assert_eq!(tool_number(2), Tool::Stamp("crater".into()));
        assert_eq!(tool_number(4), Tool::Stamp("plateau".into()));
        assert_eq!(tool_number(9), Tool::Select);
    }

    // spec: VIEW-022
    #[test]
    fn a_double_click_or_fly_to_takes_the_camera_to_the_edit_looking_at_it() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(4));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let id = e.session.selected().unwrap();
        assert_eq!(e.take_fly(), None);
        e.apply(vec![Event::FlyTo(id)]);
        assert_eq!(e.take_fly(), Some(id));
        assert_eq!(e.take_fly(), None, "once");
        let pose = e.fly_pose(id, &|_| 0.0).unwrap();
        let item_pos = e.session.item(id).unwrap().direction() * R;
        let view = (item_pos - pose.position.0).normalized();
        let camera = pose.camera(std::f64::consts::FRAC_PI_3, 1.0, 1.0);
        assert!(camera.forward.dot(view) > 0.99, "the camera looks at the edit: {}", camera.forward.dot(view));
        let altitude = pose.position.0.length() - R;
        let radius = shape::outline_radius_m(e.session.item(id).unwrap());
        assert!((altitude - 1.2 * radius).abs() < 0.01 * radius, "{altitude} vs {radius}");
        assert!(e.fly_pose(12345, &|_| 0.0).is_none());
        // A small stamp is flown to from at least 150 m away and 80 m up.
        e.key(EditKey::Tool(3));
        e.session.set_tool(Tool::Stamp("hill".into()));
        let (hill, _) = e.session.place("hill", unit(10.0, 10.0)).unwrap();
        e.session
            .editor_mut()
            .change(hill, |it| {
                it.reach_m = 50.0;
            })
            .unwrap();
        let p = e.fly_pose(hill, &|_| 0.0).unwrap();
        assert!((p.position.0.length() - R - 80.0).abs() < 1.0);
    }

    // spec: VIEW-022
    #[test]
    fn the_edits_are_written_to_the_directory_and_come_back_when_it_is_opened_again() {
        let dir = temp_dir("items");
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(Some(&dir));
        assert!(e.panel_data("").saved.starts_with("saved in "));
        e.toggle();
        e.key(EditKey::Tool(2));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let id = e.session.selected().unwrap();
        e.apply(vec![Event::Param { key: "depth_m".into(), text: "400".into() }]);
        let files: Vec<_> =
            std::fs::read_dir(dir.join("0")).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(files, [format!("{}.toml", planet_world_def::item::ulid_text(id))]);
        let hash = e.session.editor().hash();
        drop(e);
        let again = runtime(Some(&dir));
        assert_eq!(again.edit_count(), 1);
        assert_eq!(again.session.item(id).unwrap().params["depth_m"], 400.0);
        assert_eq!(again.session.editor().hash(), hash, "the definition is the same");
        assert!(again.session.status().contains("1 edits loaded"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: VIEW-022
    #[test]
    fn the_search_lists_the_planet_and_the_nearby_list_the_shown_edits() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(2));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        e.session.place("plateau", unit(-30.0, 120.0)).unwrap();
        e.session.deselect();
        frame(&mut e, &cam, false);
        assert_eq!(e.panel_data("").near.len(), 1, "only the crater is in view");
        assert!(e.panel_data("").found.is_empty(), "no search yet");
        e.panels.query = "plateau".into();
        frame(&mut e, &cam, false);
        let found = &e.panel_data("").found;
        assert_eq!(found.len(), 1);
        assert!(found[0].distance_m > 1_000_000.0, "planet-wide, from the other side");
        e.panels.query.clear();
        e.panels.kind = Some("crater".into());
        frame(&mut e, &cam, false);
        assert_eq!(e.panel_data("").found.len(), 1);
        assert_eq!(e.stats.shown_max, 1);
    }

    // spec: VIEW-022
    #[test]
    fn the_smoke_script_places_drags_types_undoes_redoes_and_deletes_and_its_verdict_says_so() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        assert!(e.smoke_verdict().is_err(), "nothing driven yet");
        for f in 0..SMOKE_EDIT_FRAMES {
            // The window hands a changed definition to the generator at the start of each frame.
            if e.take_changed_definition().is_some() {
                let _ = e.refresh_tiles(&[]);
            }
            e.smoke_step(f, &cam, VIEWPORT, &limits(), &Flat).unwrap_or_else(|why| panic!("frame {f}: {why}"));
            frame(&mut e, &cam, false);
        }
        e.smoke_verdict().unwrap();
        let s = e.stats;
        assert_eq!((s.placed, s.dragged, s.typed, s.deleted, s.undone, s.redone, s.refused), (1, 1, 1, 1, 2, 1, 0));
        assert!(e.summary().contains("1 placed") && e.summary().contains("2 undone"));
        // The same script over a sky: no pixel meets the surface and the step says so.
        let sky = Camera::at_surface_point(
            PlanetFixed(at * (R + 3_000.0)),
            Vec3::new(0.0, 0.0, 1.0),
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            1.0,
            1.0,
        );
        let away = Camera { forward: sky.forward * -1.0, ..sky };
        let mut e2 = runtime(None);
        e2.smoke_step(SMOKE_FIRST_FRAME, &away, VIEWPORT, &limits(), &Flat).unwrap();
        let err = e2.smoke_step(SMOKE_FIRST_FRAME + 2, &away, VIEWPORT, &limits(), &Flat).unwrap_err();
        assert!(err.contains("no pixel"), "{err}");
    }

    // spec: VIEW-022
    #[test]
    fn the_commands_name_the_resident_tiles_to_refresh_and_count_them() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(4));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let level = 8;
        let near = TileId::from_direction(&TangentWarp, at, level);
        let far = TileId::from_direction(&TangentWarp, unit(-40.0, 100.0), level);
        let tiles = e.refresh_tiles(&[near, far]);
        assert_eq!(tiles, vec![near], "the plateau's reach touches the near tile and not the far one");
        assert_eq!(e.stats.refresh_tiles, 1);
        assert!(e.refresh_tiles(&[near, far]).is_empty(), "the places are taken once");
        assert_eq!(e.stats.refresh_tiles, 1);
    }

    // spec: VIEW-022
    #[test]
    fn a_changed_definition_is_handed_over_once_and_the_tiles_it_changed_are_refreshed() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let near = TileId::from_direction(&TangentWarp, at, 8);
        let far = TileId::from_direction(&TangentWarp, unit(-40.0, 100.0), 8);
        let mut e = runtime(None);
        assert!(e.take_changed_definition().is_none(), "nothing changed yet");
        e.toggle();
        e.key(EditKey::Tool(4));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let def = e.take_changed_definition().expect("a placement changes the definition");
        assert_eq!(def.item_ids(BodyId::PLANET).len(), 1);
        assert!(e.take_changed_definition().is_none(), "once per change");
        assert_eq!(e.refresh_tiles(&[near, far]), vec![near], "the tiles the plateau's reach touches");
        e.key(EditKey::Undo);
        let back = e.take_changed_definition().expect("an undo changes it again");
        assert!(back.item_ids(BodyId::PLANET).is_empty());
        assert_eq!(e.refresh_tiles(&[near, far]), vec![near], "where the plateau was");
        assert_eq!(e.stats.published, 2);
    }

    /// What the window does after a change: hand the definition over and start or extend the preview.
    fn hand_over(e: &mut EditRuntime) -> Arc<WorldDefinition> {
        let def = e.take_changed_definition().expect("a change to hand over");
        e.preview_begin(&def);
        def
    }

    // spec: VIEW-023
    #[test]
    fn a_drag_in_many_steps_previews_as_one_delta_from_its_first_state() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        let base = Arc::new(WorldDefinition::single_planet(R, 7));
        assert!(e.preview_stamps(cam.position.0).is_empty(), "no edit in flight, no preview");
        assert!(!e.preview_active());
        e.toggle();
        e.key(EditKey::Tool(3));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        hand_over(&mut e);
        assert!(e.preview_active());
        let one = e.preview_stamps(cam.position.0);
        assert_eq!(one.len(), 1, "the hill is added once: {one:?}");
        // Two more steps of the same edit before any frame settles (the radius typed twice, as a drag changes it).
        e.session.set_param_text("radius_m", "333").unwrap();
        hand_over(&mut e);
        e.session.set_param_text("radius_m", "1234").unwrap();
        let last = hand_over(&mut e);
        let three = e.preview_stamps(cam.position.0);
        let from_first = preview_delta(&body_items(&base), &body_items(&last), cam.position.0).stamps;
        assert_eq!(three, from_first, "the delta runs from the state before the first hand-over to the last");
        assert_eq!(three.len(), 1, "one stamp, at its final size, nothing removed that the first step added");
        assert_eq!(e.stats.previews, 2, "the previews set with a stamp are counted");
    }

    // spec: VIEW-023
    #[test]
    fn the_preview_ends_when_the_exact_tiles_are_in_and_the_next_edit_starts_from_the_definition_it_replaces() {
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(3));
        e.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        let first = hand_over(&mut e);
        assert!(e.preview_active());
        e.preview_end();
        assert!(!e.preview_active() && e.preview_stamps(cam.position.0).is_empty(), "ended: nothing to preview");
        e.session.set_param_text("radius_m", "777").unwrap();
        let second = hand_over(&mut e);
        let stamps = e.preview_stamps(cam.position.0);
        let from_replaced = preview_delta(&body_items(&first), &body_items(&second), cam.position.0).stamps;
        assert_eq!(stamps, from_replaced, "the delta runs from the definition the second hand-over replaced, not from the empty world");
        assert_eq!(stamps.len(), 2, "the old size is taken out and the new one put in");
    }

    // spec: VIEW-022
    #[test]
    fn items_loaded_at_the_start_reach_the_generator_and_refresh_every_resident_tile() {
        let dir = temp_dir("publish-load");
        let at = unit(47.2, 8.55);
        let cam = above(at, 3_000.0);
        let near = TileId::from_direction(&TangentWarp, at, 8);
        let far = TileId::from_direction(&TangentWarp, unit(-40.0, 100.0), 8);
        let mut first = runtime(Some(&dir));
        first.toggle();
        first.key(EditKey::Tool(3));
        first.press(&cam, VIEWPORT, CENTRE, &limits(), &Flat);
        drop(first);
        let mut again = runtime(Some(&dir));
        let def = again.take_changed_definition().expect("the loaded item is not in the generator's definition");
        assert_eq!(def.item_ids(BodyId::PLANET).len(), 1);
        assert_eq!(again.refresh_tiles(&[near, far]), vec![near, far], "no command named a place: every resident tile");
        assert!(again.refresh_tiles(&[near, far]).is_empty(), "once");
        assert!(again.take_changed_definition().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: VIEW-022
    #[test]
    fn the_edits_the_camera_shows_are_markers_of_the_overview_labelled_by_name_and_a_click_on_one_selects_it() {
        let at = unit(47.2, 8.55);
        let there = unit(47.25, 8.6);
        let low = above(at, 3_000.0);
        let mut e = runtime(None);
        e.toggle();
        e.key(EditKey::Tool(4));
        e.press(&low, VIEWPORT, CENTRE, &limits(), &Flat);
        let plateau = e.session.selected().unwrap();
        let (hill, _) = e.session.place("hill", there).unwrap();
        e.session.deselect();
        let tiles = [TileId::from_direction(&TangentWarp, at, 8), TileId::from_direction(&TangentWarp, there, 8)];
        // From 200 km up the plateau (15 km, about 65 pixels) is a marker and the hill (400 m, under 2 pixels) is not.
        let high = above(at, 200_000.0);
        let markers = e.globe_markers(&high, VIEWPORT, &tiles);
        assert_eq!(markers.len(), 1);
        assert_eq!((markers[0].id, markers[0].radius_m), (EDIT_MARKER_BASE, 15_000.0));
        assert!(markers[0].dir.dot(at) > 1.0 - 1e-9);
        assert_eq!(e.marker_labels(), HashMap::from([(EDIT_MARKER_BASE, "Plateau 1".to_string())]));
        // The selected edit is a marker whatever its size, and the markers follow the order of the ids.
        e.session.select(hill);
        let markers = e.globe_markers(&high, VIEWPORT, &tiles);
        assert_eq!(markers.iter().map(|m| m.id).collect::<Vec<_>>(), [EDIT_MARKER_BASE, EDIT_MARKER_BASE + 1]);
        assert_eq!(e.marker_labels()[&(EDIT_MARKER_BASE + 1)], "Hill 1");
        // Nothing in the view, no marker; a hidden edit is none unless it is the selected one.
        assert!(
            e.globe_markers(&high, VIEWPORT, &[]).iter().map(|m| m.id).eq([EDIT_MARKER_BASE]),
            "only the selected hill, outside every tile"
        );
        e.session.toggle_hidden(plateau);
        assert_eq!(e.globe_markers(&high, VIEWPORT, &tiles).len(), 1);
        e.session.toggle_hidden(plateau);
        // A click on a marker selects its edit; a number that is no edit's changes nothing.
        let markers = e.globe_markers(&high, VIEWPORT, &tiles);
        assert_eq!(markers.len(), 2);
        assert!(e.select_marker(EDIT_MARKER_BASE));
        assert_eq!(e.session.selected(), Some(plateau));
        assert!(e.select_marker(EDIT_MARKER_BASE + 1));
        assert_eq!(e.session.selected(), Some(hill));
        for not_an_edit in [0, 1, 1000, EDIT_MARKER_BASE - 1, EDIT_MARKER_BASE + 2, u64::MAX] {
            assert!(!e.select_marker(not_an_edit), "{not_an_edit}");
        }
        assert_eq!(e.session.selected(), Some(hill));
        // The markers do not need the editor mode.
        e.toggle();
        assert!(!e.on);
        assert_eq!(e.globe_markers(&high, VIEWPORT, &tiles).len(), 2);
    }
}
