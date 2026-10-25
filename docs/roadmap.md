# Roadmap (non-binding)

Status: direction, not a plan (owner and opus-planner, 2026-10-23). Each milestone gets its own plan in `docs/briefings/` when it starts; order and content change with the look reviews. The original milestone table (background report §15) ended with M8 polish and M9 cloud tiles; M8 became the natural landscape (realism first), by the owner's choice.

## Milestones
| # | Milestone | Content | Status |
|---|---|---|---|
| M0 to M7 | Foundations to cities | coordinates, LOD, fields and cache, streaming, atmosphere and ocean, editor, cities | done |
| M8 | Natural landscape | global coupled river network, chunk erosion at 4 to 10 m (ADR 0077), horizon shadows, driver maps to materials; the owner's look reference; the speed envelope (`docs/briefings/m8-plan.md`) | running |
| M9 | Living surface | vegetation, rocks and ground cover placed from the driver maps (wetness, soil depth); GPU instancing (SPIKE-28); the speed envelope stays a regression check | next |
| M10 | Water and sky | rivers and lakes as water surfaces from the global network, FFT ocean, clouds, weather light | open |
| M11 | Settled world | cities on the natural landscape: roads follow terrain and cross rivers, settlement placement from the driver maps, night lights | open |
| M12 | Quality and reach | quality tiers, the 8 GB VRAM tier, upscalers, packaging | open |
| later | Game layer | physics, a player or vehicle, a game run mode on the planet library (below) | optional |
| later | Cloud tiles | tile server over `generators` and `cache` (the original M9) | optional |

M9 and M10 may swap after the M8 look reviews.

Generator polish, after M8 (owner, 2026-10-25: M8 proves the engine at chosen places; global realism is later work): middle-scale relief on lowland by landform instead of a flat detail amplitude (about 30 m today, `generators/terrain.rs`); erosion that forms its own tributary network inside chunks; why a stamped hill in the ocean ends just below sea level.

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
