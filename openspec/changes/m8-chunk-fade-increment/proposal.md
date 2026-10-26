## Why
The fade of `m8-chunk-fade` scales a tile's whole change plane with one weight. Three defects follow from that (numerics-reviewer, 2026-10-26; design.md 6b of `m8-chunk-fade`):
- A tile that reads a chunk that is not ready weighs 0 and draws the base, while its neighbour that reads only the ready chunks is at its weight: a crack of up to the change (58 m) that lasts as long as the neighbour chunk takes (seconds), not the fade.
- A new version of a faded chunk (every edit whose halo touches it, M8-7) shows its difference at the tile swap in one frame.
- A tile that arrives after a chunk's clock started appears at the chunk's progress.

## What Changes
- The plane of a rebuilt tile is the increment over what the tile it replaces is drawn at: `d' = final' - drawn_old` per texel, `drawn_old = final_old - (1 - w_old) x d_old` (the old tile's heights, plane and weight are held by the loop; planes are about 1.1 thousand f32). At weight 0 the new tile draws what the old one drew, at weight 1 its own surface. A tile with no predecessor has the increment over the base, as today. No shader, node record or renderer change.
- A tile's weight is the minimum over the chunks it reads that are READY (an unready chunk adds nothing to the plane, so it needs no weight): the crack next to a not-ready neighbour closes. A chunk landing later is an increment on the tiles that read it, faded from the first frame they are drawn.
- One clock per landing epoch (STRM-052): all tiles rebuilt for one landing share it, and it does not run while a drawn tile that reads the chunk is still the stale one. So a rebuilt tile is always drawn at weight 0 when its chunk's clock starts, and a tile that arrives later (it was not drawn while stale) carries the same increment at the epoch's weight: it draws `drawn_old + w x (final' - drawn_old)` like every other tile of the epoch, and the shared edges stay equal for every `w` (both sides are `drawn_old + w x (final' - drawn_old)` with equal `drawn_old` on the edge). The plane is not divided by `1 - w0` (the first version of this proposal did, assuming the old tile was on screen and advancing; it never is) and there is no end-of-fade guard. The weight a stale tile keeps from a landing until its swap is the weight it had at the landing. `d'` is computed in f64 and stored as f32.
- Tests (STRM-053): the shared edge of two tiles across a chunk border stays closed to 0.05 m through a neighbour landing and next to a chunk that is not ready, at weights 0, 0.3, 0.7 and 1; the rebuilt tile draws the old heights at weight 0; the invariant `a drawn tile that reads a chunk is never stale while its clock runs` is a property test over the frames of a landing.

## Impact
- Spec: STRM-047 (the plane and the weight), REND-039 unchanged. Tier A.
- Code: `generators/compact.rs` (`change_replacing`, written and tested in the first attempt of 2026-10-25, removed unused), `frame/managed.rs`, `frame/chunkshare.rs`.
- Cost: about 1.5 days. Explorer's M8-6 touches `compact.rs` for the tile driver block after the archive of `m8-chunk-fade`; rebase order to be agreed.
