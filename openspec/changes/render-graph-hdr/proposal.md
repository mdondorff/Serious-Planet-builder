## Why
M5 item 2 (ADR 0042 points 1-3 and 5, ADR 0038): the lit path. Terrain is drawn into a linear `Rgba16Float` scene target with a pre-exposure, an exposure comes from a luminance histogram (or is fixed for goldens), and an ACES-fit tone map with an sRGB encode in the shader writes the presentable target. The render graph becomes product code for this frame shape, with compile tests, and the frame targets are a count-based share asserted from the compiled graph (ADR 0048, an amendment to ADR 0033's remainder).

## What Changes
- `frame::look` (GPU-free, f64/libm): exposure units, the histogram and its 40-90 % band, the clamp and the adaptation step, the ACES fit, the sRGB encode, the tone-map reference pixel, the lit terrain radiance, the pre-exposure factor, `LitPlan` (exposure mode, `dt`, sun, reset, tone-map mode) with its text form.
- `TerrainPlan` gains `lit: Option<LitPlan>`; the plan text gains one `lit` line only for a lit plan, so every debug-path plan keeps its text and hash.
- `render::graph`: sized resources and persistent resources in the compile, the lit frame shape `lit_frame_graph` (terrain, exposure histogram, exposure adaptation, tone map; fixed exposure culls the histogram), the frame-target share `frame_target_share_bytes` and the compiled graph's byte counts.
- `render::lit`: `LitPath` (scene target, histogram and adaptation compute, tone map) executing the compiled graph and recompiling only when the frame shape changes; `TerrainRenderer::enable_lit` makes the HDR terrain pipelines (vertex and compact formats, entry `fs_lit`); a plan with a `LitPlan` draws through them, a plan without draws as before.
- Debug path: unchanged (no existing golden changes). Viewer window: unchanged (it blits the colour target, which the tone map also writes).
- ADR 0048 (proposed): the share, the units, the CPU-decided pre-exposure, the histogram definition, the placeholder shading, and the golden classes.

## Classes and bounds (ADR 0035)
No field is added. Every attribute below is presentation data:
- scene target value: finite, `0 <= v <= 60000`, f16 storage (REND-026).
- lit terrain radiance: within `max(1e-4 |v|, 1e-5 x factor)` of the f64 reference per channel (REND-025).
- exposure histogram: bins equal to the f64 reference away from bin edges (REND-022).
- adapted EV100: within 1e-4 EV of the CPU step (REND-022).
- tone map: within 1/255 per channel of the f64 reference (REND-023).
- pre-exposure factor: decided in f64 on the CPU, rounded once to f32 (REND-021).
No shader uses a planet-sized value (the shader scan covers the new modules), so CON-11's temporal-stability scenario does not apply to this change.

## Out of scope
The sun as a field of the plan and the climate albedo (item 4), the atmosphere and sky passes (item 5), water (item 7), per-pass GPU timestamps and the lit-path perf scenes (item 8), the histogram overlay debug view (the false-colour EV bands are in), the look file (item 3). No perf window is needed or run.

Expected golden changes:
- lit_orbit_ev15: new (first golden): the lit globe at a fixed EV100 15, ACES and sRGB.
- lit_aerial20km_ev15: new (first golden): the lit aerial view at a fixed EV100 15.
- lit_ground1m_ev15: new (first golden): the lit ground view at 1 m at a fixed EV100 15.
- lit_orbit_ev_bands: new (first golden): the false-colour EV-band debug view of the orbit frame.
No existing golden changes.
