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
Consumers first, producers last, so every PR compiles and `test gpu` still has its scenes (opus-planner, 2026-10-30: the app goes before the frame, because the app is the only caller of the frame's chunk parts). Any golden diff on the default path stops a PR and goes to the planner.
| PR | Change id | Removes | Spec deltas (REMOVED) | Tests dropped with it |
|---|---|---|---|---|
| 1 | `rm-chunks-flag` (merged, #935) | `--chunks`, `PLANET_SMOKE_CHUNKS`, `gate --chunks`, `envelope --chunks`; the window and editor start no chunk stage | STRM-046, TEST-054, TEST-064 reworded | two `gpu_window` cases |
| 2 | `rm-chunks-app` | the chunk stage of the app: `anetworld.rs`, `regionheights.rs`, `editresolve.rs`, the stage half of `chunkstage.rs` (the rest is `horizonstep.rs`), the stage in `worldfield.rs`, the stage branches of the window and of the stills, edit-latency, envelope and stream scenes; `--edit-resolve`, `--stale-workers`, `--brush`, `--base-threads`; the scene `anet-chunk` with `xtask perf anet-chunk` and its log; the chunk trace, late chunks and prefetch of the envelope log | TEST-049 removed; GEN-052, GEN-055 removed (their only tests ran the stage); TEST-048, TEST-050, TEST-052, TEST-054, TEST-055, TEST-057, TEST-058, VIEW-054 reworded; TEST-056, TEST-059, GEN-087 archived with their changes | `anet_world.rs` stage cases, `oracle_edit_resolve.rs` (the warm-up and hover cases moved to `oracle_world_warmup.rs`), the chunk cases of `horizon_world.rs` and `world_field.rs`, the chunk table and late-chunk tests |
| 3 | `rm-chunks-frame` (done, B6 PR 3) | `chunkshare.rs`, the chunk request, origin and fade parts of the plan and `managed.rs`, and the smoke verdict counter `stage_lit_frames`; the render fade (REND-039) with `gpu_chunk_fade.rs` (any golden diff stops it) | LOD-013, LOD-014, STRM-040, STRM-052 to STRM-054, REND-039, REND-042 removed; STRM-039, STRM-042, STRM-047, STRM-055, STRM-056, STRM-058 reworded to the manager and generator sides that stay until PRs 4 and 5 | `chunk_plan.rs`, `chunk_share.rs`, `oracle_chunk_flight.rs`, `gpu_chunk_fade.rs`; `height_aware.rs` stays (it tests LOD-008, TEST-012, not the stage) |
| 4 | `rm-chunks-streaming` (done, B6 PR 4) | the manager half of `chunks.rs` (the horizon half moved to `horizonjobs.rs`), `chunk_store.rs`; `m8-editor-stalls` archived | STRM-038, STRM-039, STRM-041, STRM-042, STRM-056, GEN-081, GEN-082 removed; STRM-046, STRM-048, STRM-057, STRM-060, GEN-039 reworded; WDEF-018 and the STRM-038 key parts (`QUERY_CHUNK`, `ReadRecord::Chunk`, `Scope::Chunk`) stay with their producers until PR 5 | `streaming/tests/chunks.rs`, `chunk_keys.rs`; `stage_keys.rs` stays (GEN-034) |
| 5 | `rm-chunks-generators` (done, B6 PR 5) | `anet.rs`, `route.rs`, the chunk field and pyramid, chunk drivers; GEN-039 `chunk_stage` setting and the `CHAIN_ORDER_ERA` hash term (decision 3: the setting stays parseable and hashes as its old default; TEST-075 is the planner's) | GEN-036 to GEN-039, GEN-047, GEN-049, GEN-053 to GEN-054, GEN-059, GEN-067 to GEN-078 and the rest of the audit list | `anet_incision.rs`, `anet_network.rs`, `chunkfield.rs`, `bounds_monotone.rs` |
| 6 | `rm-chunks-leftovers` | dead code, examples, docs, perf scenes (`residency_counts`, `scheduler_ablation`), status Tracked rows, learnings kept | none | none |
Reasons for the order: PR 1 changes no default (the stage is already off). The app is the only caller of the stage, so it goes first; PRs 3 and 4 remove what only the app fed. PR 5 is the bulk and only compiles once nothing above calls it.

## Decisions (opus-planner, 2026-10-29)
| # | Decision |
|---|---|
| 1 | The chunk re-solve is retired. Edits under the erosion (owner D3) go on the M9 deferred list, to be rebuilt on M9's erosion design; the ADR says so |
| 2 | Driver channels (GEN-070 to GEN-076) go if and only if the default (stage-off) path draws identical pixels without them. Every PR's acceptance: no golden change on the default path; any golden diff stops the PR and goes to the planner. M9 re-adds drivers from the stored bake |
| 3 | The definition hash must not change so that `C:/DEV/SeriousPlanetData/global16384` (read-only, rebuild about 10 h) is invalidated. A removed setting hashes as its old default. A tier-A test shows the 16384 store's definition still opens without a rebuild. If impossible: stop and report |
| 4 | One new ADR "chunk stage removed" in PR 1, superseding 0077 and its followers, with status lines only on them (ADR 0078) |
| 5 | Timing: PR 1 starts only after M8's acceptance and on the planner's go |

## Risks
| Risk | Control |
|---|---|
| A "stays" item turns out to be read only by chunk code, leaving dead code | PR 7 sweeps the compiler's dead-code warnings |
| A removed requirement's tests also cover base behaviour | Move that assertion to a surviving requirement before deleting, never delete it with the test (CLAUDE.md: no weakened tests) |
| Goldens shift when the fade leaves the draw | PR 3 only; judged through `golden-review` (ADR 0017) |
| Horizon jobs lean on `ChunkManager` types more than the grep shows | PR 5 first moves `HorizonJobs` and `ChunkSource` out as a pure move, then deletes |
| Spec removal order trips `spec-lint` requirement coverage | Each PR removes the requirement and its tests together |

## Additions from the declutter audit (spikes lane, 2026-10-30; opus-planner)
Items the plan above misses, by the PR that takes them. Each is confirmed by the compiler or `spec-lint` in its PR.
| PR | Add |
|---|---|
| 1 `rm-chunks-flag` | flags `--no-chunks` (parsed, never read), `--chunk-workers`, `--base-threads` (chunk branch), `--stale-workers`, `--no-stale-hold`, `--brush`, `--edit-resolve`; `PLANET_SMOKE_LENS`, `PLANET_ANET_CHUNKS`; the help texts that name them (`capture stills`, `gate`, xtask help); `docs/milestones/M8/acceptance-runbook.md` row 6 (`--chunk-workers 6 --base-threads 2`); TEST-057 chunk part; rewording of TEST-048 and TEST-050 (`chunk_timeout_s`, `chunk_cell_m`), TEST-060 and TEST-061 (chunk mentions); the 7 chunk-scoped entries of `docs/learnings.md` archived; the new ADR takes the next free number after 0085 |
| 2 `rm-chunks-frame` | STRM-047 (the change and its weight reach the draw), STRM-053 (a rebuilt tile carries the increment), STRM-054 (a replaced tile keeps its weight until the swap) |
| 3 `rm-chunks-render` | REND-042 (fade planes) |
| 4 `rm-chunks-edit` | rewording of STRM-058 (chunk-landing scenarios) |
| 5 `rm-chunks-streaming` | STRM-040 (landing refreshes and invalidates); `app/tests/gpu_horizon_rig.rs` rebuilt on the base terrain; the 3 ignored report tests that read chunk code (`anet_steps_report`, `anet_change_report`, `chunk_frame_cost_report`) go with their modules |
| 6 `rm-chunks-generators` | GEN-048, GEN-050, GEN-051, GEN-057, GEN-058, GEN-060, GEN-066, GEN-083, GEN-084; `xtask perf anet-chunk`, the scene `anet-chunk`, `globalperf::perf_anet_chunk`, `perf/logs/anet-chunk.json`; `world-def/tests/chunk_stage.rs` reworded, not deleted (decision 3: the setting stays parseable and hashes as its old default); rewording of VIEW-054 (driver views) |
| 7 `rm-chunks-leftovers` | batch 7 of the audit (one-off examples, dead env hooks, the "format arrives in M1" help text, the root `.code-workspace` file); deleting `plan_ablation` lowers the wrapper count at `stamp_keys.rs:259` in the same PR |

- Archived changes `m8-chunk-evict`, `m8-chunk-track-set`, `m8-edits-one-round`, `m8-edits-over-chunks`, `m8-edits-reuse`, `m8-editor-stalls`, `m8-envelope-late-chunks`, `m8-edit-latency-chunks` are archived without syncing their deltas (the audit's docs PR 1); B6 removes their subject.
- `spec-lint` now fails a retired term of `docs/domain.md` section 8 (BUILD-028) once a row loses its *code removal pending* marker, so the removal PRs end with the marker gone and no mention left in the current specs and docs.

## Merged changes that stay active until their code goes (declutter batch 1, opus-planner 2026-10-30)
Four merged changes are still in `openspec/changes/` because chunk code cites their IDs. Each is archived without syncing its delta in the PR that removes the citing code:
| Change | ID cited | Archived in |
|---|---|---|
| `m8-edits-reuse` | GEN-087 | PR 4 `rm-chunks-edit` (`regionheights.rs`, `editresolve.rs`) |
| `m8-editor-stalls` | STRM-063 (chunk-only: the main thread never makes the version of a chunk that is not ready; cited by `streaming/tests/chunks.rs` and `oracle_edit_resolve.rs`) | PR 5 `rm-chunks-streaming` |
| `m8-edit-latency-chunks` | TEST-056 | PR 1 `rm-chunks-flag` (the chunk side of the edit-latency scene) |
| `m8-envelope-late-chunks` | TEST-059 | PR 1 `rm-chunks-flag` (the envelope scene's chunk step) |
`m8-horizon-march-simd` (GEN-086) and `m8-horizon-share` (STRM-061) belong to the ADR 0085 build, not to this removal.

## Out of scope
Horizon maps and jobs (SPIKE-56). The global coupled run and its store. The base terrain, tiles, stamps and brushes. Any new erosion design.
