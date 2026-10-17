## Why
PR #222 was merged with unresolved git conflict markers in `docs/adr/README.md`, `docs/log.md` and `docs/learnings.md` (fixed in PR #232). No check caught it. `cargo xtask spec-lint` runs in both the light `docs` workflow and the full CI, so it is the cheapest place to catch it.

## What changes
- `cargo xtask spec-lint` fails on conflict markers in tracked text files, naming file and line (BUILD-009).
- Fixtures are built at run time in the test; no tracked file contains a marker line.
