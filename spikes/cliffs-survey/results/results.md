# SPIKE-94: cliffs, overhangs and steep faces next to the heightfield (survey)

Opened: 2026-11-02
Question: How do other engines and papers represent cliffs, overhangs and faces steeper than 45 degrees next to a heightfield, and which options fit our tile, LOD and streaming model?
Why it matters: the owner's question of 2026-11-01; ADR 0091 caps the heightfield's realism at 45 degrees and leaves steeper faces to a separate technology, each needing its own ADR. This page is input for an owner discussion, not a decision.
Time box: 3 h, reading only, no code (opus-planner, inbox #69); used about 1 h 15 min.
Success criteria: a ranked option table with fit, cost per frame at 1440p/60, authoring impact (ADR 0089) and size in agent-weeks; the two strongest options.
Fallback if it fails: ADR 0091 point 2's order stands as written.
Code location: none (reading only); no paper was committed.
Closed: 2026-11-02

## Our constraints
- **Terrain:** a cube-sphere quadtree of 32 x 32-cell tiles to level 22 (7.5 cm), CPU-generated per tile and streamed. The plan decides on the CPU and the renderer executes it (CON-03); bits are deterministic (CON-14).
- **Frame budget at 1440p (BUDG):** terrain, water and shadows share 5.0 ms of GPU; terrain has 10.0 ms of main thread; 3.1 ms of GPU headroom is unclaimed.
- **Instances:** an instance path with a CPU culling reference already exists (REND-011), but nothing uses it yet.
- **Authoring:** class weights and drivers (ADR 0089). Authored results are hard constraints after generation.
- **Steep share to cover (SPIKE-85, GLO-30 at 30 m):** area above 45 degrees is 4.3 % in the Alps and 8.7 % at Everest. The Highlands and Black Forest have 0.1 %.

## Sources (by family)
- **Instanced cliff and rock meshes on steep cells:**
  - Far Cry 5's Houdini world tools: "cliff rocks" over about 100 km² (Carrier, GDC 2018).
  - Far Cry 5's GPU terrain pipeline, which "supports procedural cliffs and displacement geometry" (Moore, GDC 2018).
  - Ghost Recon Wildlands' rule-based rock scattering ("align on slope", "cliff detection"; Martinez and Werle, GDC 2017).
  - Horizon Zero Dawn's GPU runtime placement (van Muijden, GDC 2017).
  - UE5 Nanite with scanned cliff kits (Karis, SIGGRAPH 2021).
  - Unreal's practice of a hole in the heightfield filled with meshes (Landscape Visibility tool docs).
- **Steep-face shading:**
  - triplanar texturing (Geiss, GPU Gems 3 ch. 1, 2007);
  - triplanar normal mapping (Golus, 2017);
  - decals in an adaptive virtual texture (Chen, Far Cry 4, GDC 2015: 10 texels/cm over 10 x 10 km).
- **Voxel or SDF layer:**
  - Transvoxel, LOD stitching of marching cubes (Lengyel, dissertation 2009/2010; a Rust crate exists);
  - No Man's Sky's runtime voxel planets (McKendrick, GDC 2017);
  - Arches, material stacks plus an implicit surface (Peytavie et al., CGF 2009);
  - feature-based volumes (Becher et al., TVCG 2019).
- **Displacement:**
  - triplanar displacement with tessellation, extruding a heightmap's steep faces at render time into overhangs and boulders (Weiss, Bayer and Westermann, Eurographics 2020 short paper);
  - flow mapping (Gamito and Maddock, 2008).
- **Layered heightfields:** material stacks per column (Beneš and Forsbach, SCCG 2001); a hybrid voxel plus heightmap (Koca, Bilkent thesis 2012).
- **Implicit amplification:** construction trees of implicit 3D features on a heightfield (Paris et al., TOG 2019), polygonised offline.
- **Survey of the field:** Galin et al., "A Review of Digital Terrain Modeling" (CGF 2019 STAR).
- **What shipped open worlds do:** every documented open world keeps the 2.5D heightfield. They add slope-driven texturing, plus cliff and rock mesh instances placed by rules or by hand on steep cells. Volumes ship only in planet-scale or editable-terrain games (No Man's Sky); the rest is research.
- **Verification:**
  - Talk slides and most paper PDFs could not be read (logins, size limits). Titles, venues and abstracts are verified; methods beyond the abstracts are marked unverified in the report.
  - No source states draw calls or ms per frame for cliff geometry. The only counts found are second-hand: the UE5 demo's 1 billion source to about 20 million drawn triangles, and Horizon's about 250 asset types per frame.

## Option table (ranked)
Cost figures are this survey's estimates from our budgets and frame structure, not measurements; a spike measures the chosen option.

| Rank | Option | What it gives | Fit with tiles, LOD, streaming | GPU cost at 1440p/60 (estimate) | Authoring (ADR 0089) | Size (agent-weeks) |
|---|---|---|---|---|---|---|
| 1 | **Steep-face shading**: triplanar rock material and normal detail on cells above about 35-45 degrees, chosen per class | rock-looking faces; the silhouette stays the heightfield's (≤ 45 degrees) | full: per-pixel, no new data, no LOD change | 0.3-0.8 ms (three texture taps only on steep pixels) | a rock material per class; the slope threshold is a parameter | 1-2 |
| 2 | **Instanced cliff meshes**: a library of rock and cliff meshes with LODs, placed deterministically per tile on cells above 45 degrees, culled and drawn through the existing instance path | real vertical faces, ledges, broken silhouettes; no overhang deeper than a mesh | good: an instance list per tile is a derived product of the tile (cache key from the field), streamed with it; the heightfield LOD is unchanged; seams by sinking the meshes into the terrain and a depth-fade blend | 1-2 ms in a mountain view (indirect draws, mesh LODs and impostors beyond about 2 km); 0 on plains | a cliff density from the steep share and the class (mountains, high mountains), overridable by class paint; an asset library to make or license | 4-6 for v1 (placement, LOD chain, shadows); collision proxies later |
| 3 | **Steep-face displacement** (triplanar or vector displacement of steep cells, Weiss 2020 style) | true geometry on faces, small overhangs, boulders | medium: wgpu has no hardware tessellation, so refinement is a compute pass that changes the tile's mesh; morphing, cracks between levels and height queries (collision) must agree | 1-3 ms plus a compute pass per steep tile; unknown | none beyond option 1's material | 4-8; high risk |
| 4 | **Voxel or SDF patches** near steep cells (Transvoxel or dual contouring) | overhangs, arches, caves | low: a second LOD and streaming system per patch, with its own store, collision and edit path; seams with the heightfield | depends on patch count; meshing on the CPU per patch | caves and overhangs as authored items (ADR 0066) | 8-12; ADR 0091 limits it to caves and overhangs |
| 5 | **Implicit 3D amplification** (Paris 2019) | sea arches, slot canyons, hoodoos, stratified cliffs | as a content source only: polygonise implicit features offline or per tile into option 2's meshes or option 4's patches | that of option 2 or 4 | features placed by class and drivers | 6-10 on top of 2 or 4; research |
| 6 | **Layered heightfields** (material stacks) | thin overhangs, layered strata | low: the LOD morph and the plan assume one surface per column | unknown | a second height channel per layer | 6-10; research |

## The two strongest options
1. **Steep-face shading (rank 1):** it is cheap and fits fully, and every other option needs it underneath. It makes the 45-degree heightfield read as rock, though the silhouette does not change.
2. **Instanced cliff meshes (rank 2):** this is what shipped open worlds use at kilometre view distances. It fits our model as a per-tile derived product drawn by the existing instance path, and leaves the heightfield's LOD alone. Its main costs are the asset library and the seam between mesh and terrain.
- Together they follow ADR 0091 point 2's order. Displacement (rank 3) is the alternative if no asset library is wanted. Voxel patches stay reserved for caves and overhangs.

## Questions for the owner discussion
- Is an asset library (made or licensed rock and cliff kits) acceptable, or must cliffs be procedural (option 5 as the source of option 2's meshes)?
- Coverage goal (ADR 0091 point 3): the Alps' 4.3 % of area above 45 degrees, measured against the real tiles; is that the target for v1?
- Collision on cliffs: is a coarse proxy enough until a game layer exists?

## Visuals
None: a reading survey; no renders.
