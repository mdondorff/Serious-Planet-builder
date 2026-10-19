## Why
M6 step U1 (docs/briefings/m6-editor-plan.md): the editor needs an immediate-mode UI in its window before any panel (U3) or the globe view (U4) can exist. SPIKE-14 showed egui 0.36.2 fits the wgpu 30 and winit 0.30 pins, costs about 0.3 ms per frame on the RTX, and which input rule keeps panel and camera apart (`spikes/egui/results/results.md`). ADR 0015 already puts the painter in `render` and the UI logic in `editor` and `app`.

## What Changes
- `egui`, `egui-wgpu`, `egui-winit` `=0.36.2`, default features off (no bundled fonts: their licences are not on the CON-27 allow-list; `cargo deny` stays green).
- `planet_render::ui` (new): `UiFrame` (GPU-free: shapes, texture changes, pixels per point) and `UiPainter` (paint over the scene with `LoadOp::Load`, apply and clear the texture changes, an offscreen `compose` for tests); `graph::lit_frame_graph_with_ui` (node `ui` after `tonemap`); `WindowSurface::present_with_ui` and `ui_painter` (`present` is `present_with_ui(None)`).
- `planet::ui` (new): `Router` (egui first, pointer latched at the press, keys to a focused text field), `EditorUi` (egui context, `egui-winit` state, painter, pending texture changes, reclaim of an unpainted frame), the shell window.
- `window.rs`: the editor window draws the UI by default (`--no-ui` turns it off), gives egui every event first, gives egui no pointer event while the mouse is captured (VIEW-010), captures only on a press the world got, and keeps a key from the camera while a text field has the focus (a release still lets go of a held key); the smoke summary counts the UI passes.
- VIEW-011.

## Fonts
No font is shipped in this change: `ui::fonts()` returns empty definitions, so labels are blank (shapes, layout and input work). The owner's gate decision 14 is "fonts under an allowed licence": a font file (an Apache-2.0 release such as Roboto v2.138, the file whose bundled licence text says Apache-2.0) with its source URL, version, licence and SHA-256 goes in behind `ui::fonts()` in a follow-up of this change once the owner has approved the download; the choice is a one-file change.

## No golden moves
Only `planet editor` draws the UI; `test-render`, the golden tests and the perf scenes never reach `window.rs`. `present` without a UI is the blit alone, bit for bit; the golden tests run unchanged (tier C on the software adapter: no candidate golden appears).

## Classes and bounds (ADR 0035)
No field and no shader value is added: the UI is drawn by egui's own shader into the presentable target after the tone map.

## Out of scope
The panels (U3), the globe view (U4), picking (U2), a font, `ScaleFactorChanged`/IME/clipboard/accesskit checks (SPIKE-14 limits).
