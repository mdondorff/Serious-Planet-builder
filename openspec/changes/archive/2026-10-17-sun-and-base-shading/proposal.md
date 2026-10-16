## Why
M5 item 4 (ADR 0039): the lit path draws a placeholder (Lambert on the relief's height tint, a fixed sun). The atmosphere goldens of M5 (ground, limb, sunset) need a surface colour that comes from the world: the climate of the tile, the slope and the height above the water. This change gives the lit terrain its base shading, and the climate per tile as a field.

## What Changes
- **The climate field (GEN-021, ADR 0039 point 2):** every tile carries temperature and precipitation at 9 x 9 points, two f16 each, 324 bytes: read from the global bake with the integer reader of ADR 0027 (`read_tile_sample`, 8-cell grid), f16 rounding as the field's definition, analytic fallback for fields without a bake. Own implementation id and version; the version word (climate, reader and bake versions) is part of every mesh key and persistent mesh key. Compact tiles keep it in the spare fourth channel of their first 162 texels (so a compact tile is still 8,840 B; it needs at least 16 cells); vertex tiles carry a 324-byte block and the GPU vertex becomes 48 bytes (44 + the vertex's climate as two f16).
- **MESH_VERSION 6 to 7.** Persistent tile payload version 2. Tiles stored under version 6 are unreachable (their keys differ); a pack whose coarse levels were pre-filled (STRM-021) holds only stale records: every lookup is a miss and `generate --prefill-dir` (or `--global-dir`) writes the tiles again. Plan snapshots that name mesh keys are re-recorded (node counts unchanged).
- **Base shading (REND-025, REND-027):** albedo from the climate field, the slope (the angle between the normal and up, up formed in the shader from `up0 + local / R` of the node record, tile-local arithmetic only) and the height above the plan's water level W; Lambert diffuse under the plan's sun; no ambient, shadows or specular (item 5 adds the sky). Debug views: albedo only, lighting only, temperature, precipitation. The f64 reference lives in `frame::look::shading`.
- **Look file schema 2 (LOOK-001, LOOK-002, LOOK-007):** tables `sun` (direction) and `albedo` (eight colours and fifteen thresholds, Earth defaults from published albedo ranges). New rule: every added table or key raises the schema version with an identity step, so an older build says "newer schema" instead of "unknown key". The version 1 fixture loads through the real chain.
- **The plan (REND-024):** the sun direction comes from the look or from a time of day (`sun_direction`); the albedo table, the water level and the debug view enter the plan text only when they differ from the default, so default plans keep their text and hash.
- **ADR 0033 headroom re-pinned (STRM-006):** the compact slot did not grow (8,712 B + 128 B); the vertex-format slot grew from 53,724 to 58,608 B, so its slot counts and fractions are re-pinned (34,304 and 52,480 slots; R at most 45 % and 30 %), the upload cap fits 17 vertex slots, and the node record grows from 192 to 208 B.
- The placeholders `ALBEDO_SCALE`, `lit_terrain_radiance` and the shader's `lit_tint` are removed; `DEFAULT_SUN` stays as the Earth default of the look's sun.

## Expected golden changes:
- lit_orbit_ev15: the surface colour comes from the climate and the slope instead of the height tint (still pending, first golden of its class)
- lit_aerial20km_ev15: same
- lit_ground1m_ev15: same
- lit_orbit_ev_bands: same
- lit_orbit_temperature_ev15: new, first golden of the temperature view
- lit_orbit_precipitation_ev15: new, first golden of the precipitation view
- lit_aerial20km_albedo_ev15: new, first golden of the albedo view
- lit_orbit_lighting_ev15: new, first golden of the lighting view

No debug-path golden changes: the debug shader outputs are untouched (they pass unchanged on WARP and on the RTX).

## Out of scope
The sky's ambient light and the atmosphere's transmittance on the sun (item 5), terrain shadows and specular, the water level as a field (item 6; the plan carries a constant W, 0), materials beyond albedo, a wired lit viewer, a perf window (none is needed for correctness; the vertex-format tile and the node record grew by 9 % and 8 %, which the next M5 window measures).
