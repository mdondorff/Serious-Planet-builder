## Placement
- A press while placing puts the box at the ground under the cursor; the hover hit drives the live preview (no extra pick a frame).
- The tile under a direction: `direction_to_face(TangentWarp)`, `i = floor((s + 1) / 2 * T)` clamped to `0..T` (the axis of `lattice_direction`).
- A box of side `w` centred on the cursor tile: `i0 = ci - (w - 1) / 2`, clamped to `lo..=hi + 1 - w` of `allowed_tiles`; so every placed box is inside the band, and `check_box` still decides.
- Resize keeps `i0, j0` and shifts back into the band when the larger side would leave it.

## State
- `region_inputs_id(def, seed)` of the editor's current definition (its items in it) and the body seed of that definition; computed once per definition hash.
- The editor builds a seed world with `skeleton::RADIUS_M`, the bake with `SEED_WORLD_RADIUS_M`: both `EARTH_RADIUS_M`; TEST-166 checks the ids agree.

## Drawing
- Outline per box: the rectangle's four edges sampled 4 points a tile on the face (`face_to_direction`), set on the ground, culled by `faces_camera`.
- Colours: ready green, stale amber, unbaked light; the selected box wider; the preview cyan when `check_box` accepts, red otherwise; the margin: the band's boundary on the face of the preview or the selected box, grey.
