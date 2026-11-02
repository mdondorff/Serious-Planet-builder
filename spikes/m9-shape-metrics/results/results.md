Opened: 2026-11-01
Question: Why did SPIKE-77 to 80 match Earth's statistics while their world map looks "generally wrong" (owner, 2026-11-01), and which shape metrics would have caught it?
Why it matters: explorer builds the engine preview (explorer inbox #37); these metrics become the scorecard's shape checks for the preview iterations.
Time box: 2 h (Sonnet, analysis only)
Success criteria: the blind spot of the old metrics named; 5 to 8 shape metrics with Earth references and a computation rule; measured on today's map and SPIKE-80's combined map; the knobs that move each; the continent generator and candidate approaches.
Fallback if it fails: the metric definitions alone.
Code location: Python scripts in `proto/` on SPIKE-69's 0.5-degree map of the generator's inputs (`target/s69/map13.f32`), resampled to 0.25 degree, and SPIKE-80's `combined.py` rerun unchanged (`s80-fields.f32`); no Rust, no cargo (a perf window ran beside it; scripts at idle priority).
Closed: 2026-11-01
Earth measured: 2026-11-01 (section 3b), on the owner's approval of the ETOPO 2022 download (planner inbox #59, #60).

**Earth data source.** NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global Relief Model. NOAA National Centers for Environmental Information. https://doi.org/10.25921/fd45-gt74 (the 60 arc-second surface-elevation grid). Licence, from NCEI's dataset record (https://data.noaa.gov/metaview/page?xml=NOAA/NESDIS/NGDC/MGG/DEM//iso/xml/etopo_2022.xml&view=getDataView&header=none): "NOAA waives any potential copyright and related rights in these data worldwide through the Creative Commons Zero 1.0 Universal Public Domain Dedication (CC0-1.0)." The file is not in the repository: `C:/DEV/SeriousPlanetData/etopo2022/ETOPO_2022_v1_60s_N90W180_surface.tif`, from https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif, 465,969,062 bytes, SHA-256 `9d27d4b8ea8e76977e2988bca667d7c8fa68b927355feffcddd6b4875a7fd08e`. Only derived figures are here.

## Headline
- **The old metrics are histograms, so they are blind to shape.** Class shares and hypsometry count land cells by class or height. Any rearrangement of the same cells scores the same: a 9,000 km line one cell wide and a compact massif of equal area at the same heights are equal to them. SPIKE-80 also tuned its belt amplitude and back-arc uplift on hypsometry itself, so that match was fitted, not predicted.
- **The continents never entered any metric, and no relief rule touches them.** The combined map has exactly today's continents (land 29.7 %, the same two masses of 119 and 31 Mkm²). The largest holds 79 % of the land; Afro-Eurasia holds about 57 % of Earth's. The coastline is smooth: box dimension 1.11 at 28-900 km.
- **The combined belts are one-cell-thin lines that run and close.** The largest connected region above 2 km is 18,100 km long; the next four are 8,300, 6,600, 5,400 and 4,200 km. Their widths at p10 are 64 km. The belts enclose 10.9 Mkm² of land they do not cover: a ring traced round the eastern continent by the coastal-arc term. Today's map has 7,200 km at most and encloses nothing.

![Today's map (top) and SPIKE-80's combined map (bottom): land grey, every connected region above 2 km in its own colour, 0.25-degree equirectangular](images/high-ground-regions.png)

## 1. Why the statistics matched
| Old metric | What it measures | Blind to |
|---|---|---|
| Class shares (61/20/19 %) | the share of land per relief class | where the classes are, their shape, their connectivity |
| Hypsometry (land above 0.5-5 km) | the height distribution of land cells | the same; and it was the tuning target of SPIKE-80's amplitude and back-arc uplift (`raw/combined-grid.log` in SPIKE-80) |
| Site scorecard (SPIKE-74 to 81) | 30 m to 5 km statistics at chosen sites | everything above 5 km; sites were re-matched to where the model put the classes |
| Aspect ratio of land above 2 km (SPIKE-79) | elongation per region | thinness, length, gaps and rings (a long thin ring is "aspect over 4") |

Off in the map, against Earth:
- belts: one-cell-thin and uniform in width; continuous over 5,000-18,000 km; a closed ring; no segmentation into ranges;
- continents: unchanged by the M9 work; two masses plus a polar band, too few and too large; coastlines smooth at 100-1,000 km scale.

## 2. Shape metrics
Computed on a global equirectangular grid (0.25 degree here; on the cube map the same on each face's lattice with the seams joined), cell areas on the sphere, 8-connectivity, regions joined across the 180-degree seam. Land is elevation above 0; high ground is elevation of at least 2,000 m.
| # | Metric | Computation | Earth reference |
|---|---|---|---|
| S1 | Land masses of at least 1 Mkm² | connected land regions by area | 5: Afro-Eurasia about 85, the Americas about 42, Antarctica 14.2, Australia 7.7, Greenland 2.2 Mkm² (standard geography) |
| S2 | Largest land mass's share of land | its area over all land | about 57 % (Afro-Eurasia over 149 Mkm²; 63 % without Antarctica) |
| S3 | Coastline dimension | box counting of the coast cells at 2-32 cells (about 28-900 km), slope of log count over log size | about 1.1-1.3 by coast: west Britain 1.25, Australia 1.13 (Richardson's data in Mandelbrot 1967, *Science* 156:636); measure Earth's global value at the same band before using it as a gate |
| S4 | High-ground width | per region above 2 km: 2 x the mean distance to its edge along its skeleton (km); area-weighted median and p10 | orogens about 100-500 km across (the Alps 150-250 km, the Himalaya 250-400 km, the Andes 200-700 km; standard geology) |
| S5 | High-ground segment length | per region: skeleton length (km); the five longest | the longest connected highland above 2 km is the Tibet-Himalaya-Pamir system, of order 3,000-4,000 km; the Andes, 7,000 km long, break below 2 km into shorter segments (approximate; to be measured on a global DEM with this code) |
| S6 | Enclosed area | land the high-ground regions surround but do not cover (filled minus covered), summed over holes of at least 0.5 Mkm² | about 0-1 Mkm² (the Tarim and Qaidam basins, partly ringed) |
| S7 | High ground share of land | land at least 2 km over all land | about 7 % without Antarctica (SPIKE-78) |
| S8 | Belt sinuosity | longest skeleton path over the distance between its ends | about 1.1-1.3 for the Andes' axis and the Himalayan arc (approximate); here measured as skeleton length over the region's largest span, which counts branches: not comparable yet, a cleaner path measure is needed |
- S5, S6 and S8 are measured on Earth with the same code in section 3b (ETOPO 2022), which also proposes the gates.

## 3. Measured
`raw/shape.json`, `raw/holes.json`; today's map is the generator's elevation channel, the combined map SPIKE-80's chosen setting.
| Metric | Today | SPIKE-80 combined | Earth |
|---|---|---|---|
| Land fraction | 29.8 % | 29.7 % | 29.2 % |
| S1 land masses of at least 1 Mkm² | 2 (119.4, 31.1) | 2 (119.1, 31.1) | 5 |
| S2 largest share of land | 79 % | 79 % | 57 % |
| S3 coastline dimension | 1.11 | 1.11 | 1.1-1.3 (to be measured) |
| S4 width, area-weighted median (p10-p90) | 376 km (75-422) | 199 km (64-303) | 100-500 km |
| S5 longest five segments | 7,219 / 2,667 / 2,051 / 1,623 / 1,510 km | 18,127 / 8,334 / 6,603 / 5,384 / 4,225 km | of order 3,000-4,000 km longest |
| S6 enclosed area | 0 | 10.9 Mkm² | about 0-1 Mkm² |
| S7 high ground share of land | 3.9 % | 6.9 % | about 7 % |
| S8 sinuosity (with branches) | 1.91 | 1.72 | not comparable yet |
| Regions above 2 km | 10 | 18 | - |
- S7 is the only shape-free metric here, and the combined map passes it; S1, S2, S5 and S6 fail clearly and would have flagged SPIKE-80.
- Today's high ground is blob-like (wide, few, short); the combined map is the opposite extreme (thin, long, ringed).

## 3b. Earth measured (ETOPO 2022)
`proto/earth.py.txt` reads the 60 arc-second grid with SPIKE-55's decoder, block-averages 15 x 15 cells to 0.25 degree and runs the same metrics on all three maps. High-ground metrics (S4-S8) are restricted to latitudes within 60 degrees on every map: the ice sheets of Antarctica and Greenland are surface elevation above 2 km, not orogens. S8 is now the longest path through a region's skeleton over the distance between its ends (regions of at least 500 km), area-weighted. `raw/earth.json`.
| Metric | Earth (ETOPO 2022) | Today | SPIKE-80 combined | Gate proposal |
|---|---|---|---|---|
| Land fraction | 29.1 % | 29.8 % | 29.7 % | 27-31 % |
| S1 land masses of at least 1 Mkm² | 5 (79.8, 38.9, 13.7, 7.7, 2.2) | 2 (119.4, 31.1) | 2 (119.1, 31.1) | 4-7 |
| S2 largest share of land | 54 % | 79 % | 79 % | 45-62 % |
| S3 coastline dimension (28-900 km) | 1.22 | 1.11 | 1.11 | at least 1.18 |
| S4 width, area-weighted median | 310 km (Tibet weighs most) | 376 km | 199 km | 150-450 km, and S5 |
| S5 longest five regions above 2 km | 6,009 / 5,877 / 2,136 / 1,980 / 1,448 km | 4,379 / 2,157 / 1,844 / 1,723 / 1,648 km | 11,031 / 5,232 / 4,635 / 4,490 / 4,345 km | longest at most 7,500 km; the third at most half the longest |
| S6 enclosed by high ground (holes of at least 0.5 Mkm²) | 0 | 0 | 10.9 Mkm² | 0 to 1 Mkm² |
| S7 high ground share of land within 60 degrees | 6.3 % | 4.3 % | 6.7 % | 5-8 % |
| S8 sinuosity (longest skeleton path / chord, regions of at least 500 km) | 1.71 | 1.81 | 1.41 | 1.4-2.0 (coarse) |
| Regions above 2 km within 60 degrees | 69 | 10 | 16 | at least 30 |
- Earth's high ground is two long systems (the Andes and Tibet-Himalaya, about 6,000 km each), a few of 1,500-2,100 km and many short ranges: 69 regions. The combined map has one 11,000 km region, four of 4,300-5,200 km and only 16 regions: too continuous and too uniform, as the image showed.
- The combined belts are straighter than Earth's high ground (S8 1.41 against 1.71): Earth's regions are ragged and branched at 0.25 degree; the Voronoi margins are smooth arcs.
- The region count is a ninth useful number: it catches the missing fragmentation that S5 shows only at the top.
- The approximate references of section 2 stand corrected by this table (S2 54 %, not 57 %: at 0.25 degree Africa and Eurasia join only through Sinai).

![Earth (ETOPO 2022 at 0.25 degree): land grey, every connected region above 2 km within 60 degrees in its own colour](images/earth-high-ground.png)

## 4. Knobs that move each metric
| Metric | Knob (where) |
|---|---|
| S1, S2, S3, land fraction | the continent field only: `fbm(seed, LAYER_CONTINENT, warped, 1.1, 5) x 1.7` over a direction warped by 0.45 (`crates/generators/src/global.rs:174-182`), cut at 0.1 (`SEA_LEVEL_C`); base frequency (1.1: one or two masses), octaves and gain (coast detail), warp amplitude (0.45: shape variety). ADR 0089's class source and the plate belts do not touch it |
| S4 width | belt half-width `HW` (90 km) and its along-strike variety `WV` (0.6-1.5x), the Gaussian profile `exp(-(d/hw)^2)`, the flank factor (3 hw) (`combined.py` `model`) |
| S5 segment length | the convergence gate `smooth(conv, 0.05, 0.35)` and the coastal gate `seg_coast` (threshold 0.40-0.52): nothing breaks a belt along strike; plate count `NP` 14 and warp 0.35 set margin length. A gap noise along strike (or segmenting by margin curvature) is the missing knob |
| S6 enclosed area | the coastal-arc term `seg_coast x exp(-(d_coast/0.8 hw)^2)` traces a whole coastline, and Voronoi margins close around small plates inside continents: allow coastal arcs only on convergent margins facing an ocean plate, and gate interior margins |
| S7 | belt amplitude `A`, back-arc uplift `U`, plateau rule (SPIKE-78, 80) |
| S8 | warp of the plate cells and margin curvature |

## 5. Continents today and candidate approaches
- Today: domain-warped fBm continentalness (5 octaves from base frequency 1.1, warp 0.45), thresholded at 0.1 for about three tenths land (`global.rs`); unchanged by ADR 0088 and 0089 and by every M9 spike.
| Approach | What it gives | Effort | Risk |
|---|---|---|---|
| A. Tune the fBm | a higher base frequency (about 1.6-2.2) and more warp give more, smaller masses and rougher coasts; S1-S3 tunable within days with the preview | 1 PR plus preview iterations | low; continents stay unrelated to the plates and belts |
| B. Multi-scale noise with shape constraints | cratons seeded at a log-normal size distribution, grown on noise, the threshold chosen per quantile to hold land at 29 % and S1/S2 at targets | 2-3 PRs | medium: constraint solving on the sphere, seams at cube faces |
| C. Plate-driven continents | the same plate model places continental crust (plate interiors, accreted terranes at convergent margins), so belts sit at continent edges by construction (S5, S6 follow) | 3-5 PRs plus a spike | medium-high: couples two systems; the plate model becomes a format question for the owner |
| D. Tectonic simulation | plates moving over time (e.g. Cortial et al. 2019, "Procedural Tectonic Planets", Computer Graphics Forum) | weeks | high: a new subsystem; determinism and bake time (CON-14) |
- A first, gated by S1-S3 in the preview; C if the owner wants belts and coasts to agree by construction.

## Limits
- 0.25-degree resampling of a 0.5-degree input: widths under about 60 km and coast detail under 100 km are not resolved.
- Section 2's Earth references are from standard geography; section 3b replaces them with ETOPO 2022 measured by the same code.
- Skeleton lengths count branches (S5, S8 overstate long, branched regions); the order of magnitude is not in doubt (18,100 km against a few thousand).
- The polar land of the generator is in S1, S2 and the largest mass (the southern band connects to the central continent).
