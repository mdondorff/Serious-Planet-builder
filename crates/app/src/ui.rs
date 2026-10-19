//! The editor UI shell (VIEW-011, M6 step U1): egui in the editor window. The egui output is GPU-free data (`planet_render::UiFrame`,
//! ADR 0015) painted by `render` after the tone mapping; this module owns the egui context and the `egui-winit` state, the input
//! router that keeps the panels and the camera apart, and the shell panel. The window is thin glue around it.
//!
//! Input (SPIKE-14): egui sees every window event first; a pointer press goes to the UI when egui wants the pointer (over a panel, a
//! window or a popup) and to the world otherwise, and the choice is latched at the press until the release, so a drag that starts in
//! the world and crosses a panel keeps turning the camera and a drag that starts on a slider never moves it. Keys go to the UI while a
//! text field has focus.

use planet_render::{UiFrame, UiPainter};

/// Who gets an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Ui,
    World,
}

/// The routing rule between the panels and the camera.
#[derive(Default)]
pub struct Router {
    latched: Option<Target>,
}

fn over_ui(ctx: &egui::Context) -> Target {
    if ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui() {
        Target::Ui
    } else {
        Target::World
    }
}

impl Router {
    /// A pointer press: decided now and kept until the release.
    pub fn pointer_press(&mut self, ctx: &egui::Context) -> Target {
        let t = over_ui(ctx);
        self.latched = Some(t);
        t
    }

    /// Pointer motion: the latched target while a button is down, else decided by where the pointer is.
    pub fn pointer_motion(&self, ctx: &egui::Context) -> Target {
        self.latched.unwrap_or_else(|| over_ui(ctx))
    }

    /// The release of the button: the target that was latched at the press, if there was one.
    pub fn pointer_release(&mut self) -> Option<Target> {
        self.latched.take()
    }

    /// A key: to the UI while a text field has the focus.
    pub fn key(&self, ctx: &egui::Context) -> Target {
        if ctx.egui_wants_keyboard_input() {
            Target::Ui
        } else {
            Target::World
        }
    }
}

/// Roboto Regular v2.138 (Apache-2.0), the one font of the UI. Provenance (CON-27): `assets/fonts/PROVENANCE.md`.
const ROBOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

/// The fonts of the UI: Roboto Regular for both families (egui's own bundled fonts are OFL and Ubuntu-font licensed, neither on the
/// CON-27 allow-list, so they are not enabled).
pub fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert("Roboto-Regular".into(), std::sync::Arc::new(egui::FontData::from_static(ROBOTO_REGULAR)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("Roboto-Regular".into());
    }
    fonts
}

/// State of the shell panel.
pub struct Shell {
    /// The screen rectangle the panel covered in the last pass, in points.
    pub panel_rect: egui::Rect,
    /// The line under the title: the active debug overlay and LOD view, e.g. `Overlay: Mesh, LOD x4` (VIEW-014).
    pub status: String,
}

impl Default for Shell {
    fn default() -> Self {
        Shell { panel_rect: egui::Rect::NOTHING, status: String::new() }
    }
}

/// The shell: a small floating window in the corner of the editor. The panels of M6 (U3) take its place; for now it proves the pass, the
/// input routing and the pixels-per-point handling.
pub fn shell(ui: &mut egui::Ui, st: &mut Shell) {
    let r = egui::Window::new("Planet editor").default_pos(egui::pos2(12.0, 12.0)).resizable(false).show(ui.ctx(), |ui| {
        ui.label("Planet editor");
        if !st.status.is_empty() {
            ui.label(&st.status);
        }
    });
    st.panel_rect = r.map_or(egui::Rect::NOTHING, |r| r.response.rect);
}

/// The markers of the globe overview for one frame: where they are on the screen, the one under the cursor, and the label of those that
/// have one (the edits are labelled by name, the others by their number).
pub struct MarkerLayer<'a> {
    pub markers: &'a [planet_frame::globe::ScreenMarker],
    pub hover: Option<u64>,
    pub labels: &'a std::collections::HashMap<u64, String>,
}

/// The item markers of the globe overview (VIEW-013), drawn over the scene: a dot at each pixel (window pixels, origin top-left) with the
/// item's id beside it, the one under the cursor larger and brighter.
pub fn draw_markers(ui: &mut egui::Ui, layer: &MarkerLayer) {
    let ppp = ui.ctx().pixels_per_point();
    let painter = ui.painter();
    let hover = layer.hover;
    for m in layer.markers {
        let at = egui::pos2(m.pixel[0] as f32 / ppp, m.pixel[1] as f32 / ppp);
        let hot = hover == Some(m.id);
        let (radius, fill) = if hot { (8.0, egui::Color32::from_rgb(255, 220, 60)) } else { (5.0, egui::Color32::from_rgb(255, 120, 40)) };
        painter.circle(at, radius, fill, egui::Stroke::new(1.5, egui::Color32::BLACK));
        painter.text(
            at + egui::vec2(radius + 3.0, -radius - 3.0),
            egui::Align2::LEFT_BOTTOM,
            layer.labels.get(&m.id).cloned().unwrap_or_else(|| format!("#{}", m.id)),
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );
    }
}

/// The egui side of the editor window.
pub struct EditorUi {
    pub ctx: egui::Context,
    winit: egui_winit::State,
    painter: UiPainter,
    /// Texture changes of passes that were not painted: they are applied with the next painted pass (the font atlas arrives with the first).
    pending: egui::TexturesDelta,
    pub router: Router,
    pub shell: Shell,
    /// Passes run, and the CPU time of their egui run and tessellation, in milliseconds.
    pub frames: u64,
    pub cpu_ms: f64,
    /// Pointer events given to the world and to the UI (the router's choices, for the smoke summary).
    pub world_pointer: u64,
    pub ui_pointer: u64,
}

impl EditorUi {
    pub fn new(window: &winit::window::Window, painter: UiPainter) -> EditorUi {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let winit = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, Some(window.scale_factor() as f32), None, None);
        EditorUi {
            ctx,
            winit,
            painter,
            pending: egui::TexturesDelta::default(),
            router: Router::default(),
            shell: Shell::default(),
            frames: 0,
            cpu_ms: 0.0,
            world_pointer: 0,
            ui_pointer: 0,
        }
    }

    /// Hand a window event to egui. Returns what the camera may do with it: `true` when the world gets it (egui did not consume it
    /// and the router says world). A key is the world's unless egui consumed it or a text field has the focus.
    pub fn window_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) -> bool {
        use winit::event::{ElementState, WindowEvent};
        let resp = self.winit.on_window_event(window, event);
        match event {
            WindowEvent::MouseInput { state, .. } => {
                let t = if *state == ElementState::Pressed {
                    self.router.pointer_press(&self.ctx)
                } else {
                    self.router.pointer_release().unwrap_or(Target::World)
                };
                let world = t == Target::World && !resp.consumed;
                if world {
                    self.world_pointer += 1;
                } else {
                    self.ui_pointer += 1;
                }
                world
            }
            WindowEvent::CursorMoved { .. } => {
                let world = self.router.pointer_motion(&self.ctx) == Target::World && !resp.consumed;
                if world {
                    self.world_pointer += 1;
                } else {
                    self.ui_pointer += 1;
                }
                world
            }
            WindowEvent::KeyboardInput { .. } => !(resp.consumed || self.router.key(&self.ctx) == Target::Ui),
            _ => true,
        }
    }

    /// Run one egui pass and tessellate it: the frame to paint after the scene.
    pub fn frame(
        &mut self,
        window: &winit::window::Window,
        status: &str,
        markers: &MarkerLayer,
        extra: Option<&mut dyn FnMut(&mut egui::Ui)>,
    ) -> UiFrame {
        let started = std::time::Instant::now();
        self.shell.status.clear();
        self.shell.status.push_str(status);
        let raw = self.winit.take_egui_input(window);
        let shell_state = &mut self.shell;
        // The editor mode (VIEW-022) draws its panels in place of the shell window.
        let mut extra = extra;
        let out = self.ctx.run_ui(raw, |ui| {
            match extra.as_mut() {
                Some(draw) => draw(ui),
                None => shell(ui, shell_state),
            }
            draw_markers(ui, markers);
        });
        self.winit.handle_platform_output(window, out.platform_output);
        self.pending.append(out.textures_delta);
        let frame = UiFrame {
            primitives: self.ctx.tessellate(out.shapes, out.pixels_per_point),
            textures_delta: std::mem::take(&mut self.pending),
            pixels_per_point: out.pixels_per_point,
        };
        self.cpu_ms += started.elapsed().as_secs_f64() * 1000.0;
        self.frames += 1;
        frame
    }

    /// The painter, to paint a frame with.
    pub fn painter(&mut self) -> &mut UiPainter {
        &mut self.painter
    }

    /// A frame that was not painted (the swap chain was out of date): keep its texture changes for the next one.
    pub fn reclaim(&mut self, mut frame: UiFrame) {
        self.pending.append(std::mem::take(&mut frame.textures_delta));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, Pos2};

    const SIZE: (u32, u32) = (1920, 1080);

    /// One egui pass of the shell over `events` at `size_px` and `ppp` pixels per point, the way the window runs it.
    fn pass(ctx: &egui::Context, st: &mut Shell, ppp: f32, events: Vec<Event>, time_s: f64) {
        ctx.set_pixels_per_point(ppp);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE.0 as f32 / ppp, SIZE.1 as f32 / ppp))),
            time: Some(time_s),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| shell(ui, st));
        out.textures_delta.clear();
    }

    fn moved(p: Pos2) -> Event {
        Event::PointerMoved(p)
    }

    fn button(p: Pos2, down: bool) -> Event {
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }

    /// Settle the layout (a window needs two passes to know its size).
    fn settled(ctx: &egui::Context, st: &mut Shell, ppp: f32) {
        for i in 0..3 {
            pass(ctx, st, ppp, vec![], f64::from(i) * 0.016);
        }
    }

    // spec: VIEW-011
    #[test]
    fn a_press_over_the_shell_goes_to_the_ui_and_one_outside_goes_to_the_world_at_both_scales() {
        for ppp in [1.0f32, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_fonts(fonts());
            let mut st = Shell::default();
            settled(&ctx, &mut st, ppp);
            assert!(st.panel_rect.width() > 20.0 && st.panel_rect.max.x < 1920.0 / ppp, "shell rect {:?}", st.panel_rect);
            let inside = st.panel_rect.center();
            let outside = Pos2::new(st.panel_rect.max.x + 300.0, st.panel_rect.max.y + 300.0);
            let mut router = Router::default();
            pass(&ctx, &mut st, ppp, vec![moved(inside)], 0.1);
            assert_eq!(router.pointer_press(&ctx), Target::Ui, "ppp {ppp}");
            pass(&ctx, &mut st, ppp, vec![button(inside, true)], 0.12);
            assert_eq!(router.pointer_release(), Some(Target::Ui));
            pass(&ctx, &mut st, ppp, vec![button(inside, false), moved(outside)], 0.14);
            assert_eq!(router.pointer_press(&ctx), Target::World, "ppp {ppp}");
            assert_eq!(router.pointer_release(), Some(Target::World));
        }
    }

    // spec: VIEW-011
    #[test]
    fn a_drag_that_starts_in_the_world_keeps_going_to_the_world_across_the_shell() {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let mut st = Shell::default();
        settled(&ctx, &mut st, 1.0);
        let start = Pos2::new(st.panel_rect.max.x + 200.0, st.panel_rect.center().y);
        let mut router = Router::default();
        pass(&ctx, &mut st, 1.0, vec![moved(start)], 0.1);
        assert_eq!(router.pointer_press(&ctx), Target::World);
        pass(&ctx, &mut st, 1.0, vec![button(start, true)], 0.12);
        let mut world_moves = 0;
        for k in 1..=10 {
            // Across the shell and out of its far side.
            let p = Pos2::new(start.x - 60.0 * k as f32, start.y);
            pass(&ctx, &mut st, 1.0, vec![moved(p)], 0.12 + 0.02 * k as f64);
            if router.pointer_motion(&ctx) == Target::World {
                world_moves += 1;
            }
        }
        assert_eq!(world_moves, 10, "every motion of the drag goes to the camera");
        pass(&ctx, &mut st, 1.0, vec![button(Pos2::new(10.0, 10.0), false)], 0.5);
        assert_eq!(router.pointer_release(), Some(Target::World));
    }

    // spec: VIEW-011
    #[test]
    fn hovering_decides_the_target_without_a_press_and_keys_are_the_worlds_without_a_text_focus() {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let mut st = Shell::default();
        settled(&ctx, &mut st, 1.0);
        let router = Router::default();
        let (over, away) = (st.panel_rect.center(), Pos2::new(st.panel_rect.max.x + 300.0, 500.0));
        pass(&ctx, &mut st, 1.0, vec![moved(over)], 0.1);
        assert_eq!(router.pointer_motion(&ctx), Target::Ui);
        pass(&ctx, &mut st, 1.0, vec![moved(away)], 0.12);
        assert_eq!(router.pointer_motion(&ctx), Target::World);
        assert_eq!(router.key(&ctx), Target::World, "no text field has the focus");
    }

    // spec: VIEW-011
    #[test]
    fn the_shipped_font_gives_labels_a_width() {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let mut width = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            width = ui.label("Planet editor").rect.width();
        });
        out.textures_delta.clear();
        assert!(width > 40.0, "a 13-character label in Roboto is wider than 40 points: {width}");
    }

    // spec: VIEW-011
    #[test]
    fn a_text_field_with_the_focus_takes_the_keys() {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let mut text = String::new();
        let mut run = |events: Vec<Event>, t: f64| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
                time: Some(t),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                ui.text_edit_singleline(&mut text);
            });
            out.textures_delta.clear();
        };
        let router = Router::default();
        run(vec![], 0.0);
        assert_eq!(router.key(&ctx), Target::World);
        // Click into the field (it is at the top left of the screen; without a font its height is not known, so scan the rows).
        let mut y = 1.0;
        while y < 60.0 && router.key(&ctx) == Target::World {
            let p = Pos2::new(20.0, y);
            run(vec![moved(p), button(p, true)], 0.1);
            run(vec![button(p, false)], 0.12);
            y += 3.0;
        }
        assert_eq!(router.key(&ctx), Target::Ui, "a click on the text field focuses it");
        run(vec![Event::Text("w".into())], 0.2);
        assert_eq!(text, "w", "the key typed into the field is not a camera key");
    }
}
