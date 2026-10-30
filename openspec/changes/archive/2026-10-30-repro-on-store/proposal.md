## Why
B5 (test robustness): `repro` baked the recorded world in memory, at most 1024 cells, so an F9 bundle of the 16384 store could not be replayed (status Tracked, "Repro of bundles on a stored bake"). CON-19 says every visual bug gets a bundle that `cargo xtask repro` replays.

## What changes
`--repro BUNDLE --store DIR` (and `--replay-path`) rebuild the recorded world from the stored bake of the recorded size, read-only. A bundle whose bake is beyond the in-memory limit and no `--store` is refused with the size and the flag. The world is still checked against the recorded hash, field id and radius.

## Not changed
The bundle format. A replay without `--store` of a small bake is what it was.
