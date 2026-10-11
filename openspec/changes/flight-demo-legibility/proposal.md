## Why

The orbit-to-1 m flight (TEST-013) was rendered in the `tile-id` view for the owner's review (TEST-014). Every tile is one flat colour there, so once the camera is a metre above the ground the frame is one or two solid colours: the final 20 % of the frames (the 5 cm glide) show nothing, and no existing view (`height`, `normals`, `depth`) does better on smooth terrain at that scale (height contours start at 10 m spacing). Jitter, vertex swimming, cracks and popping, the things TEST-014 asks the owner to judge, are invisible in exactly the frames built to show them.

## What Changes

- New terrain debug view `relief` (fixed-sun hillshade on a height tint, an adaptive metric grid of the tile-local axes from 1 cm to 1,000 km, height contours from 1 cm to 10 km). Grid and contour decades fade out once their lines are closer than about 6 px, so the picture has structure at every altitude. Shader inputs stay tile-local or camera-relative (CON-11); appended last in `TerrainView::ALL` so the other views keep their shader index.
- `planet test-render --scene terrain --flight` defaults to the `relief` view (an explicit `--view` still wins), writes `contact_sheet.png` (12 frames from orbit to 1 m, labelled with frame number and altitude) next to the frames, and adds a `finest_level` column to `flight.txt`.
- Tier-C check that no flight frame in the default view is nearly one colour, with the `tile-id` flight as a negative control showing the check can fail.
- Two first goldens for the new view (orbit, ground at 1 m); lit views are never auto-blessed (ADR 0017), so they wait for the owner in `tests/goldens/pending.txt`.
- Added TEST-017 (flight legibility, tier C); modified REND-006 (the relief view) and TEST-013 (default view, contact sheet, index column).

Expected golden changes:
- terrain_orbit_relief: new golden for the relief view (first golden, escalates to the owner).
- terrain_ground1m_relief: new golden for the relief view (first golden, escalates to the owner).

## Impact

`crates/frame` (view enum), `crates/render` (shader), `crates/app` (flight output, `sheet.rs`), tests, specs. No existing golden changes; no mesh or generator change. The earlier `flight-nvidia` review sequence is `tile-id` and stays valid for the dive, but its glide frames are uninformative; the owner may want to re-review the relief flight (TEST-014).
