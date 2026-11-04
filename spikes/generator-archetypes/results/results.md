Opened: 2026-11-04
Question: For nine landscape archetypes (coastal, tundra, badlands, alpine, fluvial, glacial, karst, aeolian, volcanic), what does the generator do today, which physical drivers each needs, and how do they fit the CPU-deterministic two-pass bake?
Why it matters: the owner judges realism by archetype, not only by mountains (owner, 2026-11-04); M9's bake fixes which drivers are baked, and anything missed means a rebuild.
Time box: about 2 days of agent work (Opus); an interim note after deliverables 1 and 2.
Success criteria: per archetype the current behaviour with evidence, the drivers mapped to boundary fields, pass 1, pass 2 or runtime, a staged design with cost estimates, a ranked M9-vs-later plan with the owner decisions it needs, and a GLO-30 reference tile list (not downloaded).
Fallback if it fails: rank the archetypes by what the M9 bake must hold, and leave the processes to later layers.
Code location: none (analysis from code, specs and earlier spike numbers; no probe runs).

# SPIKE-102: the generator by landscape archetype

Status: deliverables 1 and 2 written (interim, 2026-11-04 evening); 3 to 5 follow.

Code paths are relative to `crates/`; `gen` = `generators/src`. State: origin/main at 22f01b48 (#1173, uplift taper).

## 0. Findings first

1. **One pipeline produces every archetype, and its driver set is too small to tell them apart.**
   - The field bake has four channels: continentalness, elevation, temperature and precipitation (`gen/global.rs:121-130`).
   - The class source sets weights for 6 of its 8 classes. Badlands and sand sea are never weighted (`gen/classes.rs:435-456`; GEN-095).
   - There is no erodibility field. K varies only by the class carve factor, which is 0.5 for sand sea and 1 for every other class.
   - Climate ignores relief: there is no wind, no rain shadow and no aridity index (`gen/global.rs:402-411`).
2. **The depression fill is the largest cross-archetype conflict.**
   - The bake fills every basin. Pass 1 breaches at most 50 m and then runs a Barnes flood; pass 2 spreads its pits (`gen/bake/carve.rs:54`, `gen/bake/flood.rs:22`).
   - Water comes only from the ocean (`gen/water.rs:9-14`). Lake ids are reserved but unused.
   - Kettles, thermokarst, dolines, calderas and endorheic basins are therefore impossible.
   - The same fill also costs relief: pass 1's basin fill halves the high-mountain gain (SPIKE-101).
3. **The coast is depositional everywhere by construction.** Three things flatten it:
   - the uplift taper over 0-60 m above the sea (#1173);
   - the 3 m layer fade (GEN-113);
   - the ~64 m coast band.

   There is no shelf, and no rule for cliffed or fjord coasts.
4. **Of the nine archetypes, only fluvial and alpine have processes today.**
   - The other seven exist, if at all, as a shader colour (snow, tundra, sand, beach) or as a hand-placed stamp (crater).
   - Two archetypes rest on processes that are absent: alpine relief on Earth is largely glacial, and so are the glaciated reference sites (Alps, Highlands, Brandenburg).
5. **Most missing features sit below the layer's 101.8 m cell**:
   - dunes 20-500 m;
   - gullies 1-50 m;
   - dolines 10-300 m;
   - patterned ground 1-20 m;
   - solifluction lobes 5-50 m.

   They belong to the runtime detail stack (SPIKE-100's six scales), keyed by drivers. What the bake must hold is the drivers and the processes that move material across km scales.

## 1. Per archetype: what the pipeline does today

Pipeline stages, for reference (ADR 0088, 0089, 0098-0100; GEN-093-114):
- **Field bake.** Four channels plus a lattice of class weights. Base elevation comes from a coast ramp of 620 m, belts at 7,800 m, a back-arc of 2,000 m and plateaus of 600 m.
- **Hill band.** A ridged band at 8/4/2 km, with a per-class amplitude of 30-825 m.
- **Pass 1, per face.** 12,288 cells of about 814 m. Implicit stream power with U/K 20, K·dt 0.05 × the carve factor, 20 steps. It breaches at most 50 m, then runs a Barnes flood with jittered D8 receivers and MFD area.
- **Pass 2, per tile.** 101.8 m cells, incise only (at most 1 km², 10 steps). Then thermal at 33° ±25 % and one deposition sweep with kc 1e5.
- **Runtime.**
  - The layer is read C1 with a 3 m coast fade.
  - Creases are applied only below 200 m.
  - Materials are chosen from slope, height, T and P.
  - Drainage area (GEN-110) and deposit depth (GEN-111) are stored, but no shader reads them.

| Archetype | What exists today | Evidence | Gap |
|---|---|---|---|
| Coastal | Sea level as the pass-1 outlet; uplift taper 0-60 m; layer fade 3 m; coast band ~64 m; beach albedo band 3 m + 5 m fade | `gen/bake/carve.rs:29-31,47,67`; `gen/erosion_layer.rs:18`; `frame/src/look/shading.rs:83-85` | No shelf (ocean floor is one smoothstep to −4,500 m); no cliff, wave-cut platform, marine terrace, spit or barrier; fjords cannot form (no glacial trough to drown) |
| Tundra / periglacial | Tundra colour at −3 to 5 °C; snow colour at ≤ −8 °C on slopes with cos 0.55-0.80 | `render/src/terrain.rs:596-613`; `frame/src/look/shading.rs:79-82` | No permafrost or MAAT field; no lake-pitted plains (fill); no solifluction, patterned ground, eskers or moraines |
| Badlands | A class with k 2 that the default rules never weight; generic thermal at 33° | `gen/classes.rs:28,435-456`; GEN-095 | No soft-rock K; no caprock or strata (mesas, buttes); gully density 100-500 /km sits far below 101.8 m |
| Alpine | Belts to 7,800 m; high-mountains class; band to 825 m; stream-power carve; thermal at 33°; snow by T and slope | GEN-096-101; SPIKE-101: 0.21 of Everest's 3 km relief as built, 0.62 with breach 500 m | No glacial sculpting (cirques, arêtes, horns, troughs); talus is not a material; faces over 45° are outside the heightfield (ADR 0091; Alps 4.3 % of area) |
| Fluvial | Barnes flood; MFD area; implicit stream power; transport-limited deposition; area and deposit channels stored | GEN-100, GEN-101, GEN-110, GEN-111 | Rivers and lakes not rendered; sinuosity 1.07 against real meanders (SPIKE-40); floodplains only as deposit; no terraces |
| Glacial | Lapse rate −6.5 °C/km and snow colour only | `gen/global.rs:407` | No ice extent (present or glacial-maximum), no glacial erosion, no U-troughs, hanging valleys, cirques or moraines. Three of the nine GLO-30 sites (Alps, Highlands, Brandenburg) are glacially shaped. |
| Karst | Nothing | — | No lithology; every depression is filled or breached; towers would exceed 45° |
| Aeolian / desert | Sand-sea class (k 0.35, carve 0.5) never weighted; desert and sand colour by P < 250 mm | `gen/classes.rs:28-31`; `render/src/terrain.rs` | No wind, no sand supply, no dune geometry. Erg Chebbi passes "not worse" only at an authored k 0.35 (SPIKE-77). |
| Volcanic | Hand-placed crater and hill stamps only | `gen/stamp.rs:38-54`; `world-def/stamp.rs:41` | No sources (arcs, hotspots, rifts); no cones, shields, calderas or lava flows |

Measured context from earlier spikes, GLO-30 at 30 m (SPIKE-55):

| Site | Archetype it stands for | Slope p50 / p90 / p99 (°) | 1 km relief p50 |
|---|---|---|---|
| Brandenburg | glacial plain (moraines, kettles) | 1.3 / 7.9 / 17.3 | 25 m |
| Loess hills | aeolian deposit, fluvial dissection | 3.9 / 8.3 / 16.7 | 39 m |
| Badlands, SD | badlands | 2.8 / 8.1 / 17.3 | 34 m |
| Erg Chebbi | aeolian (star dunes) | 1.1 / 4.8 / 21.9 | 16 m |
| Valley and Ridge | fluvial on folded strata | 7.2 / 19.7 / 29.3 | 107 m |
| Black Forest | fluvial upland | 7.3 / 24.9 / 36.6 | 106 m |
| Highlands | glaciated upland | 7.2 / 18.8 / 34.2 | 133 m |
| Alps (Tyrol) | alpine, glacial | 19.3 / 39.0 / 54.4 | 366 m |
| Everest | high alpine, glacial | 29.3 / 44.0 / 58.2 | 516 m |

The scorecard sites cover badlands and aeolian at the 30 m statistics level only. No site covers a cliffed or sandy coast, tundra, karst or volcanic ground (section 5).

Stills: none were taken for this interim note. The gaps are structural (a stage or driver is absent), and no image is needed to show an absence. Section 4 asks for one slot for the combined run, with stills through the product path.

## 2. Physical drivers per archetype and how they map onto the bake

### 2.1 The driver set

The owner's example memo lists uplift U and lithology K as boundary fields, orographic P and T, stream power with diffusion, and a GPU iteration loop. Its fields carry over unchanged. Its GPU loop does not: under CON-14, every height-changing kernel stays a CPU f64 Jacobi gather or a fixed sweep.

| Driver | Kind | Grid (proposal) | Archetypes it serves | Today |
|---|---|---|---|---|
| U: uplift rate | boundary field | class lattice (257² per face) and pass 1 | alpine, coastal (terraces), volcanic (edifices) | implicit: base-elevation belts and a uniform U/K 20 × carve factor |
| K: erodibility, from lithology (hardness, strata K(z), solubility) | boundary field, analytic | evaluated per pass-1 cell, and per pass-2 sample every 4 cells | badlands (soft, caprock), karst (soluble), alpine (hard), coastal (cliff vs beach) | carve factor only (sand sea 0.5) |
| Wind: prevailing direction and steadiness | boundary field, analytic per latitude band plus warp | coarse (2048 per face) | aeolian (dune type), orographic P, coastal (wave exposure) | absent |
| P: orographic precipitation; aridity AI = P/PET | field, from an upwind sweep over the pre-carve base | 2048 per face | badlands, aeolian, glacial (accumulation), karst rate, fluvial (runoff in K·t) | latitude bands × noise, no relief term |
| T: temperature, plus a glacial-maximum offset ΔT (about −6 °C) | field (exists) plus a scalar | as T | glacial, tundra | present-day only |
| Ice: extent and ice flux, from the ELA (from T, ΔT and P) | derived field, then a process | pass 1 (flux), pass 2 (shape) | glacial, alpine, coastal (fjords), tundra (moraines, kettles) | absent |
| Permafrost: MAAT < −2 °C | derived per point | runtime, or with the material drivers | tundra | absent |
| Volcanic sources: a point set of arcs on convergent margins, hotspots and rifts, with type and age | boundary point process from the plate mask (GEN-094) | list per face | volcanic | absent |
| Sand supply | derived from the deposit (GEN-111), aridity and low relief | pass 2 output | aeolian | deposit stored, unused |
| Pit tolerance: the depth of depression the bake keeps | derived from ice history, permafrost, karst and aridity | pass 1 and pass 2 | tundra, glacial, karst, desert (playas), volcanic (craters) | 0 everywhere: fill all |
| Coast type: wave exposure × K × near-shore relief | derived | coastal tiles | coastal | absent: one depositional rule |

### 2.2 Archetype → drivers → bake stage

The stages:
- **Field** is the field bake, which is pre-bake and previewable.
- **P1** is pass 1, per face at 814 m.
- **P2** is pass 2, per tile at 101.8 m.
- **RT** is runtime: the detail stack and materials. It is also where ADR 0091's steep-face technology belongs.

| Archetype | Primary drivers | Field | P1 | P2 | RT |
|---|---|---|---|---|---|
| Coastal | sea level, shelf, wave exposure, K, U (terraces), ice (fjords) | shelf profile; coast type | sea outlet (exists); taper only where depositional | wave-cut platform and cliff line on hard rock; terrace steps where U > 0 | beach, dune and cliff materials; faces over 45° go to ADR 0091 technology |
| Tundra / periglacial | T (MAAT), ice history, pit tolerance | permafrost mask (from T) | keep shallow pits where permafrost or ice history applies | thermokarst and kettle pits kept as lakes | patterned ground, solifluction lobes, frost-shattered rock |
| Badlands | K soft with a caprock (strata K(z)), aridity, base-level drop | default-class rule from K and aridity | carve with high K; strata give mesa steps | thermal and deposition (exist) | gully network as dendritic detail 1-50 m, keyed by K and aridity |
| Alpine | U, hard K, ice, thermal at repose | belts (exist); K hard | carve; breach 500 m (SPIKE-101); glacial trough carve by ice flux | cirques at the ELA; thermal; talus fans from deposition | scree material from slope and deposit; faces over 45° go to ADR 0091 technology |
| Fluvial | P (runoff), K, U | P with orographic term | carve with K·t scaled by runoff (exists, without the P term) | incise and deposition (exist); floodplain flats from the deposit | river water and banks from the area channel (GEN-110); meanders below 100 m as runtime planform detail |
| Glacial | T, ΔT, P → ELA → ice extent and flux | ELA and ice-extent field (glacial maximum) | ice-flux erosion: troughs, overdeepening (pits kept), hanging valleys from the flux ratio | cirques; moraine ridges at the ice margin; kettles (pits kept) | ice and snow material (present-day ice only) |
| Karst | K soluble (lithology), P, T | karst mask from lithology × P | sinks as routing outlets (internal drainage) | doline point process (pits kept); polje flats | small dolines and pavement; towers over 45° go to ADR 0091 technology |
| Aeolian | aridity, wind, sand supply | wind field; sand-sea default rule from aridity × low relief | low carve (exists as factor 0.5) | sand supply from the deposit; draa (km-scale) as a P2 pattern | dune field by type (barchan, transverse, linear, star from supply and wind steadiness), 20-500 m |
| Volcanic | sources, type, age | edifice stamps in the base, or a later additive layer (section 3) | young edifices: carve masked by age; old ones eroded | caldera pits kept (crater lakes) | lava and tephra materials; flow lobes as detail |

### 2.3 Conflicts with today's design

| Conflict | Where | Effect | Resolution (proposal) |
|---|---|---|---|
| Fill everything vs keep lakes | P1 flood plus breach 50 m; P2 pit spread; water from the ocean only | No kettles, thermokarst, dolines, calderas or playas; pass 1's fill also halves high-mountain relief (SPIKE-101) | Route over a filled copy and carve the unfilled heights. Keep depressions where the pit-tolerance driver allows; breach everywhere else (SPIKE-101's 500 m). Store the fill depth as a channel, so the runtime can show the filled look until lake water exists (section 3). |
| Coast taper, coast fade and coast band vs cliffed coasts | #1173 taper; GEN-113 fade; ADR 0056 band | Every coast is a 0-60 m ramp; about 80 % of real coasts are rocky or cliffed (Emery and Kuhn 1982; literature figure, not measured here) | Make the coast type a driver. The taper, fade and band apply where it is depositional; a cliffed coast gets a wave-cut step in P2 and the cliff face goes to ADR 0091's technology. |
| 45° heightfield limit | ADR 0091 | Cliffs, karst towers, arêtes, caldera walls, mesa rims and trough walls exceed it | The heightfield carries the shape up to 45° and marks the faces. The steep share is ADR 0091's coverage goal, not a bake target. |
| Climate independent of relief | `gen/global.rs:402-411` | Arid classes have no rule (SPIKE-77); glaciers have no accumulation pattern; K·t ignores runoff | Wind field plus an orographic upwind sweep over the pre-carve base (relief above 10 km sets rain shadows), in the field bake. |
| No erodibility field | carve factor only | Badlands, mesas, cuestas, karst and the cliff-vs-beach choice all need K | Analytic lithology: hardness noise per province from plate history (SPIKE-81's H), plus strata K(z) (horizontal or folded bands) |
| Uniform uplift, no history | U/K 20 everywhere | No marine terraces; volcanic edifices cannot rise | U as a field on the class lattice (belts high, cratons 0); edifices as stamps (section 3) |
| Two scales only (814 m and 101.8 m) | ADR 0088 | Glacial troughs are 1-4 km wide, 2-5 P1 cells: coarse in P1, so the U shape must come from P2 | P1 carries the ice-flux incision; P2 widens the cross-section by the flux (a parabolic profile). |
| GPU iteration loop (owner's memo) | CON-14 | Fast to iterate, but not bit-exact | CPU f64 for every height-changing kernel; a GPU approximation may serve only as a non-canonical preview, never as a stored result. The coarse CPU preview already takes 12 s at 4 km (SPIKE-77). |

### 2.4 Approximate share of Earth's ice-free land per archetype

Literature figures, not measured here; they rank the work by area.

| Archetype | Share | Source |
|---|---|---|
| Arid lands (badlands and aeolian inside) | about 30-40 % | UNEP aridity |
| Sand seas | about 20-25 % of arid land | Wilson 1973 |
| Glaciated at the glacial maximum | about 25-30 % | |
| Glaciated today | about 10 % | |
| Permafrost | about 20-25 % of Northern Hemisphere land | |
| Mountains | about 24-25 % | Kapos 2000 |
| Carbonate karst | about 15 % | Goldscheider et al. 2020 |
| Volcanic | a few % | |
| Badlands | under 1 % | |
| Coastal zone | thin, but in nearly every view near the sea | |

## 3. Staged design with costs
(to follow)

## 4. Ranked plan, M9 against later
(to follow)

## 5. GLO-30 reference tiles wanted (not downloaded)
(to follow)

## Visuals
None yet (interim). Section 4's combined run brings stills through the product path.
