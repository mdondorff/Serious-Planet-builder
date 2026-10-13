# 0033 Memory budgets and terrain's VRAM share
Status: proposed (skeleton by opus-planner, 2026-10-13; the principle is the owner's decision of 2026-10-13, the numbers and the tile format are decided after SPIKE-05, SPIKE-06 and SPIKE-07; sonnet-explorer completes it, M4 item 5)
Date: 2026-10-13
Source: owner's decisions 2026-10-13 (`docs/status.md`, M4 queue (b) and (d)); STRM-006; constitution CON-20, CON-21; ADR 0031

## Context
CON-21 caps total VRAM (8 GB mid tier, 14 GB high tier) and RAM (8 GB); nothing reserves VRAM for anything but terrain. The owner wants the technology to grow into applications with more GPU data (ground cover, objects, a game). Today's terrain vertex is 44 bytes (11 f32, `MeshVertex`), about 50 KB per 32-cell tile; the ground-1m draw set (8,735 nodes) is about 0.4 GB, and a position-based residency set (ADR 0031) would be several times that (worked out by hand, not measured). The finest level (26, a 15 cm tile, a vertex every ~5 mm at 1 m altitude) may be more geometry than needed when shaders add detail below.

## Decision (principle decided by the owner 2026-10-13; the rest proposed)
1. **Terrain has its own VRAM share**, a count-based cap on allocated terrain bytes (meshes or height textures, index buffers, per-tile uniforms), asserted in tests on any adapter (CON-20) and separate from the tier cap. Starting values: **2 GB at the mid tier, 3 GB at the high tier**; the remainder of the cap is reserved for other content and is not used by terrain.
2. Terrain's RAM share (generated tiles and the pack store's index): to be fixed (proposal: ≤ 3 GB of the 8 GB).
3. Uploads: ≤ 2 ms per frame (CON-21), as a byte budget per frame derived from a measurement.
4. The tile format (today's 44-byte vertices or SPIKE-06's shared grid with height textures) and the finest geometry level (26, 23 or 22, SPIKE-07) are chosen here, so that the residency set at the standard views fits the share with margin. To be filled from the spikes.
5. When demand exceeds the share, ADR 0031 point 4 decides what degrades.

## Consequences
- STRM-006 gains the terrain share as its own cap (spec delta), with tier-A tests at both tiers.
- A changed tile format bumps MESH_VERSION and changes goldens (through `golden-review`).
- Ground cover and other content later get their own shares from the reserved remainder (a later ADR).

## Licences of adopted code or techniques
None.
