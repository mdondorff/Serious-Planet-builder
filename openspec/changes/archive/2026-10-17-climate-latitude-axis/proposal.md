## Why
M5 item 4 left a recorded defect (GEN-021, the doc comment of `sun_direction`): the analytic global-coarse climate took the sine of the latitude from the y component of the direction, so its poles sat at +-y and its equator was the x-z plane, while COORD puts the north pole at +z (+x is latitude 0, longitude 0; +y is longitude 90 degrees east) and the sun, the camera and the surface conversions already follow COORD. The temperature and rain bands, and with them the climate-dependent albedo of the lit terrain, were rotated by a quarter turn from where the sun and the poles are.

## What Changes
- `generators::global`: one function `sin_latitude(d)` (the z component, clamped) is the only place the field derives latitude; `sample` uses it. The equirectangular review map builds its direction in COORD's axes (poles at the top and bottom rows). No other site derived latitude (the bake, the reader, the climate texture, the terrain and eroded fields all read `sample`; the shaders and `frame` take no latitude: they read the stored climate or use COORD-based helpers).
- `GLOBAL_VERSION` 2 to 3 (the temperature and precipitation channels change; continent and elevation bits do not). The climate version word (climate, reader and bake versions, ADR 0049 point 3) and so every mesh key and persistent mesh key change; stored bakes of version 2 and stored tiles are unreachable and are written again (item 8 regenerates the store). `CLIMATE_VERSION` (1), `MESH_VERSION` (7) and the geometry are unchanged: the texel construction and the tile formats did not change, only the field they read.
- Pins and known answers re-recorded with the reason in the diff: the global known answers and bake hash, the reader pins, the climate and compact content hashes and the version word, the Reference mesh key, the persistent mesh, stage and block keys, the determinism manifest header, the terrain plan snapshots (mesh keys only; node counts, origins and morphs are unchanged).
- Tests: a failing-first known-answer test at the poles and the equator (GEN-013, GEN-021); the latitude-band tests measure latitude from z.

## Impact
Specs: GEN-013 and GEN-021 state the axis. Goldens: debug views and lit pictures that show the climate move (declared change, `golden-review`, ADR 0017/0037); the pending M5 lit and sky candidates change again and stay pending.

Expected golden changes:
- No blessed golden moves: the debug-class and shaded goldens (face, tile id, height, level, morph, normals, depth) do not read the climate.
- lit_orbit_ev15, lit_aerial20km_ev15, lit_ground1m_ev15, lit_orbit_ev_bands, lit_orbit_temperature_ev15, lit_orbit_precipitation_ev15, lit_aerial20km_albedo_ev15, lit_orbit_lighting_ev15: pending first goldens of the lit class (no committed golden); the climate colours rotate by a quarter turn. They stay pending for the owner's /bless (not applied by this change).
- sky_orbit_ev15, sky_aerial20km_ev15, sky_ground1m_ev15, sky_ground1m_sunset_ev11: pending first goldens of the sky class; the ground colour under the atmosphere changes with the albedo. They stay pending.
- sky_orbit_transmittance_lut, sky_orbit_multiscatter_lut: pending; independent of the climate (the look tables), listed because they are in the pending set; their pixels should not change.
