//! SPIKE-14 ONLY (never merged): egui in the frame. A typical editor panel (sliders, a list of 100 items, a text field), the egui
//! output as GPU-free data (ADR 0015), and the input-routing rule between the panel and the camera.

use planet_render::UiFrame;

pub const LIST_ITEMS: usize = 100;

/// State of the typical panel.
pub struct PanelState {
    pub sliders: [f32; 5],
    pub checked: bool,
    pub choice: usize,
    pub selected: Option<usize>,
    pub text: String,
    /// The screen rectangle the panel covered in the last pass, in points.
    pub panel_rect: egui::Rect,
}

impl Default for PanelState {
    fn default() -> Self {
        PanelState { sliders: [0.2, 0.4, 0.6, 0.8, 1.0], checked: true, choice: 0, selected: None, text: String::new(), panel_rect: egui::Rect::NOTHING }
    }
}

/// A side panel with five sliders, a checkbox, a combo box, a text field and a scrolling list of 100 selectable items.
pub fn panel(ui: &mut egui::Ui, st: &mut PanelState) {
    let r = egui::Panel::left("tools").default_size(300.0).show(ui, |ui| {
        ui.heading("Terrain tools");
        for (k, v) in st.sliders.iter_mut().enumerate() {
            ui.add(egui::Slider::new(v, 0.0..=1.0).text(format!("parameter {k}")));
        }
        ui.checkbox(&mut st.checked, "snap to grid");
        egui::ComboBox::from_label("brush").selected_text(format!("brush {}", st.choice)).show_ui(ui, |ui| {
            for k in 0..4 {
                ui.selectable_value(&mut st.choice, k, format!("brush {k}"));
            }
        });
        ui.text_edit_singleline(&mut st.text);
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for k in 0..LIST_ITEMS {
                if ui.selectable_label(st.selected == Some(k), format!("stamp {k:03}")).clicked() {
                    st.selected = Some(k);
                }
            }
        });
    });
    st.panel_rect = r.response.rect;
}

/// One egui pass over `events` at `size_px` and `ppp` (pixels per point): the frame to paint and whether egui wants the pointer.
pub fn run_pass(ctx: &egui::Context, st: &mut PanelState, size_px: (u32, u32), ppp: f32, events: Vec<egui::Event>, time_s: f64) -> UiFrame {
    ctx.set_pixels_per_point(ppp);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size_px.0 as f32 / ppp, size_px.1 as f32 / ppp))),
        time: Some(time_s),
        events,
        ..Default::default()
    };
    let out = ctx.run_ui(input, |ui| panel(ui, st));
    let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
    UiFrame { primitives, textures_delta: out.textures_delta, pixels_per_point: out.pixels_per_point }
}

/// Who gets an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Ui,
    World,
}

/// The routing rule: a pointer press goes to the UI when egui wants the pointer (over a panel, a window or a popup) and to the
/// world otherwise; the choice is latched at the press and holds until the release, so a drag that starts in the world and crosses
/// the panel keeps turning the camera, and a drag that starts on a slider never moves it. Keys go to the UI while a text field
/// has focus.
#[derive(Default)]
pub struct Router {
    latched: Option<Target>,
}

impl Router {
    pub fn pointer_press(&mut self, ctx: &egui::Context) -> Target {
        let t = if ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui() { Target::Ui } else { Target::World };
        self.latched = Some(t);
        t
    }

    pub fn pointer_motion(&self, ctx: &egui::Context) -> Target {
        self.latched.unwrap_or(if ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui() { Target::Ui } else { Target::World })
    }

    pub fn pointer_release(&mut self) -> Option<Target> {
        self.latched.take()
    }

    pub fn key(&self, ctx: &egui::Context) -> Target {
        if ctx.egui_wants_keyboard_input() {
            Target::Ui
        } else {
            Target::World
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, Pos2};

    const SIZE: (u32, u32) = (1920, 1080);

    fn moved(p: Pos2) -> Event {
        Event::PointerMoved(p)
    }

    fn button(p: Pos2, down: bool) -> Event {
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }

    /// Settle the layout (a panel needs two passes to know its size) and return the panel rectangle in points.
    fn settled(ctx: &egui::Context, st: &mut PanelState, ppp: f32) {
        for i in 0..3 {
            let _ = run_pass(ctx, st, SIZE, ppp, vec![], f64::from(i) * 0.016);
        }
    }

    // A click on the panel never reaches the camera; the slider under it changes.
    #[test]
    fn a_click_on_the_panel_goes_to_the_ui_and_moves_no_camera() {
        for ppp in [1.0f32, 1.5] {
            let ctx = egui::Context::default();
            let mut st = PanelState::default();
            settled(&ctx, &mut st, ppp);
            assert!(st.panel_rect.width() > 100.0 && st.panel_rect.max.x < 1920.0 / ppp, "panel rect {:?}", st.panel_rect);
            let inside = Pos2::new(st.panel_rect.min.x + 40.0, st.panel_rect.min.y + 70.0);
            let before = st.sliders;
            let mut router = Router::default();
            let _ = run_pass(&ctx, &mut st, SIZE, ppp, vec![moved(inside)], 0.1);
            assert_eq!(router.pointer_press(&ctx), Target::Ui, "ppp {ppp}");
            let _ = run_pass(&ctx, &mut st, SIZE, ppp, vec![button(inside, true)], 0.12);
            let drag = Pos2::new(inside.x + 120.0, inside.y);
            let _ = run_pass(&ctx, &mut st, SIZE, ppp, vec![moved(drag)], 0.14);
            assert_eq!(router.pointer_motion(&ctx), Target::Ui);
            let _ = run_pass(&ctx, &mut st, SIZE, ppp, vec![button(drag, false)], 0.16);
            assert_eq!(router.pointer_release(), Some(Target::Ui));
            assert_ne!(before, st.sliders, "a drag over a slider changes its value (ppp {ppp})");
        }
    }

    // Mouse look outside the panel works, also when the drag crosses the panel; the sliders do not move.
    #[test]
    fn a_drag_that_starts_in_the_world_keeps_turning_the_camera_across_the_panel() {
        let ctx = egui::Context::default();
        let mut st = PanelState::default();
        settled(&ctx, &mut st, 1.0);
        let outside = Pos2::new(st.panel_rect.max.x + 200.0, 400.0);
        let before = st.sliders;
        let mut router = Router::default();
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![moved(outside)], 0.1);
        assert_eq!(router.pointer_press(&ctx), Target::World);
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![button(outside, true)], 0.12);
        let mut world_moves = 0;
        for k in 1..=10 {
            // Across the panel and out of its far side.
            let p = Pos2::new(outside.x - 60.0 * k as f32, 400.0);
            let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![moved(p)], 0.12 + 0.02 * k as f64);
            if router.pointer_motion(&ctx) == Target::World {
                world_moves += 1;
            }
        }
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![button(Pos2::new(10.0, 400.0), false)], 0.5);
        assert_eq!(world_moves, 10, "every motion of the drag goes to the camera");
        assert_eq!(router.pointer_release(), Some(Target::World));
        assert_eq!(before, st.sliders, "the sliders under the crossed panel did not move");
    }

    // Motion with no button held: outside the panel it is the world's (hover look), over the panel it is the UI's.
    #[test]
    fn hovering_decides_the_target_without_a_press() {
        let ctx = egui::Context::default();
        let mut st = PanelState::default();
        settled(&ctx, &mut st, 1.0);
        let router = Router::default();
        let over = Pos2::new(st.panel_rect.min.x + 30.0, 200.0);
        let away = Pos2::new(st.panel_rect.max.x + 300.0, 300.0);
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![moved(over)], 0.1);
        assert_eq!(router.pointer_motion(&ctx), Target::Ui);
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![moved(away)], 0.12);
        assert_eq!(router.pointer_motion(&ctx), Target::World);
    }

    // The keyboard goes to the UI while the text field has focus, to the camera otherwise.
    #[test]
    fn keys_go_to_the_text_field_while_it_has_focus() {
        let ctx = egui::Context::default();
        let mut st = PanelState::default();
        settled(&ctx, &mut st, 1.0);
        let router = Router::default();
        assert_eq!(router.key(&ctx), Target::World);
        // Click into the text field (found by scanning the panel's rows for the one that takes keyboard focus).
        let mut focused = false;
        let mut y = st.panel_rect.min.y + 20.0;
        while y < st.panel_rect.min.y + 300.0 && !focused {
            let p = Pos2::new(st.panel_rect.min.x + 60.0, y);
            let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![moved(p), button(p, true)], 0.1);
            let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![button(p, false)], 0.12);
            focused = ctx.egui_wants_keyboard_input();
            y += 6.0;
        }
        assert!(focused, "a click on the text field focuses it");
        assert_eq!(router.key(&ctx), Target::Ui);
        let _ = run_pass(&ctx, &mut st, SIZE, 1.0, vec![Event::Text("w".into())], 0.2);
        assert_eq!(st.text, "w", "the key typed into the field is not a camera key");
    }
}
