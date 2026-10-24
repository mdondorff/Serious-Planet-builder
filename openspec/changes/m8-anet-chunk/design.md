## Context
Inputs: ADR 0077, SPIKE-33 (`spikes/landscape-evolution/results/results.md`), SPIKE-35 (`spikes/landscape-evolution-2/results/results.md`), the chunk field of `openspec/changes/m8-chunk-field/` (design sections 10 to 16), the stored coupled run (GEN-040, STRM-043). The prototype code is `spikes/landscape-evolution-2/results/landscape-evolution-2-prototype.patch` (never merged): a reference for shapes and for the hash, not code to merge.

## Decisions

### 1. The model (SPIKE-33)
Stream power `dh/dt = U - K A^m S^n` with m 0.45, n 1, K 2e-5 times lithology, U 1e-3 m/yr per 1000 m of base height, dt 1e4 years for the fine steps (the coarse run uses 1e5), solved implicitly (Braun and Willett 2013): `z = (z + dt U + f z_r) / (1 + f)`, `f = K A^m dt / distance`, visiting cells donors-after-receivers. These are the parameters of the stored coupled run too; a chunk reads them from the same `CoupledParams` and `CoupledInput` shape so the two runs agree on what a river is.

### 2. Frozen network
A chunk does no routing and no flood. Receivers and drainage area are computed once at step 0 over the initial fine surface plus the corridor and kept for all N steps (SPIKE-33: with the network a function of position alone, neighbouring chunks agree in their cores). The corridor is the coarse path of the stored run through the chunk: for every coarse cell of the chunk's halo region whose area is over 100 km2 (the river network of ADR 0077), the path from the cell to its receiver, displaced by a noise meander of 700 m (wavelength 0.9 km, zero at the coarse cell centres, points every 40 m), so every chunk finds the same path. Inflow at the halo ring is the coarse drainage area crossing the ring.

### 3. The breach rule (c4, SPIKE-35 item 1)
Three changes together: channel cells (area over 50,000 m2) take no diffusion and no slumping; a cell whose receiver is a higher ring cell is an outlet (no cascade up the channel); inside the corridor a cell drains to the lowest forward neighbour, not the best aligned one. Result on the spike's lattice: 0 uphill channel cells, breach cells 8,000 instead of 20,000, bed 4 m (p50) below its surroundings. "Uphill" means the receiver is higher by more than 1 micrometre.

### 4. Jittered D8 receivers (SPIKE-35 item 3)
The slope of each of the eight directions is scaled by `1 + j (2 h - 1)`, `h` a hash of the lattice cell and the direction (`mix64`), `j` 0.4 (0.4 to 0.8 measured: axis share 0.55 to 0.52 against 0.59 for D8, no extra cost). It is a function of position only, so every chunk makes the same choice, and it is the same function as the coupled run's `jitter_factor`: one definition, shared.

### 5. Halo and step cap
N = 30 (the cap); halo 3 km, the smallest with bit-exact cores at one site (1.5 km leaves 19 cells under 0.5 mm). The stage states the halo in metres and the rings it needs (`ChunkStage::halo_rings`, default step cap plus one; A-net returns 3 km over the cell size), which is at least cap times reach (30 x 8 m). The halo is re-checked at a site with a large river crossing the border and at the final cap (step 2 test, tier B).

### 6. Cell graph or dense raster
`chunkfield::region` builds the ball of cells around a chunk with a hash map. At 8 m cells and a 3 km halo a chunk is 1.9 million cells; the hash map costs about a second per chunk. Step 2 first measures it; if it matters, a dense fast path is added for a chunk whose halo stays inside its face (the usual case), with the graph kept for the face edges and corners (bit-exactness against the graph is a test). The A-net solve runs on the dense array either way, and chunks across a face edge keep the graph's neighbour function.

### 7. Hillslope processes (step 3)
Diffusion (explicit, `k dt / dx^2` within the stability bound, 4 neighbours, fixed order) and slumping (thermal: a cell steeper than the angle of repose gives the excess to its lowest neighbour, Kahn order in cell-index order, a fixed number of passes) on non-channel cells; channels are exempt (decision 3). All `libm`, no unordered containers, no reduction that depends on the thread count (CON-14); one thread per chunk, threads across chunks.

### 8. From change to field
The stage returns the change raster `final - base` on the chunk's raster plus apron; the pyramid, the keys and the manager are the M8-2 ones. Process hash: the parameters of decisions 1 to 7, the jitter seed, the code version.

### 9. Measure, then decide the fade (step 4)
Chunk time per km2 at 4, 8 and 10 m (release, one thread), against SPIKE-33's 0.06 s per km2 at 4 m; the step against the parent at tile levels 11 to 14 at the fixed poses (the table of `m8-chunk-field` design section 11 with A-net instead of the stand-in); the fade time `max |change| / (0.25 m x 60 fps)` for the amplitude measured. Exit criterion 3 says the switch fades in, so the fade is built (`m8-chunk-fade`: a per-vertex change plane next to the compact slots, a node weight, the shader, golden-review); the measurement decides its length and whether a top level above 14 can be used. If criterion 3 fails at every top level, stop and report.

### 10. Viewer
A world with a stored coupled channel enables the chunk stage at runtime (D7): the manager with a half-pool of workers, the disk cache in the store's directory (`PackChunkDisk`), top level 14 until the measurement says otherwise. A world without the channel has no chunk stage.

## Risks and open items
- One site, one seam for the halo; a river crossing the border is the hard case.
- The look is judged from engine renders only at step 5; hillshades of the spike are not the engine.
- Cost of a chunk with a 3 km halo against the 10 s landing budget at a 300 m pan (criterion 3): 5 km core plus 3 km halo at 8 m is about 2 s of one thread by SPIKE-33's rate, at 4 m about 7 s.
