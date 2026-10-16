## Decisions

- Budget 15 s (owner, 2026-10-16). Still a printed warning in `test fast`, still a failing step in `accept`.
- Measurements (quiet, nextest): 9.8 s run, 11.0-12.3 s elapsed; per-test table in the PR. At `-j 1` the run is 45 s of real CPU; the top test alone takes 1.3 s.
- Crash: `cargo xtask test`, `test fast`, `test gpu` and CI call only `cargo nextest run`; nextest runs each test in its own process, so the tests of `gpu_skeleton` and `gpu_warmup` never share a process or run as threads of one. Nothing to serialise; a `cargo test` run by hand does not have that isolation.
