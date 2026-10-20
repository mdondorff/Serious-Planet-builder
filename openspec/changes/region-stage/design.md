## Context
- Owner (2026-10-19): the overlay tint is the coder's call. The regions come from `region-items` (surface table with a tint per entry, polygon points in the footprint item).

## Decisions
1. **Overlay.** `Overlay::Regions` (shader code 3, plan token `overlay regions`). The polygons are not in the lit plan (`LitPlan` is `Copy` and one text line): like the edit preview (REND-033) they travel beside the plan, as a `RegionList` the app gives the renderer for each frame that shows the overlay (`TerrainRenderer::set_regions`).
2. **Polygon form.** Each polygon is held in the orthographic frame of its anchor: the anchor relative to the camera (f64, rounded once), the tangent plane's basis (`planet_world_def::region::basis`), the points in metres in that plane, the square of the reach and a band (curvature over the reach plus 20 km of relief) that rejects the far side of the planet. The shader subtracts the anchor from the fragment's camera-relative position, projects it onto the basis and runs the even-odd rule: no planet-radius value (CON-11).
3. **Difference from the mask test.** The mask (WDEF-012) tests the gnomonic projection of a direction; the overlay the orthographic projection of the fragment's position. They differ by the second order of the angle (about 0.1 m at 20 km) and by the terrain height: the edge moves about `h * s / R` m for height `h` and distance `s` from the anchor (16 m at 5 km, 20 km out). Accepted for a debug tint; a gnomonic test would put `1/R` in the shader (CON-11). A test compares the two on a grid at the datum.
4. **Globals.** The table is the end of the globals uniform: a count entry and 8 polygons of 37 entries (5 of header, 32 of points two per entry), 4,752 bytes, zeros unless the plan shows the overlay.
5. **Tint.** The surface type's sRGB tint as linear light; `mix(lit, tint * 0.8 * pre-exposure, 0.45)`, the 0.8 being the factor the wireframe overlay's lines use. The nearest polygon wins where two overlap.
6. **Source.** The editor runtime (`EditRuntime::region_list`) reads the region items of every layer of the editor's definition and hands the nearest eight; without the editor the list is empty and the overlay tints nothing.
7. **Not now.** Feathering, regions over 40 km, texture masks, per-region albedo in the exact render, the overlay in the viewer without the editor.

## Risks
- Eight polygons of 64 points is the budget of the overlay: 512 edge tests per fragment in the worst case, only on a frame that shows it; more regions are dropped farthest first and counted.
- A tile of the planet beyond the band is never tinted: a polygon is read as one that lies within the reach of the anchor (WDEF-012).
