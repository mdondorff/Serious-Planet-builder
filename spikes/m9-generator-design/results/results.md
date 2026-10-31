# SPIKE-76: the generator, re-derived for realism (design study)

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: Which rules of today's terrain chain are right, arbitrary or harmful according to SPIKE-55 to SPIKE-75? What cleaner chain would replace them (the owner, 2026-10-31: a radical re-creation is welcome)? Which terrain ADRs would it keep, amend, supersede or retire?
Why it matters: the owner decides ADR 0088 and the M9 scope on return; these are proposals for that decision.
Time box: 3 h (opus-planner, 2026-10-31; Opus); used about 1 h 30 min, with 10 min of machine time.
Success criteria:
- the inventory of rules with verdicts;
- the ADR table with verdicts and evidence;
- a proposed chain with what it deletes, keeps and changes in the store;
- a prototype where the spike code allows it;
- a candidate ADR outline;
- the effect on ADR 0088 and the M9 R-items.
Code location: branch `spike/m9-plains` (spike code, never merged): the `terrain_score` example takes `S76_DIR` (`proto/s76_block.rs.txt`).
Closed: 2026-10-31

## 1. Inventory: today's terrain chain
| # | Rule (where) | What it is for | Verdict | Evidence |
|---|---|---|---|---|
| 1 | Continents: fBm of a domain-warped direction, land above 0.1 (`global.rs` `terrain`) | land and sea, coastlines | **right** | coasts and shares are not in question in any spike; SPIKE-69's map |
| 2 | Land base elevation `1000 smoothstep(0, 0.9, land depth)` (`global.rs` `sample`) | low coasts, higher interiors | **arbitrary** | elevation is tied to distance from the coast, not to a landform; no high flat plateau can exist (SPIKE-75) |
| 3 | Ranges `5200 ridge zone inland` (`global.rs`) | mountain belts, 50-500 km | **right in shape, harmful as the only source of height** | the arid sites match into range foothills because only ranges are high (SPIKE-75); belts at 50-500 km are fine (SPIKE-69's maps) |
| 4 | Hill band, 2-8 km, 30-1,000 m by `zone inland (0.25 + 0.75 ridge)` (`global.rs`, ADR 0080 point 1) | hill relief in the bake | **too weak** | hills are 4-9x too gentle with it (SPIKE-55); amplitude x4-6 is the lever (SPIKE-65) |
| 5 | Landform driver `L = zone inland` on a lattice (GEN-062, ADR 0080 point 3) | where relief goes | **right as a source, wrong as a weight** | linear `1 + 5 L` barely acts (SPIKE-66); through a land-quantile curve it matches Earth's shares (SPIKE-69); it does not separate Alps from hills, nor arid plateaus from foothills (SPIKE-75) |
| 6 | Crease amplitude `A(E) = 80 + 900 smoothstep(-200, 300 m, E)` (`erosion.rs`, GEN-017) | rougher land than sea, a coast ramp | **arbitrary above 300 m** | full for every hill, mountain and arid site; separates nothing (SPIKE-75); sound only as a coast taper |
| 7 | Crease slope factor `carve(slope)`, floor 0.15 (`erosion.rs`) | creases where the bake is steep | **harmful on plains** | with item 8 it switches the fine creases off on flat ground: plains 91 % flat (SPIKE-73) |
| 8 | Fine creases `carve + (1 - carve) L` under 1.6 km (GEN-061, ADR 0080 point 2) | landform-keyed fine relief | **harmful** | about 0 on plains (SPIKE-73); a floor of about 0.2-0.35 is the fix |
| 9 | Coupled run: fill and routing on a coarse grid, no stream power (ADR 0080 point 4) | the river network, water | **right** | rivers and water consumers are not in question; stream power to steady state erases landforms (SPIKE-65) |
| 10 | Terrain field detail `30-300 m over 0-5 km of E` (`terrain.rs`, `cpu-terrain-v1`) | detail of the reference field | **not in the production chain** | production is `cpu-erosion-v1` (SPIKE-75) |
| 11 | Climate: temperature by latitude and height, precipitation bands (`global.rs`) | materials, snow, water | **right for climate, unusable for landform** | the generator's dry places are not its arid landforms (SPIKE-75: Highlands site at 100 mm, arid matches at 410-1,274 mm) |
| 12 | Shader detail G3 (REND-034) and height octaves G1 | the ground at 1.8 m | **keep with fixes** | gradient noise (SPIKE-67), G1 law a (owner, SPIKE-63) |
| 13 | ADR 0088's bake (proposed): `k`, `m`, two-pass carve, thermal, deposition | relief, valleys | **right in mechanism** | seams clean (SPIKE-70 to SPIKE-72, SPIKE-74); time 6-7.5 h (SPIKE-71); `k`'s source is the open part (SPIKE-74, SPIKE-75) |

## 2. Prototype: amplitude per relief class
The pipeline of SPIKE-74 with `k` and the fine-crease floor taken from an assigned class per landform, as an authored class map would give them. The class is uniform over each 55 km window and its 400 km pass-1 window.
- **This is an upper bound:** the classes are the right ones by construction, and the amplitudes were set from SPIKE-74 and SPIKE-75 at the same sites (in-sample).
- **Classes:**
  | Class | `k` | Floor |
  |---|---|---|
  | plain | 1 | 0.2 |
  | rolling plain | 1 | 0.35 |
  | badlands (arid rugged) | 1 | 0 |
  | sand sea (arid flat) | 0.35 | 0 |
  | hills | 5 | 0 |
  | high mountains | 7.5 | 0 |
- `ref` is SPIKE-74's pipeline (`k` from L).
- The floor between 0 and 0.35 is blended linearly between the two samples (an approximation of the floor's `max`).

| Landform (assigned class; k, floor) | Site | Variant | Slope gen/real (distance) | Relief 1 km gen/real (distance) | Relief 5 km distance | Flat (real) | Classes L1 |
|---|---|---|---|---|---|---|---|
| Brandenburg plain (plain; 1, 0.2) | 1 | baseline | 0.49 (0.19) | 0.38 (0.35) | 0.39 | 91.0 % (22.6 %) | 1.39 |
|  | 1 | ref | 2.01 (0.09) | 1.06 (0.09) | 0.26 | 1.2 % (22.6 %) | 0.81 |
|  | 1 | class | 1.65 (0.08) | 0.94 (0.06) | 0.13 | 4.6 % (22.6 %) | 0.79 |
|  | 2 | baseline | 0.49 (0.19) | 0.36 (0.36) | 0.44 | 89.6 % (22.6 %) | 1.38 |
|  | 2 | ref | 1.99 (0.10) | 1.01 (0.06) | 0.13 | 1.5 % (22.6 %) | 0.79 |
|  | 2 | class | 1.67 (0.07) | 0.91 (0.05) | 0.15 | 4.2 % (22.6 %) | 0.80 |
| Loess hills and plains (rolling plain; 1, 0.35) | 1 | baseline | 0.22 (0.39) | 0.33 (0.78) | 0.64 | 52.4 % (1.0 %) | 1.48 |
|  | 1 | ref | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % (1.0 %) | 0.62 |
|  | 1 | class | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % (1.0 %) | 0.62 |
|  | 2 | baseline | 0.23 (0.39) | 0.35 (0.76) | 0.56 | 48.9 % (1.0 %) | 1.43 |
|  | 2 | ref | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % (1.0 %) | 0.63 |
|  | 2 | class | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % (1.0 %) | 0.63 |
| Badlands (badlands (arid rugged); 1, 0) | 1 | baseline | 0.83 (0.12) | 0.87 (0.11) | 0.04 | 0.0 % (2.6 %) | 0.31 |
|  | 1 | ref | 4.31 (0.97) | 2.94 (1.24) | 1.77 | 0.0 % (2.6 %) | 0.44 |
|  | 1 | class | 0.96 (0.06) | 0.63 (0.22) | 0.53 | 0.1 % (2.6 %) | 0.43 |
|  | 2 | baseline | 1.39 (0.10) | 1.36 (0.21) | 0.36 | 0.0 % (2.6 %) | 0.23 |
|  | 2 | ref | 7.65 (1.70) | 5.48 (2.78) | 3.93 | 0.0 % (2.6 %) | 0.28 |
|  | 2 | class | 2.43 (0.48) | 1.83 (0.58) | 0.78 | 0.0 % (2.6 %) | 0.57 |
| Erg Chebbi desert (sand sea (arid flat); 0.35, 0) | 1 | baseline | 2.75 (0.32) | 2.83 (0.26) | 0.29 | 0.0 % (47.9 %) | 1.28 |
|  | 1 | ref | 16.83 (2.95) | 11.42 (1.59) | 1.74 | 0.0 % (47.9 %) | 1.31 |
|  | 1 | class | 3.46 (0.65) | 3.04 (0.30) | 0.27 | 0.6 % (47.9 %) | 1.34 |
|  | 2 | baseline | 3.67 (0.48) | 3.65 (0.37) | 0.38 | 0.0 % (47.9 %) | 1.30 |
|  | 2 | ref | 19.85 (3.36) | 13.25 (1.88) | 1.98 | 0.0 % (47.9 %) | 1.29 |
|  | 2 | class | 1.96 (0.21) | 1.30 (0.10) | 0.15 | 6.5 % (47.9 %) | 0.92 |
| Valley and Ridge (hills; 5, 0) | 1 | baseline | 0.18 (0.36) | 0.17 (0.46) | 0.88 | 4.6 % (0.0 %) | 0.52 |
|  | 1 | ref | 1.16 (0.04) | 0.73 (0.21) | 0.46 | 0.0 % (0.0 %) | 0.63 |
|  | 1 | class | 1.08 (0.02) | 0.71 (0.21) | 0.45 | 0.0 % (0.0 %) | 0.63 |
|  | 2 | baseline | 0.17 (0.36) | 0.16 (0.46) | 0.88 | 6.1 % (0.0 %) | 0.57 |
|  | 2 | ref | 1.23 (0.08) | 0.80 (0.17) | 0.33 | 0.0 % (0.0 %) | 0.62 |
|  | 2 | class | 1.14 (0.05) | 0.77 (0.17) | 0.33 | 0.0 % (0.0 %) | 0.63 |
| Black Forest (hills; 5, 0) | 1 | baseline | 0.18 (0.35) | 0.17 (0.46) | 0.54 | 1.8 % (0.0 %) | 0.58 |
|  | 1 | ref | 1.32 (0.07) | 0.83 (0.17) | 0.17 | 0.0 % (0.0 %) | 0.44 |
|  | 1 | class | 1.30 (0.06) | 0.84 (0.17) | 0.18 | 0.0 % (0.0 %) | 0.44 |
|  | 2 | baseline | 0.16 (0.36) | 0.16 (0.47) | 0.54 | 6.1 % (0.0 %) | 0.84 |
|  | 2 | ref | 1.37 (0.07) | 0.88 (0.16) | 0.15 | 0.0 % (0.0 %) | 0.43 |
|  | 2 | class | 1.23 (0.06) | 0.84 (0.15) | 0.14 | 0.0 % (0.0 %) | 0.45 |
| Scottish Highlands (hills; 5, 0) | 1 | baseline | 0.18 (0.30) | 0.14 (0.39) | 0.58 | 16.3 % (4.9 %) | 0.67 |
|  | 1 | ref | 1.37 (0.11) | 0.70 (0.15) | 0.29 | 0.0 % (4.9 %) | 0.75 |
|  | 1 | class | 1.21 (0.07) | 0.66 (0.16) | 0.29 | 0.0 % (4.9 %) | 0.77 |
|  | 2 | baseline | 0.19 (0.30) | 0.15 (0.39) | 0.58 | 13.8 % (4.9 %) | 0.69 |
|  | 2 | ref | 1.58 (0.15) | 0.79 (0.16) | 0.29 | 0.0 % (4.9 %) | 0.75 |
|  | 2 | class | 1.32 (0.09) | 0.72 (0.17) | 0.29 | 0.0 % (4.9 %) | 0.77 |
| Alps, Tyrol (high mountains; 7.5, 0) | 1 | baseline | 0.12 (0.43) | 0.11 (0.46) | 0.57 | 0.1 % (1.3 %) | 0.26 |
|  | 1 | ref | 0.88 (0.09) | 0.57 (0.22) | 0.28 | 0.0 % (1.3 %) | 0.42 |
|  | 1 | class | 1.11 (0.08) | 0.75 (0.15) | 0.17 | 0.0 % (1.3 %) | 0.28 |
|  | 2 | baseline | 0.11 (0.44) | 0.10 (0.46) | 0.59 | 0.0 % (1.3 %) | 0.22 |
|  | 2 | ref | 0.79 (0.11) | 0.48 (0.27) | 0.35 | 0.0 % (1.3 %) | 0.50 |
|  | 2 | class | 1.02 (0.08) | 0.61 (0.21) | 0.26 | 0.0 % (1.3 %) | 0.37 |
| Himalaya, Everest (high mountains; 7.5, 0) | 1 | baseline | 0.23 (0.63) | 0.24 (0.78) | 0.86 | 0.0 % (0.0 %) | 0.09 |
|  | 1 | ref | 1.08 (0.06) | 0.87 (0.15) | 0.12 | 0.0 % (0.0 %) | 0.28 |
|  | 1 | class | 1.19 (0.17) | 1.07 (0.20) | 0.19 | 0.0 % (0.0 %) | 0.26 |
|  | 2 | baseline | 0.14 (0.71) | 0.14 (0.89) | 1.01 | 0.0 % (0.0 %) | 0.07 |
|  | 2 | ref | 0.84 (0.18) | 0.55 (0.44) | 0.56 | 0.0 % (0.0 %) | 0.42 |
|  | 2 | class | 0.99 (0.07) | 0.70 (0.27) | 0.36 | 0.0 % (0.0 %) | 0.34 |

**Verdicts with the classes** (criterion 1: slope and 1 km relief at 0.6 of real or more at both sites; "not worse" against the baseline's distances):
| Landform | `ref` (k from L) | class |
|---|---|---|
| Valley and Ridge | pass | pass (1.08-1.14 / 0.71-0.77) |
| Black Forest | pass | pass (1.23-1.30 / 0.84) |
| Scottish Highlands | pass | pass (1.21-1.32 / 0.66-0.72) |
| Alps | fail (0.48-0.57) | **pass** (1.02-1.11 / 0.61-0.75) |
| Himalaya (not in criterion 1) | 0.55-0.87 relief | 0.70-1.07 relief |
| Badlands | much worse (distances 0.97-3.93) | still worse (0.06-0.78 against 0.04-0.36): the carve itself adds relief; the class needs `k` under 1 or a gentler carve |
| Erg Chebbi | much worse (1.59-3.36) | site 2 better than the baseline (0.10-0.21 against 0.37-0.48); site 1 mixed (slope 0.65 against 0.32) |
| Brandenburg | flats 1-2 % | flats 4-5 % (real 23 %), relief 0.91-0.94 |

## 3. Proposal: a cleaner chain
| Step | What | Keeps | Replaces |
|---|---|---|---|
| A. Structure (bake) | continents and coasts; mountain belts at 50-500 km; climate | rules 1, 3 (shape), 11 | - |
| B. **Relief-class map** (new, pre-bake, authorable) | a class per point (plain, rolling plain, hills, low and high mountains, plateau, badlands, sand sea, ...) on a lattice like the landform lattice (about 39 km), bilinear weights between classes | rule 5 as the default source | the hill band (4), the landform weight (5, 8), the elevation ramp (6) |
| B1. Default classes | from L through SPIKE-69's quantile curve, plus a second driver for mountain strength (the bake's 50 km spread or the range term), plus a plateau and arid default where the range term is low but the land is high (needs rule 2 to change) | - | - |
| B2. Authoring | painted at continent scale, soft-edged; the override is an authored item (ADR 0066) | - | - |
| C. **Amplitude per class** | per class: `k` for 1-20 km, the fine-crease floor for 30 m-1.6 km, the carve's `K t`, the deposition's `kc` | - | `A(E)` above 300 m (6), `carve(slope)` as an on/off (7), the fine-crease factor (8) |
| D. Bake (ADR 0088) | base `m + k (e - m)`; pass 1 at 814 m per face; pass 2 incise only, no uplift; thermal on active cells; deposition | rule 13 | - |
| E. Fine detail | G1 law a, G3 gradient noise (R6) | rule 12 | - |
| F. Water and rivers | the coupled run and the water fields read the new heights | rule 9 | - |

- **Deletes:**
  - the hill band (ADR 0080 point 1);
  - the landform-keyed crease factor (ADR 0080 point 2);
  - the crease amplitude's elevation ramp above 300 m;
  - `carve(slope)` as the switch for fine creases.
- **Keeps:** continents, belts, climate, the coupled run, water, the bake mechanism of ADR 0088, the shader detail.
- **Changes in the store (data format, the owner's call):**
  - the global bake loses the hill band, so `GLOBAL_VERSION` is bumped and `global16384` re-baked (about 6 min, GEN-014);
  - new channels: the relief class (or per-class weights) on a lattice, and ADR 0088's `m`;
  - `k` comes from the class, not stored separately;
  - the erosion layer of ADR 0088 (7-10 GB);
  - the base elevation rule (item 2) changes if plateaus are to exist, which re-bakes the coupled run and the water fields.
- **Pre-bake preview (the owner's global-authoring idea):** the class map and its amplitudes are known before the bake. A whole-planet preview of the classes and the expected relief takes seconds (SPIKE-69: 0.75 `k` times today's relief predicts the baked 5 km relief).

## 4. Terrain ADRs: verdicts (proposals; retiring or superseding is the owner's call)
| ADR | Subject | Verdict | What, and the evidence |
|---|---|---|---|
| 0006 | Generator purity, versioning, keys | keep | every spike kept purity and thread independence (SPIKE-71: active thermal exact at 1 and 4 threads) |
| 0014, 0016 | LOD algorithm; height-aware selection and morph | keep; re-check | level steps grow with `k` (SPIKE-66: x6); tau and the morph range are checked at R4's gate |
| 0019, 0024, 0025 | Tile pack format and codec | keep | not touched by any spike |
| 0022 | Stage reads, scopes and halos | keep | the bake is a pre-runtime stage; the runtime reads stay pointwise (SPIKE-66: a runtime stencil breaks GEN-020) |
| 0026 | Global-coarse representation | **amend** | the bake loses the hill band (version bump); class or weight channels are added beside it |
| 0028 | The terrain field from the bake | **amend** | the field becomes `m + k (e - m) +` detail by class, with no elevation ramp. Point 6 (detail by elevation) belongs to the reference field only (SPIKE-75) |
| 0030 | Erosion strategy: local filter, then a coarse global stage | **supersede by ADR 0088** | the two-pass carve, thermal and deposition (SPIKE-68 to SPIKE-72); stream power to steady state erases landforms (SPIKE-65) |
| 0041, 0056, 0058, 0071 | Water level, coastal band, coarse water, connectivity | keep | consumers of the heights; they re-bake on new heights (ADR 0088 risks) |
| 0045 | Bodies, bake resolution, storage budget (proposed) | **amend** | add the erosion layer (7-10 GB, SPIKE-61) and the class channel to the budget |
| 0066 to 0070 | Authored items, stamps, lines, regions, brushes | keep; **extend 0066** | the relief-class paint is an authored item kind (owner Q8: soft, feathered) |
| 0073 | Normals from the tile lattice | keep | not touched |
| 0074 | Too-steep tiles are not refined | keep; re-check | steeper ground (slope p50 up to 31 degrees at the Himalaya site, SPIKE-74) will cap more tiles: a gate measurement in R4 |
| 0076 | Driver maps (proposed, open) | **amend: merge** | the relief class is a driver in its sense (drivers then suitability). Its point 6 (stream-power processes before the bake) is replaced by ADR 0088's bake |
| 0077, 0081, 0082, 0084, 0086 | Chunk stage and its readings | already superseded or retired by ADR 0086 | keep 0086 as the record |
| 0080 | Hill relief in the bake, landform creases, stationary uplift | **supersede points 1 to 3** by the class amplitudes; keep point 4 (the coupled run keeps the input relief) | the hill band and the landform weight are too weak or switch relief off (SPIKE-65, SPIKE-66, SPIKE-73); point 4 stands (SPIKE-65) |
| 0085 | Shadows from a height clipmap | keep | not touched |
| 0088 | The M9 terrain bake (proposed) | **amend** | `k` comes from the relief class (with a pre-bake default from L), not from L alone (SPIKE-74, SPIKE-75); the fine-crease floor is per class (SPIKE-73) |

## 5. Candidate ADR outline (not an ADR): "Relief classes drive the terrain's amplitude"
- **Context:**
  - Relief below 20 km comes from per-landform amplitude (SPIKE-65, SPIKE-66).
  - Today's weights (the hill band, L, `A(E)`, `carve`) give 4-9x too little relief on hills (SPIKE-55), switch it off on plains (SPIKE-73) and cannot tell an arid plateau from a foothill (SPIKE-75).
  - A class assigned per landform passes criterion 1 at all four hill landforms, the Alps included (SPIKE-76).
- **Decision:**
  1. A relief-class map on a lattice, pre-bake, with a default from L, a mountain-strength driver and a plateau and arid default; authored overrides are items.
  2. The amplitudes are per class: `k`, the fine-crease floor, the carve's `K t` and the deposition's `kc`, as spec values.
  3. The hill band, the landform-keyed crease factor and the elevation ramp above 300 m are removed; the coast taper stays.
  4. The bake is ADR 0088's, with `k` from the class.
- **Consequences:**
  - a global re-bake (version bump);
  - a preview before the bake;
  - authoring at continent scale;
  - the scorecard decides the class amplitudes (R7);
  - ADR 0080 points 1 to 3 and ADR 0030 are superseded; ADR 0028 and ADR 0076 are amended.

## 6. Effect on ADR 0088 and the M9 R-items
- **ADR 0088:** decision 1's relief driver becomes the class map's `k` (the class map is the new channel); the rest stands.
- **R4 (the bake):** unchanged in mechanism. PR 1's channels become the class lattice plus `m`, and the global bake re-bakes without the hill band.
- **R7 (tuning):** amplitudes per class instead of a curve of L. The arid classes need `k` under 1 and a gentler carve: Badlands still worse at `k` 1 in the prototype.
- **New item before R4 PR 1:** the default class map (L, mountain strength, plateau and arid). It needs a base-elevation rule that allows high flat ground (item 2), so it touches the global bake. Proposed as its own small change.
- **R6 (ground detail):** unchanged.

## Findings
1. **Most of today's relief rules are arbitrary or harmful** (items 2, 4, 6, 7, 8): they key relief to elevation, slope or a weak landform weight. The bake mechanism (item 13), continents, belts, climate, rivers and water are sound.
2. **Amplitude per relief class works when the class is right:**
   - all four hill landforms pass criterion 1, the Alps included (1 km relief 0.61-0.75);
   - Erg Chebbi site 2 becomes better than the baseline;
   - Brandenburg keeps more flat ground (4-5 %).
   This is an in-sample upper bound.
3. **The Badlands stay worse even at `k` 1:** the carve adds relief there. Arid classes need `k` under 1 or a gentler carve.
4. **The class map is the missing piece**, and it also serves the owner's global authoring: pre-bake, previewable, paintable.
5. **High flat plateaus need the base elevation rule (item 2) to change.** That touches the global bake: a version bump and a re-bake.

## Open points
- The default class map (from L, mountain strength and a plateau rule) is not built; the prototype assigned the classes by hand.
- The class amplitudes are in-sample. A held-out check needs other sites or SPIKE-55's second matches.
- The class blend at boundaries (bilinear weights) and its bounds (LOD-012) are design only.
- One seed.

## Visuals
- None new: SPIKE-74's stills show `ref`; the prototype's tables are the evidence.

## Handover
- **Prototype:** on branch `spike/m9-plains`, build the `terrain_score` example and run it with `S76_DIR=<dir>` (`proto/run.sh.txt`, about 8 min). The classes are in `class_of` in `proto/s76_block.rs.txt`.
