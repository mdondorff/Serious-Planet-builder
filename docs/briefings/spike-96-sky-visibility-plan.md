# SPIKE-96 plan: sky visibility for the terrain's sky ambient
Status: plan only (crusader lane, 2026-11-03, for opus-planner); no code, no perf run. Starts after the owner's look decision on the sky ambient (ADR 0095, proposed) and a perf slot.

## Question
- The terrain's ambient is the sky radiance in the cosine lobe of the surface (ADR 0095 point 3: the terrain does not occlude the sky, so a valley floor gets the whole hemisphere).
- Which cheap, edit-aware way gives the sky ambient a visibility factor `V` in [0, 1] (the share of the sky a surface sees), so that valley floors and the foot of cliffs are darker than ridges?

## Inputs on main or on a branch
| Item | Where | Fact used |
|---|---|---|
| Clipmap march | ADR 0085, 0090, REND-049 | DDA with max-mip skipping over 11 levels, half-res at 5 degrees of sun or more, quarter-res below; layer 1 budget 1.5 ms at 1440p (REND-052) |
| Sky ambient prototype | branch `change/shadow-sky-ambient` (unpushed), ADR 0095 / REND-054 proposed | five taps of the sky-view table (ring 0.766, lift 0.6428), cosine-weighted; one table lookup each |
| Horizon bake | `generators::horizon` (GEN-044), `cache::horizon_store`, STRM-064 | per texel, 16 azimuths, elevation of the highest terrain out to the range; 8 m cell; kept until this spike decides (owner, 2026-11-02) |
| Horizon finding | owner, 2026-11-02 | hard sun shadows expose horizon-map errors (glints, hard contrast); soft hemispherical light averages them out |

## Candidates
| Id | Method | Runtime cost | Edits | Risk |
|---|---|---|---|---|
| A | Short marches per sky tap: for each of the five sky taps march the clipmap a short reach (about 1 to 2 km) towards the tap direction; `V_i` weights tap `i` | up to 5 marches per pixel; temporal rotation (one tap per frame, history as REND-050) cuts it to 1 | live: the march reads the stamp deltas (REND-045) | cost against REND-052's 1.5 ms; noise at one tap per frame |
| B | Per-tile baked sky-view factor: `SVF = 1 - mean_k(sin^2 h_k)` from the horizon bake, stored as one channel per tile cell and read as a texture | one texture read per fragment | baked again after an edit (STRM-064 jobs), so it lags | needs the horizon jobs running again; coarse lattice (8 m cell, 16 azimuths) smooths small relief |
| C | Hybrid: B for the far field, A with one tap near the camera | between the two | near: live; far: baked | two paths to keep equal |

## Measures (decided before the runs)
- Reference: a CPU brute-force `V` per pixel (at least 256 rays on the sphere, cosine-weighted, marched over the same height field), tier A on three synthetic fields (a plain: V = 1; a V-valley with 30 degree walls; a ridge) and on the six package poses (hills-b, river-r1 at 300 m and 30 m).
- Error: the GPU result against the reference, mean and p99 of |V - V_ref| per pose; the bound is proposed as p99 below 0.1 (the look is soft light, not a shadow).
- Cost, GPU timestamps at 1440p on the reference machine, median of 41 frames, in the perf slot: the sky visibility pass or term alone; the proposed limit is 0.5 ms inside the terrain share (to be set in BUDG-002 by the planner).
- Edits: a dragged stamp changes `V` under it within 3 frames for A, within the rebake latency for B (reported, not judged).
- Looks (owner): stills at 2 and 7 degrees of sun, valley floor and ridge, with and without `V`; the owner picks by eye.

## Steps
1. S1: CPU reference `sky_visibility` in `frame` or `testkit` (f64, libm, CON-14) and its tier-A tests on the synthetic fields.
2. S2: candidate A in the terrain ambient on the prototype branch (taps from ADR 0095), stills only; error against S1.
3. S3: candidate B: read the existing horizon store (chunk maps) into an SVF map on the CPU, upload as one texture channel per resident chunk, error against S1. Needs the horizon jobs enabled by the harness (the viewer runs none since PR 6).
4. S4: perf slot: cost of A (one tap per frame and five taps) and of B; the owner's look stills.
5. S5: an ADR with the choice and its numbers (planner decides who writes it); the horizon bake data path is removed only if B loses (PRs 7 and 8 of `shadow-clipmap-march`).

## Out of scope
- Terrain occlusion of the sky radiance itself (the sky is the camera's), ground bounce, water ambient (REND-054's follow-up).
- Sun shadows (layer 1) and object shadows (layer 2).

## Stop conditions
- A's five-tap form costs more than 0.5 ms and the one-tap form is visibly noisy: report, run C.
- B's lattice error shows at the owner's stills: report before building anything.
