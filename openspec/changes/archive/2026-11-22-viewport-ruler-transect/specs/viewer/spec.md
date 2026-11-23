## ADDED Requirements

### Requirement: VIEW-076 Viewport ruler, elevation transect, and cursor readout
`planet_editor::ruler` SHALL compute a float64 (`libm`) cursor readout (`lat/lon`, height, pole-safe finite-difference slope) and a great-circle two-point `TransectProfile` (surface arc distance, 3D chord distance, elevation delta, net grade, sampled height/slope profile, and intervals exceeding the 45-degree heightfield limit of ADR 0091), plus `faces_camera`-culled 3D `RulerOverlay` projections. `Tool::Ruler` SHALL measure on the planet without altering `WorldDefinition::hash()`. `HoverPicker` SHALL evaluate cursor height and local slope off the main thread, while the status bar, ruler inspector section, interactive inline cross-section/slope plot (marking the 45-degree limit with hover scrubbing), and 3D viewport overlay present measurements without blocking rendering or modifying terrain pixels.

Verify: A
Status: active
Source: companion backlog 1A (2026-11-22), ADR 0091, CON-05, CON-11, CON-14, CON-22

#### Scenario: Cursor readout and steep transect across cliff
- **WHEN** cursor readouts and two-point transects run over flat terrain, a pole, a 45-degree slope, and a steeper scarp in `planet_editor` and `EditRuntime`
- **THEN** distances, elevation deltas, slopes, and `over_limit` flags match analytical terrain geometry without mutating `WorldDefinition::hash()`

#### Scenario: Headless panel rig ruler controls and plot scrubbing
- **WHEN** the Ruler tool button is clicked, steep cursor/transect data are rendered, and the inline plot, swap, and clear controls are exercised
- **THEN** cursor readout, steep warning, plot hover scrub, `SwapRuler`, and `ClearRuler` events work within fixed inspector width
