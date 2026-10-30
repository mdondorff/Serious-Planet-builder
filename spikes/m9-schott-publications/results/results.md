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

## Visuals
A desk study: nothing rendered.
