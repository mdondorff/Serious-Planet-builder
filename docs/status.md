# Status log

Newest entries at the top. One line per event (change archived, ADR drafted, spike opened or closed, quarantine, milestone report). Stop reports get a short section.

## Waiting for the owner
- **Accept or change the 12 proposed ADRs** (docs/adr/0001..0012). Work proceeds on their leanings. ADR 0003 departs from the report's leaning (tangent-adjusted warp first, equal-area evaluated at the M2 gate).
- **CI**: `gh` is installed but not on the Git Bash PATH; use `C:Program FilesGitHub CLIgh.exe`. Branches are being pushed; CI results are logged below.
- **Candidate goldens for `/bless`** (adapter `dx12-microsoft-basic-render-driver`): `hello_triangle`, `debug_view_face`, `debug_view_tile_id`, `debug_view_height` (see `cargo xtask bless`; image in target/review/candidates/). Not blocking: later work does not depend on it. Set `PLANET_GOLDEN_STRICT=1` in CI once blessed (the xtask now passes that variable through).
- **Performance harness on Rasierklinge**: skeleton only; a real run needs your go (charger, performance power profile, discrete GPU mode), then `cargo xtask perf --machine-ready`.

## Stop report (2026-10-10): CI cannot run yet
Done: M0 and the three M1 changes are archived and pushed as stacked PRs #1 (m0-foundations to main), #2, #3, #4 (each based on the previous branch). Blocked: GitHub reports no workflows, no check suites and no runs for any pushed branch, although Actions is enabled and `.github/workflows/ci.yml` is on the branches (push on all branches and workflow_dispatch added). Likely the first workflow only registers once it exists on `main`, and `main` accepts changes only through a pull request. CLAUDE.md allows self-merge only with green CI, so nothing is merged. Needed from you: either check Settings > Actions on the repo, or tell me to merge PR #1 once to bootstrap the workflow. After that CI runs on the stacked PRs, which also gives the first Linux/lavapipe verification.

## Log
- 2026-10-10 m1-frameplan-skeleton archived (REND-001, 006, TEST-008, 009, 010, BUILD-007, LOD-006 active). M1 acceptance pending CI + owner items.
- 2026-10-10 m1-generator-first-tile archived (GEN-001..006, 009 active; GENERATOR_VERSION 2). Cross-OS tile hash equality (GEN-003) awaits CI.
- 2026-10-10 m1-coordinates archived (COORD-001..010 active; numerics-reviewer and verifier findings fixed; libm pinned =0.2.16).
- 2026-10-10 ADRs 0001-0012 drafted (proposed).
- 2026-10-10 M0 archived (change m0-foundations; report docs/milestones/M0-report.md). Verifier 15/15 active requirements pass; CI not yet run.
- 2026-10-09 Repository seeded from the starter bundle (constitution v1, CLAUDE.md, agents, skills). Next: M0 per docs/background/starter.md §8.
