## Context

CPU-bound: 20 test threads, 129 s of summed test time, so the wall time is the summed work divided by the cores plus the tail. Only the work can shrink; the tests keep their assertions.

## Decision

Reuse per-tile work across the steps of a sweep through the production `SelectionCache`, which holds pure functions of the tile (LOD-008, `a_warm_selection_cache_never_changes_a_plan`). A warm sweep also tests the path the app flies. A cold selection every 16th step (and `assert_eq` of whole selections) keeps the cache honest inside the sweeps themselves. Alternatives rejected: fewer or coarser steps (weakens), moving tests to tier B (their requirements LOD-004/LOD-008 are tier A), disabling debug assertions (changes what the tests exercise).
