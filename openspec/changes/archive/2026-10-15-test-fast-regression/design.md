## Decisions

- **Parallelise inside the test, not across more tests.** The two frustum sweeps and the steep `height_aware` sweep were single-threaded tails of 8-30 s next to hundreds of 0.1 s tests. Scoped threads over independent cameras or views keep one test per requirement and the assertion messages (a panic is re-raised with its message). A wall-time gain only; the CPU time is unchanged by it.
- **Deduplicate before parallelising.** The four heading variants of one view cut nearly the same tiles; building each distinct tile's mesh once cuts the mesh work by more than half (36,000 checked tiles, 15,600 distinct on the steep field) without touching what is asserted.
- **Tier split only where the sweep is exhaustive and a slice stays.** What remained after the above was mesh work proportional to the number of checked tiles. The tier-A tests keep every camera of the reference sweep and, on the steep field, both views at two headings, and check every eighth dropped tile; the previous density (every second tile, all cameras) is the tier-B `oracle_frustum_cut_sweep.rs`, which cites LOD-007 as well. The scenario text of LOD-007 names both. spec-lint still finds tier-A evidence for LOD-007.
- **A printed warning, not a failing limit.** `test fast` prints the slowest five tests and the existing warning above 10 s. Per-test times of a run on 20 threads include contention, so the list says so. `cargo xtask accept` keeps failing above the budget (it already did).
- **Start order, not selection.** nextest `priority` starts the eight longest tests first so that they do not run last and alone; nothing is skipped or retried.

## Rejected

- Fewer or coarser cameras in every tier: weakens what is asserted.
- Raising `opt-level` of `planet-cache` and `planet-streaming`: no measurable gain (9.2 s against 9.3 s), reverted.
- Lengthening the budget: forbidden.
