# determinism-threads

Why: ADR 0114 condition 3 of the external review (2026-11-11): determinism is shown on actual Windows and Linux CI, across thread counts and repeated runs. The CI determinism job diffed the manifest of the tile generator across the two operating systems, but that part of the manifest is serial and says nothing about thread counts, and the pipeline that bakes the world (regional solve, hydrology, channel graph, K7, the writer) was not in it.

What: `planet_cache::region_bake::determinism_fixture(threads)` bakes a small analytic face to a commit with a channel graph and hashes every file it wrote. `generate --manifest` writes three lines (`regionbake t1`, `t1again`, `t8`): one thread twice and eight threads. A tier A test requires the three equal in one process (a thread count or a repeat that changes a byte fails it); the existing `det-diff` of the CI determinism job compares the lines across Windows and Linux. No CI step changes.

Requirement ID: BUILD-051 (bulldozer, 2026-11-12). The bake fixture is the crusader's GEN-177 fixture (same spec and analytic base).
