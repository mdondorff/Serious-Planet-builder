# 0001 Coordinate frames and precision
Status: proposed
Date: 2026-10-10
Source: background report §3, §17 item 7; constitution CON-11, CON-13

## Context
Precision cannot be retrofitted cheaply (Star Citizen spent eight months converting to 64 bit; Unreal reworked its tile-based GPU approach). At planet radius float32 has 0.5 m spacing, and consumer GPUs run float64 at 1:32 to 1:64 of float32 speed. The constitution already fixes the invariants (CON-11, CON-13); this ADR chooses the concrete scheme and names the frames.

## Options (with trade-offs)
1. **float64 planet-fixed on the CPU + camera-relative float32 on the GPU.** Nanometre precision at the surface, about 30 µm at 1 AU (enough for later multi-body work). Cost: one f64 subtraction per tile per frame on the CPU.
2. i64 integer cell + f32 offset (big_space). Needed only at galaxy scale; more bookkeeping everywhere.
3. float32 with floating-origin rebasing. Cheap, but rebasing events leak into physics, caches and authored data (KSP "Kraken" class of bugs).

## Decision
Option 1. Frames, in order: `PlanetInertial` (reserved, f64) → **`PlanetFixed`** (f64, right-handed, +Z through the north pole, +X through lat 0 / lon 0, +Y through lat 0 / lon 90°E; the authoritative frame for all world data) → `TileLocal` (f32, origin = tile centre in PlanetFixed) → `CameraRelative` (f32; the only thing shaders see) → `LocalTangent` (ENU, f64 origin + f32 offsets; authored objects, physics islands, gizmos).
- Per frame the CPU computes `tile_origin − camera_position` in f64 and uploads it as f32.
- Tile vertices are stored as f32 (or quantised) offsets from the tile origin, so vertex precision is bounded by tile size, not planet size.
- No shader computes a value of planet-radius magnitude (CON-11); this is checked by review and by the 1 m altitude precision test.
- The i64-cell scheme is reserved for a multi-system extension; nothing in M0–M5 depends on it.

## Consequences
- `core` offers typed positions per frame (`PlanetFixed`, `TileLocal`, `CameraRelative`) so frames cannot be mixed by accident.
- Easier: determinism and debugging (one authoritative frame). Harder: every GPU-bound quantity needs an explicit conversion step.
- Tests: geo ↔ PlanetFixed ↔ CameraRelative round trips ≤ 1 mm (COORD specs, tier A); jitter < 0.25 px at 1 m distance far from the origin (tier C, M2).

## Licences of adopted code or techniques
None adopted; the technique is common practice (camera-relative rendering).
