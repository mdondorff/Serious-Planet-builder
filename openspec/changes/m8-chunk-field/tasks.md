## 1. R1: one home for the field chain (about a day; PR on its own)
- [ ] 1.1 List the differences between `generators::build_field` and `app/worldfield.rs:661-700`; write a tier-A test that both build the same field (same keys, same heights) for the reference world and a stamped world (spec: GEN-016, GEN-018)
- [ ] 1.2 Move the chain and the world's sources into one GPU-free module in `generators`; `app` calls it; no behaviour change, all existing tests green
- [ ] 1.3 `cargo xtask test fast`, `cargo xtask test`; verifier

## 2. R2: split `HeightField` (about a day; PR on its own)
- [x] 2.1 (R2a, owner and planner, 2026-10-23) `height_at_level` added with a default of `height`, forwarded by every wrapper (Bound, StampStage, WaterBounded, CancelGuarded, editscale Timed, plan_ablation Counting); the split into value, key and bounds traits (50+ impls) is deferred until after M8-2 (spec: GEN-036)
- [x] 2.2 Tier-A tests: default equals `height` at every level; each wrapper answers as its inner field does (learning 2026-10-16)
- [ ] 2.3 numerics-reviewer (core/generators); verifier

## 3. M8-2: chunk field infrastructure (five steps, each at most a day, each its own PR)
### 3a. Scope, key, grid (GEN-034, GEN-035)
- [x] 3a.1 `Scope::Chunk(ChunkId)`, `ChunkId` on level 11, `scope_fits`, the stage key over grid, halo and step cap
- [x] 3a.2 Tests: re-keying (grid, halo, cap), scope separation from `Tile`, chunk ids at face edges and corners
### 3b. Stand-in compute and the pyramid (GEN-036, GEN-037, GEN-038)
- [x] 3b.1 `ChunkStandIn` behind the test feature; chunk-stage interface (raster in, change raster out, halo, cap, bound)
- [x] 3b.2 Box-filtered pyramid; `height_at_level` for a chunk field; the apply level from the cell size
- [x] 3b.3 Tests: border agreement (halo rule), bit-exact across 1, 2 and 8 threads, step against the parent at most 0.25 m p99 on the stand-in; known-answer hash for CI on Windows and Linux
- [ ] 3b.4 numerics-reviewer
### 3c. Plan-side requests, bounds, key chaining (LOD-013, STRM-038, STRM-040)
- [ ] 3c.1 The plan returns `B`; sampling never requests; bounds widen by the field's bound; a chunk field with no chunk ready plans identically to one with all ready
- [ ] 3c.2 Tile keys chain the chunk's content hash (word 0 when not ready); tiles above the apply level key as the base
- [ ] 3c.3 Invalidate origin heights of tiles under a landed chunk
### 3d. Jobs, landing, stand-in and fade (STRM-039, STRM-041)
- [ ] 3d.1 Chunk job class on the injected spawner; distance priority; `chunk_workers`; fake-clock latency model
- [ ] 3d.2 Landing refreshes tiles through the stale-tile machinery; CON-22 test over a scripted flight (no frame waits)
- [ ] 3d.3 Fade weight: decide compact tile or uniform, implement, test the fade on the software adapter; counts of stand-in share of the plan at the apply level exposed in the per-frame log (for M8-V)
- [ ] 3d.4 Eviction by need; disk cache in the pack store with verification
### 3e. Prefetch, layer setting, tight bound (STRM-042, GEN-039)
- [ ] 3e.1 Chunk prefetch along the direction of travel; rotation prefetches nothing
- [ ] 3e.2 `chunk_stage` layer setting in the world definition; `Under` refused with an error naming the layer
- [ ] 3e.3 Tight bound from a ready chunk's grid, only if the plan stays a pure function of position and world version; otherwise cut and note in the PR
- [ ] 3e.4 Flight and pan scenes on the software adapter (SPIKE-34's flight 30 km to 1.8 m and a 300 m pan) as tier-B/C checks; spec-guardian; verifier

## 4. Close
- [ ] 4.1 All requirements pass: statuses `planned` to `active`, spec-guardian, verifier, archive, one line in `docs/status.md` via the planner
