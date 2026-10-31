# SPIKE-80: the re-derived generator, combined

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: what does the re-derived generator give when its parts run together? The parts:
- SPIKE-77's class map and plateau rule;
- SPIKE-79's plate-margin belts, with one try at width variety and a back-arc plateau;
- the class amplitudes (SPIKE-76);
- ADR 0088's two-pass bake.
Why it matters: the last synthesis before the owner decides decisions 1, 2 and 4 of `docs/briefings/m9-decisions.md`.
Time box: 3 h (opus-planner, 2026-10-31); used about 2 h, with 15 min of machine time.
Success criteria:
- global hypsometry and class shares against Earth;
- the scorecard per landform at re-matched sites (baseline → combined → real);
- world maps and stills.
Code location:
- the global model is a Python prototype (`proto/combined.py.txt`) on SPIKE-69's 0.5-degree map, resampled to 0.25 degree;
- the sites run on branch `spike/m9-plains` (commit b1d02081): `S80_DIR` in the `terrain_score` example (`proto/s80_block.rs.txt`, `proto/run.sh.txt`) and `global::spike80_hill_pattern`;
- the stills use SPIKE-74's column-cast renderer (`proto/stills.py.txt`).
Spike code, never merged.
Closed: 2026-10-31

## Method
- **Global model** (`combined.py`): today's continents and coast ramp (`base`, 0-1,000 m), then:
  | Part | Rule | Setting |
  |---|---|---|
  | Belts | SPIKE-79's plates (14, convergent margins, warp 0.35, coastal arcs), `A B (c + (1 - c) ridge^2)` | `A` 5,500 m, `c` 0.5 |
  | Width variety | half-width `90 (0.6 + 0.9 n)` km, `n` a 2-octave fBm along strike | 54-135 km |
  | Back-arc plateau | behind margins with convergence over 0.7, on the plate with more land, flat top to 450 km, edge to 700 km | uplift 2,000 m |
  | Old margins | the non-convergent margins (worn-down orogens) and belt flanks (3 half-widths) | hill uplift 200 m |
  | Plateau rule | SPIKE-77 (window 0.50-0.62), away from belts and back-arc plateaus | 1,200 m |
  | Hill band | dropped (SPIKE-76) | - |
  - The amplitude, back-arc uplift and convergence threshold were tuned on hypsometry (`raw/combined-grid.log`). The best fit (error 0.18, back-arc 1,000 m) tops out at 7.0 km; the chosen setting (error 0.28, back-arc 2,000 m) reaches 8.0 km.
- **Default class map:** memberships by smoothstep:
  - mountains: the belt core;
  - high mountains: the core above 2-3 km;
  - hills: belt flanks and old margins;
  - plateau: the plateau rule or a back-arc plateau;
  - rolling plain or plain: the rest, split by a noise.
  `k` per class as SPIKE-76 (plain 1, rolling plain 1, hills 5, mountains 6, high mountains 7.5, plateau 1), blended by membership. Class shares two ways: the class with the largest membership, and SPIKE-69's relief classes from `k R14`, with today's field relief at `k` 1 as the base relief.
- **Sites re-matched** (`raw/sites-rematched.toml`):
  - per landform, the class (plain, rolling plain, plateau, hills, mountains, high mountains) and the real window's mean height, two sites at least 8 degrees apart;
  - the windows lie well inland (at least 99 % land within 1 degree);
  - hills and mountains stand on today's ground at 300 m or more.
  - The last two limits come from the stand-in below. Run 1 without them (`raw/run1/`) put mountain sites on coastal arcs, and the delta made cliffs at the coast (Himalaya 5 km relief 12 times real).
- **Site pipeline** (`S80_DIR`):
  - today's stored field at the new site, plus the delta (combined elevation minus today's sample, bilinear from the 0.25-degree grid) as a stand-in for a re-baked store;
  - `k` and the crease floor per cell from the class map, with the coast taper;
  - pass 1, the carve at 814 m; pass 2, incise only; active-cell thermal at 33 degrees; one deposition sweep (as SPIKE-74).
  - **Variants:** `combined` (as above); `combined+hb` (plus a class-driven hill band: today's ridged 2-8 km pattern at 30 m on plains, 175 m at `k` 5, 825 m at `k` 7.5); `combined+authored` on the plateau landforms (Badlands: no carve at `k` 1, SPIKE-78; Erg Chebbi: `k` 0.35 with the carve at half strength, SPIKE-77).
- **Baseline:** today's generator at SPIKE-55's sites (SPIKE-75's tables).
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`, `raw/summary.txt`).

## Results
### Global
| Land above (under 60 degrees) | 0.5 km | 1 km | 2 km | 3 km | 4 km | 5 km | Log error | Highest |
|---|---|---|---|---|---|---|---|---|
| Today (zone blobs, 7,000 m, plateau) | 53.0 % | 27.8 % | 7.1 % | 3.1 % | 1.4 % | 0.75 % | 0.44 | 7,974 m |
| SPIKE-79 plates (uniform width) | 46.4 % | 24.4 % | 5.8 % | 3.2 % | 1.8 % | 0.81 % | 0.30 | 6,814 m |
| **Combined** | **48.1 %** | **27.0 %** | **7.6 %** | **3.3 %** | **1.8 %** | **0.75 %** | **0.28** | **8,005 m** |
| Earth (2-5 km without Antarctica; 0.5-1 km with ice) | 46.7 % | 27.2 % | 6.7 % | 3.2 % | 1.9 % | 0.8 % | - | 8,849 m |

| Class shares | Plains | Hills | Mountains (high) | Plateau |
|---|---|---|---|---|
| Combined, by class membership | 61.7 % (plain 32.3, rolling 29.4) | 18.9 % | 5.5 % (4.0) | 13.8 % |
| Combined, relief classes `k R14` | 60.8 % | 20.0 % | 19.1 % (1.9) | - |
| Earth (SPIKE-69's targets) | 61 % | 14 % | 25 % | - |
- **Hypsometry fits Earth at every height from 0.5 to 5 km** (error 0.28) and the highest land rises to 8.0 km.
- **The class map is narrow in the mountains:** the belt cores are 5.5 % of land by membership. By relief (`k R14`) mountains reach 19 % against Earth's 25 %; hills are 20 % against 14 %.
- `k` changes by up to 6.5 per 28 km at belt edges (p99 1.9): sharper than SPIKE-77's 5.05 per 55 km.
- `images/elevation-today-vs-combined.png`: elevation 0 to 6 km, today (left) and combined (right).
- `images/classes-and-k.png`: classes (left: plain light green, rolling plain yellow-green, hills brown, mountains grey, high mountains white, plateau tan) and `k` from 1 to 7.5 (right).
- The back-arc plateaus are small: at a convergence threshold of 0.7 only the strongest margins get one, and they show as widened belts, not as a Tibet. More back-arc area overshoots the 2 km share (`raw/combined-grid.log`).

### The scorecard (slope and 1 km relief, generator over real, site 1 / site 2; distances in brackets)
| Landform | Baseline (today) | Combined | Combined + class hill band (or authored arid class) | Classes L1 baseline → final | Verdict |
|---|---|---|---|---|---|
| Brandenburg plain | 0.49 / 0.38, 0.49 / 0.36 | 1.34 / 0.76, 1.35 / 0.76 | 1.64 / 1.02 (0.07 / 0.06), 1.66 / 1.01 (0.07 / 0.07) | 1.39 → 0.73 | better; flat 3-4 % against 22.6 % |
| Loess | 0.22 / 0.33, 0.23 / 0.35 | 0.72 / 0.73, 0.72 / 0.72 | 0.76 / 0.79 (0.08 / 0.22), 0.65 / 0.64 (0.13 / 0.41) | 1.48 → 0.59 | better |
| Badlands | 0.83 / 0.87, 1.39 / 1.36 | 1.63 / 1.55, 1.02 / 0.94 | authored: 0.24 / 0.34 (0.30 / 0.44), 0.26 / 0.31 (0.30 / 0.45) | 0.31 → 1.59 | **worse** (the new sites lie on today's flat lowland) |
| Erg Chebbi | 2.75 / 2.83, 3.67 / 3.65 | 4.73 / 4.65, 3.14 / 2.48 | authored: 0.95 / 1.12 (0.06 / 0.07), 0.98 / 0.98 (0.06 / 0.07) | 1.28 → 0.50 | **better** on every distance |
| Valley and Ridge | 0.18 / 0.17, 0.17 / 0.16 | 0.92 / 0.59, 2.27 / 2.21 | 2.08 / 1.72, 2.78 / 2.89 | 0.52 → 0.36 | criterion 1 pass; overshoots |
| Black Forest | 0.18 / 0.17, 0.16 / 0.16 | 0.63 / 0.42, 0.69 / 0.47 | 1.12 / 0.87 (0.04 / 0.12), 0.98 / 0.73 (0.06 / 0.17) | 0.58 → 0.34 | **criterion 1 pass** |
| Highlands | 0.18 / 0.14, 0.19 / 0.15 | 1.99 / 1.02, 1.09 / 0.56 | 3.10 / 1.97, 1.87 / 1.08 | 0.67 → 0.37 | criterion 1 pass; site 1 overshoots |
| Alps | 0.12 / 0.11, 0.11 / 0.10 | 0.68 / 0.37, 0.43 / 0.24 | 1.03 / 0.72 (0.04 / 0.15), 0.68 / 0.47 (0.14 / 0.27) | 0.26 → 0.29 | criterion 1 fail at site 2 (0.47) |
| Himalaya | 0.23 / 0.24, 0.14 / 0.14 | 0.50 / 0.29, 0.67 / 0.37 | 1.15 / 0.98 (0.15 / 0.11), 1.12 / 0.92 (0.14 / 0.11) | 0.09 → 0.35 | better on slope and relief; classes L1 worse |
Criterion 1: slope and 1 km relief at 0.6 of real or more at both sites (hill landforms and the Alps).
- **Without a hill band the combined generator fails criterion 1 at all four hill landforms** (1 km relief 0.24-0.59 at one site each).
  - SPIKE-76's class `k` values were calibrated at SPIKE-55's sites, which lie in today's range zones: there the hill band (175-730 m, wavelengths 2-8 km) is part of the field that `k` multiplies.
  - The re-matched sites have no hill band (their range zone is low), so `k` alone falls short.
- **A class-driven hill band restores it:** all three hill landforms pass, and the Himalaya reaches 0.92-0.98. The Alps fail at site 2 (0.47): that site stands on today's 436 m ground, where today's crease amplitude is not full.
- **The two overshoots are stand-in effects:** Valley and Ridge site 2 and Highlands site 1 stand on today's high ground (3,746 m and 1,238 m means). The stand-in keeps today's mountain relief there and lowers it by the delta, so `k` multiplies mountain relief.
- **The plateau landforms split:** Erg Chebbi with its authored sand-sea class is the best result of the M9 spikes at that tile (distances 0.06-0.15 against the baseline's 0.26-0.48). The Badlands get worse: their new sites lie on today's flat lowland (87 % flat), so a no-carve class keeps that flatness. The default plateau class with the full carve passes site 2 (0.94) and overshoots site 1 (1.55).

### Stills
- `images/<tile>-stills.jpg` for the nine landforms (rows real, baseline at SPIKE-55's site 1, combined at the re-matched site 1; eye 300 m and 30 m): `combined+hb` for plains, hills and mountains, `combined+authored` for Badlands and Erg Chebbi.

## Findings
1. **The combined generator matches Earth's hypsometry from 0.5 to 5 km** (48 / 27 / 7.6 / 3.3 / 1.8 / 0.75 % against 47 / 27 / 6.7 / 3.2 / 1.9 / 0.8 %). The highest land is 8.0 km, with high ground in belts (SPIKE-79).
2. **The class shares stay near Earth's:** plains 61-62 % against 61 %. Hills come out a little high and mountains a little low (relief classes 20 / 19 % against 14 / 25 %).
3. **The hill band must not be deleted; it must be keyed to the class.** Without it the four hill landforms fail criterion 1 at the re-matched sites. A class-driven hill band (30 m on plains to 825 m in high mountains) passes three and the Himalaya.
   - This corrects SPIKE-76's proposal ("the hill band is replaced by class amplitude"): the class amplitudes were calibrated with the hill band inside the field.
4. **The plate-margin belts, plateaus and authored arid classes compose without conflict**, apart from the Badlands. Erg Chebbi is better than ever; the Badlands need re-matched ground or a class with its own relief, not "no carve".
5. **The width variety and back-arc plateau work but stay small:**
   - belts vary from 54 to 135 km in half-width;
   - back-arc plateaus at 2,000 m lift the highest land to 8.0 km but cover little area. A Tibet-sized plateau overshoots the 2 km share.
6. **The stand-in limits the site evidence:**
   - today's stored field carries today's elevation ramps, so a re-matched site on low ground has too little fine relief, and one on today's mountains too much;
   - a store re-baked with the combined rules is the real test (decision 4).

## Open points
- The site evidence is a stand-in (the delta on today's field), not a re-bake; the ramps of today's field leak into it (finding 6).
- The class hill band's amplitudes (30 / 175 / 825 m) follow today's curve and are not tuned.
- `k` steps at the belt edges (6.5 per 28 km at most) are sharper than SPIKE-77's; the class blend needs a wider edge before R4.
- The back-arc plateau's area against the 2 km share; a Tibet needs a lower base around it or a lower belt amplitude.
- One seed. The prototype's noise and plates are not the generator's, and the class map is a prototype rule.

## Visuals
- `images/elevation-today-vs-combined.png`: elevation 0 to 6 km, equirectangular, today (left) and combined (right).
- `images/classes-and-k.png`: the default class map (left) and `k` (right).
- `images/<tile>-stills.jpg`: nine still sheets for the owner's phone.

## Handover
- Global: `S80_HILLU=200 S80_FIX=5500,2000,0.7 python -I proto/combined.py.txt <SPIKE-69 map13.f32> <out> raw/real-stats.json` (about 20 s). It writes `s80-fields.f32` (12 MB, kept out of git), `combined.json`, the maps and the re-matched sites.
- Sites: on branch `spike/m9-plains`, build the `terrain_score` example; write `sites-rematched.toml` from `combined.json` (the site list in `raw/sites-rematched.toml`); run `proto/run.sh.txt`; then `python -I proto/summarise.py.txt raw/run.log`.
- Stills: `python -I proto/stills.py.txt target/s74/r <out> <tile> "<title>" target/s80/r <variant>`.
