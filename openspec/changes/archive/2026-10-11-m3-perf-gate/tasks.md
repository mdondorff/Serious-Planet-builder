## 1. Gate and validity rules
- [x] 1.1 Pure rules with a tier-A test for every refusal (AC, scheme, adapter, processes and the process tree, CPU, GPU, temperature, unreadable values)
- [x] 1.2 Throttle mask classification, noise retry rule, validity
- [x] 1.3 Platform queries and parsers (tested), measurement of the 10 s window

## 2. Wiring
- [x] 2.1 Terrain and skeleton workloads: gate, override, throttling and noise validity, one retry per view
- [x] 2.2 `cargo xtask perf` passes `--machine-ready` through only as an override
- [x] 2.3 Log: `owner_override`, validity reasons in warnings

## 3. Close-out
- [x] 3.1 TEST-012 modified; CLAUDE.md stop condition reworded
- [x] 3.2 Reviews (numerics n/a), verifier, archive, status line
- [x] 3.3 Handed over: the first real gate run is the architect's (docs/status.md, Waiting for the owner/architect); coders never run `cargo xtask perf`
