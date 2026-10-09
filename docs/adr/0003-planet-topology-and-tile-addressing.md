# 0003 Planet topology and tile addressing
Status: proposed
Date: 2026-10-10
Source: background report §2, §17 item 11; constitution CON-08, CON-15

## Context
Terrain needs a regular-grid quadtree on a sphere with trivial tile ids. Face mapping decides distortion; the id layout decides cache keys and storage order. Both appear in cache keys, but tiles are disposable caches (CON-05), and authored anchors are stored as geo-coordinates (CON-15), so a later change of mapping costs regeneration, not data loss.

## Options (with trade-offs)
1. **Cube-sphere quadtree, six face roots, with a per-face warp.** Regular grids, GPU friendly, no pole singularity; seams at face edges and eight corners.
2. HEALPix / S2 / icosahedral / H3 / A5. Better distortion or neighbour properties, but awkward texture grids and per-pixel maths. Import-only or gameplay-only.
3. Lat-long. Pole singularities, huge distortion. Import-only.

Face warps considered (report §2): tangent-adjusted gnomonic (`u = tan(s·π/4)`; max angular distortion 31.1°, area ratio 1.41 per Lambers & Kolb 2012) and the approximately equal-area warps of JCGT 2018.

## Decision
Option 1. Details:
- **Faces** 0..5 = +X, −X, +Y, −Y, +Z, −Z of PlanetFixed (ADR 0001). Each face has axes `(u, v)` with `u × v = outward normal`: +X (u=+Y, v=+Z), −X (u=−Y, v=+Z), +Y (u=−X, v=+Z), −Y (u=+X, v=+Z), +Z (u=+X, v=+Y), −Z (u=+X, v=−Y).
- **Face warp:** start with the **tangent-adjusted warp** behind a `FaceMapping` seam. It is closed form and invertible with `libm`, and its distortion numbers are published. *This departs from the report's leaning (approximately equal-area)* because the warp is a cache-only choice that can be swapped behind the seam, and the equal-area variants are evaluated at the M2 go/no-go gate against real LOD error. Everything near face edges is computed from the 3D unit direction, never from face UV (report §2).
- **Tile id:** 64-bit: bits 63..61 face, bits 60..56 level (0..=28), the low `2·level` bits hold the Morton-interleaved child path (x bit above y bit at each level), unused bits zero. 28 levels cover about 10 cm at 256 samples per tile (report §4, §17).
- Hilbert-ordered storage keys (S2 style) are a separate decision with the tile storage ADR (M3–M4).

## Consequences
- `core` gets `FaceMapping`, `TileId`, parent/child/neighbour functions including cross-face neighbours and the eight corners.
- Tests (tier A): id round trips, neighbour symmetry across all 12 face edges, direction ↔ face-UV round trip, partition of the sphere (every direction belongs to exactly one tile per level).
- If the warp changes, all cache keys change (the mapping id is part of `implementation_id`); authored data is unaffected.

## Licences of adopted code or techniques
Mathematical mappings from published papers; no code adopted. Re-check if an implementation of the JCGT 2018 warps is ever copied.
