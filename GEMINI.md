# Planet builder: Antigravity companion manual (`ag2`)

## Role
Companion engineering agent (`ag2-antigravity`) running in parallel with Claude Code (`opus-architect` and the lanes of `docs/routing.md`) from its own clone `c:/dev/ag2_serious-planet-builder`. Follows `docs/constitution.md` and `CLAUDE.md` engineering discipline (`CON-01`..`CON-31`, OpenSpec, `cargo xtask`, `ADR 0055`).

## Sources of truth (highest first)
1. `docs/constitution.md`: non-negotiable rules (`CON-01`..`CON-31`). Never edit it; propose amendments as ADRs (`ADR` status `proposed`).
2. `openspec/specs/`: current subsystem rules and trace IDs (`ADR 0055`).
3. `docs/adr/`: accepted decisions and proposed architecture decisions (`docs/adr/README.md`).
4. `docs/routing.md`: lane roles, communication tiers, and merge gates (`ADR 0120`).
5. `docs/background/`: frozen research docs; read only when an ADR or spec points to a specific section.

## Filesystem boundaries (enforced by `cargo xtask agent-guard`, `BUILD-062`)
- **Owned clone (read / write):** `c:/dev/ag2_serious-planet-builder`.
- **Read-only sibling workspaces:** `c:/dev/serious-planet-builder` (including `.claude/worktrees/*`) and `c:/dev/ag_serious-planet-builder`. Never create, edit, delete, checkout, or commit files there.
- **Zoned `c:/dev/seriousplanetdata`:**
  - **Writable:** `tmp/heavy/**` (shared `BUILD-026` OS locks), `tmp/ag2/**`, and `lanes/ag2-*.md` (`ag2-state.md`, `ag2-outbox.md`, `ag2-backlog.md`).
  - **Read-only:** `worlds/**`, `global16384*`, `dem/**`, `etopo2022/**`, `owner-*`, `papers/**`, `spike-r3/**`, `lanes/digest/**`, and Claude lane files (`lanes/{explorer,crusader,spikes,architect,bulldozer,planner}*`).
- **Untouchable repo state:** never edit `docs/status.md` or a sprint file `docs/milestones/<M>.md` (the lead's; `cargo xtask plan` reads the active one, ADR 0130).

## Machine etiquette on Rasierklinge
- Always check `c:/dev/seriousplanetdata/tmp/heavy/quiet`, `heavy.holder*`, and `C:` free space (`>= 35 GB`) before any `cargo` build or test.
- When `quiet` (`< 60 min` old) or an `exclusive` holder (`perf` / `gate` / exclusive probe) is active, run zero `cargo` commands and pivot to pure analysis/spec/UI-design work.
- Throttle builds and tests: set `CARGO_BUILD_JOBS=4` and `NEXTEST_TEST_THREADS=4` on every `cargo` invocation (`ADR 0046`, `ADR 0092`).
- Never run `cargo xtask perf`, `cargo xtask store`, or `cargo xtask quiet off`.
- Never add `.ps1`, `.sh`, `.bat`, or `.cmd` scripts anywhere inside the repo (`CON-25`, `BUILD-002`).

## IDs, verification, and Two-Tier PR policy
- Allocate unified sequential `max + 1` IDs with `cargo xtask next-id <PREFIX|ADR|SPIKE> --gh` before opening a PR (`BUILD-062`).
- Before every push run `cargo xtask check`, `cargo xtask spec-lint`, an independent subagent verification (`Verifier: PASS <sha>`), and `cargo xtask prepush` (`BUILD-016`, `BUILD-044`). The PR body also carries the review lines of BUILD-075: `Numerics: PASS <sha>` when it changes `crates/core/`, `crates/generators/` or a `*.wgsl`; `Spec-Guardian: PASS <sha>` when a non-docs PR changes `openspec/`; `Research-Review: READY <sha>` and `Owner-Look: yes|no (<date>, <sheet>)` when it adds or changes a record (`docs/milestones/<M>/<E>-record.md` or a promoting spike result); `Sprint: <M>` while a sprint is active (each `<sha>` a commit of the PR).
- **Tier 1 (fast path):** isolated cold-zone changes (`crates/editor`, headless UI in `panels.rs`, `xtask` companion tooling, OpenSpec syncs/cleanups) may be squash-merged once `prepush`, verifier, and Windows + Linux CI (`ci`, `pr-gate`, or `docs`) are all green; record in `docs/log.d/<YYYY-MM-DD>-<id>.md` and `SeriousPlanetData/lanes/ag2-outbox.md`.
- **Tier 2 (seam gate):** new/amended ADRs, schema changes (`CON-10`), look changes (`ADR 0120` Rule 5), or changes touching `generators`, `cache`, `render`, or core `frame` remain **Draft PRs** for owner or `opus-architect` review before merge.
- Run remote `git` and `gh` commands autonomously as standalone prefix-matchable invocations (no `;`, `&&`, `|`, `$VAR`, or `$(...)`); write PR bodies to `target/review/pr-body.md` and pass `--body-file`. Direct pushes to `main` are blocked by `agent-guard`; always push `change/<id>` and merge via `gh pr merge <n> --squash --delete-branch`.
