# Status log

Newest entries at the top. One line per event (change archived, ADR drafted, spike opened or closed, quarantine, milestone report). Stop reports get a short section.

## Waiting for the owner
- **Accept or change the 12 proposed ADRs** (docs/adr/0001..0012). Work proceeds on their leanings. ADR 0003 departs from the report's leaning (tangent-adjusted warp first, equal-area evaluated at the M2 gate).
- **Install GitHub CLI** (`winget install GitHub.cli`, `gh auth login`) and push branches, so CI (Windows WARP, Linux lavapipe) can run. Until then CI has never run and Linux is unverified; changes stay on local branches and nothing is merged to `main`.
- **Candidate goldens for `/bless`**: `dx12-microsoft-basic-render-driver/hello_triangle` (see `cargo xtask bless`; image in target/review/candidates/). Not blocking: later work does not depend on it. Set `PLANET_GOLDEN_STRICT=1` in CI once blessed (the xtask now passes that variable through).
- **Performance harness on Rasierklinge**: skeleton only; a real run needs your go (charger, performance power profile, discrete GPU mode), then `cargo xtask perf --machine-ready`.

## Log
- 2026-10-10 m1-coordinates archived (COORD-001..010 active; numerics-reviewer and verifier findings fixed; libm pinned =0.2.16).
- 2026-10-10 ADRs 0001-0012 drafted (proposed).
- 2026-10-10 M0 archived (change m0-foundations; report docs/milestones/M0-report.md). Verifier 15/15 active requirements pass; CI not yet run.
- 2026-10-09 Repository seeded from the starter bundle (constitution v1, CLAUDE.md, agents, skills). Next: M0 per docs/background/starter.md §8.
