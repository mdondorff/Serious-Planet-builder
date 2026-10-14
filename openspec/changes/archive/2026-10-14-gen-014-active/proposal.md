## Why

M3 gate condition (CON-17, GEN-014): GEN-014 (global bake time) is tier C and can only be measured in a perf window, never in a test, so it stayed `planned`: spec-lint knows no tier-C evidence but a `tests/gpu_*.rs` test. A new tier would touch the constitution.

## What Changes

- spec-lint (`xtask/src/speclint.rs`): a `// spec: ID` comment directly above a perf-harness scene entry point (a `fn perf_*` item in `crates/app/src/`; only comments and attributes in between) counts as tier-C evidence of that requirement. No new tier; tier A and B are unaffected; a citation elsewhere or above another function covers nothing.
- `crates/app/src/globalperf.rs`: `perf_global_bake` cites `// spec: GEN-014`.
- TEST-001 text states the rule; GEN-014 becomes `active`.
- Tier-A test of the rule: the citation passes, removing it fails, and wrong function name, wrong crate path, test directory and tier A requirements are not satisfied by it.
