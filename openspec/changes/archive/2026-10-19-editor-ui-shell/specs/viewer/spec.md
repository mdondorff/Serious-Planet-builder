## ADDED Requirements

### Requirement: VIEW-011 Editor UI shell
`planet editor` SHALL draw an immediate-mode UI (egui; `egui`, `egui-wgpu` and `egui-winit` pinned `=0.36.2` with default features off, SPIKE-14, ADR 0004, ADR 0015) over the scene in its window, and nothing else SHALL draw it (the headless modes, `test-render` and the golden paths never do, so no golden moves); `--no-ui` turns it off. The UI is one frame-graph node `ui` after `tonemap` that reads and writes the presentable target only, so no HDR, depth or histogram resource, lifetime or byte count of the lit frame changes (ADR 0042 point 2, node 8). The editor produces GPU-free egui output (`planet_render::UiFrame`: tessellated shapes, the texture changes since the last painted frame, pixels per point) and `render` paints it (`UiPainter`, loading the scene and blending over it); the texture changes of a pass that is not painted (the swap chain was out of date) are kept for the next one, and the painter clears the changes it applied. Pixels the UI does not cover SHALL equal the blit alone. Input: egui sees every window event first; a pointer press goes to the UI when egui wants the pointer (over a panel, a window or a popup) and to the world otherwise, the choice latched until the release, so a drag that starts in the world keeps turning the camera across a panel and a drag that starts on a slider never moves it; hover without a button is decided per motion; keys go to the UI while a text field has the focus (the camera and the commands get no press, and a release still lets go of a held key); while the mouse is captured (VIEW-010) egui gets no pointer event, and a click captures the mouse only when the world got the press. The one font is Roboto Regular v2.138 under Apache-2.0 (CON-27: egui's bundled fonts are OFL and Ubuntu-font licensed and stay off), with its provenance in `crates/app/assets/fonts/PROVENANCE.md`.

Verify: A, C
Status: active
Source: SPIKE-14, ADR 0015, ADR 0042 point 2, owner (M6 step U1; gate decision 14: fonts under an allowed licence), VIEW-010

#### Scenario: The node follows the tone map
- **WHEN** the lit frame graph gets the `ui` node at 2560x1440 without and with the atmosphere and the water
- **THEN** the order ends `tonemap`, `ui`, every other pass keeps its place, and the frame target bytes and lifetimes are those of the graph without it

#### Scenario: Only the UI's pixels change
- **WHEN** a rectangle is painted over a blitted frame at 1 and 2 pixels per point
- **THEN** every pixel clear of the rectangle's edge equals the blit alone and the inside is the rectangle's colour

#### Scenario: Routing
- **WHEN** a press lands over the shell and another outside it (at 1 and 1.5 pixels per point), a drag starts in the world and crosses the shell, the pointer only hovers, and a text field has the focus
- **THEN** the first goes to the UI and the second to the world, every motion of the drag goes to the world, hovering is decided by where the pointer is, and a key typed with the focus lands in the field and is not a camera key

#### Scenario: The window draws it
- **WHEN** `planet editor --smoke` runs on the software adapter, with and without `--no-ui`
- **THEN** the summary counts a UI pass for the frames it presented, and with `--no-ui` there is none
