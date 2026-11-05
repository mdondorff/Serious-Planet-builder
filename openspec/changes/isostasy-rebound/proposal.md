## Why
Owner 2026-11-05 (planner inbox #72 item 1): eroded mass unloads the crust, and a flexural response raises the column, so peaks rise as valleys incise. Pass 1's carve has an uniform uplift (`U / K` 20) and no isostatic feedback. The measurement on 2/12/10 (seed 14, production grid; design.md) shows what the rebound does and does not do: it lifts the land by about 0.8 of the regional mean erosion and leaves the relief unchanged.

## What Changes (proposed, nothing built)
- GEN-124 (reserved): an isostatic rebound in pass 1's carve: after each step the mass the step removed, smoothed by a flexural kernel of width set by the elastic thickness `Te`, is returned to the non-outlet cells as `rebound` (the crust-to-mantle density ratio, 0.82) times the smoothed removal.
- `CarveParams` gains `rebound` and `rebound_sigma_cells` (both in `LayerSpec::bytes`, so the layer key changes); off at 0 (bit for bit today's carve). `BAKE_KERNEL_VERSION` bumps when it is switched on in production.
- Pass 2 does not need it (its local carve only incises).

## Decision needed
Whether the rebound is wanted for the elevation (it raises every elevation by about 250 to 290 m on the mountain tile, which is already too high against GLO-30) or for the relief (it does not deliver it). See design.md.
