## 1. Reader and field
- [x] 1.1 `read_elevation_c1`, `READER_C1_VERSION`, `C1_OVERSHOOT`, `C1_SLOPE_FACTOR` (global.rs)
- [x] 1.2 `scan_base` with the C1 window, range and step factors (terrain.rs)
- [x] 1.3 `cpu-erosion-v2`: `ErosionGenerator::field_c1`, `HeightFieldKind::ErodedC1`, keys and versions
- [x] 1.4 manifest rows `erosionc1` and the header line

## 2. Tests (tier A, tests/c1_base.rs, GEN-107)
- [x] 2.1 lattice point is its own sample; C1 across cell edges and across face edges; same bits from either face on every edge and corner; corner finite and continuous
- [x] 2.2 bounds hold (random caps, edges and corners, spikes and steps); the overshoot is real and below the bound
- [x] 2.3 batch and thread independence; identity and keys; known answers pinned

## 3. Review and evidence
- [x] 3.1 numerics-reviewer (one HIGH: bounds beside a face edge, fixed with a test; four LOW fixed)
- [x] 3.2 field_probe re-run on v2 (flank and belt-east): base slope and profiles in target/e2b-v2
- [x] 3.3 CPU cost table (c1_cost example) in the PR body

## 4. After the preview
- [ ] 4.1 The owner's decision on the default; the list of moved goldens and pins; the archive
