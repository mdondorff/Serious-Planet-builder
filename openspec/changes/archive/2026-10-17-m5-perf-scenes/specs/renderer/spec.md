## ADDED Requirements

### Requirement: REND-028 Per-pass GPU timestamps
A timed frame (`TerrainRenderer::render(.., true)`) SHALL report the GPU time of every pass of the frame it ran in `FrameTiming::passes`: the terrain pass, the water pass, the atmosphere's sky-view and aerial-perspective products and its composite, the exposure histogram and adaptation passes and the tone map, each from its own pair of timestamp queries, plus the scene copy (the gap between the end of the terrain pass and the start of the water pass) and the frame's GPU span (first timestamp to last). A pass the frame did not run (no water, no atmosphere, fixed exposure culling the histogram) SHALL report nothing, a zero or negative interval SHALL report nothing instead of 0 ms, and an adapter without timestamp queries SHALL report nothing at all. The terrain and water passes SHALL keep their queries, so `gpu_ms` and `water_gpu_ms` equal the terrain and water entries. Timestamps SHALL change no pixel, no draw and no pipeline, and the passes SHALL not be timed on a frame that is not timed (`render(.., false)`, `submit`). The static tables' upload is a queue write on a body's load and no pass of a steady frame, so it has no entry.

Verify: A, C
Status: active
Source: M5 item 8 (status.md), ADR 0042 point 2, ADR 0034, TEST-012, REND-017

#### Scenario: Every pass of a lit frame
- **WHEN** a timed lit frame with the atmosphere, water and adaptive exposure is drawn on an adapter with timestamps
- **THEN** all eight passes and the scene copy have a time above zero, the span is at least the longest pass, `gpu_ms` is the terrain entry and `water_gpu_ms` the water entry, and the pixels equal those of an untimed frame

#### Scenario: Passes that did not run
- **WHEN** a debug frame, a lit frame without the atmosphere and with fixed exposure, and a lit frame without water are timed
- **THEN** the debug frame has the terrain pass alone, the second has no sky-view, composite or histogram entry but the adaptation and the tone map, and the third has no water entry and no scene copy

#### Scenario: From ticks
- **WHEN** resolved ticks of passes 2 ns apart are converted, with a pass of zero interval and a water pass starting before the terrain pass ended
- **THEN** the times are the ticks times the period, the scene copy is the gap, the span runs from the first to the last timestamp, and the zero interval and the negative gap report nothing
