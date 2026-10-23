Opened: 2026-10-23
Closed: 2026-10-24
Question: Can the engine run a regional field per block of a fixed grid (ADR 0077) inside streaming, the cache, the plan and the editor without breaking CON-22, bounded edits or the frame, whatever the block's compute is?
Why it matters: M8 implementation starts on 2026-10-25 (owner); ADR 0077 needs the integration risks measured alongside SPIKE-33's method question.
Time box: about half a day of agent work, indicative timings, no perf window; report by 2026-10-24 evening.
Success criteria: the measurements of the brief below with a fit verdict per rule (CON-06, CON-08, CON-22, edit invalidation, LOD stability, memory).
Fallback if it fails: a list of what in streaming, the cache or the plan blocks a block field, with the change each needs.
Code location: branch `spike/block-stage` (never merged); a prototype patch against main in this folder.

# SPIKE-34 brief: a block stage in the engine (block-stage)

## Set-up
- A block field as a `HeightField` over the store: `Scope::Block` on a fixed grid of whole coarse cells, a halo in metres, keyed like a stage output (ADR 0022's key plus the block grid).
- The compute is a stand-in: SPIKE-32's `RegionStage` (droplet erosion, 4 m cells) run per block with a halo, or a timed sleep of the measured cost per km2; the method is SPIKE-33's question, not this one's.
- Block jobs run on the streaming workers through the injected interfaces (CON-04); a tile whose block is not ready draws from the coarse field.

## Measurements
| # | What | Output |
|---|---|---|
| 1 | Flight from 30 km to 1.8 m over SPIKE-32's site and a scripted pan at 300 m: blocks requested, ready latency, tiles drawn from the coarse stand-in, frame CPU time | table |
| 2 | CON-22: no frame waits for a block; the switch from stand-in to block result (a visible pop: height difference at the switch, p99) | numbers; a still pair |
| 3 | Seams at block borders and between a fine tile and its parent (heights, normals) | numbers |
| 4 | Plan and bounds: `height_bounds` from a block's run; plan refusals or capped tiles over blocks | counts |
| 5 | Edits: a stamp near a block border invalidates which blocks; time until the area is exact again | table |
| 6 | Memory and cache: blocks resident at 300 m and 30 m, bytes per block, eviction | table |
| 7 | Block size: 5, 10 and 20 km blocks against items 1, 5 and 6 | recommendation |

## Rules for this spike
- Own worktree and branch; the owner's 16384 store is read-only (copy what a run modifies).
- No `cargo xtask perf`; timings indicative, other sessions and the display setup noted.
- Report: numbers in tables, the changes each subsystem needs (streaming, cache key, plan, editor), and a Visuals section.

# Results

## Verdict
A block field fits the engine as built, under four conditions the measurements below set: **(1)** block detail applies only to tiles at or above an *apply level* that the block's cell size supports (level 15 for 8 m cells); **(2)** block requests come from the plan's nodes at that level, not from height samples; **(3)** the block's change is a band-limited pyramid, so a tile reads the level of its own vertex spacing; **(4)** a block costs seconds, not tens of seconds. Under them: CON-22 holds by construction, a pan at 300 m needs 3 to 4 blocks and 4 MB, and an edit invalidates 1 to 4 blocks. Without (1) to (3) the field pops tens of metres. Edits wait one block run (seconds), not the tens of milliseconds of a tile refresh today.

## Set-up (what was built)
- `BlockField` (`block-stage-prototype.patch`; branch `spike/block-stage`, never merged): a `HeightField` over the eroded field of a 512-cell in-memory bake (seed 1, SPIKE-32's site lat -23.882, lon -15.284), with blocks as tiles of level 11, 10 or 9 (4.9, 9.8, 19.5 km at the face centre; whole cells of a 16384-cell bake at 8, 16 and 32 cells), a halo in cells of the run's grid, and the block's output as a height change against the base on the interior nodes.
- The compute is a stand-in for any iterative regional process: ridged relief of 800, 200 and 50 m (amplitude 30 m) added to the base, then `steps` relaxations of 3x3 (reach one cell per step) with the outer border of the halo fixed. Deterministic (fixed order, one thread, `libm`). Its real run time is measured; the simulation's latency uses SPIKE-32's cost, 0.25 s per km2 per thread, on 6 virtual workers (nearest block to the camera first).
- Flight and pan run headless at 10 plans per second of virtual time over the engine's own cached plan (`plan_terrain_cached`, `SelectionCache`, `LodParams` of cells 32, 720 p, tau 3.3, finest level 22); tiles of a block that is not ready are drawn from the base field. A landed block invalidates the plan cache's tiles under it. Blocks not needed for 5 s are evicted.
- **Environment:** CPU only (no GPU, no window): the owner's lid was closed and no display was attached, and nothing here is a GPU timing. Other sessions were building and running tests; timings are indicative.

## 1 and 2. Flight, CON-22, the switch (6 workers, 0.25 s per km2)
Flight 30 km to 1.8 m over the site in 60 s, then the pan at 300 m (100 m/s east, 30 s). "Stand-in" is the share of the plan's nodes at or above the apply level whose block is not ready at that tick.

| Block | Path | Band | Stand-in share | Blocks needed (max) | Ready latency | Resident (max) | Frame CPU p50 / p99 / max (cached plan) |
|---|---|---|---|---|---|---|---|
| 4.9 km | flight | above 5 km | no node at the apply level | 0 | | 0 | 9.4 / 19.5 / 28.5 ms |
| 4.9 km | flight | 300 m to 5 km | 18 % (8,022 of 44,154) | 3 | 6.0 s | 3 MB | |
| 4.9 km | flight | below 300 m | 1 % (1,453 of 226,034) | 3 | 6.0 s | 4 MB | |
| 4.9 km | pan | 300 m | 19 % | 3 | 6.0 s | 4 MB | 4.9 / 6.5 / 75.7 ms |
| 9.8 km | flight | 300 m to 5 km | 100 % | 2 | 23.9 s | 0 | 7.5 / 15.9 / 34.1 ms |
| 9.8 km | flight | below 300 m | 23 % | 2 | 23.9 s | 12 MB | |
| 9.8 km | pan | 300 m | 75 % | 2 | 23.9 s | 12 MB | 5.1 / 11.5 / 71.0 ms |
| 19.5 km | flight and pan | all | 100 % (no block lands in 65 s: 95.5 s each) | 2 | 95.5 s | 0 | 8.7 / 21.8 / 46.3 ms |

- **CON-22:** `height` never waits: a block that is not ready is the base field, whatever the caller. In the runs 2,000 to 3,000 of 3,200 to 3,900 height calls were the coarse stand-in. The block run happens outside the frame (the simulation computed landed blocks between ticks). The worst frame times (28 to 76 ms) are plans after a landing invalidated the plan cache's tiles under the block (300 to 600 tiles per flight), not waits. Not tested: real threads (a landing writes the block table while the plan reads it; the prototype uses one `RwLock`).
- **The switch (pop):** the height change when a block lands on the visible fine tiles is the block's own change: p99 24.3 to 25.4 m, max 27.4 m for the stand-in's 30 m amplitude (324,000 to 672,000 vertices). The size is the process's, not the engine's; the engine must expect a pop of that size at the switch and hide it with a fade of the change over time (not built) or accept it far from the camera. A still pair was not made: the prototype is not in the renderer (headless harness only).
- **Requests:** sampling any height can request a block: 435 to 739 blocks asked in a run against 2 to 3 that the plan's fine nodes need. The request must come from the plan's nodes at the apply level (the prototype's `take_requests` from samples is what a naive field does).
- **The apply level:** a tile of level 11 has 32 cells of 153 m, so a block of 8 m cells shows only in tiles whose vertex spacing is at most 16 m: level 15 and finer (level 15: 9.6 m). Levels 11 to 14 draw the base for good; they never wait. A naive field (the block applied to every tile that overlaps it) changes coarse tiles too: 40,000 to 52,000 vertices of coarser tiles under the landed blocks changed by p99 25 m, max 27.8 m, so every coarse tile over the block would need a refresh.

## 3. Seams
Two blocks side by side (4.9 km, 8 m cells, 611 cells): the border row from each run. `steps` is the stand-in's relaxation steps, `halo` cells of the run's grid.

| Steps | Halo cells | max abs A-B | p99 | Kink across the border (m per cell; the noise floor is 0.71 to 0.93) |
|---|---|---|---|---|
| 8 | 0 | 0.0000 | 0.0000 | 4.04 |
| 8 | 2 | 0.2653 | 0.2256 | 0.98 |
| 8 | 4 | 0.0036 | 0.0028 | 0.93 |
| 8 | 8 or more | 0.0000 | 0.0000 | 0.93 |
| 32 | 0 | 0.0000 | 0.0000 | 4.41 |
| 32 | 2 | 1.9824 | 1.6499 | 1.16 |
| 32 | 4 | 0.3208 | 0.2616 | 0.77 |
| 32 | 8 | 0.0067 | 0.0048 | 0.71 |
| 32 | 16 or more | 0.0000 | 0.0000 | 0.71 |

- **Halo rule, in the run's cell units:** the halo in cells is at least the step count times the reach per step in cells (1 for a 3x3 stencil) for borders that agree exactly (the stencil radius: a border node then reads nothing the halo's edge changed); in this relaxation it is below 0.1 mm from halo = steps/2 (8 steps: halo 4 gives 3.6 mm and halo 8 gives 0; 32 steps: halo 16 gives 0.0000 at four decimals) because the influence decays; halo = steps/4 gives under 1 cm; steps/8 gives 0.3 m. SPIKE-33's steps translate as `halo_cells = steps x reach_cells_per_step` for an exact seam, `steps/2 x reach` for a seam under 0.1 mm for a diffusive process; in metres: x 8 m at this grid. A droplet process: the longest droplet path in cells.
- **Halo 0** has a zero height step (both runs hold the border at the base) and a kink of 4 m per cell: the surface bends at the border (the shading shows it), the height does not jump.
- **The gate seam** (new): a tile at the apply level (gated) against its level-14 parent, and a gated tile beside an ungated neighbour, share the vertices of the parent's grid. A hard gate (block change present at level 15 and finer, absent above) leaves at those vertices the block's change: **p99 25.2 m, max 27.9 m** at 30 m amplitude (p99 6.8 m at 8 m amplitude). **Morph does not hide it:** the shader morphs a tile's odd vertices to the average of the tile's own even neighbours (`terrain_vertex` in `compact.rs`), so at morph 1 a gated tile has its own even heights, which carry the change, while the parent draws the base; the step stays at the end of the morph. A band-limited change (a box-filtered pyramid: a tile of level l reads the filter of its own vertex spacing, 9.6 m at level 15, 19.1 m at 14) cuts the step at the parent's vertices to **p99 0.13 m, max 0.28 m** (p99 0.07 m at 8 m amplitude). The stand-in's relief is smooth (nothing under 50 m), so a process with sharper detail gives a larger residual: measure it with SPIKE-33's output. The tile data of level l then reads the pyramid level of l, which needs a `height_at_level(dir, level)` on the field (`HeightField::height` has no level today).

## 4. Plan and bounds
Plan over the base alone, the block field with no block ready, and with the blocks under the camera ready (4.9 km blocks, bounds widened by 2 x amplitude = 60 m, the plan cannot know which block is ready):

| Altitude | Base alone: nodes | Block field: nodes | capped | refused | Plan ms cold, base / block |
|---|---|---|---|---|---|
| 30 km | 626 | 626 | 0 | 0 | 10 / 25 to 30 |
| 5 km | 852 | 852 | 0 | 0 | 20 / 41 to 42 |
| 300 m | 1,347 | 1,404 | 0 | 0 | 29 / 69 |
| 30 m | 1,760 | 1,507 | 89 | 0 | 36 / 112 to 114 |
| 1.8 m | 1,955 | 1,502 | 87 | 0 | 39 / 117 to 119 |

- No refusal. The widened bounds make the plan hold 87 to 89 tiles back from refinement (LOD-008, "too steep to morph") close to the ground: the plan is coarser there by about a fifth (1,502 nodes against 1,955). Ready or not gives the same plan, because the bounds do not depend on readiness. The lever is a tight bound from a ready block's grid (its minimum and maximum, a few metres wide) and the process's own bound (±2 amplitude is a stand-in's guess); not measured here.
- The plan costs about 3 times as much cold (a `locate` and a table read per height sample on top of the base's): 117 ms against 39 ms at 1.8 m; with the plan cache 5 to 10 ms median. The cache's origin heights of the tiles under a landed block must be invalidated (`invalidate_tiles`; 300 to 600 tiles per flight).

## 5. Edits
Blocks a stamp invalidates: those within its reach plus the halo (320 m: 40 cells of 8 m), and the time until they are exact on 6 workers at 0.25 s per km2. The tile-level baseline is the tiles of level 15 the reach covers.

| Block | Stamp place | Reach 400 m | Reach 2,000 m | Time until exact | Level-15 tiles (400 m / 2 km) |
|---|---|---|---|---|---|
| 4.9 km | centre | 1 block | 3 (1 without the halo) | 6 s | about 13 / 177 |
| 4.9 km | 30 m inside an edge | 2 | 2 | 6 s | |
| 4.9 km | corner | 4 | 4 | 6 s | |
| 9.8 km | centre / edge / corner | 1 / 2 / 4 | 1 / 2 / 4 | 24 s | |
| 19.5 km | centre / edge / corner | 1 / 2 / 4 | 1 / 2 / 4 | 96 s | |

- The halo adds blocks only where the reach plus halo crosses a border (3 against 1 at the 4.9 km block with 2 km reach; none for blocks of 9.8 km). A stamp at a corner touches 4 blocks.
- **The latency is the finding:** an edit is exact after one wave of block runs: 6, 24 or 96 s with SPIKE-32's cost, against tens of milliseconds for the 13 tiles refreshed today (EDIT-latency). The editor needs stale-while-revalidate for blocks too (REND-032 draws a stale tile while its refresh runs; the block result of the old state stands in), a preview that does not wait (VIEW-036 transient stamps already draw the edit on the old tiles), and the block run restricted to the edited area when the process allows it (not measured).

## 6. Memory and cache
| Block | Cell | Cells | Bytes per block (f32 change) | Stand-in run, one thread | Model, 0.25 s per km2 |
|---|---|---|---|---|---|
| 4.9 km | 8 m | 611 | 1.5 MB | 0.8 s | 6.0 s |
| 4.9 km | 4 m | 1,222 | 6.0 MB | 3.0 s | 6.0 s |
| 9.8 km | 8 m | 1,222 | 6.0 MB | 2.8 s | 23.9 s |
| 9.8 km | 4 m | 2,443 | 23.9 MB | 9.4 s | 23.9 s |
| 19.5 km | 8 m | 2,443 | 23.9 MB | 11.1 s | 95.5 s |

- Resident blocks: 3 to 4 at 300 m and at 30 m for 4.9 km blocks (4 MB), 2 for 9.8 km (12 MB); the flight asks for 3 blocks in all (the site's block and two neighbours). Eviction after 5 s without need evicted 0 to 2 blocks per run. Memory is not the constraint; with heights only. A drainage or sediment channel at the same grid multiplies the bytes by the channel count.
- **Cache key:** `inputs_hash_for(tile)` chains the base's key with the content hash of the block result the tile reads (a tile coarser than the block grid has the base's key, whatever the blocks hold; a tile whose block is not ready has its own word 0), so a stand-in tile and an exact tile never share a key. ADR 0022's stage key plus the block grid, as the ADR says; the same shape as the stamp stage's chain (STRM-027).

## 7. Block size
| Block | Request count over the flight (blocks served) | Latency at 0.25 s per km2 | Stand-in share at 300 m (pan) | Edit time until exact | Bytes per block (8 m) |
|---|---|---|---|---|---|
| 4.9 km | 3 | 6 s | 19 % | 6 s | 1.5 MB |
| 9.8 km | 2 | 24 s | 75 % | 24 s | 6 MB |
| 19.5 km | 2 | 96 s | 100 % | 96 s | 24 MB |

**Recommendation:** blocks of about 5 km (level 11) with cells of 8 m for this cost; the cost per block should stay under about 10 s on the available threads (the cost grows with the area: a 2.5 km block is a quarter of it, with four times the blocks to request). A larger block is only viable if the process costs about a tenth per km2 or the blocks are prefetched along the travel (STRM-018's prefetch set for blocks: the pan moves 100 m/s through a 4.9 km block in 49 s, so a prefetch of the next block one block ahead hides 6 s fully; the 24 s of the 9.8 km block also needs one block ahead plus a camera under 200 m/s).

## What each subsystem needs
- **Streaming:** a block job class beside the tile job (injected spawner, CON-04), priority by distance, prefetch along the travel; requests come from the plan's nodes at the apply level (a plan-side set `B`, as R and D are); eviction by need over time.
- **Cache:** the tile key chains the block result's content hash (a block field version and grid in the stage key, ADR 0077 point 1); a tile whose block is not ready is a stand-in with its own key and is refreshed when the block lands (the stale-tile machinery of REND-032); tiles below the apply level are identity on the base (STRM-029's rule).
- **Plan:** the apply level; origin heights of the tiles under a landed block invalidated; bounds from the ready block's grid (tight) or the process's bound, since a loose bound holds back about a fifth of the tiles near the ground.
- **Field API:** a level-aware sample (`height_at_level`) so that a tile of level l reads the pyramid level of l (item 3's gate). `HeightField::height` is position-only today.
- **Editor:** blocks are slow to refresh: stale-while-revalidate, the transient preview, and a restricted rerun where possible; the edit-latency budget (tens of ms) cannot be met by a block run of seconds.

## Not done and limits
- No render still pair (item 2): the prototype is headless; the pop is given as numbers of the stand-in's change (its amplitude is an assumption).
- The compute is a stand-in; its latency is SPIKE-32's recipe, not SPIKE-33's method. The seams and the gate residual depend on the process (diffusive, smooth).
- Face edges and poles are untested (the site is mid-face); the halo beyond a face edge continues on the extended cube plane.
- Tight bounds, a restricted rerun for edits, real threads, and a time fade of the switch are not measured.
- Raw output: `raw-flight-pan.txt`, `raw-edits.txt`, `raw-bounds.txt`, `raw-gate.txt`; the run commands are in `crates/frame/examples/block_stage.rs` of the patch (`seams`, `cost`, `flight`, `pan`, `edits`, `bounds`, `gate`).
