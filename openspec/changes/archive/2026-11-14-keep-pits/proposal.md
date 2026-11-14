## Why
SPIKE-102 section 6 (tile 2/12/10, seed 14): keeping the pits that survive the breach is the larger relief lever of the SPIKE-101 pair (the high mountains' 3 km relief p50 from 0.62 to 0.85 of Everest's; the mountains' from 0.86 to 0.94 of the Alps'), and the gain survives the fill (679 m against 506 m on the filled surface). The basins no longer eat the carve's work. Owner decision 2026-11-05 (SPIKE-102 D1): keep every pit that survives the breach, store the fill depth, and let the runtime add it back (the filled look) until lake water and a pit-tolerance rule exist, so neither needs a rebuild.

## What Changes
- **GEN-100 / `CarveParams::keep_pits` (on):** a cell below its receiver on the filled route gains its uplift only; pass 2's incise-only carve ignores it.
- **GEN-120:** the fill depth. Pass 1 floods the carved face (`fill_depth`: the flood minus the heights, 0 under 0.5 m) and codes each own cell in a byte; pass 2 takes, per core cell, the larger of its window's flood depth and the depth below pass 1's spill level where pass 1 has a pit (so a basin that crosses a tile edge is as deep as the face's flood found it). `pass2_tile_with_fill` takes pass 1's depth; `pass2_tile` is the version without it.
- **STRM-076:** record kinds 16 (face) and 17 (tile), keyed with the channels' key, the code's step in the key (`FILL_STEPS_PER_M`, 1 m now; the owner has not fixed the depth code, so it may change before the planet bake). `CHANNELS_VERSION` 2.
- **Runtime:** `StoreLayer` adds the tile's depth to its change (`TileResult::with_fill_added`), so the pyramid and the manifest range hold the filled change; today's look stays.
- `bake layer` reads pass 1's fill depth for pass 2 and prints the pit share and deepest pit of each tile.
- Known limits, measured in the preview: the depth is not blended across cube edges or tile edges (a basin on a seam can step in level); the 1 m code leaves 0.5 m terraces on a flat lake; the local incision after pass 1 can leave a lake above its outlet notch; a pit deeper than 255 m is clipped (the bake prints the clipped count per tile).
- `BAKE_KERNEL_VERSION` 5, `PASS2_KERNEL_VERSION` 5: every stored layer is another layer.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-100, GEN-112 (planned, in `m9-bake`); GEN-120 added
- `streaming-cache`: STRM-076 added

## Impact
- Code: `generators::bake` (carve, flood, face, tile), `cache` (key kinds 16, 17; `layer_fill`; `layer_store`, `layer_tiles`, `layer_channels` key), `cache/examples/bake_layer`. No golden moves (the layer is not on a render path before the bake).
- Tests: `generators/tests/bake_kernels.rs`, `bake_tile.rs`, `cache/tests/layer_fill.rs`.
- Pass 1's fill is held until pass 2 has read it; a pass-1 store from before this change is refused as another key (the kernel version is in the key).
- Departure flagged to the planner: none from an accepted ADR. The pit depth is clipped at 255 steps (255 m at 1 m); deeper pits fill to 255 m only, which pass 1 shows at the 95th percentile of 221 m in SPIKE-102 run (b).
