## Why
M8-3 (`docs/briefings/m8-plan.md`, ADR 0077 points 1 and 3): the chunk stage built in M8-2 runs a stand-in; the owner's reference image needs the landforms of SPIKE-33's A-net: stream-power incision along the river network the global coupled run fixes, then diffusion and slumping, at 4 to 10 m on the fixed 5 km chunk grid. The global run is stored (GEN-040, STRM-043, 545 s at 2048) and `ChunkStage`, the chunk manager, the pyramid and the keys exist (M8-2). SPIKE-35 closed the open items: a breach rule with no uphill channel cell (c4), the exact halo (3 km at N = 30), a jittered D8 receiver that removes most of the axis alignment, and bit-exact threads.

## What Changes (if accepted)
Five steps, one PR each:
1. **The network in the chunk raster** (GEN-047): per chunk, the coarse receivers, drainage area and heights of the stored coupled channel become the corridor path (a position-only meander of the coarse path), the inflow at the halo ring and the initial fine heights (coarse plus the base field's detail).
2. **A-net incision** (GEN-048, GEN-049, GEN-050): implicit stream power along the frozen network with the c4 breach rule (no uphill channel cell), jittered D8 receivers, N = 30 steps, halo 3 km, ported from SPIKE-35's prototype bit-exact against its hash on the same inputs.
3. **Diffusion and slumping** (GEN-051): hillslope processes on the non-channel cells.
4. **Replace the stand-in and measure** (GEN-052, STRM-046): `ANet` implements `ChunkStage`; the viewer runs it at runtime with the disk cache beside the store; measure chunk time per km2, the step against the parent above top level 14 (criterion 3), the fade time needed, and decide how the fade is built (`m8-chunk-fade`).
5. **Renders at the fixed poses** for look review 1 (M8-4): 300 m, 30 m and 1.8 m at sunset and midday through the lit capture scene with the chunk stage on.

## Impact
- Specs: GEN-047 to GEN-052 (generator-pipeline) and STRM-046 (streaming-cache), planned until each lands.
- Code (explorer lane): `generators` (a new `anet` module and its inputs), `app/worldfield.rs` (enable the stage for a world with a stored coupled channel), `cache` (read of the coupled channel by chunk, already `load_coupled`).
- Cost: step 1 about a day, step 2 about two, steps 3 and 4 about a day each, step 5 half a day.
- Out of scope: horizon shadows and driver maps per chunk (M8-5, M8-6), edits over chunks (M8-7), MFD incision, glacial terms, the shader fade (`m8-chunk-fade`).
- Risks: the stage's halo (3 km, 375 rings at 8 m) makes a chunk compute on 1.9 million cells; the cell-graph region of `chunkfield` is a hash map and may need a dense fast path inside a face; criterion 3 above top level 14 depends on A-net's amplitude at coarse filters (unmeasured); the 3 km halo was measured at one site and one seam.
