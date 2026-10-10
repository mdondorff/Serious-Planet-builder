## Why

ADR 0009 review follow-up 3 (docs/status.md on change/adr-0009-review, owner asked; M3 will be built against it): spec-lint counted any `// spec:` comment anywhere, so a citation above a helper or a constant, or a unit test standing in for a GPU requirement, satisfied coverage.

## What Changes

- `cargo xtask spec-lint` counts a citation only when the `// spec:` comment sits directly above a `#[test]` function (comments and other attributes may sit between), and requires every active tier A, B or C requirement to be cited by at least one test of its own tier: integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B, everything else (including unit tests under `src/`) tier A. The test runner uses the same prefix match, `binary(/^gpu_/)`, instead of its former substring match (`~gpu_`).
- Stray citations still have to name an existing requirement or constitution rule.
- The current repository already satisfies the rule (every `// spec:` citation, 198 before this change, is attached to a test, and every active requirement has a test at its tier), so no test files change.
- Not done: TEST-008 stays Verify A although its "replays to the recorded pixels" scenario is only checkable at tier C (a requirement declares one tier); splitting it is a decision for the owner, recorded in docs/status.md.
- Modified: TEST-001.
