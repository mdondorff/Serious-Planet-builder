//! The key table (VIEW-055): the one list of the keys the window takes. The key handling, the usage text, the F1 list and the editor's Keys tab
//! are all made from it, so a key that is handled is listed and a key that is listed is handled (a tier-A test holds both ways).
//!
//! A key is named by the `Debug` name of its winit `KeyCode` (`KeyP`, `F3`, `BracketLeft`), which this GPU-free crate does not need winit to
//! hold (CON-03); the window turns a code into its name and asks the table.

use crate::viewer::{Command, Held};

/// A key the editor mode takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    /// A tool by number: 1 select, 2 and on the stamp kinds in order.
    Tool(usize),
    Undo,
    Redo,
    Delete,
    /// Esc: abandon a drag, else drop the draft of a line or region, else leave the tool, else deselect.
    Escape,
    /// Enter: finish the line or region being drawn.
    Finish,
    /// Backspace: take back the last point of the draft. It never deletes an edit (Delete does, opus-planner 2026-10-20, easy to reverse).
    Back,
    /// F1 or `?`: show or hide the list of keys.
    Help,
}

/// The held keys (the free camera), one slot of [`Held`] each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldSlot {
    Forward,
    Back,
    Left,
    Right,
    Down,
    Up,
    LookLeft,
    LookRight,
    LookUp,
    LookDown,
    Boost,
}

impl HeldSlot {
    /// The flag of `held` this key sets.
    pub fn slot(self, held: &mut Held) -> &mut bool {
        match self {
            HeldSlot::Forward => &mut held.forward,
            HeldSlot::Back => &mut held.back,
            HeldSlot::Left => &mut held.left,
            HeldSlot::Right => &mut held.right,
            HeldSlot::Down => &mut held.down,
            HeldSlot::Up => &mut held.up,
            HeldSlot::LookLeft => &mut held.look_left,
            HeldSlot::LookRight => &mut held.look_right,
            HeldSlot::LookUp => &mut held.look_up,
            HeldSlot::LookDown => &mut held.look_down,
            HeldSlot::Boost => &mut held.boost,
        }
    }
}

/// When the window accepts the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Whatever the mode.
    Both,
    /// Only with the editor off.
    Viewer,
    /// Only with the editor on.
    Editor,
}

impl Mode {
    /// Whether an entry of this mode is accepted with the editor `editor_on`.
    pub fn accepts(self, editor_on: bool) -> bool {
        match self {
            Mode::Both => true,
            Mode::Viewer => !editor_on,
            Mode::Editor => editor_on,
        }
    }
}

/// The group an entry is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Editing,
    Camera,
    View,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Editing, Group::Camera, Group::View];

    pub fn title(self) -> &'static str {
        match self {
            Group::Editing => "Editing",
            Group::Camera => "Camera and movement",
            Group::View => "View and debug",
        }
    }
}

/// What a key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A viewer command ([`command_for`]).
    View(Command),
    /// A key of the editor mode ([`edit_key_for`]); the chords (`Ctrl+Z`, `Ctrl+Y`) and `?` have no code, the window takes them by their label.
    Edit(EditKey),
    /// A held key ([`held_for`]).
    Held(HeldSlot),
    /// Handled outside the lookups (Tab, the mouse): listed so the list is whole, never looked up.
    Other,
}

/// The state a row shows beside its key (the panel and the title read it from the viewer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKey {
    Paused,
    Camera,
    Lit,
    Vsync,
    Overlay,
    CoarseLod,
    GroundDetail,
    ScaleRefs,
    EditingLight,
    CoupledLens,
    ShadeView,
    SunTime,
    Globe,
}

/// One entry of the table.
#[derive(Clone, Copy, Debug)]
pub struct KeyDef {
    /// The `Debug` names of the `KeyCode`s that do it; empty for a chord, the mouse and `?`.
    pub codes: &'static [&'static str],
    /// How the key is written in the lists (`Ctrl+Z`, `P or Space`, `WASD/QE`).
    pub label: &'static str,
    pub action: Action,
    pub group: Group,
    pub mode: Mode,
    /// What it does, in the words of the help.
    pub help: &'static str,
    /// The state shown beside it, if it has one.
    pub state: Option<StateKey>,
}

const fn k(
    codes: &'static [&'static str],
    label: &'static str,
    action: Action,
    group: Group,
    mode: Mode,
    help: &'static str,
    state: Option<StateKey>,
) -> KeyDef {
    KeyDef { codes, label, action, group, mode, help, state }
}

use Action::{Edit, Held as H, Other, View};
use Group::{Camera, Editing, View as V};
use Mode::{Both, Editor, Viewer};

/// The usage text for the viewer (VIEW-055), made from the table.
pub static KEYS_HELP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| help_text(false));

/// The usage text and the F1 list of the editor mode (VIEW-055), made from the table.
pub static EDIT_KEYS_HELP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| help_text(true));

/// The table. The order is the order of the lists.
pub static KEYS: &[KeyDef] = &[
    // ---- Editing (the editor mode: Tab turns it on) ----
    k(&["Tab"], "Tab", Other, Editing, Both, "editor mode on or off (the panels, the outlines of the edits the camera shows; the planet is for editing: the pointer is free)", None),
    k(&["Digit1"], "1", Edit(EditKey::Tool(1)), Editing, Editor, "select", None),
    k(&["Digit2"], "2", Edit(EditKey::Tool(2)), Editing, Editor, "crater, then click the planet to place it", None),
    k(&["Digit3"], "3", Edit(EditKey::Tool(3)), Editing, Editor, "hill, then click the planet to place it", None),
    k(&["Digit4"], "4", Edit(EditKey::Tool(4)), Editing, Editor, "plateau, then click the planet to place it", None),
    k(&["Digit5"], "5", Edit(EditKey::Tool(5)), Editing, Editor, "ridge", None),
    k(&["Digit6"], "6", Edit(EditKey::Tool(6)), Editing, Editor, "channel", None),
    k(&["Digit7"], "7", Edit(EditKey::Tool(7)), Editing, Editor, "region, then click the planet to add points (Enter finishes, Backspace takes the last point back)", None),
    k(&["Digit8"], "8", Edit(EditKey::Tool(8)), Editing, Editor, "raise", None),
    k(&["Digit9"], "9", Edit(EditKey::Tool(9)), Editing, Editor, "lower", None),
    k(&["Digit0"], "0", Edit(EditKey::Tool(10)), Editing, Editor, "smooth", None),
    k(&["Minus"], "-", Edit(EditKey::Tool(11)), Editing, Editor, "flatten: a brush, drag on the planet to paint a stroke (the release puts it, one undo takes it back)", None),
    k(&["Enter", "NumpadEnter"], "Enter", Edit(EditKey::Finish), Editing, Editor, "finish the line or region being drawn (in the inspector it keeps the field)", None),
    k(&["Backspace"], "Backspace", Edit(EditKey::Back), Editing, Editor, "take back the last point of the draft", None),
    k(&["Delete"], "Delete", Edit(EditKey::Delete), Editing, Editor, "delete the selected edit", None),
    k(&[], "Ctrl+Z", Edit(EditKey::Undo), Editing, Editor, "undo", None),
    k(&[], "Ctrl+Y or Ctrl+Shift+Z", Edit(EditKey::Redo), Editing, Editor, "redo", None),
    k(&["F1"], "F1 or ?", Edit(EditKey::Help), Editing, Editor, "list the keys", None),
    k(&["Escape"], "Esc", Edit(EditKey::Escape), Editing, Editor, "step back (drag, draft, tool, selection); it does not quit", None),
    k(&[], "click", Other, Editing, Editor, "select an edit (click again at the same spot to go through the edits under the cursor, smallest first; double-click in a list to fly there); drag its handles (circle: move, square: radius, light circle: height, amber dots: the points of a line or region) or type values in the inspector (Up and Down step the value, Shift ten times as much)", None),
    k(&[], "Shift+click", Other, Editing, Editor, "on a line or region adds a point", None),
    k(&[], "Ctrl+click", Other, Editing, Editor, "on a point removes it", None),
    k(&[], "mouse wheel", Other, Editing, Editor, "sets the brush radius (Shift+wheel its strength); both are also in the inspector", None),
    // ---- Camera and movement ----
    k(&["KeyW", "KeyA", "KeyS", "KeyD", "KeyQ", "KeyE"], "WASD/QE", H(HeldSlot::Forward), Camera, Both, "free camera: move (W forward, A left, S back, D right, Q down, E up)", None),
    k(&["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"], "arrows", H(HeldSlot::LookLeft), Camera, Both, "free camera: look", None),
    k(&["ShiftLeft", "ShiftRight"], "Shift", H(HeldSlot::Boost), Camera, Both, "boost the movement", None),
    k(&[], "right mouse button", Other, Camera, Editor, "hold it to look around", None),
    k(&[], "click in the world", Other, Camera, Viewer, "captures the mouse to look (Esc releases it)", None),
    k(&["KeyC"], "C", View(Command::ToggleCamera), Camera, Both, "flight or free camera (does nothing while the editor is on)", Some(StateKey::Camera)),
    k(&["KeyP", "Space"], "P or Space", View(Command::TogglePause), Camera, Both, "pause", Some(StateKey::Paused)),
    k(&["KeyR"], "R", View(Command::Restart), Camera, Both, "restart the flight", None),
    k(&["KeyG"], "G", View(Command::ToggleGlobe), Camera, Both, "globe overview (drag turns it, wheel zooms, a click on a marker or the surface flies there and lands in the free camera)", Some(StateKey::Globe)),
    // ---- View and debug ----
    k(&["KeyL"], "L", View(Command::ToggleLit), V, Both, "lit path (with the atmosphere) or debug path", Some(StateKey::Lit)),
    k(&["KeyV"], "V", View(Command::NextView), V, Both, "next debug view (debug path)", None),
    k(&["KeyJ"], "J", View(Command::CycleShadeView), V, Both, "shade view (normal, horizon, shadow: the horizon elevation at the sun and its shadow factor; lit path, needs a store with a coupled run)", Some(StateKey::ShadeView)),
    k(&["BracketLeft"], "[", View(Command::SunEarlier), V, Both, "the sun half an hour earlier in local solar time at the camera (the title shows the time)", Some(StateKey::SunTime)),
    k(&["BracketRight"], "]", View(Command::SunLater), V, Both, "the sun half an hour later in local solar time at the camera", Some(StateKey::SunTime)),
    k(&["Backslash"], "Backslash", View(Command::SunReset), V, Both, "back to the look's sun", Some(StateKey::SunTime)),
    k(&["KeyH"], "H", View(Command::ToggleEditingLight), V, Both, "editing light (a sun fixed over your shoulder, 35 degrees up, so no slope is black; off by default)", Some(StateKey::EditingLight)),
    k(&["KeyK"], "K", View(Command::CycleCoupledLens), V, Both, "what the climate shows (the climate, the coupled run's heights, its drainage area: the global river network; lit path; the run loads or is made once, the title says when it is ready)", Some(StateKey::CoupledLens)),
    k(&["F3"], "F3", View(Command::CycleOverlay), V, Both, "cycles the overlay (off, tiles, mesh, regions, stale tiles; lit path)", Some(StateKey::Overlay)),
    k(&["F4"], "F4", View(Command::ToggleCoarseLod), V, Both, "coarse LOD (error bound x4)", Some(StateKey::CoarseLod)),
    k(&["F6"], "F6", View(Command::CycleGroundDetail), V, Both, "ground detail (natural, full, off; the procedural detail and gloss of the ground, on at the start)", Some(StateKey::GroundDetail)),
    k(&["F7"], "F7", View(Command::ToggleScaleRefs), V, Both, "scale references (drops a chair 0.9 m, a house 8 m, a boat 10 m, a tower 291 m and a block of boxes 1 km across on the ground in front of you; again takes them away; lit path)", Some(StateKey::ScaleRefs)),
    k(&["F9"], "F9", View(Command::SaveRepro), V, Both, "writes the camera path of the last minute and a repro bundle (target/review)", None),
    k(&["KeyT"], "T", View(Command::ToggleVsync), V, Both, "vsync on or off", Some(StateKey::Vsync)),
    k(&["Escape"], "Esc", View(Command::Quit), V, Viewer, "quits", None),
];

/// The viewer command a key asks for, by the name of its `KeyCode`.
pub fn command_for(code: &str) -> Option<Command> {
    KEYS.iter().find_map(|d| match d.action {
        Action::View(c) if d.codes.contains(&code) => Some(c),
        _ => None,
    })
}

/// The key of the editor mode a code asks for (the chords and `?` are the window's, by the character of the key).
pub fn edit_key_for(code: &str) -> Option<EditKey> {
    KEYS.iter().find_map(|d| match d.action {
        Action::Edit(e) if d.codes.contains(&code) => Some(e),
        _ => None,
    })
}

/// The held key a code is.
pub fn held_for(code: &str) -> Option<HeldSlot> {
    const SLOTS: [(&str, HeldSlot); 11] = [
        ("KeyW", HeldSlot::Forward),
        ("KeyS", HeldSlot::Back),
        ("KeyA", HeldSlot::Left),
        ("KeyD", HeldSlot::Right),
        ("KeyQ", HeldSlot::Down),
        ("KeyE", HeldSlot::Up),
        ("ArrowLeft", HeldSlot::LookLeft),
        ("ArrowRight", HeldSlot::LookRight),
        ("ArrowUp", HeldSlot::LookUp),
        ("ArrowDown", HeldSlot::LookDown),
        ("ShiftLeft", HeldSlot::Boost),
    ];
    let listed = KEYS.iter().any(|d| matches!(d.action, Action::Held(_)) && d.codes.contains(&code));
    if !listed {
        return None;
    }
    SLOTS.iter().find(|(n, _)| *n == code).map(|(_, s)| *s).or((code == "ShiftRight").then_some(HeldSlot::Boost))
}

/// The entries the window accepts with the editor `editor_on`, in table order.
pub fn accepted(editor_on: bool) -> impl Iterator<Item = &'static KeyDef> {
    KEYS.iter().filter(move |d| d.mode.accepts(editor_on))
}

/// The usage text and the F1 list for the viewer (`editor_on` false) or the editor mode: every accepted entry as `label help`.
pub fn help_text(editor_on: bool) -> String {
    let mut s = String::from(if editor_on { "editor mode: " } else { "" });
    let mut first = true;
    for d in accepted(editor_on) {
        if !first {
            s.push_str(", ");
        }
        first = false;
        s.push_str(d.label);
        s.push(' ');
        s.push_str(d.help);
    }
    s
}

/// The state of the viewer the rows show, as text (the window fills the one `SunTime` needs: it knows the camera).
pub fn state_text(key: StateKey, v: &crate::viewer::ViewerState, sun_time: Option<&str>) -> String {
    let on = |b: bool| if b { "on" } else { "off" }.to_string();
    match key {
        StateKey::Paused => if v.clock.is_paused() { "paused" } else { "running" }.to_string(),
        StateKey::Camera => if v.is_free() { "free camera" } else { "flight" }.to_string(),
        StateKey::Lit => if v.lit { "lit path" } else { "debug path" }.to_string(),
        StateKey::Vsync => on(v.vsync),
        StateKey::Overlay => v.overlay.name().to_string(),
        StateKey::CoarseLod => on(v.coarse_lod),
        StateKey::GroundDetail => v.ground_detail.name().to_string(),
        StateKey::ScaleRefs => on(v.scale_refs),
        StateKey::EditingLight => on(v.editing_light),
        StateKey::CoupledLens => match v.coupled_lens {
            None => "climate".to_string(),
            Some(planet_generators::coupled::CoupledLens::Height) => "run heights".to_string(),
            Some(planet_generators::coupled::CoupledLens::Area) => "drainage area".to_string(),
        },
        StateKey::ShadeView => v.shade_view.name().to_string(),
        StateKey::SunTime => match (sun_time, v.sun_offset_h) {
            (Some(t), o) if o != 0.0 => t.to_string(),
            _ => "the look's sun".to_string(),
        },
        StateKey::Globe => on(v.globe.is_some()),
    }
}

/// One row of the Keys panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    pub group: Group,
    pub label: &'static str,
    pub help: &'static str,
    /// The state beside the key, if it has one.
    pub state: Option<String>,
}

/// The rows the panel lists with the editor `editor_on`: exactly the entries the window accepts, in table order, with their state.
pub fn rows(editor_on: bool, v: &crate::viewer::ViewerState, sun_time: Option<&str>) -> Vec<KeyRow> {
    accepted(editor_on)
        .map(|d| KeyRow { group: d.group, label: d.label, help: d.help, state: d.state.map(|s| state_text(s, v, sun_time)) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command of the viewer: the match has no wildcard, so a new command stops compiling here until it is in the list (and so in the table).
    fn every_command() -> Vec<Command> {
        let all = [
            Command::TogglePause,
            Command::Restart,
            Command::NextView,
            Command::ToggleVsync,
            Command::ToggleCamera,
            Command::ToggleLit,
            Command::CycleOverlay,
            Command::ToggleCoarseLod,
            Command::CycleGroundDetail,
            Command::ToggleScaleRefs,
            Command::CycleCoupledLens,
            Command::ToggleEditingLight,
            Command::CycleShadeView,
            Command::SunEarlier,
            Command::SunLater,
            Command::SunReset,
            Command::ToggleGlobe,
            Command::SaveRepro,
            Command::Quit,
        ];
        for c in all {
            match c {
                Command::TogglePause
                | Command::Restart
                | Command::NextView
                | Command::ToggleVsync
                | Command::ToggleCamera
                | Command::ToggleLit
                | Command::CycleOverlay
                | Command::ToggleCoarseLod
                | Command::CycleGroundDetail
                | Command::ToggleScaleRefs
                | Command::CycleCoupledLens
                | Command::ToggleEditingLight
                | Command::CycleShadeView
                | Command::SunEarlier
                | Command::SunLater
                | Command::SunReset
                | Command::ToggleGlobe
                | Command::SaveRepro
                | Command::Quit => {}
            }
        }
        all.to_vec()
    }

    // spec: VIEW-055
    #[test]
    fn every_command_and_every_editor_key_has_an_entry_and_every_entry_is_found_by_its_own_lookup() {
        for c in every_command() {
            assert!(KEYS.iter().any(|d| d.action == Action::View(c)), "{c:?} has no entry in the key table");
        }
        for tool in 1..=11 {
            assert!(KEYS.iter().any(|d| d.action == Action::Edit(EditKey::Tool(tool))), "tool {tool}");
        }
        for e in [EditKey::Undo, EditKey::Redo, EditKey::Delete, EditKey::Escape, EditKey::Finish, EditKey::Back, EditKey::Help] {
            assert!(KEYS.iter().any(|d| d.action == Action::Edit(e)), "{e:?} has no entry in the key table");
        }
        for d in KEYS {
            assert!(!d.label.is_empty() && !d.help.is_empty(), "{d:?}");
            for code in d.codes {
                match d.action {
                    Action::View(c) => assert_eq!(command_for(code), Some(c), "{code} of {}", d.label),
                    Action::Edit(e) => assert_eq!(edit_key_for(code), Some(e), "{code} of {}", d.label),
                    Action::Held(_) => assert!(held_for(code).is_some(), "{code} of {}", d.label),
                    Action::Other => {}
                }
            }
            match d.action {
                // A held entry names its keys; a chord, the mouse and `?` have no code; the others have one at least.
                Action::Held(_) | Action::View(_) => assert!(!d.codes.is_empty(), "{}", d.label),
                _ => {}
            }
        }
        // A key code belongs to one action of a kind: no code is listed twice among the commands, the editor's keys or the held keys.
        for kind in 0..3 {
            let mut seen = std::collections::HashSet::new();
            for d in KEYS {
                let is_kind = matches!((kind, d.action), (0, Action::View(_)) | (1, Action::Edit(_)) | (2, Action::Held(_)));
                if is_kind {
                    for code in d.codes {
                        assert!(seen.insert(*code), "{code} is listed twice (kind {kind})");
                    }
                }
            }
        }
    }

    // spec: VIEW-055
    #[test]
    fn the_modes_list_exactly_what_they_accept_and_the_help_and_the_rows_come_from_the_same_entries() {
        let viewer: Vec<&str> = accepted(false).map(|d| d.label).collect();
        let editor: Vec<&str> = accepted(true).map(|d| d.label).collect();
        assert!(viewer.contains(&"Esc") && viewer.contains(&"click in the world") && !viewer.contains(&"Ctrl+Z") && !viewer.contains(&"1"));
        assert!(
            editor.contains(&"Ctrl+Z") && editor.contains(&"1") && editor.contains(&"F1 or ?") && editor.contains(&"right mouse button")
        );
        assert!(editor.contains(&"Esc"), "the editor's Esc steps back");
        assert_eq!(accepted(true).filter(|d| d.label == "Esc").count(), 1, "one Esc in the editor: it does not quit");
        assert_eq!(accepted(false).filter(|d| d.label == "Esc").count(), 1, "one Esc in the viewer: it quits");
        let v = crate::viewer::ViewerState::new(0, 3, 0, true);
        for editor_on in [false, true] {
            let rows = rows(editor_on, &v, None);
            assert_eq!(rows.len(), accepted(editor_on).count());
            let help = help_text(editor_on);
            for r in &rows {
                assert!(help.contains(r.label) && help.contains(r.help), "{} is in the help of the mode", r.label);
            }
        }
        // A row with a state shows it.
        let rows = rows(false, &v, None);
        let t = rows.iter().find(|r| r.label == "T").and_then(|r| r.state.clone());
        assert_eq!(t.as_deref(), Some("on"), "vsync is on at the start");
        let sun = rows.iter().find(|r| r.label == "[").and_then(|r| r.state.clone());
        assert_eq!(sun.as_deref(), Some("the look's sun"));
        let j = rows.iter().find(|r| r.label == "J").and_then(|r| r.state.clone());
        assert_eq!(j.as_deref(), Some("lit"));
    }
}
