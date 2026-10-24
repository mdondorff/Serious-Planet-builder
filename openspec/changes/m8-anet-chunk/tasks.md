## 1. The network in the chunk raster (GEN-047), about a day
- [x] 1.1 Tests first (tier A, `generators`): two neighbouring chunks find the same corridor path through their shared border; the path starts and ends at the coarse cell centres and is zero-meander there; the inflow at the ring equals the coarse area crossing it; a chunk with no river has no corridor (spec: GEN-047)
- [x] 1.2 `generators::anet::Network`: the coarse receivers, area and heights of a `CoupledState` by chunk, the corridor polyline with the position-only meander, the initial fine heights (coarse upsampled plus the base field's detail)
- [ ] 1.3 numerics-reviewer; verifier; PR

## 2. A-net incision (GEN-048, GEN-049, GEN-050), about two days
- [x] 2.1 Tests first: 0 uphill channel cells, the jittered receivers' axis share at most 0.56 and below D8's, determinism (spec: GEN-048, GEN-050); the SPIKE-35 hash is not reproduced (the prototype is planar with its own corridor and noise, design.md section 12)
- [x] 2.2 Port the solver: implicit stream power along the frozen network, the c4 breach rule, jittered D8 receivers sharing `coupled::jitter_factor`, Kahn order in cell-index order, `libm` only
- [x] 2.3 `ChunkStage::halo_rings` (default step cap plus one); the halo test: two neighbouring chunks at halos 1.5, 3 and 5 km against the whole-region run, 0 differing core cells at 3 km, at a site with a river crossing the border and at N = 30 (spec: GEN-049)
- [ ] 2.4 Measure the cell-graph region at 8 m and a 3 km halo; add the dense in-face fast path only if it costs more than a tenth of the compute, bit-exact against the graph
- [ ] 2.5 numerics-reviewer; verifier; PR

## 3. Diffusion and slumping (GEN-051), about a day
- [x] 3.1 Tests first: channel, corridor and fixed cells are untouched by the processes while the slopes are smoothed, slumping moves the excess to the lower neighbours and conserves it (also at a cube corner), 1, 2 and 8 threads give one hash (spec: GEN-051, GEN-038); the steepness bound after the passes is not asserted (design.md section 15)
- [x] 3.2 Diffusion and thermal slumping on non-channel cells, fixed order
- [ ] 3.3 numerics-reviewer; verifier; PR

## 4. Replace the stand-in and measure (GEN-052, STRM-046), about a day
- [x] 4.1 `ANet: ChunkStage` with its process hash; the viewer enables it for a world with a stored coupled channel (`WorldTerrain::enable_chunks`), disk cache beside the store (spec: STRM-046)
- [x] 4.2 The step against the parent at tile levels 11 to 14 with A-net (the table of `m8-chunk-field` design section 11); the fade time needed; top level chosen; `m8-chunk-fade` scoped from the numbers
- [ ] 4.3 Chunk time at 8 m (default), 4 and 10 m: the perf entry (needs the planner's window; GEN-052 is Verify C) - first run 2026-10-24 (`cargo xtask perf anet-chunk`, eroded world, one thread): 8 m 9.9 s, 4 m 41.9 s, 10 m 6.5 s; the double base read and the area sweep cut 8 m to 5.4 s (base field 2.2 s, incision steps 1.6 s); a re-measure in a window is open
- [ ] 4.4 spec-guardian; verifier; PR. Stop and report if criterion 3 fails at every top level

## 5. Renders at the fixed poses (tier D, for look review 1), half a day
- [ ] 5.1 The lit capture scene with the chunk stage on at 300 m, 30 m and 1.8 m, sunset and midday, on the 16384 store; images to `docs/dev_screenshots/` for the owner (M8-4)

## 6. Close
- [ ] 6.1 All requirements pass: statuses `planned` to `active`, spec-guardian, verifier, archive, one line in `docs/status.md` via the planner
