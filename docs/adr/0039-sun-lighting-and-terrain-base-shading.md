# 0039 Sun lighting and terrain base shading
Status: proposed (sketch by opus-planner, 2026-10-15, for M5, revised after the spec-guardian review; decided under the owner's delegation of 2026-10-12 once the M5 queue starts)
Date: 2026-10-15
Source: architecture report M5 row (ground, limb and sunset goldens need a lit surface); `docs/status.md` M5 section; ADR 0026 (bake channels; point 5, reading the bake, still open); ADR 0033 (compact tile); ADR 0035, ADR 0038, ADR 0040, ADR 0041; constitution CON-02, CON-07, CON-11, CON-14

## Context
Terrain is drawn only in debug views. Atmosphere goldens of the ground, the limb and a sunset are meaningless over a debug colour, so M5 needs a minimal lit surface: a sun, a surface colour and the atmosphere's light on it. Materials, texturing and ground cover come later.

## Decision (proposed)
1. **Sun:** one directional light; direction from a time-of-day value in the `FramePlan` (set by the view: testkit views, perf scenes, the viewer's flight; CON-04), illuminance 1 in the units of ADR 0038 point 1. The light reaching a surface point is attenuated by the transmittance LUT (ADR 0040).
2. **Climate per tile (a field):** temperature and precipitation reach each tile as a small per-tile climate texture (9x9 texels, two `f16` channels, 324 B) built on the CPU from the bake. It is stored and keyed with the tile, so it is a **field** (ADR 0035 point 1): its own implementation id, a key that includes `GLOBAL_VERSION`, `libm`, known answers, CON-14's bound (abs 0.01 °C and 0.1 mm). **Reading the bake:** ADR 0027 (accepted; it decides ADR 0026 point 5: integer lattice position, face-edge-symmetric interpolation, halo hold, land channel) for the per-tile texture, which is a tile-array consumer, and its `READER_VERSION` folds into this field's version. The texture is part of the compact tile (MESH_VERSION bump; ADR 0033's headroom pins re-checked with the new bytes).
3. **Surface colour (presentation data):** albedo from the climate, slope and the water level: rock by slope, snow by temperature and slope, vegetation and desert tints by temperature and precipitation, a beach band by height above **W** (ADR 0041), never a hard-coded sea level. Colour tables are look data (ADR 0042 point 6). Declared bound: the GPU result matches an f64 evaluation of the same function at the same inputs within 1/255 per channel on probe points (tier B); the reference lives in a GPU-free module (`frame::look`, CON-02).
4. **Shading:** Lambert diffuse with the stored shading normal (ADR 0033) plus sky ambient from the camera-independent multiple-scattering and transmittance LUTs (ADR 0040) at the point's altitude and the sun's zenith angle; not the sky-view LUT, which is valid only near the camera. **No terrain shadows and no specular on land in M5** (sunset goldens are read with that in mind).
5. **Debug views:** albedo only, lighting only, climate channels.
6. **Time box and fallback:** three days; fallback: albedo from height and slope only (no climate field), reported.

## Consequences
- The first view that looks like a planet; look tables are data and reviewed at the gate (ADR 0037).
- A new field with keys and known answers, and a compact-tile format change (MESH_VERSION, goldens through `golden-review`).
- Snow and sand as layers come with the post-M5 surface-materials work.

## Licences of adopted code or techniques
None (Lambert shading; albedo values from public measurement tables, cited where used).
