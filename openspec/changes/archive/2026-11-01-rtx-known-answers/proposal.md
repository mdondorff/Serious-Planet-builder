# rtx-known-answers

## Why
- The RTX known-answer hashes of `gpu_compact_blocks.rs` and `gpu_water.rs` were stale after MESH_VERSION 8 to 10 and the driver update to 621.01.

## What
- Re-recorded the RTX hashes with `cargo xtask test gpu --real` (2026-11-01, MESH_VERSION 10, RTX 3080 Ti Laptop GPU, NVIDIA 621.01): one_shot and the lit water frames moved; persistent is unchanged.
- Each RTX table carries a comment with the date, mesh version and adapter. Another adapter or driver skips the comparison and prints its hash; it never compares against the RTX hash.
- No spec delta: REND-016 and ATMO-004 are unchanged.
