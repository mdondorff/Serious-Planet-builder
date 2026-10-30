# SPIKE-57 H. Schott's publications for M9

Opened: 2026-10-30
Question: which publications at https://h-schott.github.io/publications/ are relevant to M9 "Realistic terrain", and can this engine adopt each one?
Why it matters: M9 planning (roadmap M9 and its design constraints of 2026-10-28); queue row RS (owner, 2026-10-30).
Time box: desk study, 3-4 h.
Success criteria: one row per publication (what it does, M9 relevance, fit with CON-03/11/14, the bake and store model and the edit model, runtime cost, build time, verdict), and a section on terrain descriptors with a minimal prototype design (not built).
Fallback if it fails: list the publications with the project pages' summaries and mark what could not be read.
Code location: none (desk only).
Closed: 2026-10-30

## Sources and limits
- Read: the publication list and the six project pages (h-schott.github.io/p/...); the MIT-licensed reference code on GitHub (READMEs and the compute shaders and headers named below); SPIKE-52, which had a helper read the full texts of the 2023 and 2024 erosion papers (timings quoted from it).
- **Not read: the paper PDFs.** Every HAL link (hal.science/.../document) returned a bot-check page to the fetcher. Getting past bot checks is out of bounds, so nothing was downloaded and nothing was stored in `C:/DEV/SeriousPlanetData/papers/schott/`. Missing as a result: the descriptors paper's timing table, its correlation matrix and its artist study, and the PhaseTree, iso-contour and sphere-carving papers' numbers.
- Next step if wanted: the owner downloads the descriptors PDF by hand into that folder, and a short follow-up adds the correlation matrix (which cheap descriptor stands in for which expensive one).
- **Follow-up, 2026-10-30:** the owner downloaded the descriptors and the 2024 multi-scale erosion papers by hand (to `C:/DEV/SeriousPlanetData/papers/`, outside the repo). Both were read; the section "Follow-up: the two full texts" at the end holds what they change. They correct the scorecard design and the per-chunk estimate of the sections below.
- Licences, checked through the GitHub API: Contours, SphereCarvingRelease, terrain-descriptors, MultiScaleErosion and StreamPowerErosion are MIT. The PhaseTree has no code link.

## Publications
| # | Publication | What it does | M9 relevance | Fit (CON-03/11/14, bake and store, edits) | Runtime cost | Build (agent days) | Verdict |
|---|---|---|---|---|---|---|---|
| 1 | Terrain descriptors for landscape synthesis, analysis and simulation (Argudo, Guérin, Schott, Galin; CGF 44(2), EG 2025 STAR) | A survey and reference implementation of several dozen terrain metrics: local, relief, visibility, landforms, hydrology, orometry | **High**: M9's "slope targets checked on real elevation samples" needs a metric set beyond SPIKE-55's slope and relief; a few metrics also extend GEN-070's driver set | CPU, f64, pure functions on a raster: fits CON-14 when written with `libm` and a fixed reduction order; no GPU; analysis runs offline (xtask), drivers in the chunk stage | Analysis: none per frame. Driver channels: chunk time (local metrics O(N), TPI O(N) with summed-area tables, geomorphons about 8 x 33 reads per cell at 1 km and 30 m) | Scorecard 4-6; each new driver channel 2-4 | **Adopt** as the M9 realism scorecard; adapt 2-3 metrics as drivers |
| 2 | Terrain Amplification using Multi-Scale Erosion (Schott, Galin, Guérin, Peytavie, Paris; TOG 43(4), SIGGRAPH 2024) | Upsample x2, then GPU stream-power erosion, thermal and deposition passes, repeated per level (256 to 2048 in the preset) | **High**: M9's hill realism (SPIKE-55: hills and mountains 4 to 9 times too gentle); thermal and deposition give slopes at the angle of repose and filled valley floors | Kernels are Jacobi gathers (ping-pong buffers, no atomics): thread-count independent, so a CPU port meets CON-14. The GPU original breaks CON-07/14 for a field. The thermal pass wraps at the borders and uses an `fract(sin)` hash and f32 simplex noise (replace both). Whole-grid passes, a halo of one cell per iteration (preset: 400 erosion and 6,000 thermal passes at 2048) | Erosion 19.2 ms per pass at 8192^2 on an RTX 3080 (SPIKE-52). CPU estimate for a 5 km chunk at 4 m (1,250^2): about 10-20 ms per pass on 16 threads, thermal and deposition 2,000-5,000 passes, about 30-90 s a chunk: over GEN-052's chunk budget at runtime | Chunk-stage thermal and deposition (CPU, halo bound) 8-12; whole amplification bake 25-35 plus an ADR | **Adapt**: the thermal and deposition kernels in the chunk stage or the bake, on the CPU, few passes; not the full amplification |
| 3 | Large-scale terrain authoring through interactive erosion simulation (Schott, Paris, Fournier, Guérin, Galin; TOG 42(5), SIGGRAPH 2023) | The user paints uplift; GPU stream power with drainage by iterative relaxation runs to steady state; inverse modelling recovers uplift from a DEM | Medium: the uplift view is already in the bake (GEN-063, the stationary uplift); M9's deferred "uplift that does not follow the input network" (SPIKE-45) is this question | Steady state needs global drainage. SPIKE-33 measured that dynamic coupled evolution has no bounded light cone (p99 28 m at a 5 km halo); the A-net (ADR 0077) replaced it. Uplift painting is not WYSIWYG (SPIKE-52) against the edit model (D3) | Converge: 1024^2 9.1 s, 4096^2 1,036 s on a GTX 1080 Ti (SPIKE-52) | 25-40 (authoring tool plus a global solve) | **Reference only** (taken: GEN-063; open item SPIKE-45) |
| 4 | Terrain synthesis and authoring based on iso-contours (Huftier, Schott, Galin, Argudo, Peytavie, Guérin; CGF, EG 2026) | Terrain as nested vector contours grown by Open Eden Growth, a TIN by marching triangles, contour edits | Low: an authoring representation; the output elevation feeds erosion and amplification | Contours are global vector data, not a per-point field: no batch or partition invariance (GEN-005, GEN-020) without a raster bake; the edit model is stamps and brushes (GEN-026/030) | Not stated on the page | 30+ | **Not applicable** for M9 (an editor idea for later: contour stamps) |
| 5 | Sphere Carving: Bounding Volumes for Signed Distance Fields (Schott, Thonat, Lambert, Guérin, Galin, Paris; TOG 44(4), SIGGRAPH 2025) | Tight bounding volumes of an implicit surface from a few conservative SDF queries; speeds up sphere tracing and polygonisation | Low: terrain is a height field with per-tile height bounds (GEN-011); no SDF in M9 | The idea maps onto GEN-011/LOD bounds (conservative bounds from Lipschitz queries), which exist already | None for M9 | - | **Reference only** (tighter tile bounds, if a bound ever limits LOD) |
| 6 | The PhaseTree: Multiphase Signed Distance Fields (Galin, Hubert-Brierre, Schott, Cani, Peytavie, Guérin; TOG, SIGGRAPH 2026) | One construction tree for volumes of several materials; multiphase sphere tracing at under 25 % overhead | None for M9: no overhangs, caves or strata volumes on the roadmap | Volumes conflict with the height-field tile format (GEN-019) | - | - | **Not applicable** (caves or arches would revisit it) |

Fit notes common to rows 2 and 3:
- Both originals are OpenGL 4.3/4.5 compute shaders on f32 buffers. A generated field must have a CPU reference and a GPU twin within 1 mm (CON-07, CON-14). Thousands of iterated passes do not stay within 1 mm across adapters, so either the kernels run on the CPU or the bake's output is stored and becomes the canonical data. The second option is a new decision (an ADR, like the owner's shadow exception, roadmap 2026-10-26).
- Disk (M9 constraint: about 28 GB per body). The 16384 store is about 1.6 G cells at about 610 m. A stored global x2 amplification is about 6.4 G cells, 13 GB at 16 bits; x4 is 52 GB. So a stored amplification of the whole body stops at x2, replacing data rather than adding it.
- Bake time (M9 constraint: about 10 h). A global x2 at the preset's final-level pass counts is about 6.4 G cells x about 6,500 passes x 0.29 ns per cell-pass on a desktop RTX 3080, roughly 3-5 h on the GPU. On the CPU it is 20-50x slower and over budget. Within budget only on the GPU, which needs the ADR above.

## Terrain descriptors: what they could control
The engine already computes descriptors as drivers: GEN-070 (slope, mean curvature, wetness), GEN-075 (soil depth from slope, curvature, flow, lithology, altitude), GEN-076 (insolation from slope and aspect), GEN-061/062/064 (a landform driver for crease octaves and the hill band). SPIKE-55 measured slope and relief only.

| Descriptor (survey group) | Cost class | Use in this engine | Where |
|---|---|---|---|
| Slope, aspect, profile and plan (contour) curvature, Laplacian (local) | 3x3 stencil, O(N) | Profile and plan curvature split GEN-070's mean curvature: convex ridges against concave hollows for soil depth and wetness (M10 vegetation) | GEN-070, GEN-075 |
| Local relief, TPI at two radii, roughness, TRI (relief) | O(N) with summed-area tables | **M9 targets** at 1 and 5 km (SPIKE-55's relief) plus TPI-based landform classes (ridge, slope, valley, plain); TPI as a driver for crease amplitude instead of the coarse landform lattice | scorecard; GEN-061, GEN-064 |
| Geomorphons (landforms) | 8 directions x L samples per cell | Landform class histograms per site against GLO-30: the most direct "does it look like hills" number | scorecard; later a driver for materials (GEN-072) |
| Hillslope asymmetry | O(N) per window | Checks the north/south slope difference that real hills show; the generator has none today | scorecard |
| Drainage area D8 and MFD, wetness index, stream power, drainage density, distance and height above nearest drainage | priority-flood O(N log N) | Drainage density 0.5-3 km per km2 (SPIKE-40 target, SPIKE-47); HAND (height above nearest drainage) as a driver for valley-floor wetness and soil | scorecard; GEN-070, GEN-075 |
| Sky-view factor, openness, accessibility (visibility) | directions x march per cell | Sky-view as a baked ambient term and a vegetation and snow driver (M10); total viewshed is O(N^2), out | later driver |
| Peak prominence and isolation, divide tree (orometry) | global trees | Peak count and spacing per landform against GLO-30 | scorecard only |
| Hypsometric integral, elevation histogram | O(N) | Basin shape per landform | scorecard |

What descriptors could control in the generator:
- **Targets, not inputs, first.** Per landform class, the GLO-30 distribution of each scorecard descriptor becomes an M9 acceptance number (tier B: distance between histograms below a stated bound at named sites), extending SPIKE-55.
- **Then tuning.** The amplitudes and wavelengths of GEN-061's crease octaves, GEN-064's hill band (`HILL_BAND_MAX_M`), and erosion strengths are tuned per landform until the scorecard meets the targets. This is inverse tuning offline, not a runtime loop.
- **Then drivers.** Profile and plan curvature, TPI and HAND join GEN-070's set where M10 materials and vegetation need them (ADR 0076's driver list).

## Minimal prototype (design only; not built in this spike)
- An xtask subcommand (`cargo xtask terrain-score`) with no GPU: it reads a GLO-30 tile (the SPIKE-55 reader) and the generator at the SPIKE-55 matched sites, both resampled to 30 m on a 0.5 degree square.
- It computes eight descriptors in Rust, f64, `libm`, rayon with a fixed reduction order:
  - slope;
  - profile and plan curvature;
  - local relief at 1 and 5 km;
  - TPI at 300 m and 2 km;
  - geomorphon class at 1 km;
  - D8 drainage area with drainage density at a channel threshold of 1e5 m2;
  - HAND;
  - hillslope asymmetry.
- Output per site: percentiles of each continuous descriptor, the geomorphon class histogram, and a distance to the real tile per descriptor (1-D Wasserstein over the percentiles, histogram L1 for classes) in a table under `target/review/terrain-score/`, plus one colour map per descriptor.
- Cost estimate: 13 M cells per tile, geomorphons about 3.4 G reads (2-5 s on 16 threads), priority-flood about 1-2 s, the rest under 1 s; under 30 s for nine tiles and eighteen sites, plus the generator's sampling time (SPIKE-55).
- Done when: the scorecard reproduces SPIKE-55's slope and relief numbers, and one generator change (for example GEN-064's band amplitude) moves the scorecard in the expected direction.
- Build: 4-6 agent days as an OpenSpec change (testing or build-tooling spec, tier B), the reference code read for definitions only (MIT; any adopted algorithm named in the change's licence section).

## Recommendation
1. Adopt the terrain-descriptor scorecard as M9's realism measure (the prototype above), before any generator change, so every M9 change is judged by it.
2. Adapt Multi-Scale Erosion's thermal and deposition kernels as CPU Jacobi passes with a bounded halo, first as a spike on one chunk against the scorecard (cost per chunk, slope distribution at the angle of repose).
3. Keep a stored global x2 amplification as an option only with an ADR for a GPU bake whose stored output is the canonical field (CON-07, CON-14); disk and bake time fit at x2, not at x4.
4. The uplift paper stays a reference (GEN-063 has taken its idea; the A-net decision stands). The iso-contour, sphere-carving and PhaseTree papers do not apply to M9.

## Follow-up: the two full texts (2026-10-30)
Read as untrusted data (text extracted to a temporary folder outside the repo; the correlation figure viewed as an image). Neither the PDFs nor the text are in git.
- Terrain descriptors for landscape synthesis, analysis and simulation, CGF 44(2), https://doi.org/10.1111/cgf.70080
- Terrain Amplification using Multi-Scale Erosion, TOG 43(4), https://doi.org/10.1145/3658200

### Descriptors paper: timing table (its Tables 1 and 2)
- Method: C++, **one CPU thread**, a 1024^2 height field of 952 km2 in the Alps (about 30 m cells), mean of five runs. Every cost is a factor of the average slope's time, **18 ms** (about 17 ns a cell). Metrics with a radius were timed at 100, 500, 1,500 and 5,000 m and cost grows with the square of the radius.

| Descriptor | Cost factor (radius 100 / 500 / 1,500 / 5,000 m) |
|---|---|
| Average slope | 1 |
| Laplacian | 1 |
| Curvatures (min, max, mean, profile, contour, tangential; quadric fit) | 4 / 23 / 175 / 1,800 |
| Local relief | 1 / 5 / 50 / 530 |
| Local mean and variance | 1 / 5 / 30 / 340 |
| TPI | 1 / 4 / 27 / 300 (8 at any radius with summed-area tables) |
| TRI / rugosity / roughness | 1-6 at 100 m, 400-960 at 5 km |
| Hillslope asymmetry | 12 / 18 / 120 / 950 |
| Openness (positive, negative) | 8 / 20 / 48 / 130 |
| Geomorphons | 6 / 15 / 41 / 130 |
| Sky-view factor, accessibility | 120-150 per ray direction |
| Approximate sky-view factor | 2 |
| Viewshed (one point) / visibility index | 2e5 / 1e11 |
| Drainage area D8 / MFD | 7 / 13 |
| Stream power index | 9 |
| Height above nearest drainage | 11 |
| Prominence, dominance (incl. divide tree) | 20, 15 |
| Max branch length | 75 |

### Descriptors paper: correlation matrix (its Figure 52, one example terrain; values read from the figure, about ±0.1)
| Group | Strongly correlated (about 0.7-0.95) | What it means for a scorecard |
|---|---|---|
| Relief and roughness | local variance, TRI, rugosity with each other; all moderately with slope (about 0.5) | local relief stands for the group |
| Curvatures | max, mean, min, tangential and profile with each other; contour less; Gaussian apart | keep profile and contour; mean adds nothing |
| Position | fractional Laplacian, TPI, percentile, ORS, Jut with each other (about 0.8-0.9), and with distance and depth to water (about 0.7) | TPI stands for the group; the authors name TPI, fractional Laplacian, ORS and Jut as cheap proxies of the visibility index |
| Sky | accessibility, sky-view factor and its approximation with each other; the approximation strongly anti-correlated with slope | the cheap approximation (factor 2) replaces the ray-cast ones |
| Hydrology | drainage area D8 and MFD (about 0.95), stream power (m = n = 1) with them; wetness index weaker | one drainage-area variant suffices |
| Light | irradiance at 45 degrees N with northness | GEN-076's insolation already covers it |
- Geomorphons, drainage density and HAND are not in the matrix.

### Descriptors paper: artist study
- Professional artists from the games industry rated grey-scale maps of the metrics on their own synthetic terrains.
- Useful in their eyes:
  - maximum branch length for mountain ridges;
  - curvature variants for detail on bare bedrock;
  - openness for snow patterns;
  - drainage area as the flow map;
  - the fractional Laplacian as a proxy for sediment deposition;
  - viewsheds for placing villages or castles.
- Too imprecise for a pipeline: hillslope asymmetry, distance to a river (HAND's relative), roughness.

### What changes in the scorecard (the minimal prototype above)
- **Descriptors, 8 become 7.**
  - Kept: slope; profile and contour curvature; local relief at 1 and 5 km; TPI at 300 m and 2 km; geomorphons at 1 km; D8 drainage area with drainage density.
  - Dropped: hillslope asymmetry (rated imprecise, factor 12-950); HAND (correlated with TPI through depth to water, factor 11).
  - Local variance, TRI and rugosity stay out: local relief stands for them.
- **Cost, revised from the paper's factors.** Per 3600^2 raster on one thread: about 75 slope-times = 75 x 0.22 s, about 16 s. The terms: slope 1, two curvatures 8, relief by blocks about 2, two TPIs with summed-area tables 16, geomorphons at 1-1.5 km about 41, D8 7. Geomorphons are half the cost.
  - For 9 tiles and 18 sites (27 rasters): about 7 min on one thread, **about 1 min** with one raster per thread. The earlier "under 30 s" was low. Generator sampling time is extra (SPIKE-55).
- **M10 drivers.** Openness and the approximate sky-view factor are cheap (2-48) and artist-endorsed for snow and ambient terms: candidates beside GEN-076. Not scorecard items.

### Multi-scale erosion paper: what changes
- **Timings (its Table 2, ms per iteration on an RTX 3080, GLSL compute):**

| Resolution | Erosion | Thermal | Deposition |
|---|---|---|---|
| 1024^2 | 0.33 | 0.06 | 0.69 |
| 2048^2 | 1.27 | 0.23 | 2.70 |
| 8192^2 | 19.2 | 3.51 | 41.5 |

  Deposition costs about twice erosion; thermal a fifth. Height retargeting and multi-scale breaching run on the CPU and take "a few seconds" at 4096^2.
- **Iteration counts (its Table 1):** 300-1,700 erosion iterations per level at 1024-2048, 400-6,000 at 128-512; deposition is controlled by its step count (400 and 800 steps illustrated).
- **Drainage is one routing step per erosion iteration**, reusing the previous step's area. Flow therefore spreads only as far as the iteration count reaches. The authors' limitations say the method needs the whole map at every step, and that patches beyond 8,192^2 leave drainage across patch borders as an open challenge. That is this engine's chunk problem, which the A-net solves with a global network (ADR 0077).
- **Revised per-chunk estimate** for a 5 km chunk at 4 m (about 1.5 M cells, 1.5x the 1024^2 timings):
  - Full E+T+D at about 1,000 iterations: 1.6 ms a GPU iteration, so 20-50x on the CPU gives **30-80 s**. The 30-90 s range holds.
  - Thermal and deposition only, with drainage seeded from the A-net: 1.1 ms GPU, 23-56 ms CPU an iteration. At 400-800 deposition steps that is **9-45 s**, still far over GEN-052's 4 s.
  - Thermal alone is cheap: about 2-5 ms CPU a pass, so 100 passes take 0.2-0.5 s.
- **Verdict stays ADAPT, with a narrower runtime scope:**
  - In the chunk stage only thermal stabilisation with a noise-perturbed critical slope (the paper perturbs it to avoid uniform slopes), about 100 passes.
  - Deposition and fluvial erosion belong in the bake, with the drainage area seeded from the A-net instead of iterated.
  - Recommendation 2 changes accordingly: the one-chunk spike tests thermal passes in the chunk stage, and deposition on a bake region.

## Visuals
A desk study: nothing rendered.
