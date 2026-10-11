## Why

The orbit-to-1 m flight (TEST-013) was rendered in the `tile-id` view for the owner's review (TEST-014). Every tile is one flat colour there, so once the camera is a metre above the ground the frame is one or two solid colours: the final 20 % of the frames (the 5 cm glide) show nothing, and no existing view (`height`, `normals`, `depth`) does better on smooth terrain at that scale (height contours start at 10 m spacing). Jitter, vertex swimming, cracks and popping, the things TEST-014 asks the owner to judge, are invisible in exactly the frames built to show them.

## What Changes

- New terrain debug view `relief` (fixed-sun, wrapped hillshade on a height tint, an adaptive metric grid from 1 cm to 1,000 km, height contours from 1 m to 10 km). Grid and contour decades fade out once their lines are closer than about 6 px, so the picture has structure at every altitude. The grid is anchored to the world: the node uniform carries the f64 tile origin modulo 1 m and modulo 1,000 km (`grid_phases`), so tiles and LOD splits agree on every line (a per-tile grid broke at tile borders in the first version). Shader inputs stay tile-local, camera-relative or small remainders (CON-11); the view is appended last in `TerrainView::ALL` so the other views keep their shader index, and its work runs only when it is selected (no cost in the perf workload).
- `planet test-render --scene terrain --flight` defaults to the `relief` view (an explicit `--view` still wins), writes `contact_sheet.png` (12 frames from orbit to 1 m, labelled with frame number and altitude) next to the frames, and adds a `finest_level` column to `flight.txt`.
- Tier-C check that no flight frame in the default view is nearly one colour (edge density, production cells 32 at 768x432), with the `tile-id` flight as a negative control showing the check can fail. Tier-A test of the grid phases; determinism tests of the relief view and of the default flight.
- Two first goldens for the new view (orbit, ground at 1 m); lit views are never auto-blessed (ADR 0017), so they wait for the owner in `tests/goldens/pending.txt`.
- Added TEST-017 (flight legibility, tier C); modified REND-006 (the relief view), TEST-010 (relief is the one lit debug view), TEST-013 (default view, contact sheet, index column) and TEST-014 (the review is of the relief flight).

Expected golden changes:
- terrain_orbit_relief: new golden for the relief view (first golden, escalates to the owner).
- terrain_ground1m_relief: new golden for the relief view (first golden, escalates to the owner).

## Impact

Known limits: contours, tint and shading use the fine vertex's own height and normal (not geomorphed), so contours can pop at LOD transitions like the height view's do; contours start at 1 m because an absolute f32 height is only good to a fraction of a millimetre at a few km; the sun is a shader constant, not plan data (a real lit view takes it from the plan, ADR 0009). The relief goldens are classed `lit` (never auto-blessed) by choice; the owner may reclass them `shaded` when blessing.

`crates/frame` (view enum), `crates/render` (shader), `crates/app` (flight output, `sheet.rs`), tests, specs. No existing golden changes; no mesh or generator change. The earlier `flight-nvidia` review sequence is `tile-id` and stays valid for the dive, but its glide frames are uninformative; the owner may want to re-review the relief flight (TEST-014).
