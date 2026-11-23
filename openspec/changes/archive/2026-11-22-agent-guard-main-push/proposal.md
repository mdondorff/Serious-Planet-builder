# Block Direct Push to Main in `agent-guard` (`BUILD-062`) & Record `G3`/`P4` Downstream Hydrology Seams

## Why
1. Unblocking autonomous remote `git` and `gh` operations on feature branches requires a deterministic guardrail in `cargo xtask agent-guard` so direct pushes to `main` (including `HEAD:main`, bare `git push`, `-u origin` without refspec, `:main`, and `+` force refspecs) are refused locally while explicit feature-branch pushes and commit messages referencing `main` remain permitted.
2. Persisting the downstream `M10`–`M13` and game-layer producer invariants in `docs/design/hydrology-downstream-seams.md` ensures the `P3` `G3` production ADRs and `P4` productionization work preserve the cheap data and query seams without dragging downstream consumer APIs prematurely into `P3`.

## What Changes
- Parse shell segments and quoted tokens in `xtask/src/agentguard.rs` so `git push` and `gh repo delete`/`archive` checks inspect actual command invocations rather than commit-message strings, blocking bare, force, `--all`/`--mirror`/`--tags`, and direct `main`/`master`/`HEAD` pushes (`BUILD-062`).
- Document autonomous prefix-clean `git`/`gh` invocation rules (`--body-file`) in `GEMINI.md`.
- Add `docs/design/hydrology-downstream-seams.md` (indexed in `docs/README.md` and `docs/parking.md`) routing downstream invariants into `G3` production ADRs and `P4` productionization.
