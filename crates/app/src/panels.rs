//! The editor panels (VIEW-021): the tool rail, the tabs of nearby edits, layers and chain, the inspector with typed metric values, the
//! status bar, and the outlines and handles drawn over the planet. egui draws them; the layout (widths, side, tabs) is the data of
//! `planet_editor::layout::Layout`. The panels hold no editing state: they read plain data and return [`Event`]s, which the window turns
//! into session calls, so every flow is tested here without a window.

use planet_editor::layout::{Layout, Side, Tab};
use planet_editor::lists::{LayerGroup, Row};
use planet_editor::overlay::OverlayItem;
use planet_editor::params::{format_metres, Inspector, STAMP_KINDS};
use planet_editor::session::Tool;
use planet_editor::session::LINE_KINDS;
use planet_editor::shape::Handle;
use planet_world_def::stamp::BrushKind;
use planet_world_def::ItemId;
use std::collections::BTreeMap;

const AMBER: egui::Color32 = egui::Color32::from_rgb(240, 169, 59);
const BLUE: egui::Color32 = egui::Color32::from_rgb(76, 141, 246);
const LIGHT: egui::Color32 = egui::Color32::from_rgb(232, 230, 225);
const ORANGE: egui::Color32 = egui::Color32::from_rgb(214, 126, 60);
const TEAL: egui::Color32 = egui::Color32::from_rgb(70, 170, 200);
const GREEN: egui::Color32 = egui::Color32::from_rgb(110, 190, 100);
const BRUSH_COLOUR: egui::Color32 = egui::Color32::from_rgb(200, 130, 220);

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
    /// The problems list (VIEW-047): reduce the strengths of a brush, merge its stack of strokes, delete the edit.
    Reduce(ItemId),
    Merge(ItemId),
    DeleteItem(ItemId),
    /// The `?` button or the help key: show or hide the list of keys.
    ToggleHelp,
    /// The brush radius, strength or flatten height was typed (metres; the flatten height may be empty or "auto").
    BrushRadius(String),
    BrushStrength(String),
    BrushTarget(String),
    /// The surface type of the selected region was picked (an index of the table).
    SurfaceType(usize),
    /// The mask of the selected region was set: clip `layer` inside its outline, or (not `inside`) only label.
    Mask {
        inside: bool,
        layer: Option<String>,
    },
    /// Save current camera view as a bookmark (VIEW-073, VIEW-075).
    SaveBookmark {
        name: String,
    },
    /// Fly camera to a saved bookmark by id.
    JumpBookmark(u64),
    /// Rename an existing bookmark.
    RenameBookmark {
        id: u64,
        name: String,
    },
    /// Delete a saved bookmark by id.
    DeleteBookmark(u64),
    /// Jump camera to typed `LAT,LON[,ALT]` coordinates.
    JumpCoords(String),
    /// Trigger an F9 repro bundle capture from the Sites tab (VIEW-074, VIEW-075).
    CaptureRepro,
    /// Re-scan the repro directory for `.bundle` captures.
    RefreshRepros,
    /// Jump camera to the exact pose of repro entry `index`.
    JumpRepro(usize),
    /// Promote repro entry `index` into a persistent bookmark.
    BookmarkRepro(usize),
    /// Arm the placement of a new region box under `name` (empty: `region-<n>`, VIEW-077).
    PlaceRegion {
        name: String,
    },
    /// The side of a new region box in L tiles.
    RegionSize {
        width: u32,
        height: u32,
    },
    /// Arm the re-placement of the named region box.
    MoveRegion(String),
    /// Select the named region box (its face's margin is drawn).
    SelectRegion(String),
    /// Set the named region box's side in L tiles.
    ResizeRegion {
        name: String,
        width: u32,
        height: u32,
    },
    /// Delete the named region box.
    DeleteRegion(String),
    /// Print the command that opens the named box's commit (VIEW-069).
    OpenRegion(String),
    /// Swap A and B endpoints of the pinned ruler transect (VIEW-076).
    SwapRuler,
    /// Clear the active ruler anchor/measurement (VIEW-076).
    ClearRuler,
}

/// The keys of the editor mode as the help window lists them: the key, what it does.
pub const HELP_KEYS: [(&str, &str); 18] = [
    ("Tab", "editor on / off"),
    ("1", "select"),
    ("2  3  4", "place a crater / hill / plateau"),
    ("5  6", "draw a ridge / a channel"),
    ("7", "draw a region"),
    ("8  9  0  -", "brush: raise / lower / smooth / flatten"),
    ("drag (brush)", "paint a stroke; the release puts it, one undo takes it back"),
    ("click", "add a point (line and region tools), else select"),
    ("Enter", "finish the line or region"),
    ("Backspace", "take the last point back (drawing)"),
    ("Esc", "cancel the drag, the draft, the tool, the selection"),
    ("drag a handle", "move a point, the radius, the height"),
    ("Shift + click", "add a point on the outline of the selected line or region"),
    ("Ctrl + click", "remove the point under the cursor"),
    ("Delete", "delete the selected edit"),
    ("Ctrl+Z  Ctrl+Y", "undo / redo"),
    ("right mouse, WASD QE", "look around and fly"),
    ("F1  ?", "this list"),
];

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
    /// The list of keys is shown.
    pub help: bool,
    /// The text of the brush fields while they are edited, by field name.
    brush_text: BTreeMap<&'static str, String>,
    /// Draft label for creating a new site bookmark.
    pub bookmark_name: String,
    /// Draft name of a new region box (VIEW-077).
    pub region_name: String,
    /// Draft coordinate jump string (`LAT,LON[,ALT]`).
    pub jump_text: String,
    /// Active inline rename buffer for an existing bookmark `(id, text)`.
    bookmark_rename: Option<(u64, String)>,
    /// Normalized fraction `0..=1` hovered on the ruler transect chart (VIEW-076).
    pub ruler_scrub: Option<f64>,
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
            help: false,
            brush_text: BTreeMap::new(),
            bookmark_name: String::new(),
            region_name: String::new(),
            jump_text: String::new(),
            bookmark_rename: None,
            ruler_scrub: None,
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

/// One saved bookmark row shown in the Sites tab (VIEW-075).
#[derive(Clone, Debug, PartialEq)]
pub struct BookmarkRow {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub site_arg: String,
    pub distance_m: f64,
}

/// One scanned F9 repro entry shown in the Sites tab (VIEW-074, VIEW-075).
#[derive(Clone, Debug, PartialEq)]
pub struct ReproRow {
    pub index: usize,
    pub stem: String,
    pub summary: String,
    pub compatibility: planet_editor::repro_shelf::ReproCompatibility,
    pub has_path: bool,
    pub path_samples: Option<usize>,
    pub has_png: bool,
    pub replay_cmd: String,
}

/// Plain data backing the Sites & Repros tab (`Tab::Sites`, VIEW-075).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SitesView {
    pub readout: Option<planet_editor::bookmarks::CameraReadout>,
    pub bookmarks: Vec<BookmarkRow>,
    pub bookmarks_where: String,
    pub repros: Vec<ReproRow>,
    pub repros_dir: String,
    pub broken_repros: usize,
}

/// What the panels show.
pub struct PanelData<'a> {
    pub layout: &'a Layout,
    pub tool: &'a Tool,
    /// The brush's numbers while a brush tool is on.
    pub brush: Option<planet_editor::session::BrushSettings>,
    pub selected: Option<ItemId>,
    /// The edits the camera shows, nearest first.
    pub near: &'a [Row],
    /// The search results (planet-wide), nearest first; used when the query or a kind filter is set.
    pub found: &'a [Row],
    pub groups: &'a [LayerGroup],
    pub chain: &'a [Row],
    /// The problems list (VIEW-047), the steepest first.
    pub problems: &'a [planet_editor::problems::Problem],
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
    /// The rows of the Keys tab (VIEW-057): the keys the window accepts now, by group, with their state.
    pub keys: &'a [planet_frame::keytable::KeyRow],
    /// The item store's count and load time against the revisit trigger of ADR 0066 point 6 (AUTH-006); `None` where there is no editor.
    pub store: Option<&'a planet_editor::store_watch::StoreWatch>,
    /// Live camera readout, site bookmarks, and F9 repro captures for the Sites tab (VIEW-075).
    pub sites: &'a SitesView,
    /// The region boxes of the regions file (VIEW-077).
    pub regions: &'a crate::regionedit::RegionsView,
    /// Live cursor coordinates, elevation, and local slope readout (VIEW-076).
    pub cursor: Option<planet_editor::ruler::CursorReadout>,
    /// Active two-point ruler transect (or rubber-band preview while anchored, VIEW-076).
    pub ruler: Option<&'a planet_editor::ruler::TransectProfile>,
    /// True while `ruler` is a live rubber-band preview (point B not pinned yet).
    pub ruler_rubber: bool,
}

fn kind_color(kind: &str) -> egui::Color32 {
    match kind {
        "crater" | "hill" | "plateau" => AMBER,
        "ridge" => ORANGE,
        "channel" => TEAL,
        "region" => GREEN,
        "brush" => BRUSH_COLOUR,
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
        Tool::Line(k) => {
            let n = 2 + STAMP_KINDS.len() + LINE_KINDS.iter().position(|s| s == k).unwrap_or(0);
            format!("{} (draw)  [{n}]", planet_editor::params::kind_title(k))
        }
        Tool::Region => format!("Region (draw)  [{}]", 2 + STAMP_KINDS.len() + LINE_KINDS.len()),
        Tool::Brush(k) => {
            let at = BrushKind::ALL.iter().position(|b| b == k).unwrap_or(0);
            let key = ["8", "9", "0", "-"][at];
            format!("{} brush  [{key}]", planet_editor::params::kind_title(k.name()))
        }
        Tool::Ruler => "Ruler".to_string(),
    }
}

/// The text of a row: the label, the kind and, when the row has one, its detail (the parts of a split line, the strokes of a brush, the
/// points of a line or a region, VIEW-016).
pub fn row_text(row: &Row) -> String {
    let kind = planet_editor::params::kind_title(&row.kind);
    match row.detail() {
        d if d.is_empty() => format!("{}  ({kind})", row.label),
        d => format!("{}  ({kind}) · {d}", row.label),
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
    let detail = row.detail();
    let detail = if detail.is_empty() { String::new() } else { format!(" · {detail}") };
    ui.small(format!("{} · reach {}{detail}", planet_editor::params::kind_title(&row.kind), format_metres(row.reach_m)));
    if row.steep {
        let r = ui.colored_label(AMBER, format!("⚠ {}", planet_editor::slope::WARNING));
        st.marks.push((format!("steep:{}", row.label), r.rect));
    }
}

fn rail(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    let mut tools = vec![Tool::Select];
    tools.extend(STAMP_KINDS.iter().map(|k| Tool::Stamp((*k).to_string())));
    tools.extend(LINE_KINDS.iter().map(|k| Tool::Line((*k).to_string())));
    tools.push(Tool::Region);
    tools.extend(BrushKind::ALL.iter().map(|k| Tool::Brush(*k)));
    tools.push(Tool::Ruler);
    for t in tools {
        let on = d.tool == &t;
        let r = ui.add(egui::Button::new(tool_label(&t)).selected(on));
        st.marks.push((format!("tool:{}", tool_label(&t)), r.rect));
        if r.clicked() {
            ev.push(Event::Tool(t));
        }
    }
}

/// How Up and Down step a value field (VIEW-041).
#[derive(Clone, Copy, Debug)]
pub struct FieldStep {
    pub value: f64,
    pub step: f64,
    pub unit: planet_editor::params::Unit,
    pub range: Option<(f64, f64)>,
}

/// A numeric field of the inspector or the brush (VIEW-041): the text is typed and committed on Enter or when the focus leaves, Enter keeps the
/// focus in the field, and Up and Down (Shift: ten times) step the value and commit it at once. Returns the response and the text to commit now.
pub fn value_field(ui: &mut egui::Ui, buf: &mut String, current: &str, step: FieldStep) -> (egui::Response, Option<String>) {
    use planet_editor::params::{format_value, parse_value, stepped};
    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0));
    let mut commit = None;
    if r.has_focus() {
        // Shift first: egui matches Shift logically, so the plain key would also take the shifted press.
        let key = |ui: &mut egui::Ui, k: egui::Key| {
            ui.input_mut(|i| {
                if i.consume_key(egui::Modifiers::SHIFT, k) {
                    Some(true)
                } else if i.consume_key(egui::Modifiers::NONE, k) {
                    Some(false)
                } else {
                    None
                }
            })
        };
        let pressed = key(ui, egui::Key::ArrowUp).map(|s| (true, s)).or_else(|| key(ui, egui::Key::ArrowDown).map(|s| (false, s)));
        if let Some((up, shift)) = pressed {
            let base = if buf.trim() == current { step.value } else { parse_value(buf, step.unit).unwrap_or(step.value) };
            let text = format_value(stepped(base, step.step, step.range, up, shift), step.unit);
            *buf = text.clone();
            commit = Some(text);
        }
    }
    if r.lost_focus() {
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if commit.is_none() && buf.trim() != current {
            commit = Some(buf.trim().to_string());
        }
        if enter {
            // Enter keeps the focus in the field: the next number can be typed or stepped at once.
            r.request_focus();
        }
    }
    (r, commit)
}

/// The brush's numbers: the radius and the strength in metres (typed like any value) and, for a flatten, the height it levels towards
/// ("auto": the terrain height under the start of the stroke).
fn brush_section(ui: &mut egui::Ui, st: &mut PanelState, b: &planet_editor::session::BrushSettings, flatten: bool, ev: &mut Vec<Event>) {
    ui.weak("Brush");
    type Field<'a> = (&'static str, &'a str, String, fn(String) -> Event);
    let fields: [Field; 3] = [
        ("brush-radius", "Radius", format_metres(b.radius_m), Event::BrushRadius),
        ("brush-strength", "Strength", format_metres(b.strength_m), Event::BrushStrength),
        ("brush-target", "Flatten height", b.target_m.map_or("auto".to_string(), format_metres), Event::BrushTarget),
    ];
    egui::Grid::new("brush-fields").num_columns(2).show(ui, |ui| {
        for (name, label, current, make) in fields {
            if name == "brush-target" && !flatten {
                continue;
            }
            ui.label(label);
            let buf = st.brush_text.entry(name).or_insert_with(|| current.clone());
            // A brush number is metres; the field steps by 1 % of it, at least a metre.
            let value = planet_editor::params::parse_value(&current, planet_editor::params::Unit::Metres).unwrap_or(0.0);
            let range = match name {
                "brush-radius" => Some(planet_editor::brush::RADIUS_RANGE_M),
                "brush-strength" => Some(planet_editor::brush::STRENGTH_RANGE_M),
                _ => None,
            };
            let step = FieldStep {
                value,
                step: if name == "brush-strength" { 0.1 } else { 1.0 },
                unit: planet_editor::params::Unit::Metres,
                range,
            };
            let (r, commit) = value_field(ui, buf, &current, step);
            st.marks.push((name.to_string(), r.rect));
            if let Some(text) = commit {
                ev.push(make(text));
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if r.lost_focus() && !enter {
                st.brush_text.remove(name);
            } else if !r.has_focus() && !r.lost_focus() {
                *buf = current;
            }
            ui.end_row();
        }
    });
    ui.small("Metres; the strength is the largest height change at the stroke's centre line.");
    ui.separator();
}

fn left_panel(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    ui.horizontal_wrapped(|ui| {
        for tab in &d.layout.tabs {
            let title = if *tab == Tab::Problems && !d.problems.is_empty() {
                format!("{} ({})", tab.title(), d.problems.len())
            } else {
                tab.title().to_string()
            };
            let r = ui.selectable_value(&mut st.tab, *tab, title);
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
        Tab::Problems => problems_tab(ui, st, d, ev),
        Tab::Sites => sites_tab(ui, st, d, ev),
        Tab::Keys => keys_tab(ui, st, d),
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

/// The Sites tab (VIEW-075): live camera coordinates, `LAT,LON[,ALT]` jump box, saved site bookmarks, and F9 repro shelf.
fn sites_tab(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    use planet_editor::repro_shelf::ReproCompatibility;

    ui.strong("Camera");
    if let Some(readout) = &d.sites.readout {
        let r = ui.label(readout.summary());
        st.marks.push(("site:readout".to_string(), r.rect));
        ui.monospace(readout.site_flag());
    } else {
        let r = ui.weak("No camera pose yet.");
        st.marks.push(("site:readout".to_string(), r.rect));
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let jr = ui.add(egui::TextEdit::singleline(&mut st.jump_text).hint_text("LAT, LON[, ALT]").desired_width(180.0));
        st.marks.push(("site:jump-input".to_string(), jr.rect));
        let enter = jr.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let go = ui.button("Go");
        st.marks.push(("site:jump-go".to_string(), go.rect));
        if enter || go.clicked() {
            ev.push(Event::JumpCoords(st.jump_text.trim().to_string()));
        }
    });

    ui.separator();
    ui.strong("Bookmarks");
    if !d.sites.bookmarks_where.is_empty() {
        ui.label(egui::RichText::new(&d.sites.bookmarks_where).small().weak());
    }
    ui.horizontal(|ui| {
        let nr = ui.add(egui::TextEdit::singleline(&mut st.bookmark_name).hint_text("Site name (optional)").desired_width(165.0));
        st.marks.push(("site:new-name".to_string(), nr.rect));
        let enter = nr.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let save = ui.button("Save view");
        st.marks.push(("site:save".to_string(), save.rect));
        if enter || save.clicked() {
            ev.push(Event::SaveBookmark { name: std::mem::take(&mut st.bookmark_name) });
        }
    });

    if d.sites.bookmarks.is_empty() {
        ui.weak("No saved sites yet.");
    }
    for b in &d.sites.bookmarks {
        ui.push_id(b.id, |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let current =
                        st.bookmark_rename.as_ref().filter(|(id, _)| *id == b.id).map(|(_, s)| s.clone()).unwrap_or_else(|| b.name.clone());
                    let mut edited = current;
                    let r = ui.add(egui::TextEdit::singleline(&mut edited).desired_width(145.0));
                    st.marks.push((format!("bookmark:name:{}", b.id), r.rect));
                    if r.has_focus() || r.lost_focus() {
                        st.bookmark_rename = Some((b.id, edited.clone()));
                    }
                    if r.lost_focus() {
                        let trimmed = edited.trim().to_string();
                        if !trimmed.is_empty() && trimmed != b.name {
                            ev.push(Event::RenameBookmark { id: b.id, name: trimmed });
                        }
                        st.bookmark_rename = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(format_metres(b.distance_m));
                    });
                });
                ui.label(egui::RichText::new(&b.summary).small().weak());
                ui.horizontal(|ui| {
                    let jump = ui.button("Jump");
                    st.marks.push((format!("bookmark:jump:{}", b.id), jump.rect));
                    if jump.clicked() {
                        ev.push(Event::JumpBookmark(b.id));
                    }
                    let del = ui.add(egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::from_rgb(242, 179, 179))));
                    st.marks.push((format!("bookmark:delete:{}", b.id), del.rect));
                    if del.clicked() {
                        ev.push(Event::DeleteBookmark(b.id));
                    }
                    ui.monospace(format!("--site {}", b.site_arg));
                });
            });
        });
    }

    ui.separator();
    regions_section(ui, st, d, ev);

    ui.separator();
    ui.horizontal(|ui| {
        ui.strong("F9 Repro Shelf");
        let cap = ui.button("Capture F9");
        st.marks.push(("repro:capture".to_string(), cap.rect));
        if cap.clicked() {
            ev.push(Event::CaptureRepro);
        }
        let ref_btn = ui.button("Refresh");
        st.marks.push(("repro:refresh".to_string(), ref_btn.rect));
        if ref_btn.clicked() {
            ev.push(Event::RefreshRepros);
        }
    });
    if !d.sites.repros_dir.is_empty() {
        ui.label(egui::RichText::new(&d.sites.repros_dir).small().weak());
    }
    if d.sites.broken_repros > 0 {
        ui.colored_label(ORANGE, format!("⚠ {} unreadable .bundle file(s) skipped", d.sites.broken_repros));
    }
    if d.sites.repros.is_empty() {
        ui.weak("No F9 repro bundles found yet.");
    }
    for r in &d.sites.repros {
        ui.push_id(&r.stem, |ui| {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&r.stem);
                    let (badge, color) = match r.compatibility {
                        ReproCompatibility::MatchesWorld => ("match", GREEN),
                        ReproCompatibility::DifferentDefinition => ("diff world", AMBER),
                        ReproCompatibility::DifferentSeed => ("seed !=", ORANGE),
                    };
                    ui.colored_label(color, badge);
                    if let Some(samples) = r.path_samples {
                        ui.weak(format!("path ({samples})"));
                    } else if r.has_path {
                        ui.weak("path");
                    }
                    if r.has_png {
                        ui.weak("png");
                    }
                });
                ui.label(egui::RichText::new(&r.summary).small().weak());
                ui.horizontal(|ui| {
                    let jump = ui.button("Jump");
                    st.marks.push((format!("repro:jump:{}", r.stem), jump.rect));
                    if jump.clicked() {
                        ev.push(Event::JumpRepro(r.index));
                    }
                    let bm = ui.button("Bookmark");
                    st.marks.push((format!("repro:bookmark:{}", r.stem), bm.rect));
                    if bm.clicked() {
                        ev.push(Event::BookmarkRepro(r.index));
                    }
                });
                ui.label(egui::RichText::new(&r.replay_cmd).monospace().small());
            });
        });
    }
}

/// The Keys tab (VIEW-057): the keys the window accepts, by group, each with what it does and the state it is in.
fn keys_tab(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData) {
    if d.keys.is_empty() {
        ui.weak("No keys listed.");
        return;
    }
    for group in planet_frame::keytable::Group::ALL {
        let rows: Vec<_> = d.keys.iter().filter(|r| r.group == group).collect();
        if rows.is_empty() {
            continue;
        }
        ui.strong(group.title());
        for row in rows {
            ui.horizontal_wrapped(|ui| {
                let key = ui.monospace(row.label);
                st.marks.push((format!("key:{}", row.label), key.rect));
                if let Some(state) = &row.state {
                    ui.label(egui::RichText::new(format!("[{state}]")).weak());
                }
            });
            ui.label(egui::RichText::new(row.help).small().weak());
        }
        ui.separator();
    }
}

/// The problems tab (VIEW-047): each edit the engine cannot draw fully or that costs more than it should, what is wrong, and what to do:
/// fly there, reduce the strengths of a brush, merge its stack of strokes into one, delete it.
fn problems_tab(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    if d.problems.is_empty() {
        ui.weak("No problem: every edit can be drawn fully.");
        return;
    }
    ui.label(egui::RichText::new("Edits the engine cannot draw fully, or that cost more than they should").small().weak());
    for p in d.problems {
        ui.group(|ui| {
            let row = &p.row;
            ui.horizontal(|ui| {
                ui.colored_label(kind_color(&row.kind), "●");
                let r = ui.selectable_label(d.selected == Some(row.id), &row.label);
                st.marks.push((format!("problem:{}", row.label), r.rect));
                if r.clicked() {
                    ev.push(Event::Select(row.id));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format_metres(row.distance_m));
                });
            });
            for issue in &p.issues {
                let r = ui.colored_label(AMBER, format!("⚠ {}", issue.text()));
                st.marks.push((format!("issue:{}", row.label), r.rect));
            }
            ui.horizontal_wrapped(|ui| {
                let mut button = |ui: &mut egui::Ui, name: &str, text: &str, enabled: bool, hover: &str, event: Event| {
                    let r = ui.add_enabled(enabled, egui::Button::new(text)).on_hover_text(hover);
                    st.marks.push((format!("{name}:{}", row.label), r.rect));
                    if r.clicked() {
                        ev.push(event);
                    }
                };
                button(ui, "fly", "Fly to", true, "Put the camera where it sees this edit", Event::FlyTo(row.id));
                button(
                    ui,
                    "reduce",
                    "Reduce",
                    p.can_reduce,
                    "Cut the strengths of its raises and lowers so that it can be drawn fully",
                    Event::Reduce(row.id),
                );
                button(
                    ui,
                    "merge",
                    "Merge strokes",
                    p.can_merge,
                    "Replace the stack of overlapping strokes of one kind by one stroke of the summed strength along the first one's path",
                    Event::Merge(row.id),
                );
                button(ui, "delete", "Delete", true, "Delete this edit (Ctrl+Z takes it back)", Event::DeleteItem(row.id));
            });
        });
    }
}

fn row_button_inline(ui: &mut egui::Ui, st: &mut PanelState, ev: &mut Vec<Event>, row: &Row, selected: bool) {
    let r = ui.selectable_label(selected, row_text(row));
    st.marks.push((format!("layer-row:{}", row.label), r.rect));
    if r.double_clicked() {
        ev.push(Event::FlyTo(row.id));
    } else if r.clicked() {
        ev.push(Event::Select(row.id));
    }
}

/// The rows of a region: its surface type (a combo box over the table) and its mask (does it remove the edits of another layer inside its
/// outline, and which layer; [`planet_editor::params::MASK_CUT_LABEL`]). Keeping them only inside (the mask mode 1) is not offered: the world
/// definition refuses it for now.
fn region_rows(ui: &mut egui::Ui, st: &mut PanelState, r: &planet_editor::params::RegionRows, ev: &mut Vec<Event>) {
    egui::Grid::new("region-rows").num_columns(2).show(ui, |ui| {
        ui.label("Surface type");
        let current = r.surface_names.get(r.surface_type).copied().unwrap_or("?");
        let combo = egui::ComboBox::from_id_salt("surface-type").selected_text(current).show_ui(ui, |ui| {
            for (index, name) in r.surface_names.iter().enumerate() {
                let resp = ui.selectable_label(index == r.surface_type, *name);
                st.marks.push((format!("surface:{name}"), resp.rect));
                if resp.clicked() {
                    ev.push(Event::SurfaceType(index));
                }
            }
        });
        st.marks.push(("surface-type".to_string(), combo.response.rect));
        ui.end_row();
        ui.label("Mask");
        let mut inside = r.clips_inside;
        let check = ui.checkbox(&mut inside, planet_editor::params::MASK_CUT_LABEL).on_hover_text(planet_editor::params::MASK_CUT_HINT);
        st.marks.push(("mask-inside".to_string(), check.rect));
        if check.changed() {
            // Switching it on picks the layer it had, else the first other layer; with none to choose the session says so.
            let layer = r.mask_layer.clone().or_else(|| r.layers.first().cloned());
            ev.push(Event::Mask { inside, layer });
        }
        ui.end_row();
        if r.clips_inside {
            ui.label("Layer cut");
            let shown = r.mask_layer.clone().unwrap_or_default();
            let combo = egui::ComboBox::from_id_salt("mask-layer").selected_text(shown.clone()).show_ui(ui, |ui| {
                for layer in &r.layers {
                    let resp = ui.selectable_label(*layer == shown, layer);
                    st.marks.push((format!("layer:{layer}"), resp.rect));
                    if resp.clicked() {
                        ev.push(Event::Mask { inside: true, layer: Some(layer.clone()) });
                    }
                }
            });
            st.marks.push(("mask-layer".to_string(), combo.response.rect));
            ui.end_row();
        }
    });
}

fn draw_transect_plot(ui: &mut egui::Ui, st: &mut PanelState, p: &planet_editor::ruler::TransectProfile) {
    let w = ui.available_width().clamp(160.0, 290.0);
    let total_h = 148.0_f32;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, total_h), egui::Sense::hover());
    st.marks.push(("ruler-plot".to_string(), rect));
    st.ruler_scrub = resp.hover_pos().map(|pos| f64::from((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0));

    let elev_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 84.0));
    let slope_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + 94.0), rect.max);
    let painter = ui.painter();
    let bg = egui::Color32::from_gray(24);
    let border = egui::Stroke::new(1.0, egui::Color32::from_gray(55));
    painter.rect_filled(elev_rect, 2.0, bg);
    painter.rect_stroke(elev_rect, 2.0, border, egui::StrokeKind::Inside);
    painter.rect_filled(slope_rect, 2.0, bg);
    painter.rect_stroke(slope_rect, 2.0, border, egui::StrokeKind::Inside);

    let pad = ((p.max_height_m - p.min_height_m) * 0.08).max(1.0);
    let (h_min, h_max) = (p.min_height_m - pad, p.max_height_m + pad);
    let h_span = (h_max - h_min).max(1.0);
    let x_of = |f: f64| rect.left() + (f.clamp(0.0, 1.0) as f32) * rect.width();
    let y_elev = |h: f64| elev_rect.bottom() - (((h - h_min) / h_span).clamp(0.0, 1.0) as f32) * elev_rect.height();

    if h_min <= 0.0 && h_max >= 0.0 {
        let y0 = y_elev(0.0);
        painter.hline(elev_rect.x_range(), y0, egui::Stroke::new(1.0, egui::Color32::from_gray(80)));
        painter.text(
            egui::pos2(elev_rect.right() - 4.0, y0 - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            "0 m",
            egui::FontId::monospace(9.5),
            egui::Color32::from_gray(145),
        );
    }

    let slope_top = p.max_slope_deg.max(60.0).max(p.slope_limit_deg + 10.0);
    let y_slope = |deg: f64| slope_rect.bottom() - ((deg / slope_top).clamp(0.0, 1.0) as f32) * slope_rect.height();
    let y_lim = y_slope(p.slope_limit_deg);
    let limit_stroke = egui::Stroke::new(1.0, AMBER.gamma_multiply(0.8));
    let mut x = slope_rect.left();
    while x < slope_rect.right() {
        let x2 = (x + 5.0).min(slope_rect.right());
        painter.line_segment([egui::pos2(x, y_lim), egui::pos2(x2, y_lim)], limit_stroke);
        x += 9.0;
    }
    painter.text(
        egui::pos2(slope_rect.right() - 4.0, (y_lim - 2.0).max(slope_rect.top() + 10.0)),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0}° ADR 0091", p.slope_limit_deg),
        egui::FontId::monospace(9.5),
        AMBER,
    );

    for (i, w_pair) in p.samples.windows(2).enumerate() {
        let steep = libm::fabs(p.interval_slope_deg(i)) > p.slope_limit_deg;
        let col = if steep { AMBER } else { TEAL };
        let (x0, x1) = (x_of(w_pair[0].fraction), x_of(w_pair[1].fraction));
        painter.line_segment(
            [egui::pos2(x0, y_elev(w_pair[0].height_m)), egui::pos2(x1, y_elev(w_pair[1].height_m))],
            egui::Stroke::new(1.8, col),
        );
        painter.line_segment(
            [egui::pos2(x0, y_slope(w_pair[0].slope_deg)), egui::pos2(x1, y_slope(w_pair[1].slope_deg))],
            egui::Stroke::new(1.5, col),
        );
    }

    painter.text(
        elev_rect.left_top() + egui::vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0} m", p.max_height_m),
        egui::FontId::monospace(9.5),
        LIGHT,
    );
    painter.text(
        elev_rect.left_bottom() + egui::vec2(4.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{:.0} m", p.min_height_m),
        egui::FontId::monospace(9.5),
        LIGHT,
    );

    if let Some(f) = st.ruler_scrub {
        let xs = x_of(f);
        let cross = egui::Stroke::new(1.0, LIGHT.gamma_multiply(0.65));
        painter.vline(xs, elev_rect.y_range(), cross);
        painter.vline(xs, slope_rect.y_range(), cross);
        if let Some(s) = p.sample_at_fraction(f) {
            ui.small(format!("at {} · h {:.1} m · slope {:.1}°", format_metres(s.distance_m), s.height_m, s.slope_deg));
        }
    }
}

fn ruler_section(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    ui.strong("Ruler & Transect");
    if let Some(c) = &d.cursor {
        let text = c.summary();
        if c.over_limit {
            ui.colored_label(AMBER, text);
        } else {
            ui.small(text);
        }
    }
    let Some(p) = d.ruler else {
        ui.small("Click point A on the planet, then point B to sample a surface elevation and slope transect.");
        return;
    };
    egui::Grid::new("ruler-metrics").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
        ui.weak("Surface arc");
        ui.label(format_metres(p.arc_distance_m));
        ui.end_row();
        ui.weak("3D chord");
        ui.label(format_metres(p.chord_distance_m));
        ui.end_row();
        ui.weak("Elevation Δh");
        ui.label(format!("{:+.1} m ({:.0} -> {:.0} m)", p.delta_height_m, p.start_height_m, p.end_height_m));
        ui.end_row();
        ui.weak("Net slope");
        ui.label(format!("{:+.1}° ({:+.1}%)", p.net_slope_deg, p.net_grade * 100.0));
        ui.end_row();
        if !d.ruler_rubber {
            ui.weak("Elev range");
            ui.label(format!("{:.0} to {:.0} m", p.min_height_m, p.max_height_m));
            ui.end_row();
            ui.weak("Max slope");
            let max_text = format!("{:.1}° ({:.0}% > {:.0}°)", p.max_slope_deg, p.steep_fraction * 100.0, p.slope_limit_deg);
            let r = if p.max_slope_deg > p.slope_limit_deg { ui.colored_label(AMBER, max_text) } else { ui.label(max_text) };
            st.marks.push(("ruler-steep".to_string(), r.rect));
            ui.end_row();
        }
    });
    if !d.ruler_rubber {
        ui.add_space(4.0);
        draw_transect_plot(ui, st, p);
    } else {
        st.ruler_scrub = None;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let swap = ui.add_enabled(!d.ruler_rubber, egui::Button::new("Swap A↔B"));
        st.marks.push(("ruler-swap".to_string(), swap.rect));
        if swap.clicked() {
            ev.push(Event::SwapRuler);
        }
        let clear = ui.button("Clear ruler");
        st.marks.push(("ruler-clear".to_string(), clear.rect));
        if clear.clicked() {
            ev.push(Event::ClearRuler);
        }
    });
}

fn inspector(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    if let (Some(b), Tool::Brush(kind)) = (&d.brush, d.tool) {
        brush_section(ui, st, b, *kind == BrushKind::Flatten, ev);
    }
    if d.tool == &Tool::Ruler || d.ruler.is_some() {
        ruler_section(ui, st, d, ev);
        if d.inspector.is_none() {
            return;
        }
        ui.separator();
    }
    let Some(i) = d.inspector else {
        ui.weak("Inspector");
        ui.label("Nothing selected. Click an edit on the planet or pick one from the list. Choose a stamp tool on the left and click the planet to place one.");
        return;
    };
    ui.weak(format!("{} · {}", i.kind, i.layer));
    if let Some(w) = &i.warning {
        let r = ui.colored_label(AMBER, format!("⚠ {w}"));
        st.marks.push(("steep-warning".to_string(), r.rect));
    }
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
            let (r, commit) = value_field(ui, buf, &p.text, FieldStep { value: p.value, step: p.step, unit: p.unit, range: p.range });
            st.marks.push((format!("param:{}", p.key), r.rect));
            if let Some(text) = commit {
                ev.push(Event::Param { key: p.key.clone(), text });
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if r.lost_focus() && !enter {
                st.typed.remove(&key);
            } else if !r.has_focus() && !r.lost_focus() {
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
    if let Some(r) = &i.region {
        ui.add_space(6.0);
        region_rows(ui, st, r, ev);
    }
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

/// The diagnostics line of the item store (AUTH-006): the largest body's item count and the last load time with their limits, empty without one.
pub fn store_line(d: &PanelData) -> String {
    d.store.map_or(String::new(), |w| w.summary())
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
        let r = ui.add(egui::Button::new("?").selected(st.help));
        st.marks.push(("help".to_string(), r.rect));
        if r.clicked() {
            ev.push(Event::ToggleHelp);
        }
        ui.separator();
        ui.label(if d.status.is_empty() { "Ready" } else { d.status });
        if let Some(c) = &d.cursor {
            ui.separator();
            let text = c.summary();
            let r = if c.over_limit { ui.colored_label(AMBER, text) } else { ui.weak(text) };
            st.marks.push(("cursor-readout".to_string(), r.rect));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(counts_line(d));
            let store = store_line(d);
            if !store.is_empty() {
                ui.separator();
                ui.weak(store);
            }
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
    if st.help {
        let r = egui::Window::new("Editor keys").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
            egui::Grid::new("editor-keys").num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
                for (key, what) in HELP_KEYS {
                    ui.strong(key);
                    ui.label(what);
                    ui.end_row();
                }
            });
            if ui.button("Close  (F1)").clicked() {
                ev.push(Event::ToggleHelp);
            }
        });
        if let Some(r) = r {
            st.covered.push(r.response.rect);
        }
    }
    if l.inspector_side == Side::Right {
        let r = egui::Panel::right("editor-inspector")
            .exact_size(l.inspector_width)
            .resizable(false)
            .show(ui, |ui| inspector(ui, st, d, &mut ev));
        st.covered.push(r.response.rect);
    }
    ev
}

/// Draw the line or region being drawn: the path through its points, the points, and a thin line to the cursor where the next goes.
pub fn draw_draft(ui: &egui::Ui, draft: &planet_editor::overlay::DraftOverlay, to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    if draft.points.is_empty() {
        return;
    }
    let painter = ui.painter();
    let stroke = egui::Stroke::new(2.5, LIGHT);
    for w in draft.path.windows(2) {
        if let (Some(a), Some(b)) = (w[0], w[1]) {
            painter.line_segment([to_points(a), to_points(b)], stroke);
        }
    }
    if let (Some(Some(last)), Some(cursor)) = (draft.points.last(), draft.rubber) {
        painter.line_segment([to_points(*last), to_points(cursor)], egui::Stroke::new(1.0, LIGHT.gamma_multiply(0.6)));
    }
    for p in draft.points.iter().flatten() {
        painter.circle(to_points(*p), 5.0, AMBER, egui::Stroke::new(1.5, egui::Color32::BLACK));
    }
}

/// Draw the brush tool: the stroke being painted and the ring of the radius under the cursor.
pub fn draw_brush(ui: &egui::Ui, brush: &planet_editor::overlay::BrushOverlay, to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    let painter = ui.painter();
    for w in brush.path.windows(2) {
        if let (Some(a), Some(b)) = (w[0], w[1]) {
            painter.line_segment([to_points(a), to_points(b)], egui::Stroke::new(2.5, AMBER));
        }
    }
    let n = brush.ring.len();
    for k in 0..n {
        if let (Some(a), Some(b)) = (brush.ring[k], brush.ring[(k + 1) % n]) {
            painter.line_segment([to_points(a), to_points(b)], egui::Stroke::new(1.5, LIGHT));
        }
    }
}

/// Draw the 3D viewport ruler overlay: surface transect segments (amber above `ADR 0091` 45°), A/B endpoints,
/// callout label, and chart hover scrub marker (`VIEW-076`).
pub fn draw_ruler(ui: &egui::Ui, r: &planet_editor::ruler::RulerOverlay, to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    let painter = ui.painter();
    let width = if r.is_rubber_band { 1.8 } else { 2.6 };
    let alpha = if r.is_rubber_band { 0.7 } else { 1.0 };
    for seg in &r.segments {
        let base = if seg.over_limit { AMBER } else { TEAL };
        painter.line_segment([to_points(seg.a), to_points(seg.b)], egui::Stroke::new(width, base.gamma_multiply(alpha)));
    }
    if let Some(a) = r.start.map(&to_points) {
        painter.circle(a, 5.5, TEAL, egui::Stroke::new(1.5, egui::Color32::BLACK));
        painter.text(a + egui::vec2(0.0, -10.0), egui::Align2::CENTER_BOTTOM, "A", egui::FontId::proportional(12.0), LIGHT);
    }
    if let Some(b) = r.end.map(&to_points) {
        painter.circle(b, 5.5, AMBER, egui::Stroke::new(1.5, egui::Color32::BLACK));
        painter.text(b + egui::vec2(0.0, -10.0), egui::Align2::CENTER_BOTTOM, "B", egui::FontId::proportional(12.0), LIGHT);
    }
    if let (Some(mid), Some(label)) = (r.midpoint.map(&to_points), r.label.as_deref()) {
        let at = mid + egui::vec2(0.0, -16.0);
        let text = painter.text(at, egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(12.5), LIGHT);
        painter.rect_filled(text.expand(3.0), 3.0, egui::Color32::from_black_alpha(190));
        painter.text(at, egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(12.5), LIGHT);
    }
    if let Some(scrub) = r.scrub.map(&to_points) {
        painter.circle(scrub, 5.0, LIGHT, egui::Stroke::new(1.5, egui::Color32::BLACK));
    }
}

/// The level-1 preview over the planet (VIEW-067): the trunk rivers as lines coloured by the watershed they drain to (the lines the edit changed thicker and brighter, the big rivers thicker),
/// and a label marking it as a preview that is not committed.
pub fn draw_preview_overlay(ui: &egui::Ui, p: &planet_editor::preview_overlay::PreviewOverlay, to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    let painter = ui.painter();
    let heavy = p.lines.len() / 10;
    for (k, l) in p.lines.iter().enumerate() {
        let hue = planet_editor::preview_overlay::watershed_hue(l.watershed) / 360.0;
        let colour: egui::Color32 = egui::ecolor::Hsva::new(hue, 0.65, 0.95, 1.0).into();
        let width = if k < heavy { 2.4 } else { 1.1 };
        let (width, strength) = if l.changed { (width + 0.8, 1.0) } else { (width, 0.7) };
        painter.line_segment([to_points(l.a), to_points(l.b)], egui::Stroke::new(width, colour.gamma_multiply(strength)));
    }
    let at = ui.max_rect().left_top() + egui::vec2(14.0, 52.0);
    let text = painter.text(at, egui::Align2::LEFT_TOP, &p.label, egui::FontId::proportional(13.0), LIGHT);
    painter.rect_filled(text.expand(3.0), 3.0, egui::Color32::from_black_alpha(190));
    painter.text(at, egui::Align2::LEFT_TOP, &p.label, egui::FontId::proportional(13.0), LIGHT);
}

/// The Regions section of the Sites tab (VIEW-077): the boxes of the regions file with their state, the placement, resize, move, delete and open.
fn regions_section(ui: &mut egui::Ui, st: &mut PanelState, d: &PanelData, ev: &mut Vec<Event>) {
    use planet_cache::regions_file::{RegionState, MAX_TILES, MIN_TILES};
    let r = d.regions;
    ui.strong("Regions");
    ui.label(egui::RichText::new(&r.where_label).small().weak());
    if !r.has_file {
        return;
    }
    ui.horizontal(|ui| {
        let nr = ui.add(egui::TextEdit::singleline(&mut st.region_name).hint_text("Box name (optional)").desired_width(120.0));
        st.marks.push(("region:new-name".to_string(), nr.rect));
        let place = ui.button("Place");
        st.marks.push(("region:place".to_string(), place.rect));
        if place.clicked() {
            ev.push(Event::PlaceRegion { name: std::mem::take(&mut st.region_name) });
        }
    });
    ui.horizontal(|ui| {
        ui.label("new box");
        for (axis, value) in [("w", r.size.0), ("h", r.size.1)] {
            let less = ui.small_button(format!("{axis}-"));
            st.marks.push((format!("region:size:{axis}-"), less.rect));
            ui.monospace(format!("{axis} {value}"));
            let more = ui.small_button(format!("{axis}+"));
            st.marks.push((format!("region:size:{axis}+"), more.rect));
            let step = if less.clicked() {
                -1
            } else if more.clicked() {
                1
            } else {
                0
            };
            if step != 0 {
                let v = (value as i64 + step).clamp(i64::from(MIN_TILES), i64::from(MAX_TILES)) as u32;
                if v == value {
                    continue;
                }
                let (width, height) = if axis == "w" { (v, r.size.1) } else { (r.size.0, v) };
                ev.push(Event::RegionSize { width, height });
            }
        }
        ui.weak("L tiles");
    });
    if let Some(p) = &r.placing {
        ui.colored_label(AMBER, p);
    }
    if r.rows.is_empty() {
        ui.weak("No region boxes yet.");
    }
    for row in &r.rows {
        ui.push_id(&row.name, |ui| {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    let name = ui.selectable_label(row.selected, egui::RichText::new(&row.name).strong());
                    st.marks.push((format!("region:select:{}", row.name), name.rect));
                    if name.clicked() {
                        ev.push(Event::SelectRegion(row.name.clone()));
                    }
                    let (badge, colour) = match row.state {
                        RegionState::Ready => ("ready", GREEN),
                        RegionState::Stale => ("stale", AMBER),
                        RegionState::Unbaked => ("unbaked", LIGHT),
                    };
                    let b = ui.colored_label(colour, badge);
                    st.marks.push((format!("region:state:{}", row.name), b.rect));
                    ui.weak(format!("face {}  {}x{}  {}", row.face, row.width, row.height, row.tiles));
                });
                ui.horizontal_wrapped(|ui| {
                    for (label, w, h) in [("w-", -1, 0), ("w+", 1, 0), ("h-", 0, -1), ("h+", 0, 1)] {
                        let b = ui.small_button(label);
                        st.marks.push((format!("region:{label}:{}", row.name), b.rect));
                        if b.clicked() {
                            ev.push(Event::ResizeRegion {
                                name: row.name.clone(),
                                width: (row.width as i64 + w).max(0) as u32,
                                height: (row.height as i64 + h).max(0) as u32,
                            });
                        }
                    }
                    let mv = ui.button("Move");
                    st.marks.push((format!("region:move:{}", row.name), mv.rect));
                    if mv.clicked() {
                        ev.push(Event::MoveRegion(row.name.clone()));
                    }
                    let del = ui.add(egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::from_rgb(242, 179, 179))));
                    st.marks.push((format!("region:delete:{}", row.name), del.rect));
                    if del.clicked() {
                        ev.push(Event::DeleteRegion(row.name.clone()));
                    }
                    if row.state == RegionState::Ready {
                        let open = ui.button("Open");
                        st.marks.push((format!("region:open:{}", row.name), open.rect));
                        if open.clicked() {
                            // The command goes to the clipboard, as the status says (VIEW-077).
                            if let Some(cmd) = &row.open_command {
                                ui.ctx().copy_text(cmd.clone());
                            }
                            ev.push(Event::OpenRegion(row.name.clone()));
                        }
                    }
                });
            });
        });
    }
    if let Some(cmd) = &r.open_command {
        ui.horizontal_wrapped(|ui| {
            let copy = ui.small_button("Copy");
            st.marks.push(("region:copy".to_string(), copy.rect));
            if copy.clicked() {
                ui.ctx().copy_text(cmd.clone());
            }
            ui.label(egui::RichText::new(cmd).monospace().small());
        });
    }
}

/// Draw the region boxes, the placement box and the face-edge margin on the ground (VIEW-077).
pub fn draw_regions(ui: &egui::Ui, lines: &[crate::regionedit::RegionLine], to_points: impl Fn([f64; 2]) -> egui::Pos2) {
    use crate::regionedit::LineKind;
    use planet_cache::regions_file::RegionState;
    let painter = ui.painter();
    for l in lines {
        let stroke = match l.kind {
            LineKind::Box { state, selected } => {
                let colour = match state {
                    RegionState::Ready => GREEN,
                    RegionState::Stale => AMBER,
                    RegionState::Unbaked => LIGHT,
                };
                egui::Stroke::new(if selected { 3.0 } else { 1.8 }, colour)
            }
            LineKind::Preview { ok } => {
                egui::Stroke::new(2.2, if ok { egui::Color32::from_rgb(110, 220, 240) } else { egui::Color32::from_rgb(240, 90, 90) })
            }
            LineKind::Margin => egui::Stroke::new(1.2, egui::Color32::from_gray(150).gamma_multiply(0.8)),
        };
        let n = l.points.len();
        for k in 0..n {
            if let (Some(a), Some(b)) = (l.points[k], l.points[(k + 1) % n]) {
                painter.line_segment([to_points(a), to_points(b)], stroke);
            }
        }
    }
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
        // A ring around a stamp is convex and filled when selected; a line or a region is a path (a region may be concave: no fill).
        let is_path = matches!(o.kind.as_str(), "ridge" | "channel" | "region");
        if pts.iter().all(Option::is_some) && o.selected && !is_path {
            let ring: Vec<egui::Pos2> = pts.iter().flatten().copied().collect();
            painter.add(egui::Shape::convex_polygon(ring, colour.gamma_multiply(0.18), stroke));
        } else {
            let segments = if o.closed { pts.len() } else { pts.len().saturating_sub(1) };
            for k in 0..segments {
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
                    Handle::Point(_) => {
                        painter.circle(p, 5.0, AMBER, egui::Stroke::new(1.5, egui::Color32::BLACK));
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
            Vec3::from_lat_lon(la, lo)
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
        /// The texts the panels put on the clipboard.
        copied: Vec<String>,
    }

    impl Rig {
        fn new(layout: &Layout) -> Rig {
            let ctx = egui::Context::default();
            ctx.set_fonts(crate::ui::fonts());
            Rig { ctx, st: PanelState::new(layout), t: 0.0, copied: Vec::new() }
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
            for c in &out.platform_output.commands {
                if let egui::OutputCommand::CopyText(t) = c {
                    self.copied.push(t.clone());
                }
            }
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
        problems: Vec<planet_editor::problems::Problem>,
        inspector: Option<Inspector>,
        selected: Option<ItemId>,
        can_undo: bool,
        brush: Option<planet_editor::session::BrushSettings>,
        sites: SitesView,
        regions: crate::regionedit::RegionsView,
        cursor: Option<planet_editor::ruler::CursorReadout>,
        ruler: Option<planet_editor::ruler::TransectProfile>,
        ruler_rubber: bool,
    }

    fn held(f: &Fixture, layout: Layout) -> Held {
        Held {
            layout,
            tool: Tool::Select,
            near: f.s.near(&f.ids, f.cam),
            found: f.s.search("crater", None, f.cam),
            groups: f.s.layer_groups(f.cam),
            chain: f.s.chain(f.cam),
            problems: Vec::new(),
            inspector: f.s.inspector(f.cam),
            selected: f.s.selected(),
            can_undo: true,
            brush: None,
            sites: SitesView::default(),
            regions: crate::regionedit::RegionsView::default(),
            cursor: None,
            ruler: None,
            ruler_rubber: false,
        }
    }

    fn data(h: &Held) -> PanelData<'_> {
        PanelData {
            layout: &h.layout,
            tool: &h.tool,
            brush: h.brush,
            selected: h.selected,
            near: &h.near,
            found: &h.found,
            groups: &h.groups,
            chain: &h.chain,
            problems: &h.problems,
            hidden: &[],
            inspector: h.inspector.as_ref(),
            can_undo: h.can_undo,
            can_redo: false,
            undo_depth: 2,
            status: "Hill 1 placed",
            edit_count: 2,
            saved: "not saved",
            view_status: "",
            keys: &[],
            store: None,
            sites: &h.sites,
            regions: &h.regions,
            cursor: h.cursor,
            ruler: h.ruler.as_ref(),
            ruler_rubber: h.ruler_rubber,
        }
    }

    // spec: AUTH-006
    #[test]
    fn the_status_bar_has_a_store_line_with_both_values_only_when_the_editor_has_a_store() {
        let f = fixture();
        let h = held(&f, Layout::default());
        assert_eq!(store_line(&data(&h)), "", "no editor, no line");
        let mut w = planet_editor::store_watch::StoreWatch::default();
        w.record_load(planet_world_def::BodyId(0), 20_001, std::time::Duration::from_millis(2_100));
        let d = PanelData { store: Some(&w), ..data(&h) };
        let line = store_line(&d);
        assert!(
            line.contains("20001 items (limit 20000)") && line.contains("last load 2100 ms (limit 2000 ms)") && line.contains("REVISIT"),
            "{line}"
        );
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

    // spec: VIEW-034
    #[test]
    fn a_text_field_keeps_the_keyboard_until_it_is_let_go_of_and_then_the_keys_are_the_cameras() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        // A click into the name field gives it the keyboard; a button does not.
        let p = rig.mark("fly");
        rig.click(&data(&h), p);
        assert!(!rig.ctx.egui_wants_keyboard_input(), "a button takes no keyboard focus");
        let p = rig.mark("name");
        rig.click(&data(&h), p);
        assert!(rig.ctx.egui_wants_keyboard_input(), "a text field keeps the focus after the click");
        // What the window does when the pointer is captured for looking around (`Ui::release_keyboard_focus` calls this).
        crate::ui::release_focus(&rig.ctx);
        rig.settle(&data(&h));
        assert!(!rig.ctx.egui_wants_keyboard_input(), "the focus is gone: the movement keys are the camera's");
    }

    // spec: VIEW-033
    #[test]
    fn a_region_in_the_inspector_has_a_surface_type_and_a_mask_and_the_mask_asks_for_a_layer_to_clip() {
        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let rows = |clips: bool, layer: Option<&str>| planet_editor::params::RegionRows {
            surface_names: vec!["rock", "sand", "grass"],
            surface_type: 1,
            clips_inside: clips,
            mask_layer: layer.map(String::from),
            layers: vec!["stamps".to_string(), "water".to_string()],
        };
        h.inspector.as_mut().unwrap().region = Some(rows(false, None));
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert!(rig.st.mark("surface-type").is_some() && rig.st.mark("mask-inside").is_some());
        assert!(rig.st.mark("mask-layer").is_none(), "no layer to choose while it clips nothing");
        // Switching the mask on picks the first other layer.
        let p = rig.mark("mask-inside");
        assert_eq!(rig.click(&data(&h), p), vec![Event::Mask { inside: true, layer: Some("stamps".into()) }]);
        // Clipping: the layer chooser shows, and switching it off asks for labelling only.
        h.inspector.as_mut().unwrap().region = Some(rows(true, Some("water")));
        rig.settle(&data(&h));
        assert!(rig.st.mark("mask-layer").is_some());
        let p = rig.mark("mask-inside");
        assert_eq!(rig.click(&data(&h), p), vec![Event::Mask { inside: false, layer: Some("water".into()) }]);
        // The surface type combo opens on a click and offers every type.
        let p = rig.mark("surface-type");
        rig.click(&data(&h), p);
        assert!(
            ["rock", "sand", "grass"].iter().all(|n| rig.st.mark(&format!("surface:{n}")).is_some()),
            "{:?}",
            rig.st.marks.iter().map(|m| &m.0).collect::<Vec<_>>()
        );
        let p = rig.mark("surface:grass");
        assert_eq!(rig.click(&data(&h), p), vec![Event::SurfaceType(2)]);
    }

    // spec: VIEW-033
    #[test]
    fn the_mask_label_says_what_the_stage_does_the_edits_are_removed_inside_the_outline_and_stay_outside_it() {
        use planet_core::Vec3;
        use planet_generators::height::{HeightField, Reference};
        use planet_generators::stamp::{ItemReader, StampStage};
        use planet_world_def::stamp::{Stamp, StampKind};
        use planet_world_def::{BodyId, ItemSpec, WorldDefinition};
        const R: f64 = 6_371_000.0;
        let unit = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            Vec3::from_lat_lon(la, lo)
        };
        // A hill of the layer "stamps" reaches across a square region of mode 2 that names "stamps".
        let centre = unit(20.0, 30.0);
        let hill = ItemSpec {
            id: 1,
            layer: "stamps".into(),
            priority: 0,
            dir: centre,
            content_hash: 0,
            reach_m: 6_000.0,
            max_delta_m: 50.0,
            stamp: None,
        }
        .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
        let corners: Vec<Vec3> =
            [(19.98, 29.98), (19.98, 30.02), (20.02, 30.02), (20.02, 29.98)].iter().map(|(a, b)| unit(*a, *b)).collect();
        let region = ItemSpec {
            id: 2,
            layer: "stamps".into(),
            priority: 0,
            dir: corners[0],
            content_hash: 0,
            reach_m: 8_000.0,
            max_delta_m: 0.0,
            stamp: None,
        }
        .with_stamp(Stamp::new(StampKind::Region, [2.0, 2.0, 0.0]).with_points(&corners));
        let mut def = WorldDefinition::single_planet(R, 1);
        def.put_item_spec(BodyId::PLANET, hill).unwrap();
        def.put_item_spec(BodyId::PLANET, region).unwrap();
        let def = std::sync::Arc::new(def);
        let stage =
            StampStage::new(std::sync::Arc::new(Reference(1)), def as std::sync::Arc<dyn ItemReader>, BodyId::PLANET, "stamps", 0, 3)
                .unwrap();
        let base = Reference(1);
        let inside = unit(20.0, 30.0);
        let outside = unit(20.0, 30.04);
        assert_eq!(stage.height(inside).to_bits(), base.height(inside).to_bits(), "inside the outline the layer's edit is removed");
        assert!(stage.height(outside) > base.height(outside) + 1.0, "outside it the edit stays");
        // The words say so: removes, inside, stay, outside; and nothing that reads as keeping the edits inside.
        let (label, hint) = (planet_editor::params::MASK_CUT_LABEL.to_lowercase(), planet_editor::params::MASK_CUT_HINT.to_lowercase());
        assert!(label.contains("removes") && label.contains("inside"), "{label}");
        assert!(hint.contains("cut away inside") && hint.contains("stay outside"), "{hint}");
        for wrong in ["only inside the outline", "clips another layer", "keeps the edits inside"] {
            assert!(!label.contains(wrong) && !hint.replace("keeping them only inside is not offered yet", "").contains(wrong), "{wrong}");
        }
    }

    // spec: VIEW-043
    #[test]
    fn an_item_over_the_brush_slope_limit_wears_the_warning_in_its_row_and_in_the_inspector() {
        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert!(rig.st.marks.iter().all(|m| !m.0.starts_with("steep")), "no warning on an item within the limit");
        let label = h.near[0].label.clone();
        h.near[0].steep = true;
        h.inspector.as_mut().unwrap().warning = Some(planet_editor::slope::WARNING.to_string());
        rig.settle(&data(&h));
        assert!(rig.st.mark(&format!("steep:{label}")).is_some(), "the row carries the warning");
        assert!(rig.st.mark("steep-warning").is_some(), "the inspector carries it");
    }

    // spec: VIEW-057
    #[test]
    fn the_keys_tab_lists_every_key_the_editor_accepts_by_group_with_its_state() {
        use planet_frame::keytable::{rows, Group};
        let f = fixture();
        let layout = Layout::default();
        assert_eq!(layout.tabs.last(), Some(&Tab::Keys), "the tab is last in the default layout");
        let h = held(&f, layout.clone());
        let mut viewer = planet_frame::viewer::ViewerState::new(0, 3, 0, true);
        viewer.lit = true;
        let listed = rows(true, &viewer, None);
        assert!(
            !listed.is_empty() && Group::ALL.iter().all(|g| listed.iter().any(|r| r.group == *g)),
            "every group has keys in the editor mode"
        );
        let mut rig = Rig::new(&layout);
        // Without rows the tab says so; with the window's rows every key is listed once, and a key with a state shows it.
        rig.settle(&data(&h));
        rig.click(&data(&h), rig.mark("tab:keys"));
        assert_eq!(rig.st.tab, Tab::Keys);
        assert!(rig.st.marks.iter().all(|m| !m.0.starts_with("key:")));
        rig.settle(&PanelData { keys: &listed, ..data(&h) });
        for r in &listed {
            assert!(rig.st.mark(&format!("key:{}", r.label)).is_some(), "the key {} is listed", r.label);
        }
        assert_eq!(rig.st.marks.iter().filter(|m| m.0.starts_with("key:")).count(), listed.len(), "each key once");
        assert!(listed.iter().any(|r| r.state.is_some()), "some keys carry a state");
    }

    // spec: VIEW-047
    #[test]
    fn the_problems_tab_lists_each_flagged_edit_with_what_is_wrong_and_asks_for_fly_to_the_fixes_and_delete() {
        use planet_editor::problems::{Issue, Problem};
        let f = fixture();
        let layout = Layout::default();
        assert!(layout.tabs.contains(&Tab::Problems), "the tab is in the default layout");
        let mut h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        // No problem: the tab says so and has no row.
        rig.settle(&data(&h));
        rig.click(&data(&h), rig.mark("tab:problems"));
        assert_eq!(rig.st.tab, Tab::Problems);
        assert!(rig.st.marks.iter().all(|m| !m.0.starts_with("problem:")));
        // Two problems: a brush that is steep and stacked (both fixes), an edit near capped tiles (no fix but fly to and delete).
        let (a, b) = (h.near[0].clone(), h.near[1].clone());
        h.problems = vec![
            Problem {
                row: a.clone(),
                issues: vec![Issue::TooSteep, Issue::Stacked { depth: 9, strokes: 12 }],
                can_reduce: true,
                can_merge: true,
            },
            Problem { row: b.clone(), issues: vec![Issue::TilesCapped], can_reduce: false, can_merge: false },
        ];
        rig.settle(&data(&h));
        for r in [&a, &b] {
            for name in ["problem", "issue", "fly", "reduce", "merge", "delete"] {
                assert!(rig.st.mark(&format!("{name}:{}", r.label)).is_some(), "{name}:{}", r.label);
            }
        }
        // Each button asks for its event.
        let ev = rig.click(&data(&h), rig.mark(&format!("fly:{}", b.label)));
        assert_eq!(ev, vec![Event::FlyTo(b.id)]);
        let ev = rig.click(&data(&h), rig.mark(&format!("reduce:{}", a.label)));
        assert_eq!(ev, vec![Event::Reduce(a.id)]);
        let ev = rig.click(&data(&h), rig.mark(&format!("merge:{}", a.label)));
        assert_eq!(ev, vec![Event::Merge(a.id)]);
        let ev = rig.click(&data(&h), rig.mark(&format!("delete:{}", b.label)));
        assert_eq!(ev, vec![Event::DeleteItem(b.id)]);
        let ev = rig.click(&data(&h), rig.mark(&format!("problem:{}", a.label)));
        assert_eq!(ev, vec![Event::Select(a.id)]);
        // A fix that does not apply is disabled: clicking it asks for nothing.
        assert!(rig.click(&data(&h), rig.mark(&format!("reduce:{}", b.label))).is_empty());
        assert!(rig.click(&data(&h), rig.mark(&format!("merge:{}", b.label))).is_empty());
    }

    // spec: VIEW-031
    #[test]
    fn the_rail_has_the_line_and_region_tools_and_the_question_mark_opens_the_list_of_keys() {
        let f = fixture();
        let layout = Layout::default();
        let h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        // The rail: ridge [5], channel [6], region [7] pick their tools.
        for (label, tool) in [
            ("tool:Ridge (draw)  [5]", Tool::Line("ridge".into())),
            ("tool:Channel (draw)  [6]", Tool::Line("channel".into())),
            ("tool:Region (draw)  [7]", Tool::Region),
        ] {
            let p = rig.mark(label);
            assert_eq!(rig.click(&data(&h), p), vec![Event::Tool(tool)], "{label}");
        }
        // The question mark asks for the list; shown, it covers a rectangle and its close button asks to hide it.
        let covered = rig.st.covered.len();
        let p = rig.mark("help");
        assert_eq!(rig.click(&data(&h), p), vec![Event::ToggleHelp]);
        rig.st.help = true;
        rig.settle(&data(&h));
        assert_eq!(rig.st.covered.len(), covered + 1, "the key list covers part of the window");
        assert_eq!(HELP_KEYS.len(), 18);
        assert!(HELP_KEYS.iter().any(|(k, w)| *k == "Enter" && w.contains("finish")));
        assert!(HELP_KEYS.iter().any(|(k, _)| *k == "Shift + click") && HELP_KEYS.iter().any(|(k, _)| *k == "Ctrl + click"));
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

    // spec: VIEW-016
    #[test]
    fn the_layers_tab_has_a_row_for_a_split_line_a_region_and_a_brush_with_their_details_and_the_row_selects_and_hides_the_item() {
        use planet_editor::session::Tool as T;
        use planet_world_def::stamp::BrushKind;
        let mut f = fixture();
        let (la, lo) = (47.2f64.to_radians(), 8.55f64.to_radians());
        let at = |m_east: f64, m_north: f64| {
            let (dla, dlo) = (m_north / R, m_east / R / libm::cos(la));
            Vec3::from_lat_lon(la + dla, lo + dlo)
        };
        // A ridge of 150 km (split into parts), a region and a brush stroke.
        f.s.set_tool(T::Line("ridge".into()));
        for k in 0..=16 {
            f.s.add_draft_point(at(f64::from(k) * 9_000.0, 20_000.0)).unwrap();
        }
        let (ridge, _) = f.s.finish_draft().unwrap();
        f.s.set_tool(T::Region);
        for (e, n) in [(0.0, 40_000.0), (8_000.0, 40_000.0), (4_000.0, 46_000.0)] {
            f.s.add_draft_point(at(e, n)).unwrap();
        }
        f.s.finish_draft().unwrap();
        f.s.set_tool(T::Brush(BrushKind::Raise));
        f.s.begin_stroke(at(0.0, -20_000.0), 0.0).unwrap();
        f.s.extend_stroke(at(300.0, -20_000.0));
        f.s.end_stroke().unwrap();
        f.s.set_tool(T::Select);
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        h.groups = f.s.layer_groups(f.cam);
        let rows: Vec<planet_editor::lists::Row> = h.groups.iter().flat_map(|g| g.rows.clone()).collect();
        let by_kind = |k: &str| rows.iter().find(|r| r.kind == k).unwrap_or_else(|| panic!("no {k} row")).clone();
        let (r_ridge, r_region, r_brush) = (by_kind("ridge"), by_kind("region"), by_kind("brush"));
        assert_eq!(rows.iter().filter(|r| r.kind == "ridge").count(), 1, "a split line is one row");
        assert!(row_text(&r_ridge).contains(" parts") && row_text(&r_ridge).starts_with(&r_ridge.label), "{}", row_text(&r_ridge));
        assert_eq!(row_text(&r_region), format!("{}  (Region) · 3 points", r_region.label));
        assert_eq!(row_text(&r_brush), format!("{}  (Brush) · 1 stroke", r_brush.label));
        assert_eq!(r_ridge.id, f.s.editor().line_items(ridge).unwrap()[0], "the row is the first part");
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        rig.click(&data(&h), rig.mark("tab:layers"));
        for row in [&r_ridge, &r_region, &r_brush] {
            assert!(rig.st.mark(&format!("layer-row:{}", row.label)).is_some(), "a layers row for {}", row.label);
            assert!(rig.st.mark(&format!("show:{}", row.label)).is_some(), "a show box for {}", row.label);
        }
        // A click selects the item of the row; the check box hides it (the session hides the whole group).
        let p = rig.mark(&format!("layer-row:{}", r_brush.label));
        assert_eq!(rig.click(&data(&h), p), vec![Event::Select(r_brush.id)]);
        let p = rig.mark(&format!("show:{}", r_ridge.label));
        assert_eq!(rig.click(&data(&h), p), vec![Event::ToggleHidden(r_ridge.id)]);
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
        // Enter keeps the focus in the field (VIEW-041); leaving it ends the typing.
        rig.click(&data(&h), Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0));
        // The fields follow the value when they are not being typed in.
        let mut h2 = held(&f, layout.clone());
        if let Some(i) = h2.inspector.as_mut() {
            i.params[0].text = "777 m".into();
        }
        rig.settle(&data(&h2));
        rig.settle(&data(&h2));
        assert_eq!(rig.st.typed.get(&(f.ids[0], "radius_m".into())).map(String::as_str), Some("777 m"));
    }

    // spec: VIEW-035
    #[test]
    fn the_rail_has_the_four_brushes_and_their_radius_strength_and_flatten_height_are_typed_in_metres() {
        use planet_editor::session::BrushSettings;
        use planet_world_def::stamp::BrushKind;
        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        assert!(rig.st.mark("brush-radius").is_none(), "no brush fields while no brush is on");
        for (label, kind) in [
            ("Raise brush  [8]", BrushKind::Raise),
            ("Lower brush  [9]", BrushKind::Lower),
            ("Smooth brush  [0]", BrushKind::Smooth),
            ("Flatten brush  [-]", BrushKind::Flatten),
        ] {
            let p = rig.mark(&format!("tool:{label}"));
            assert_eq!(rig.click(&data(&h), p), vec![Event::Tool(Tool::Brush(kind))], "{label}");
        }
        // A raise shows the radius and the strength; a flatten also the height it levels towards.
        h.tool = Tool::Brush(BrushKind::Raise);
        h.brush = Some(BrushSettings::default());
        rig.settle(&data(&h));
        assert!(rig.st.mark("brush-radius").is_some() && rig.st.mark("brush-strength").is_some());
        assert!(rig.st.mark("brush-target").is_none());
        h.tool = Tool::Brush(BrushKind::Flatten);
        rig.settle(&data(&h));
        assert!(rig.st.mark("brush-target").is_some());
        // Typing 250 in the radius field and leaving it asks for it; leaving it unchanged asks for nothing.
        let field = rig.mark("brush-radius");
        let ev = {
            rig.click(&data(&h), field);
            rig.click(&data(&h), Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0))
        };
        assert!(!ev.iter().any(|e| matches!(e, Event::BrushRadius(_))), "{ev:?}");
        rig.click(&data(&h), field);
        let all = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        rig.pass(&data(&h), vec![key(egui::Key::A, all)]);
        rig.pass(&data(&h), vec![Raw::Text("250".into())]);
        let mut got = rig.pass(&data(&h), vec![key(egui::Key::Enter, Modifiers::NONE)]);
        got.extend(rig.pass(&data(&h), vec![]));
        assert!(got.contains(&Event::BrushRadius("250".into())), "{got:?}");
        // Enter keeps the focus in the field (VIEW-041); leaving it ends the typing.
        rig.click(&data(&h), Pos2::new(SIZE.0 / 2.0, SIZE.1 / 2.0));
        // The fields show the settings in metres, the flatten height "auto" until one is typed.
        h.brush = Some(BrushSettings { radius_m: 250.0, strength_m: 7.5, target_m: Some(1_200.0) });
        rig.settle(&data(&h));
        rig.settle(&data(&h));
        assert_eq!(rig.st.brush_text.get("brush-radius").map(String::as_str), Some("250 m"));
        assert_eq!(rig.st.brush_text.get("brush-target").map(String::as_str), Some("1.2 km"));
        assert_eq!(rig.st.brush_text.get("brush-strength").map(String::as_str), Some("7.5 m"));
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
            closed: true,
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

    // spec: VIEW-067
    #[test]
    fn the_level_1_preview_draws_its_lines_and_its_label_and_nothing_when_there_is_none() {
        use planet_editor::preview_overlay::{DrawnLine, PreviewOverlay};
        let lines: Vec<DrawnLine> = (0..20)
            .map(|k| DrawnLine {
                a: [100.0 + 10.0 * k as f64, 200.0],
                b: [110.0 + 10.0 * k as f64, 260.0],
                watershed: k % 3,
                changed: k % 5 == 0,
                weight: 100.0 - k as f64,
            })
            .collect();
        let p = PreviewOverlay { id: 9, label: "preview (not committed): 20 trunk cells, 4 changed".into(), lines, dropped: 0 };
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui::fonts());
        let count = |p: &PreviewOverlay, ppp: f32| {
            ctx.set_pixels_per_point(ppp);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1920.0 / ppp, 1080.0 / ppp))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| draw_preview_overlay(ui, p, |q| Pos2::new(q[0] as f32 / ppp, q[1] as f32 / ppp)));
            let n = out.shapes.len();
            out.textures_delta.clear();
            n
        };
        assert!(count(&p, 1.0) > 20, "a line for each trunk line and the label");
        let empty = PreviewOverlay { lines: Vec::new(), ..p.clone() };
        assert!(count(&empty, 1.0) >= 1 && count(&empty, 1.0) < count(&p, 1.0), "no lines: the label only");
        assert!(count(&p, 1.5) > 20, "at another scale too");
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

    // spec: VIEW-041
    #[test]
    fn enter_keeps_the_focus_in_a_value_field_and_up_and_down_step_it_and_shift_steps_ten_times() {
        use planet_editor::params::Unit;
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui::fonts());
        let step = FieldStep { value: 400.0, step: 10.0, unit: Unit::Metres, range: Some((50.0, 2_000.0)) };
        let mut buf = String::from("400 m");
        let mut t = 0.0;
        let mut id = None;
        let mut commits = Vec::new();
        let mut pass = |events: Vec<Raw>, buf: &mut String, id: &mut Option<egui::Id>, commits: &mut Vec<String>| {
            t += 0.05;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
                time: Some(t),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let (r, commit) = value_field(ui, buf, "400 m", step);
                *id = Some(r.id);
                commits.extend(commit);
            });
            out.textures_delta.clear();
        };
        let key = |k: egui::Key, mods: Modifiers| Raw::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: mods };
        pass(vec![], &mut buf, &mut id, &mut commits);
        ctx.memory_mut(|m| m.request_focus(id.unwrap()));
        pass(vec![], &mut buf, &mut id, &mut commits);
        assert!(ctx.memory(|m| m.has_focus(id.unwrap())), "the field has the focus");
        // Up: one step (the larger of 10 m and 1 % of 400 m), committed at once, the focus stays.
        pass(vec![key(egui::Key::ArrowUp, Modifiers::NONE)], &mut buf, &mut id, &mut commits);
        assert_eq!((commits.as_slice(), buf.as_str()), (&["410 m".to_string()][..], "410 m"));
        // Shift+Down from the typed text: ten steps.
        pass(vec![key(egui::Key::ArrowDown, Modifiers::SHIFT)], &mut buf, &mut id, &mut commits);
        assert_eq!(commits.last().map(String::as_str), Some("310 m"), "{commits:?}");
        // Enter commits what was typed and keeps the focus.
        buf = "500".to_string();
        pass(vec![key(egui::Key::Enter, Modifiers::NONE)], &mut buf, &mut id, &mut commits);
        pass(vec![], &mut buf, &mut id, &mut commits);
        assert_eq!(commits.last().map(String::as_str), Some("500"), "{commits:?}");
        assert!(ctx.memory(|m| m.has_focus(id.unwrap())), "Enter leaves the focus in the field");
    }

    // spec: VIEW-077
    // spec: TEST-166
    #[test]
    fn the_sites_tabs_regions_section_places_resizes_moves_deletes_and_opens_boxes() {
        use crate::regionedit::{RegionRow, RegionsView};
        use planet_cache::regions_file::RegionState;
        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let row = |name: &str, state| RegionRow {
            name: name.into(),
            face: 2,
            tiles: "60..=61 x 60..=61".into(),
            width: 2,
            height: 2,
            state,
            selected: false,
            open_command: (state == RegionState::Ready)
                .then(|| format!("cargo run --release -p planet -- editor --seed 14 --commit w/regions/{name}")),
        };
        h.regions = RegionsView {
            rows: vec![row("grt", RegionState::Ready), row("new", RegionState::Unbaked)],
            where_label: "saved in w/regions.toml".into(),
            has_file: true,
            placing: Some("click the ground to place a new box".into()),
            size: (2, 2),
            open_command: Some("cargo run --release -p planet -- editor --seed 14 --commit w/regions/grt".into()),
        };
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        rig.click(&data(&h), rig.mark("tab:sites"));
        rig.settle(&data(&h));

        rig.click(&data(&h), rig.mark("region:new-name"));
        rig.pass(&data(&h), vec![Raw::Text("ridge".into())]);
        let ev = rig.click(&data(&h), rig.mark("region:place"));
        assert!(ev.contains(&Event::PlaceRegion { name: "ridge".into() }), "{ev:?}");
        let p = rig.mark("region:size:w+");
        assert_eq!(rig.click(&data(&h), p), vec![Event::RegionSize { width: 3, height: 2 }]);
        let p = rig.mark("region:size:h-");
        assert!(rig.click(&data(&h), p).is_empty(), "a side does not go under 2");
        let p = rig.mark("region:w+:grt");
        assert_eq!(rig.click(&data(&h), p), vec![Event::ResizeRegion { name: "grt".into(), width: 3, height: 2 }]);
        let p = rig.mark("region:h-:grt");
        assert_eq!(rig.click(&data(&h), p), vec![Event::ResizeRegion { name: "grt".into(), width: 2, height: 1 }]);
        let p = rig.mark("region:move:grt");
        assert_eq!(rig.click(&data(&h), p), vec![Event::MoveRegion("grt".into())]);
        let p = rig.mark("region:select:new");
        assert_eq!(rig.click(&data(&h), p), vec![Event::SelectRegion("new".into())]);
        let p = rig.mark("region:open:grt");
        assert_eq!(rig.click(&data(&h), p), vec![Event::OpenRegion("grt".into())]);
        assert_eq!(
            rig.copied.last().map(String::as_str),
            Some("cargo run --release -p planet -- editor --seed 14 --commit w/regions/grt"),
            "Open copies the command"
        );
        assert!(rig.st.mark("region:open:new").is_none(), "only a ready box opens");
        assert!(rig.st.mark("region:copy").is_some());
        let p = rig.mark("region:delete:new");
        assert_eq!(rig.click(&data(&h), p), vec![Event::DeleteRegion("new".into())]);

        // Without a file the section only says how to name one.
        h.regions = RegionsView { where_label: "no regions file".into(), ..RegionsView::default() };
        rig.settle(&data(&h));
        assert!(rig.st.mark("region:place").is_none());
    }

    // spec: VIEW-075
    #[test]
    fn the_sites_tab_drives_coordinate_jumps_bookmarks_and_repro_shelf_events() {
        use planet_editor::bookmarks::CameraReadout;
        use planet_editor::repro_shelf::ReproCompatibility;

        let f = fixture();
        let layout = Layout::default();
        assert!(layout.tabs.contains(&Tab::Sites), "Sites tab is in default layout");
        assert_eq!(layout.tabs.last(), Some(&Tab::Keys), "Keys tab stays last (VIEW-057)");

        let mut h = held(&f, layout.clone());
        h.sites = SitesView {
            readout: Some(CameraReadout { lat_deg: 47.2, lon_deg: 8.55, altitude_m: 1_500.0, yaw_deg: 45.0, pitch_deg: -35.0 }),
            bookmarks: vec![BookmarkRow {
                id: 1,
                name: "Ridge Overlook".into(),
                summary: "47.20000° N, 8.55000° E · alt 1.5 km · hdg 045.0° · pitch -35.0°".into(),
                site_arg: "47.20000,8.55000".into(),
                distance_m: 2_400.0,
            }],
            bookmarks_where: "saved in bookmarks.toml".into(),
            repros: vec![ReproRow {
                index: 0,
                stem: "frame-42".into(),
                summary: "seed 7 · relief · 20.00000° N, 30.00000° E · alt 2 km · hdg 000.0° · pitch -25.0°".into(),
                compatibility: ReproCompatibility::MatchesWorld,
                has_path: true,
                path_samples: Some(12),
                has_png: true,
                replay_cmd: "cargo xtask repro target/review/frame-42-path.txt --window".into(),
            }],
            repros_dir: "target/review".into(),
            broken_repros: 1,
        };

        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));
        rig.click(&data(&h), rig.mark("tab:sites"));
        assert_eq!(rig.st.tab, Tab::Sites);
        assert!(rig.st.mark("site:readout").is_some());

        // Type coordinate jump and click Go.
        rig.click(&data(&h), rig.mark("site:jump-input"));
        rig.pass(&data(&h), vec![Raw::Text("-12.5, 40.0, 2 km".into())]);
        let ev = rig.click(&data(&h), rig.mark("site:jump-go"));
        assert!(ev.contains(&Event::JumpCoords("-12.5, 40.0, 2 km".into())), "{ev:?}");

        // Type optional bookmark name and click Save view.
        rig.click(&data(&h), rig.mark("site:new-name"));
        rig.pass(&data(&h), vec![Raw::Text("Basin Rim".into())]);
        let ev = rig.click(&data(&h), rig.mark("site:save"));
        assert!(ev.contains(&Event::SaveBookmark { name: "Basin Rim".into() }), "{ev:?}");

        // Jump and delete on existing bookmark row.
        let p = rig.mark("bookmark:jump:1");
        assert_eq!(rig.click(&data(&h), p), vec![Event::JumpBookmark(1)]);
        let p = rig.mark("bookmark:delete:1");
        assert_eq!(rig.click(&data(&h), p), vec![Event::DeleteBookmark(1)]);

        // Repro shelf capture, refresh, jump, and bookmark promotion.
        let p = rig.mark("repro:capture");
        assert_eq!(rig.click(&data(&h), p), vec![Event::CaptureRepro]);
        let p = rig.mark("repro:refresh");
        assert_eq!(rig.click(&data(&h), p), vec![Event::RefreshRepros]);
        let p = rig.mark("repro:jump:frame-42");
        assert_eq!(rig.click(&data(&h), p), vec![Event::JumpRepro(0)]);
        let p = rig.mark("repro:bookmark:frame-42");
        assert_eq!(rig.click(&data(&h), p), vec![Event::BookmarkRepro(0)]);
    }

    // spec: VIEW-076
    #[test]
    fn ruler_tool_button_cursor_readout_and_inline_transect_plot_fire_ruler_events() {
        use planet_core::surface::RAD_TO_DEG;
        use planet_editor::polyline::GeoPoint;
        use planet_editor::ruler::{CursorReadout, TransectProfile, HEIGHTFIELD_SLOPE_LIMIT_DEG};

        let f = fixture();
        let layout = Layout::default();
        let mut h = held(&f, layout.clone());
        let mut rig = Rig::new(&layout);
        rig.settle(&data(&h));

        // Rail Ruler button emits Event::Tool(Tool::Ruler) and help keys count stays intact (18).
        assert_eq!(HELP_KEYS.len(), 18);
        let p = rig.mark("tool:Ruler");
        assert_eq!(rig.click(&data(&h), p), vec![Event::Tool(Tool::Ruler)]);

        // Activate Ruler with live steep cursor readout and sampled transect profile exceeding 45°.
        h.tool = Tool::Ruler;
        h.inspector = None;
        h.cursor =
            Some(CursorReadout { point: GeoPoint::new(47.2, 8.55), height_m: 1_240.0, gradient: 1.2, slope_deg: 50.2, over_limit: true });
        let a = GeoPoint::new(0.0, 0.0);
        let b = GeoPoint::new(0.0, 1_000.0 / R * RAD_TO_DEG);
        let profile =
            TransectProfile::sample(a, b, R, 17, HEIGHTFIELD_SLOPE_LIMIT_DEG, &|d| 1_200.0 * (R * d.y).clamp(0.0, 1_000.0) / 1_000.0);
        assert!(profile.max_slope_deg > HEIGHTFIELD_SLOPE_LIMIT_DEG);
        h.ruler = Some(profile);

        rig.settle(&data(&h));
        assert!(rig.st.mark("cursor-readout").is_some(), "status bar shows cursor readout");
        assert!(rig.st.mark("ruler-steep").is_some(), "inspector marks steep >45° summary");
        let plot_pos = rig.mark("ruler-plot");
        let right = rig.st.covered.iter().find(|r| (r.max.x - SIZE.0).abs() < 1.0 && r.width() < 500.0).expect("inspector panel");
        assert!((right.width() - layout.inspector_width).abs() < 2.0, "inspector stayed within exact width: {right:?}");

        // Hovering over the inline transect plot sets ruler_scrub cleanly.
        rig.pass(&data(&h), vec![Raw::PointerMoved(plot_pos)]);
        let scrub = rig.st.ruler_scrub.expect("plot hover sets scrub fraction");
        assert!((scrub - 0.5).abs() < 0.05, "scrub={scrub}");

        // Swap and Clear buttons emit Event::SwapRuler / Event::ClearRuler.
        let p = rig.mark("ruler-swap");
        assert_eq!(rig.click(&data(&h), p), vec![Event::SwapRuler]);
        let p = rig.mark("ruler-clear");
        assert_eq!(rig.click(&data(&h), p), vec![Event::ClearRuler]);
    }
}
