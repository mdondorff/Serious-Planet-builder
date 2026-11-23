# SPIKE-143: which stage makes the pyramid and chainsaw looks?

Opened: 2026-11-23 (lead #176, amended #176b; G3 gate row G3-9, risks R11 and R12, adopted by the owner on 2026-11-23)
Question: On RG's own chain (spike tip, today's defaults), which stage makes (a) the pyramid look (lattice-scale facets or bumps at 611 m or 814 m) and (b) the chainsaw look (triangles and saw-tooth banks along river beds)? Which layer makes the parked looks: K5's diagonal washboard (SPIKE-139), straight rivers, diamond hills?
Why it matters: G3-9 removes the pyramid and chainsaw looks at their source stage before the port; O5 decides which parked look items join it; O8 decides on straight rivers.
Time box: 1.5 days of agent work.
Success criteria: renders per stage on R1h, F1 and R3b; the pre-registered metric per stage and window; each look attributed to its stage and mechanism, with candidate source fixes named.
Fallback if it fails: the looks stay unattributed G3-9 items, judged on renders only.
Code location: `spikes/look-attribution/results/raw/` (analysis and render scripts). The chain is `spike/rg-prototype` 7fe4e73d with this lane's local switches, all off.
Status: open. The metric below was registered before any render was looked at (#176b).

## Pre-registered metric (committed before any render is looked at)
Inputs per window (R1h, F1, R3b) and per stage surface:
- S0;
- S_KH (`kh`);
- after KS0 (`ks0`);
- after KS1 and its connect (`ks1c`);
- after band6 and its connect (`band6c`);
- after KV (`kv`);
- final (`final`).
Also the base B at L (`base`) and z_s at H1 (`zs`), as layers. All are counted inside the box (1,600-cell margin) on land (S0 above the sea).

### M1: lattice facet excess (the pyramid look)
- Kink field: the squared second differences of the surface, d_xx = z(x+1) - 2 z(x) + z(x-1) along x and d_yy along y; d_d1 along the (1,1) diagonal and d_d2 along (1,-1), each divided by 2 for the sqrt(2) spacing.
- Phase profile, per lattice period P (6 L cells = 611 m, the base lattice; 8 L cells = 814 m, one H1 cell): the share of d_xx energy at each x mod P (all rows), and of d_yy energy at each y mod P. Global L cell indices are used.
- Line share LS_P: the largest share held by two adjacent phase bins (cyclic), the mean of the x and y profiles. A lattice-free surface gives 2/P (33.3 % for P = 6, 25.0 % for P = 8).
- Lattice excess X_P = LS_P / (2/P) - 1.
- Quad-diagonal excess X_diag: the same with d_d2 energy against (x + y) mod 8 and d_d1 energy against (x - y) mod 8 (kinks on the diagonals of 814 m quads), the mean of the two.
- Reference: the same metric on GLO-30 (Copernicus 1 arc-second tiles in SeriousPlanetData/dem/glo30-samples), resampled to 101.8 m, with the lattice phase taken from the tile's own grid. Its excesses give the lattice-free noise level X_ref (the largest of the tiles' |X_P| per P).
- A look is present on a surface when X_P (or X_diag) > max(0.10, 3 X_ref).
- The stage that makes it is the first stage whose value is present and exceeds the previous stage's by more than 0.05.

### M2: channel saw-tooth (the chainsaw look)
- Lines: the vertex cells of the final graph's L reaches inside the box (graph.txt), in reach order.
- Diagonal-step share: the share of consecutive vertex steps that are diagonal (8-connected), per window. This is a property of the graph, not of a stage.
- Bank saw-tooth, per stage surface. At an interior vertex k:
  - n is the unit normal of the chord from vertex k - 1 to vertex k + 1.
  - The bank heights b+(k) and b-(k) are bilinear samples at +-2 cells along n.
  - The saw-tooth is s(k) = |b(k) - (b(k-1) + b(k+1)) / 2|, per bank.
- Reported per stage: p50, p90 and the mean of s over every interior vertex of reaches with at least 5 vertices; separately on diagonal-step vertices and on axis-step vertices.
- Reference: the same lines sampled on S0, which carries no carving.
- A chainsaw look is present on a stage when its p90 exceeds both 1.5 x S0's p90 and S0's p90 + 1 m.
- The stage that makes it is the first stage where the look is present and p90 rises by more than 20 % over the previous stage.

### M3: straight runs and sinuosity (straight rivers)
- Straight-run share: the share of KG-trunk image length (the L reaches whose parent is an H1 trunk reach) lying in runs of one repeated step direction longer than 2 km.
- Sinuosity per reach: the vertex path length over the straight distance between its ends, for reaches over 2 km. Reported as p50 and the length-weighted mean.
- GLO-30 reference: D8 channels with over 75 km2 of area on the same tiles as M1 at 101.8 m, cut at confluences into reaches over 2 km, with the same two numbers.
- Coincidence with the chainsaw: the share of vertices with s(k) above the final surface's p90 that lie in straight runs, against the share of all vertices in straight runs (a ratio above 1.5 counts as coinciding).

### Renders and the mesh
- Shaded relief from each stage's own height grid at a 10-degree sun from two azimuths (45 and 315 degrees), on the same crops per stage: a river reach, a hill flank, a lake rim per window.
- The crops are chosen from the final surface's M2 hot spots and the kept-lake rims before the stage renders are drawn.
- Mesh: one crop of the final surface drawn as the viewer's triangulation (each cell split along (i, j) to (i + 1, j + 1), `planet_generators::mesh::grid_indices`) beside the shaded height grid, so a renderer-only cause shows apart.
- Looks whose source is main's own read (for example the 611 m bilinear facets of draft #1137) are named as main's, not as an RG switch.
