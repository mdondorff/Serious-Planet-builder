## Why
The fade of `m8-chunk-fade` scales a tile's whole change plane with one weight. Three defects follow from that (numerics-reviewer, 2026-10-26; design.md 6b of `m8-chunk-fade`):
- A tile that reads a chunk that is not ready weighs 0 and draws the base, while its neighbour that reads only the ready chunks is at its weight: a crack of up to the change (58 m) that lasts as long as the neighbour chunk takes (seconds), not the fade.
- A new version of a faded chunk (every edit whose halo touches it, M8-7) shows its difference at the tile swap in one frame.
- A tile that arrives after a chunk's clock started appears at the chunk's progress.

## What Changes
- The plane of a rebuilt tile is the increment over what the tile it replaces is drawn at: `d' = final' - drawn_old` per texel, `drawn_old = final_old - (1 - w_old) x d_old` (the old tile's heights, plane and weight are held by the loop; planes are about 1.1 thousand f32). At weight 0 the new tile draws what the old one drew, at weight 1 its own surface. A tile with no predecessor has the increment over the base, as today. No shader, node record or renderer change.
- A tile's weight is the minimum over the chunks it reads that are READY (an unready chunk adds nothing to the plane, so it needs no weight): the crack next to a not-ready neighbour closes. A chunk landing later is an increment on the tiles that read it, faded from the first frame they are drawn.
- One clock per landing epoch (as now): all tiles rebuilt for one landing share it; a tile that arrives later joins at the epoch's progress with its increment scaled to match (`w' = w_epoch`, increment against the old tile as it is drawn then), so it neither pops nor lags.
- Tests: neighbour landing and a not-ready neighbour leave the shared-edge texels of two tiles equal frame to frame (CPU mirror); a rebuild slower than the fade, a new version of a faded chunk and a late tile each draw at the old heights on their first frame.

## Impact
- Spec: STRM-047 (the plane and the weight), REND-039 unchanged. Tier A.
- Code: `generators/compact.rs` (`change_replacing`, written and tested in the first attempt of 2026-10-25, removed unused), `frame/managed.rs`, `frame/chunkshare.rs`.
- Cost: about 1.5 days. Explorer's M8-6 touches `compact.rs` for the tile driver block after the archive of `m8-chunk-fade`; rebase order to be agreed.
