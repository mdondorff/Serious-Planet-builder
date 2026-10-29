## Why
Owner decision 2026-10-29 (queue row CS): remove the chunk stage from the code; git keeps the history. The stage is off by default since 2026-10-28 (STRM-046, TEST-064) and its gate runs carry known failures (status Tracked). Horizon maps stay until SPIKE-56's ADR. Not started before M8's acceptance and the planner's go. This file is the plan only; nothing is removed by it.

Method: import and symbol search over `crates/` and `xtask/` at main 9ee61974 (2026-10-29). Sizes are lines. The compiler and `spec-lint` decide each PR; every "stays" below is a claim to confirm in that PR, not a proven fact.

## Scope
"Chunk stage" means everything that exists only to compute, hold, show or measure A-net chunk detail on top of the base terrain: `--chunks`, the A-net incision and routing, the chunk field and its pyramid, the chunk manager (jobs, residency, disk), the stand-in share and fade, edit re-solve of chunks, and their tests, specs and perf scenes.

## What stays (shared with the default path)
| Item | Why it stays | Where |
|---|---|---|
| Horizon jobs, `HorizonJobs`, `HorizonConfig`, `HorizonBaker`, `HorizonDisk`, `ChunkSource`, `BaseTerrain` | The default window runs horizon jobs alone from the base terrain (STRM-064); SPIKE-56 decides their future | `streaming/chunks.rs` (the lower half, from line 476), `app/horizonworld.rs`, `frame/horizonview.rs`, `generators/horizon.rs` |
| `ChunkKey`, the chunk lattice and grid level (GEN-034, GEN-035) | The horizon maps are keyed per chunk; `stage.rs` outputs name a `ChunkId` | `generators/chunk.rs`, the lattice part of `chunkfield.rs` |
| The global coupled run, its store and the coarse horizon (GEN-040, STRM-043, GEN-045) | The coarse horizon reads the stored run; the editor's erosion layer and criterion 4 read it | `generators/coupled*.rs`, `coarsehorizon.rs`, `app/coupledworld.rs` |
| Pack disk cache and `HorizonDisk` on the pack | Horizon maps are stored there (STRM-050) | `cache/pack.rs`, `cache/crc32c.rs` |
| Job classes, frame loop, prefetch track, plan delivery | Used by tiles and horizon work; the chunk calls in them go, the loop stays | `frame/managed.rs`, `frame/pipeline.rs`, `streaming/` job classes |
| Hill band and landform driver of the bake (GEN-064), water cut, base horizon | Base terrain, not chunk work | `generators/global.rs` and the water cut |

To split, not delete: `streaming/chunks.rs` (799 lines; the manager half, lines 1 to 475, goes; the horizon half stays and moves to `horizonjobs.rs`), `generators/chunkfield.rs` (954; the lattice stays, the field and pyramid go), `app/worldfield.rs` (1,146; `chunk_*` accessors go, horizon accessors stay), `app/window.rs` and `app/lib.rs` (`--chunks`, `PLANET_SMOKE_CHUNKS`).

## What goes
| Area | Modules and items | Lines |
|---|---|---|
| G. Generators | `anet.rs` (A-net incision, routing, hillslope, drivers per chunk); `route.rs` if only anet uses it; the field, pyramid and level-aware height in `chunkfield.rs`; chunk drivers (GEN-070 to GEN-076) and the `drivers.rs` parts only they read; `compact.rs` chunk-detail inputs | about 3,800 |
| S. Streaming | `ChunkManager`, `ChunkConfig`, `ChunkStats`, `ChunkUpdate`, `ChunkDisk`, `ChunkVersions`, `PackChunkDisk`, `update_with`, prefetch ask, stale hold | about 470 |
| C. Cache | `chunk_store.rs` (the A-net chunk pack), `stage_key.rs` chunk results in tile keys (STRM-038) if nothing else reads them | about 270 |
| F. Frame | `chunkshare.rs` (stand-in share, fade weight, `prefetch_chunks`), the chunk request and origin parts of the plan (LOD-013, LOD-014), chunk calls in `managed.rs`, `pipeline.rs`, `lod.rs`, `terrain.rs` | about 600 |
| R. Render | the chunk fade (REND-039): uniform, pass code, `gpu_chunk_fade.rs` | to measure |
| A. App | `anetworld.rs`, `chunkstage.rs`, `regionheights.rs`, `editresolve.rs` (re-solve of chunks after an edit, STRM-056, GEN-087), `--chunks`, `PLANET_SMOKE_CHUNKS`, `StageOptions`, the chunk parts of `envelopeperf.rs`, `streamperf.rs`, `stills.rs`, `editperf.rs`, `globalperf.rs`, `gate.rs`; examples `chunk_incision.rs`, `residency_counts.rs`, `scheduler_ablation.rs` | about 3,500 |
| X. xtask | `gate --chunks`, the KNOWN ceilings of the stage-on run, `envelope --chunks` | small |

## Order (one area per PR, each leaves main green)
Top-down: users first, producers last, so every PR compiles and `test gpu` still has its scenes.
| PR | Change id | Removes | Spec deltas (REMOVED) | Tests dropped with it |
|---|---|---|---|---|
| 1 | `rm-chunks-flag` | `--chunks`, `PLANET_SMOKE_CHUNKS`, `gate --chunks`, `envelope --chunks`; the window and editor no longer call `enable_anet*` or `enable_chunks` (A, X) | STRM-046, TEST-064 (the default becomes the only mode: reword to "the gate runs the base terrain"), TEST-054, TEST-055, TEST-049, TEST-052 chunk channel | `gpu_window` case with `--chunks`, KNOWN stage-on ceilings, `anet_world.rs` stage tests |
| 2 | `rm-chunks-frame` | `chunkshare.rs`, the chunk request, origin and fade parts of the plan and `managed.rs`; TEST-058's "same chunk step" reworded to the remaining steps (F) | LOD-013, LOD-014, STRM-039 (frame side), STRM-042, STRM-052 | `chunk_plan.rs`, `chunk_share.rs`, `oracle_chunk_flight.rs`, chunk cases of `managed.rs`, `height_aware.rs` |
| 3 | `rm-chunks-render` | the fade weight in the draw and the shader (R) | REND-039 | `gpu_chunk_fade.rs` and its goldens, through `golden-review` |
| 4 | `rm-chunks-edit` | `editresolve.rs`, `regionheights.rs`, chunk staleness (A) | STRM-056, STRM-057 chunk side, GEN-081, GEN-082, GEN-087 | `oracle_edit_resolve.rs`, edit cases that wait for a chunk |
| 5 | `rm-chunks-streaming` | the manager half of `chunks.rs`, `anetworld.rs`, `chunkstage.rs`, `chunk_store.rs`, STRM-038 keys (S, C, A) | STRM-038, STRM-041, STRM-048 chunk side, WDEF-018 | `streaming/tests/chunks.rs`, `chunk_keys.rs`, `stage_keys.rs`, `world_field.rs` chunk cases |
| 6 | `rm-chunks-generators` | `anet.rs`, `route.rs`, the chunk field and pyramid, chunk drivers (G); GEN-039 `chunk_stage` setting and the `CHAIN_ORDER_ERA` hash term | GEN-036 to GEN-039, GEN-047, GEN-049, GEN-052 to GEN-055, GEN-059, GEN-067 to GEN-078 | `anet_incision.rs`, `anet_network.rs`, `chunkfield.rs`, `bounds_monotone.rs` |
| 7 | `rm-chunks-leftovers` | dead code, examples, docs, perf scenes (`residency_counts`, `scheduler_ablation`), status Tracked rows, learnings kept | none | none |
Reasons for the order: PR 1 changes no default (the stage is already off), so it needs no golden and no look review. PRs 2 and 3 remove what only PR 1's callers fed. PR 5 and 6 are the bulk and only compile once nothing above calls them.

## Decisions the plan needs from the planner or owner
| # | Question | Default taken here |
|---|---|---|
| 1 | The tile driver block and driver views (GEN-071 to GEN-074, VIEW debug views): they read chunk drivers. Remove with the stage, or keep as a base-terrain feature? | Remove; they have no source without chunks |
| 2 | `WDEF-018` and the `chunk_stage` setting change the definition hash; removing it changes keys of worlds with items. Cache invalidation is allowed (CON-05) but old stores rebuild | Remove; rebuild |
| 3 | The M8-7 "edits under the erosion" path (re-solve within 10 s, criterion 6) is built on chunk re-solve. Its removal drops that feature: edits show only the base terrain until a new erosion design exists | Needs the owner: confirm that criterion 6 is retired or re-based |
| 4 | Do `Static memos` (Tracked: `landform_lattice`) and the coupled run's erodibility inputs stay for the global run only? | Stay |
| 5 | An ADR: 0077 (chunk stage) and the ones built on it are superseded in part; ADR 0078 allows only a status line | One ADR "chunk stage removed", status lines on 0077 and followers, in PR 1 |

## Risks
| Risk | Control |
|---|---|
| A "stays" item turns out to be read only by chunk code, leaving dead code | PR 7 sweeps the compiler's dead-code warnings |
| A removed requirement's tests also cover base behaviour | Move that assertion to a surviving requirement before deleting, never delete it with the test (CLAUDE.md: no weakened tests) |
| Goldens shift when the fade leaves the draw | PR 3 only; judged through `golden-review` (ADR 0017) |
| Horizon jobs lean on `ChunkManager` types more than the grep shows | PR 5 first moves `HorizonJobs` and `ChunkSource` out as a pure move, then deletes |
| Spec removal order trips `spec-lint` requirement coverage | Each PR removes the requirement and its tests together |

## Out of scope
Horizon maps and jobs (SPIKE-56). The global coupled run and its store. The base terrain, tiles, stamps and brushes. Any new erosion design.
