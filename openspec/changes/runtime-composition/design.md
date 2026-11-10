# Item 15 design notes: sampler composition, lakes, rivers, G-RT

Sources: `docs/design/terrain-target.md` (A3) sections 3.1, 3.3, 5.2, 5.3, 5.5, 11.2, 12.1 (G-SURF, G-PROFILE, G-BASIN, G-RT); ADR 0101 (fill depth), ADR 0041 and 0056 (water field and coastal band), ADR 0072 (stale pre-emption); briefing items 7, 11, 14, 15, 16. Written 2026-11-10 from the documents and the code on main; nothing measured for this note.

## 1. State on main
| Part | What exists | Where |
|---|---|---|
| Base `B` | the lattice's C1 read, `read_elevation_c1` and the cached read, pure in the direction, f32 once (GEN-107) | `crates/generators/src/global.rs` |
| Local change `L` | kind-13 layer of the old two-pass bake; the runtime adds `change + fill depth` (GEN-112, ADR 0101 point 3) | tile source, `terrain.rs`, `erosion.rs` |
| New chain | K4 (rounds), K6 (H1: channels, basins, `Q_w`), K7 on a tile, the complete build on the phase plan (GEN-158); none of it reaches the runtime | `crates/generators/src/bake/` |
| Water | the water field comes from ADR 0041's shoreline and the coastal band (ADR 0056); a `WaterBook` holds the tile water; no lake object, no river drawing | `crates/frame/src/water.rs`, `crates/app/src/worldfield.rs` |
| Creases `C` | evaluated at read time (GEN-109) | `terrain.rs` |
| G-SURF | gate at the cell centres holds (GEN-148); vertices reported as G-SURF-I | explorer, item 11 |

## 2. Step 1: sampler composition (A3 section 3.1)
Goal: the runtime reads `S` with `L` from the new chain's K7 (total change against `B`) and no fill depth.
- Switch point: A3 3.1 makes the retirement of the fill depth wait for G-BASIN and the lake producer: until a lake is drawn, a basin would show a dry hole. So the switch cannot ship before step 2 for any world with basins; the order is step 2 first, then the switch.
- The input is a stored layer: item 14 (G-STORE) defines the serialized `L` and the key table; item 15 reads it through the existing tile source and folds `base_id` into the key (A3 3.2). A mismatch is refused naming both keys (STRM-065).
- Gate: G-SURF and G-SURF-I on a tile, then G-PROFILE (section 4).

## 3. Step 2: lakes (A3 section 5.5, K11)
Goal: a basin kept by H1 is drawn as water at the level of the balance; the fill depth stops being a surface term.
- Producer: from the basin object of the build (floor, spill, area curve, class, `Q_in`, `E_open`), the level per section 5.5's three cases, `q_out` at the spill. All of it is bake output (tile data, not per-frame work, A3 11.2): the producer is a bake-time step writing the level and the class per basin into the store; the runtime only reads.
- Runtime side: the water field gets a source besides the shoreline: per tile the cells below the basin's level at the tile's resolution (the area curve answers "what is flooded at level x" without a raster, A3 5.5).
- Endorheic playa and `Q_in = 0`: the floor, no water surface.
- It needs item 11's pass (the surface the basins are taken on), G-BASIN's fixtures for the levels (1e-9), and the finding of item 13 (results section 6b, finding 2): the straight-edged wedges of the 1:1 sheets are H1 channel cells inside kept lakes, so a lake draws as water and the channel inside it is not drawn as a river line (A3 5.2: a channel passes through a basin and continues below its spill point).
- Gate: G-BASIN (the explorer's fixtures) plus a view of the prototype region from 2 km at three lakes: the level against the sill.

## 4. Step 3: rivers (A3 sections 3.3, 5.2)
- Source: K6 segments (centrelines with the level profile and `Q_w`), stored as tile data (item 14).
- Drawing: a ribbon or a water-pass mask along the segment from a width law of `Q_w` (a power law to be named); the river's water level is the segment's profile, which K7 burned into `L` (burn-in rule), so the ground and the water agree to the quantisation step.
- Gate G-PROFILE (A3 12.1): along every segment through the complete read `S` at the local nodes, `C` off and on; no rise above 0.25 m; tile-border step at most 0.25 m. It needs the stored segments and `L`; first a test on the fixture tile (K7 of one tile exists: the stage manifest's `k7-f1-t3-4`).
- Risk: a C1 read lifts diagonal links between nodes by design (reported, not gated); a river drawn at the node profile can float over a lifted link by the reported rise, so the width law must cover it.

## 5. G-RT: the region's flight
No held-back tiles; frame within BUDG-001 and BUDG-002. The flight is the prototype region at 12,288 cells per face edge (item 13's windows R3a and R3b are candidates for a second flight). Measured by `test-render --perf` on the lit path with the new `L`: a perf window (the planner authorises, bulldozer coordinates). The smoke runs on the software adapter first (the run-time stamps of TEST-106 and the streaming smoke's marks tell where a run is).

## 6. Order and sizes (estimate, one lane)
1. Lake producer and the lake water source (bake step, store field, water source): the largest step, about 3 PRs (producer with fixtures, store field, water pass source).
2. Sampler switch (retire the fill depth): 1 PR after step 1 and item 14's store field.
3. Rivers: segment store read, ribbon drawing, G-PROFILE: about 3 PRs.
4. G-RT flight and the report: 1 PR plus a perf window.
Each PR carries a requirement or two in `streaming-cache`, `renderer` or `generator-pipeline`; ids from the planner.

## 7. Questions for the architect (before step 1)
1. Is the lake producer at bake time in `generators` (proposed) or in the runtime from the stored basin objects?
2. The river width law: named parameters in the look file or derived from `Q_w` only?
3. Does item 14 store `L` as a new kind or as a version of kind 13? The answer fixes step 2's key.
4. Which of the two windows (R3a, R3b) or which stored face region is G-RT's flight?
5. Is the drawing of H1 channel cells inside kept lakes dropped at the producer (stored) or at draw time?
