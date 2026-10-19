//! The editor panels (VIEW-021): the tool rail, the tabs of nearby edits, layers and chain, the inspector with typed metric values, the
//! status bar, and the outlines and handles drawn over the planet. egui draws them; the layout (widths, side, tabs) is the data of
//! `planet_editor::layout::Layout`. The panels hold no editing state: they read plain data and return [`Event`]s, which the window turns
//! into session calls, so every flow is tested here without a window.

use planet_editor::layout::{Layout, Side, Tab};
use planet_editor::lists::{LayerGroup, Row};
use planet_editor::overlay::OverlayItem;
use planet_editor::params::{format_metres, Inspector, STAMP_KINDS};
use planet_editor::session::Tool;
use planet_editor::shape::Handle;
use planet_world_def::ItemId;
use std::collections::BTreeMap;

const AMBER: egui::Color32 = egui::Color32::from_rgb(240, 169, 59);
const BLUE: egui::Color32 = egui::Color32::from_rgb(76, 141, 246);
const LIGHT: egui::Color32 = egui::Color32::from_rgb(232, 230, 225);

/// What the panels ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A tool was picked in the rail.
    Tool(Tool),
    /// An edit was picked in a list.
    Select(ItemId),
    /// An edit was double-clicked in a list, or Fly to was pressed: the camera goes there.
    FlyTo(ItemId),
    Deselect,
    Undo,
    Redo,
    /// Delete the selected edit.
    Delete,
    /// A value was typed for a parameter of the selected edit.
    Param {
        key: String,
        text: String,
    },
    /// The selected edit was renamed.
    Rename(String),
    /// The checkbox of an edit in the layer list.
    ToggleHidden(ItemId),
}

/// What the panels remember between frames.
pub struct PanelState {
    pub tab: Tab,
    pub query: String,
    /// The kind the nearby tab is restricted to.
    pub kind: Option<String>,
    /// The text of the inspector's fields while they are edited.
    typed: BTreeMap<(ItemId, String), String>,
    name: Option<(ItemId, String)>,
    /// The rectangles of the named widgets in the last pass, in points (the tests click them; the window reads `covered`).
    pub marks: Vec<(String, egui::Rect)>,
    /// The rectangles the panels cover in the last pass, in points.
    pub covered: Vec<egui::Rect>,
}

impl PanelState {
    pub fn new(layout: &Layout) -> PanelState {
        PanelState {
            tab: layout.tabs[0],
            query: String::new(),
            kind: None,
            typed: BTreeMap::new(),
            name: None,
            marks: Vec::new(),
            covered: Vec::new(),
        }
    }

    /// The rectangle of a named widget in the last pass.
    pub fn mark(&self, name: &str) -> Option<egui::Rect> {
        self.marks.iter().find(|(n, _)| n == name).map(|(_, r)| *r)
    }

    /// Whether a point (in points) is over a panel.
    pub fn covers(&self, p: egui::Pos2) -> bool {
        self.covered.iter().any(|r| r.contains(p))
    }
}

/// What the panels show.
pub struct PanelData<'a> {
    pub layout: &'a Layout,
    pub tool: &'a Tool,
    pub selected: Option<ItemId>,
    /// The edits the camera shows, nearest first.
    pub near: &'a [Row],
    /// The search results (planet-wide), nearest first; used when the query or a kind filter is set.
    pub found: &'a [Row],
    pub groups: &'a [LayerGroup],
    pub chain: &'a [Row],
    pub hidden: &'a [ItemId],
    pub inspector: Option<&'a Inspector>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_depth: usize,
    pub status: &'a str,
    pub edit_count: usize,
    /// Where the edits are saved (a directory), or a note that they are not.
    pub saved: &'a str,
    /// The window's own status line (path, overlay).
    pub view_status: &'a str,
}

fn kind_color(kind: &str) -> egui::Color32 {
    match kind {
        "crater" | "hill" | "plateau" => AMBER,
        _ => BLUE,
    }
}

/// The text of a tool's button.
pub fn tool_label(tool: &Tool) -> String {
    match tool {
        Tool::Select => "Select  [1]".to_string(),
        Tool::Stamp(k) => {
            let n = STAMP_KINDS.iter().position(|s| s == k).map_or(0, |i| i + 2);
            format!("{}  [{n}]", planet_editor::params::kind_title(k))
        }
    }
}

fn row_button(ui: &mut egui::Ui, st: &mut PanelState, ev: &mut Vec<Event>, row: &Row, selected: bool, name: &str) {
    ui.horizontal(|ui| {
        ui.colored_label(kind_color(&row.kind), "●");
        let r = ui.selectable_label(selected, &row.label);
        st.marks.push((format!("{name}:{}", row.label), r.rect));
        if r.double_clicked() {
            ev.push(Event::FlyTo(row.id));
        } else if r.clicked() {
            ev.push(Event::Select(row.id));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(format_metres(row.distance_m));
        });
    });
    ui.small(format!("{} · reach {}", planet_editor::params::kind_title(&row.kind), format_metres(row.reach_m)));
}

fn rail(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    let mut tools = vec![Tool::Select];
    tools.extend(STAMP_KINDS.iter().map(|k| Tool::Stamp((*k).to_string())));
    for t in tools {
        let on = d.tool == &t;
        let r = ui.add(egui::Button::new(tool_label(&t)).selected(on));
        st.marks.push((format!("tool:{}", tool_label(&t)), r.rect));
        if r.clicked() {
            ev.push(Event::Tool(t));
        }
    }
    ui.separator();
    for later in ["Brush (later)", "Line (later)", "Region (later)"] {
        ui.add_enabled(false, egui::Button::new(later));
    }
}

fn left_panel(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    ui.horizontal(|ui| {
        for tab in &d.layout.tabs {
            let r = ui.selectable_value(&mut st.tab, *tab, tab.title());
            st.marks.push((format!("tab:{}", tab.key()), r.rect));
        }
    });
    ui.separator();
    if !d.layout.tabs.contains(&st.tab) {
        st.tab = d.layout.tabs[0];
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match st.tab {
        Tab::Near => {
            let r =
                ui.add(egui::TextEdit::singleline(&mut st.query).hint_text("Search edits by name or type").desired_width(f32::INFINITY));
            st.marks.push(("search".to_string(), r.rect));
            ui.horizontal_wrapped(|ui| {
                let mut kinds = vec![None];
                kinds.extend(STAMP_KINDS.iter().map(|k| Some((*k).to_string())));
                for k in kinds {
                    let title = k.as_deref().map_or("All".to_string(), planet_editor::params::kind_title);
                    let r = ui.selectable_label(st.kind == k, &title);
                    st.marks.push((format!("kind:{title}"), r.rect));
                    if r.clicked() {
                        st.kind = k;
                    }
                }
            });
            let searching = !st.query.trim().is_empty() || st.kind.is_some();
            let rows = if searching { d.found } else { d.near };
            ui.label(
                egui::RichText::new(if searching { "Matching, nearest to the camera" } else { "Nearest to the camera" }).small().weak(),
            );
            for row in rows {
                row_button(ui, st, ev, row, d.selected == Some(row.id), "row");
            }
            if rows.is_empty() {
                ui.weak(if searching {
                    "No edit matches. Clear the search or the filter."
                } else {
                    "No edit in view. Pick a stamp tool and click the planet."
                });
            }
        }
        Tab::Layers => {
            ui.label(egui::RichText::new("Apply order: bottom to top").small().weak());
            for g in d.groups {
                ui.strong(&g.layer);
                for row in &g.rows {
                    ui.horizontal(|ui| {
                        let mut shown = !d.hidden.contains(&row.id);
                        let c = ui.checkbox(&mut shown, "");
                        st.marks.push((format!("show:{}", row.label), c.rect));
                        if c.changed() {
                            ev.push(Event::ToggleHidden(row.id));
                        }
                        row_button_inline(ui, st, ev, row, d.selected == Some(row.id));
                    });
                }
            }
            ui.separator();
            ui.weak("Procedural base: locked, generator pinned");
        }
        Tab::Graph => {
            ui.label(egui::RichText::new("Chain, top applies first").small().weak());
            ui.group(|ui| {
                ui.weak("Procedural base");
            });
            for row in d.chain {
                ui.label("↓");
                row_button(ui, st, ev, row, d.selected == Some(row.id), "chain");
            }
            ui.label("↓");
            ui.group(|ui| {
                ui.weak("Tiles: regenerated where the edits touch");
            });
        }
    });
}

fn row_button_inline(ui: &mut egui::Ui, st: &mut PanelState, ev: &mut Vec<Event>, row: &Row, selected: bool) {
    let r = ui.selectable_label(selected, format!("{}  ({})", row.label, planet_editor::params::kind_title(&row.kind)));
    st.marks.push((format!("layer-row:{}", row.label), r.rect));
    if r.double_clicked() {
        ev.push(Event::FlyTo(row.id));
    } else if r.clicked() {
        ev.push(Event::Select(row.id));
    }
}

fn inspector(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    let Some(i) = d.inspector else {
        ui.weak("Inspector");
        ui.label("Nothing selected. Click an edit on the planet or pick one from the list. Choose a stamp tool on the left and click the planet to place one.");
        return;
    };
    ui.weak(format!("{} · {}", i.kind, i.layer));
    // The name, edited in place.
    let current = st.name.as_ref().filter(|(id, _)| *id == i.id).map(|(_, s)| s.clone()).unwrap_or_else(|| i.title.clone());
    let mut name = current;
    let r = ui.add(egui::TextEdit::singleline(&mut name).font(egui::TextStyle::Heading).desired_width(f32::INFINITY));
    st.marks.push(("name".to_string(), r.rect));
    if r.has_focus() || r.lost_focus() {
        st.name = Some((i.id, name.clone()));
    }
    if r.lost_focus() {
        if name.trim() != i.title {
            ev.push(Event::Rename(name.trim().to_string()));
        }
        st.name = None;
    }
    ui.small(&i.id_text);
    ui.add_space(4.0);
    egui::Grid::new("facts").num_columns(2).show(ui, |ui| {
        ui.weak("Anchor");
        ui.label(format!("{} · {}", i.latitude, i.longitude));
        ui.end_row();
        ui.weak("Distance");
        ui.label(format!("{} from the camera", i.distance));
        ui.end_row();
        ui.weak("Order");
        ui.label(&i.order);
        ui.end_row();
        ui.weak("Reach");
        ui.label(&i.reach);
        ui.end_row();
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Shape (type a value, or drag the handles on the planet)").small().weak());
    egui::Grid::new("params").num_columns(2).show(ui, |ui| {
        for p in &i.params {
            ui.label(&p.label);
            let key = (i.id, p.key.clone());
            let buf = st.typed.entry(key.clone()).or_insert_with(|| p.text.clone());
            let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0));
            st.marks.push((format!("param:{}", p.key), r.rect));
            if r.lost_focus() {
                if buf.trim() != p.text {
                    ev.push(Event::Param { key: p.key.clone(), text: buf.trim().to_string() });
                }
                st.typed.remove(&key);
            } else if !r.has_focus() {
                // Not being typed in: follow the value (a drag on the planet changes it).
                *buf = p.text.clone();
            }
            if let Some((lo, hi)) = p.range {
                ui.weak(format!(
                    "{} to {}",
                    planet_editor::params::format_value(lo, p.unit),
                    planet_editor::params::format_value(hi, p.unit)
                ));
            }
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let r = ui.button("Fly to");
        st.marks.push(("fly".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::FlyTo(i.id));
        }
        let r = ui.button("Deselect");
        st.marks.push(("deselect".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::Deselect);
        }
        let r = ui.add(egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::from_rgb(242, 179, 179))));
        st.marks.push(("delete".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::Delete);
        }
    });
}

/// The right-hand text of the status bar: the count of edits, the undo depth, where the edits are saved and the window's own status.
pub fn counts_line(d: &PanelData) -> String {
    format!("{} edits · undo {} · {} · {}", d.edit_count, d.undo_depth, d.saved, d.view_status)
}

fn status_bar(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    ui.horizontal(|ui| {
        let r = ui.add_enabled(d.can_undo, egui::Button::new("Undo  Ctrl+Z"));
        st.marks.push(("undo".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::Undo);
        }
        let r = ui.add_enabled(d.can_redo, egui::Button::new("Redo  Ctrl+Y"));
        st.marks.push(("redo".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::Redo);
        }
        ui.separator();
        ui.label(if d.status.is_empty() { "Ready" } else { d.status });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(counts_line(d));
        });
    });
}

/// Run the panels in `ui` (the root of an egui pass): the rail and the left panel, the inspector on the layout's side, the status bar.
/// Returns what was asked for; the covered rectangles are in the state.
pub fn show(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData) -> Vec<Event> {
    let mut ev = Vec::new();
    st.marks.clear();
    st.covered.clear();
    let l = d.layout;
    if l.status_bar {
        let r = egui::Panel::bottom("editor-status").show(ui, |ui| status_bar(ui, st, d, &mut ev));
        st.covered.push(r.response.rect);
    }
    if l.rail_width > 0.0 {
        let r =
            egui::Panel::left("editor-rail").exact_size(l.rail_width.max(110.0)).resizable(false).show(ui, |ui| rail(ui, st, d, &mut ev));
        st.covered.push(r.response.rect);
    }
    if l.inspector_side == Side::Left {
        let r = egui::Panel::left("editor-inspector")
            .exact_size(l.inspector_width)
            .resizable(false)
            .show(ui, |ui| inspector(ui, st, d, &mut ev));
        st.covered.push(r.response.rect);
    }
    let r = egui::Panel::left("editor-left").exact_size(l.left_width).resizable(false).show(ui, |ui| left_panel(ui, st, d, &mut ev));
    st.covered.push(r.response.rect);
    if l.inspector_side == Side::Right {
        let r = egui::Panel::right("editor-inspector")
            .exact_size(l.inspector_width)
            .resizable(false)
            .show(ui, |ui| inspector(ui, st, d, &mut ev));
        st.covered.push(r.response.rect);
    }
    ev
}

/// Draw the outlines, the names and the handles over the planet. `to_points` turns the overlay's pixels into points (the window's scale).
pub fn draw_overlay(ui: &egui::Ui, items: &[OverlayItem], to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    let painter = ui.painter();
    for o in items {
        let colour = kind_color(&o.kind);
        let (width, strength) = if o.selected {
            (2.5_f32, 1.0_f32)
        } else if o.hovered {
            (2.5, 0.9)
        } else {
            (1.5, 0.67)
        };
        let stroke = egui::Stroke::new(width, colour.gamma_multiply(strength));
        let pts: Vec<Option<egui::Pos2>> = o.outline.iter().map(|p| p.map(&to_points)).collect();
        if pts.iter().all(Option::is_some) && o.selected {
            let ring: Vec<egui::Pos2> = pts.iter().flatten().copied().collect();
            painter.add(egui::Shape::convex_polygon(ring, colour.gamma_multiply(0.18), stroke));
        } else {
            for k in 0..pts.len() {
                if let (Some(a), Some(b)) = (pts[k], pts[(k + 1) % pts.len()]) {
                    painter.line_segment([a, b], stroke);
                }
            }
        }
        if o.selected || o.hovered {
            if let Some(c) = o.centre.map(&to_points) {
                let galley_pos = c + egui::vec2(0.0, -16.0);
                let text = painter.text(galley_pos, egui::Align2::CENTER_BOTTOM, &o.label, egui::FontId::proportional(13.0), LIGHT);
                painter.rect_filled(text.expand(3.0), 3.0, egui::Color32::from_black_alpha(190));
                painter.text(galley_pos, egui::Align2::CENTER_BOTTOM, &o.label, egui::FontId::proportional(13.0), LIGHT);
            }
        }
        if o.selected {
            let at = |h: Handle| o.handles.iter().find(|(x, _)| *x == h).and_then(|(_, p)| p.map(&to_points));
            if let (Some(a), Some(b)) = (at(Handle::Move), at(Handle::Height)) {
                painter.line_segment([a, b], egui::Stroke::new(1.0, LIGHT.gamma_multiply(0.7)));
            }
            for (h, p) in &o.handles {
                let Some(p) = p.map(&to_points) else { continue };
                match h {
                    Handle::Move => {
                        painter.circle(p, 6.0, AMBER, egui::Stroke::new(1.5, egui::Color32::BLACK));
                    }
                    Handle::Radius => {
                        painter.rect_filled(egui::Rect::from_center_size(p, egui::vec2(11.0, 11.0)), 1.0, LIGHT);
                    }
                    Handle::Height => {
                        painter.circle(p, 5.5, LIGHT, egui::Stroke::new(1.5, egui::Color32::BLACK));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event as Raw, Modifiers, PointerButton, Pos2};
    use planet_core::Vec3;
    use planet_editor::commands::Editor;
    use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
    use planet_editor::overlay::OverlayItem;
    use planet_editor::session::Session;
    use planet_world_def::item_store::MemoryStore;
    use planet_world_def::{BodyId, WorldDefinition};

    const R: f64 = 6_371_000.0;
    const SIZE: (f32, f32) = (1920.0, 1080.0);

    /// A session with two stamps, the hill selected.
    struct Fixture {
        s: Session<MemoryStore>,
        ids: Vec<ItemId>,
        cam: Vec3,
    }

    fn fixture() -> Fixture {
        let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(3)));
        let mut s = Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R);
        let dir = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
        };
        let (a, _) = s.place("hill", dir(47.2, 8.55)).unwrap();
        let (b, _) = s.place("crater", dir(47.3, 8.6)).unwrap();
        s.select(a);
        Fixture { s, ids: vec![a, b], cam: dir(47.2, 8.55) * (R + 2_400.0) }
    }

    struct Rig {
        ctx: egui::Context,
        st: PanelState,
        t: f64,
    }

    impl Rig {
        fn new(layout: &Layout) -> Rig {
            let ctx = egui::Context::default();
            ctx.set_fonts(crate::ui::fonts());
            Rig { ctx, st: PanelState::new(layout), t: 0.0 }
        }

        fn pass(&mut self, d: &PanelData, events: Vec<egui::Event>) -> Vec<Event> {
            self.t += 0.05;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(SIZE.0, SIZE.1))),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let st = &mut self.st;
            let mut got = Vec::new();
            let mut out = self.ctx.run_ui(input, |ui| got = show(ui, st, d));
            out.textures_delta.clear();
            got
        }

        fn settle(&mut self, d: &PanelData) {
            for _ in 0..4 {
                self.pass(d, vec![]);
            }
        }

        /// A press and a release at `p`, in separate passes; the events of all of them.
        fn click(&mut self, d: &PanelData, p: Pos2) -> Vec<Event> {
            let button = |down| Raw::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE };
            let mut all = self.pass(d, vec![Raw::PointerMoved(p)]);
            all.extend(self.pass(d, vec![button(true)]));
            all.extend(self.pass(d, vec![button(false)]));
            all.extend(self.pass(d, vec![]));
            all
        }

        fn mark(&self, name: &str) -> Pos2 {
            self.st
                .mark(name)
                .unwrap_or_else(|| panic!("no widget '{name}' in {:?}", self.st.marks.iter().map(|m| &m.0).collect::<Vec<_>>()))
                .center()
        }
    }

    struct Held {
        layout: Layout,
        tool: Tool,
        near: Vec<Row>,
        found: Vec<Row>,
        groups: Vec<LayerGroup>,
        chain: Vec<Row>,
        inspector: Option<Inspector>,
        selected: Option<ItemId>,
        can_undo: bool,
    }

    fn held(f: &Fixture, layout: Layout) -> Held {
        Held {
            layout,
            tool: Tool::Select,
            near: f.s.near(&f.ids, f.cam),
            found: f.s.search("crater", None, f.cam),
            groups: f.s.layer_groups(f.cam),
            chain: f.s.chain(f.cam),
            inspector: f.s.inspector(f.cam),
            selected: f.s.selected(),
            can_undo: true,
        }
    }

    fn data(h: &Held) -> PanelData<'_> {
        PanelData {
            layout: &h.layout,
            tool: &h.tool,
            selected: h.selected,
            near: &h.near,
            found: &h.found,
            groups: &h.groups,
            chain: &h.chain,
            hidden: &[],
            inspector: h.inspector.as_ref(),
            can_undo: h.can_undo,
            can_redo: false,
            undo_depth: 2,
            status: "Hill 1 placed",
            edit_count: 2,
            saved: "not saved",
            view_status: "",
        }
    }

    // spec: VIEW-021
    #[test]
    fn the_panels_sit_where_the_layout_says_and_the_pointer_over_one_is_covered() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert_eq!(rig.st.covered.len(), 4, "status bar, rail, left panel, inspector");
        let right =
            rig.st.covered.iter().find(|r| (r.max.x - SIZE.0).abs() < 1.0 && r.width() < 500.0).expect("the inspector is on the right");
        assert!((right.width() - layout.inspector_width).abs() < 2.0, "{right:?}");
        let left = rig.st.covered.iter().find(|r| (r.width() - layout.left_width).abs() < 2.0).expect("the left panel has its width");
        assert!(rig.st.covers(left.center()) && rig.st.covers(right.center()));
        assert!(!rig.st.covers(Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0)), "the middle of the window is the planet's");
        // Another layout: the inspector on the left, no rail, no status bar, one tab.
        let l2 = Layout::from_toml("inspector_side = \"left\"\nrail_width = 0\nstatus_bar = false\nleft_width = 360\ntabs = [\"graph\"]\n")
            .unwrap();
        let h2 = held(&f, l2.clone());
        let mut rig2 = Rig::new(&l2);
        rig2.settle(&data(&h2));
        assert_eq!(rig2.st.covered.len(), 2);
        assert!(rig2.st.mark("tool:Select  [1]").is_none() && rig2.st.mark("undo").is_none(), "no rail, no status bar");
        assert!(rig2.st.covered.iter().any(|r| r.min.x < 1.0), "a panel at the left edge");
        assert!(rig2.st.covered.iter().all(|r| r.max.x < 800.0), "nothing on the right");
        assert_eq!(rig2.st.tab, Tab::Graph, "the first tab of the layout");
        assert!(rig2.st.mark("tab:near").is_none());
    }

    // spec: VIEW-021
    #[test]
    fn the_rail_picks_a_tool_and_the_tabs_switch_without_asking_for_anything() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        let ev = rig.click(&data(&h), rig.mark("tool:Crater  [2]"));
        assert_eq!(ev, vec![Event::Tool(Tool::Stamp("crater".into()))]);
        let ev = rig.click(&data(&h), rig.mark("tool:Select  [1]"));
        assert_eq!(ev, vec![Event::Tool(Tool::Select)]);
        let ev = rig.click(&data(&h), rig.mark("tab:layers"));
        assert!(ev.is_empty() && rig.st.tab == Tab::Layers);
        assert!(
            rig.st.mark("row:Hill 1").is_none() && rig.st.mark("layer-row:Hill 1").is_some(),
            "the layers tab lists the edits by layer"
        );
        rig.click(&data(&h), rig.mark("tab:graph"));
        assert_eq!(rig.st.tab, Tab::Graph);
        assert!(rig.st.mark("chain:Hill 1").is_some() && rig.st.mark("chain:Crater 1").is_some());
        rig.click(&data(&h), rig.mark("tab:near"));
        assert_eq!(rig.st.tab, Tab::Near);
    }

    // spec: VIEW-021
    #[test]
    fn a_click_on_a_row_selects_a_double_click_flies_and_the_layer_checkbox_hides() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        let ev = rig.click(&data(&h), rig.mark("row:Crater 1"));
        assert_eq!(ev, vec![Event::Select(f.ids[1])]);
        // A double click: two presses at one place within the double-click time.
        let p = rig.mark("row:Crater 1");
        let d = data(&h);
        let button = |down| Raw::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE };
        rig.pass(&d, vec![Raw::PointerMoved(p)]);
        let mut all = Vec::new();
        for _ in 0..2 {
            all.extend(rig.pass(&d, vec![button(true)]));
            all.extend(rig.pass(&d, vec![button(false)]));
        }
        assert!(all.contains(&Event::FlyTo(f.ids[1])), "{all:?}");
        rig.click(&data(&h), rig.mark("tab:layers"));
        let ev = rig.click(&data(&h), rig.mark("show:Hill 1"));
        assert_eq!(ev, vec![Event::ToggleHidden(f.ids[0])]);
    }

    // spec: VIEW-021
    #[test]
    fn the_search_box_and_the_kind_filter_switch_the_list_to_the_planet_wide_results() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert!(rig.st.mark("row:Hill 1").is_some(), "the nearby list at first");
        let p = rig.mark("search");
        rig.click(&data(&h), p);
        rig.pass(&data(&h), vec![Raw::Text("crater".into())]);
        rig.settle(&data(&h));
        assert_eq!(rig.st.query, "crater");
        assert!(
            rig.st.mark("row:Crater 1").is_some() && rig.st.mark("row:Hill 1").is_none(),
            "only the results (the fixture's search answers `crater`)"
        );
        rig.st.query.clear();
        rig.click(&data(&h), rig.mark("kind:Hill"));
        assert_eq!(rig.st.kind.as_deref(), Some("hill"));
        rig.click(&data(&h), rig.mark("kind:All"));
        assert_eq!(rig.st.kind, None);
    }

    fn key(k: egui::Key, modifiers: Modifiers) -> Raw {
        Raw::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers }
    }

    // spec: VIEW-021
    #[test]
    fn a_typed_value_is_asked_for_when_the_field_is_left_and_only_if_it_changed() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        let field = rig.mark("param:radius_m");
        // Focus the field and click elsewhere without a change: nothing is asked.
        rig.click(&data(&h), field);
        let ev = rig.click(&data(&h), Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0));
        assert!(!ev.iter().any(|e| matches!(e, Event::Param { .. })), "{ev:?}");
        // Focus, select all, type, Enter.
        rig.click(&data(&h), field);
        let all = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        rig.pass(&data(&h), vec![key(egui::Key::A, all)]);
        rig.pass(&data(&h), vec![Raw::Text("1.5 km".into())]);
        let mut got = rig.pass(&data(&h), vec![key(egui::Key::Enter, Modifiers::NONE)]);
        got.extend(rig.pass(&data(&h), vec![]));
        assert!(got.contains(&Event::Param { key: "radius_m".into(), text: "1.5 km".into() }), "{got:?}");
        // The fields follow the value when they are not being typed in.
        let mut h2 = held(&f, layout.clone());
        if let Some(i) = h2.inspector.as_mut() {
            i.params[0].text = "777 m".into();
        }
        rig.settle(&data(&h2));
        rig.settle(&data(&h2));
        assert_eq!(rig.st.typed.get(&(f.ids[0], "radius_m".into())).map(String::as_str), Some("777 m"));
    }

    // spec: VIEW-021
    #[test]
    fn the_buttons_ask_for_undo_redo_delete_deselect_and_fly_and_a_disabled_one_asks_for_nothing() {
        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert_eq!(rig.click(&data(&h), rig.mark("undo")), vec![Event::Undo]);
        assert!(rig.click(&data(&h), rig.mark("redo")).is_empty(), "nothing to redo, the button is disabled");
        assert_eq!(rig.click(&data(&h), rig.mark("delete")), vec![Event::Delete]);
        assert_eq!(rig.click(&data(&h), rig.mark("deselect")), vec![Event::Deselect]);
        assert_eq!(rig.click(&data(&h), rig.mark("fly")), vec![Event::FlyTo(f.ids[0])]);
        h.can_undo = false;
        rig.settle(&data(&h));
        assert!(rig.click(&data(&h), rig.mark("undo")).is_empty());
        // Without a selection the inspector is a hint and has none of these.
        h.inspector = None;
        h.selected = None;
        rig.settle(&data(&h));
        assert!(rig.st.mark("delete").is_none() && rig.st.mark("param:radius_m").is_none());
    }

    // spec: VIEW-021
    #[test]
    fn the_overlay_draws_outlines_names_and_handles_and_skips_what_is_off_the_screen() {
        let ring = |r: f64| -> Vec<Option<[f64; 2]>> {
            (0..64).map(|k| Some([500.0 + r * libm::cos(k as f64 * 0.098), 400.0 + r * libm::sin(k as f64 * 0.098)])).collect()
        };
        let mut half = ring(50.0);
        for p in half.iter_mut().take(20) {
            *p = None;
        }
        let item = |id, label: &str, kind: &str, selected, hovered, outline, centre, handles| OverlayItem {
            id,
            label: label.into(),
            kind: kind.into(),
            selected,
            hovered,
            outline,
            centre,
            handles,
        };
        let items = vec![
            item(1, "Hill 1", "hill", false, false, ring(80.0), Some([500.0, 400.0]), vec![]),
            item(2, "Crater 1", "crater", false, true, half, Some([500.0, 400.0]), vec![]),
            item(
                3,
                "Plateau 1",
                "plateau",
                true,
                false,
                ring(120.0),
                Some([500.0, 400.0]),
                vec![(Handle::Move, Some([500.0, 400.0])), (Handle::Radius, Some([620.0, 400.0])), (Handle::Height, None)],
            ),
            item(4, "Gone", "hill", false, false, vec![None; 64], None, vec![]),
        ];
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui::fonts());
        let count = |items: &[OverlayItem], ppp: f32| {
            ctx.set_pixels_per_point(ppp);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1920.0 / ppp, 1080.0 / ppp))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| draw_overlay(ui, items, |p| Pos2::new(p[0] as f32 / ppp, p[1] as f32 / ppp)));
            let n = out.shapes.len();
            out.textures_delta.clear();
            n
        };
        assert!(count(&items, 1.0) >= 8, "outlines, names, handles");
        assert_eq!(count(&items[3..], 1.0), 0, "an edit with nothing on the screen draws nothing");
        assert!(count(&items[..1], 1.0) > 0 && count(&items, 1.5) >= 8, "at another scale too");
    }

    // spec: VIEW-021
    #[test]
    fn a_name_typed_in_the_inspector_is_asked_for_when_the_field_is_left_and_the_counts_line_says_where_things_are() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        let field = rig.mark("name");
        rig.click(&data(&h), field);
        let all = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        rig.pass(&data(&h), vec![key(egui::Key::A, all)]);
        rig.pass(&data(&h), vec![Raw::Text("Old Man".into())]);
        let mut got = rig.pass(&data(&h), vec![key(egui::Key::Enter, Modifiers::NONE)]);
        got.extend(rig.pass(&data(&h), vec![]));
        assert!(got.contains(&Event::Rename("Old Man".into())), "{got:?}");
        // Left unchanged it asks for nothing.
        rig.click(&data(&h), field);
        let ev = rig.click(&data(&h), Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0));
        assert!(!ev.iter().any(|e| matches!(e, Event::Rename(_))), "{ev:?}");
        let d = data(&h);
        assert_eq!(counts_line(&d), "2 edits · undo 2 · not saved · ");
        let d = PanelData { saved: "saved in edits", view_status: "Place Hill", edit_count: 7, undo_depth: 12, ..data(&h) };
        assert_eq!(counts_line(&d), "7 edits · undo 12 · saved in edits · Place Hill");
    }
}
