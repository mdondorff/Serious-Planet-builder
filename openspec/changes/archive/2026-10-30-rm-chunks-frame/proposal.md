## Why
B6 PR 3 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086): after PR 2 nothing in the app sets a chunk view, a chunk fade or lands a chunk, so the frame and render sides that served the stage go.

## What changes
- Removed in `frame`: `chunkshare.rs` (`ChunkShare`, `ChunkView`, `ChunkFade`, `ManagerView`, `prefetch_chunks`, `track_chunks`, `landing_note`); in `managed.rs` the chunk view, fade, stand-in code and `FrameCounts.chunk`; in `pipeline.rs` `LandedChunk` and `invalidate_chunk_later`; in `lod.rs` `chunk_requests`, `chunk_requests_with` and the chunk invalidation of the selection cache; the per-level origin batch of the plan; `TerrainDraw::chunk_weight`; `SmokeCounts::stage_lit_frames` and `horizon_steps` with their smoke-verdict clause.
- Removed in `render`: the fade lane `phase[6].w` of the node record, `faded_height` and the fade of the drivers in the compact shader, the fade planes (`FADE_PLANES`, `FadePlaneGrants`, `fade_plane_vec4s`), `TerrainStats::faded_tiles` and `fade_overflow`, `ExecError::NonFiniteChange`, and the check of `CompactTile::change` on upload. The tile-reference word that held a plane entry is reserved (0).
- App leftovers: the envelope log drops `stand_in_share`, `chunk_nodes`, `chunk_stand_in`, `chunk_field`, `stand_in_reason` and `stand_in_weighting` and keeps `chunk_stage` (always false), `coarse_share` and `steady_curve` (read by `accept M8`); `chunks_busy` is `horizon_busy`.
- Requirements removed: LOD-013, LOD-014, STRM-040, STRM-052, STRM-053, STRM-054, REND-039, REND-042. Reworded: STRM-039 (no fade, no stand-in share), STRM-042 (the manager's prefetch only), STRM-047 (the compact tile's change and its CPU mirror; the generators' chunk field goes in PR 5), STRM-055, STRM-056, STRM-058, TEST-047, TEST-054, TEST-055, TEST-058, TEST-062, and the details of REND-038 and REND-041.
- Tests removed with the code: `chunk_plan.rs`, `chunk_share.rs`, `oracle_chunk_flight.rs`, `gpu_chunk_fade.rs`, the fade-grant and fade-lane unit tests of `terrain.rs`, the chunk cases of `frame/tests/managed.rs`, the chunk stand-in case of `envelopeperf.rs`, and the smoke-verdict case of TEST-058. The queue case of STRM-058 stays without its chunk steps.

## Moved to later PRs
`ChunkManager`, `chunk_store`, STRM-038 keys and STRM-039/041/042 tests (PR 4); `CompactTile::change`, `change_replacing`, `shade_vertex_faded`, `HeightField::level_change` and `level_aware`, the driver blend (`tile_drivers` keeps the constant weight 1 so the lit path is bit-identical), the generators' chunk field (PR 5).

## Default path
No golden, pixel or hash changes by design: the fade lane was 0 and the tile-reference plane word 0 for every default node, so the node record and reference bytes are what they were; the plan text form and summary hash never carried the weight. The tile-reference buffer and the node-record share no longer reserve the fade planes, so the node capacity may rise.
