# Viewport ruler, elevation transect, and cursor readout (`1A`)

Adds the `3D Viewport Ruler, Slope & Elevation Transect Tool` (`VIEW-076`, companion backlog item `1A`) strictly inside cold-zone crates (`crates/editor`, `crates/app`).

- `planet_editor::ruler`: GPU-free `f64` (`libm`) cursor coordinate/height/finite-difference slope readout, `RulerState` two-point workflow, great-circle `TransectProfile` sampler with 45-degree `ADR 0091` steepness flagging, and `faces_camera`-culled `RulerOverlay` projection.
- `crates/app`: off-main-thread `HoverPicker` `HoverSample` readout (`CON-22`), `Tool::Ruler` rail button, status-bar cursor readout, inspector metrics & interactive inline elevation/slope cross-section plot with chart-to-3D hover scrubbing, `Swap A<->B` / `Clear` controls, and 3D viewport overlay rendering (`draw_ruler`). Zero `WorldDefinition::hash()`, shader, or `planet_frame::keytable` changes (`Tier 1`).
