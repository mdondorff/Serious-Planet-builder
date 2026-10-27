# Roadmap (non-binding)

Status: direction, not a plan (owner and opus-planner, 2026-10-23). Each milestone gets its own plan in `docs/briefings/` when it starts; order and content change with the look reviews. The original milestone table (background report §15) ended with M8 polish and M9 cloud tiles; M8 became the natural landscape (realism first), by the owner's choice.

## Milestones
| # | Milestone | Content | Status |
|---|---|---|---|
| M0 to M7 | Foundations to cities | coordinates, LOD, fields and cache, streaming, atmosphere and ocean, editor, cities | done |
| M8 | Natural landscape | global coupled river network, chunk erosion at 4 to 10 m (ADR 0077), plausible relief, horizon shadows, driver maps to materials; the speed envelope (`docs/briefings/m8-plan.md`) | running |
| M9 | Realistic terrain | hill terrain against the owner's look reference: erosion that shapes hills and forms its own tributaries inside chunks; slope targets checked on real elevation samples (owner, 2026-10-25: likely M9's main focus; hydrology and relief together were too much for one milestone) | next |
| M10 | Living surface | vegetation, rocks and ground cover placed from the driver maps (wetness, soil depth); GPU instancing (SPIKE-28); the speed envelope stays a regression check | open |
| M11 | Water and sky | rivers and lakes as water surfaces from the global network, FFT ocean, clouds, weather light | open |
| M12 | Settled world | cities on the natural landscape: roads follow terrain and cross rivers, settlement placement from the driver maps, night lights | open |
| M13 | Quality and reach | quality tiers, the 8 GB VRAM tier, upscalers, packaging | open |
| later | Game layer | physics, a player or vehicle, a game run mode on the planet library (below) | optional |
| later | Cloud tiles | tile server over `generators` and `cache` (the original M9) | optional |

M10 and M11 may swap after the M8 look reviews. Spikes for M9 start once M8's hydrology and shadows are close to landing (owner, 2026-10-25).

Option (owner, 2026-10-26): horizon shadows may become an exception to identical results across machines (e.g. a GPU bake) if speed needs it; not now, needs an ADR when taken.

Generator polish, after M8: why a stamped hill in the ocean ends just below sea level.

Data later (owner's idea, 2026-10-25): stores read through a local or remote data server, the local disk a cache, larger bakes streamed on demand; a backend behind the pack store's interface.

## Deferred to M9 (running list, opus-planner)
- Uplift that does not follow the input network (SPIKE-45; ADR 0080 point 4: the coupled run has no stream-power erosion at M8).
- Coarse/fine network: the coarse network climbs beyond a 1 km node slide; deep cuts where the routed ground rises (Tracked).
- Fine tributary tier inside chunks (ADR 0081 point 3; SPIKE-40 drainage density 0.5-3 km per km2).
- Hill terrain realism against the look reference; slope targets from real elevation samples.
- An elevation data source and its licence.
- Edits on top of the erosion (D3, per-layer setting).
- The photo layer (D6).
- Flight lead margin (Tracked): ask the next chunk on entering the current one; lead at least the solve time.

## Game layer: direction
- One workspace and one technology: the planet crates (`core`, `world-def`, `generators`, `cache`, `streaming`, `frame`, `render`) stay a library without game logic; a game is its own crate and run mode on top.
- Embedding into another engine (Bevy, Unreal) is not planned: it would give up the frame loop and the test harness.
- Determinism serves physics and multiplayer: collision comes from the same chunk fields as the image; clients regenerate identical terrain from the world definition, so only edits and moving objects need syncing. Multiplayer stays a non-goal of v1.
- Three seams keep the door open at no game cost:

| # | Seam | First user |
|---|---|---|
| 1 | One query interface: height, normal, material and water at a point, and a collision patch around a point, from the chunk fields (`height_at_level` is its start) | M8 chunks |
| 2 | The renderer draws outside objects in the same camera-relative frame and depth | M9 instancing |
| 3 | A fixed-step simulation clock separate from the frame clock | the spike below |

## Proposed spike: game-layer exploration (after M8)
| Field | Value |
|---|---|
| Question | Do the query interface and local physics frames carry a driven vehicle on chunk terrain at ground level without precision or seam faults? |
| Setup | Rapier (Rust; f64 option) in a local ENU frame (background report §5) around the vehicle; collision patch from chunk heights at 1.8 m; fixed 60 Hz step; the existing renderer draws the vehicle |
| Measures | contact jitter at chunk borders and frame rebases, collision patch build time, step time, height difference between collision and the drawn surface |
| Time box | 1 day, on a spike branch; no merge into the main crates |
| Decides | whether a game run mode needs new engine seams before M12 |
