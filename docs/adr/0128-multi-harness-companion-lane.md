# 0128 Multi-harness companion lane (`ag2`) alongside Claude Code
Status: accepted (owner, 2026-11-27); proposed by ag2-antigravity, 2026-11-19
Date: 2026-11-19
Source: `docs/constitution.md` (`c0d6f479`, owner 2026-11-19), `ADR 0005`, `ADR 0120`, `ADR 0121`, `BUILD-062`

## Context
- `docs/constitution.md` (`c0d6f479`) permits any agent to propose amendments and operate under the repository's rules.
- `Claude` lanes run intensive P3 hydrology research in `c:/dev/serious-planet-builder`, leaving cold areas (`crates/editor`, UI ergonomics, OpenSpec backlog, ahead-of-milestone prep) paused.
- Sharing one physical machine (`Rasierklinge`) requires serialized heavy-lock access (`BUILD-026`) and strict path/CPU boundaries without adding scripts (`CON-25`).

## Options
1. **External wrapper config outside git:** unversioned hooks/rules, duplicated skill paths, harder CI integration.
2. **In-repo `.agents/` + `GEMINI.md` backed by `xtask` Rust subcommands:** single source of truth, `CON-25` compliant, docs-only CI parity, isolated clone for execution.

## Decision
1. **In-repo companion config:** `GEMINI.md` and `.agents/` live in the repository alongside `CLAUDE.md` and `.claude/`, and share `.claude/skills` via `.agents/skills.json`.
2. **Pure-Rust guardrails (`CON-25`):** `cargo xtask agent-guard` enforces read-only sibling workspaces, zoned `SeriousPlanetData` writes (`tmp/heavy/**`, `tmp/ag2/**`, `lanes/ag2-*.md`), and quiet/exclusive yield checks without adding `.ps1`/`.sh` files.
3. **Unified sequential IDs (`BUILD-062`):** `cargo xtask next-id` allocates contiguous `max + 1` IDs across `openspec/`, `docs/`, open PRs, and `lanes/`; `merged_spec_lint` (`BUILD-022`) guards against races before merge.
4. **Two-tier PR gate (`ADR 0120` alignment):** isolated cold-zone changes merge on green CI + `Verifier: PASS <sha>` (`Tier 1`); shared seams, ADRs, schemas (`CON-10`), look changes, or hot-crate edits open as Draft PRs for owner or `opus-architect` review (`Tier 2`).
5. **Lane separation:** `ag2` works in its own clone (`c:/dev/ag2_serious-planet-builder`), throttles builds/tests (`CARGO_BUILD_JOBS=4`, `NEXTEST_TEST_THREADS=4`), and never edits `docs/status.md` or `docs/milestones/P3.md`.

## Consequences
- Companion work advances UI, tooling, and spec debt in parallel without stalling `opus-architect` or colliding on worktrees, locks, or requirement IDs.
- `.agents/**` joins `DOCS_PATTERNS` so companion config changes run only the light `docs` workflow (`spec-lint`).
- Requires owner review and acceptance of this proposed ADR before `Tier 2` merge.

## Licences of adopted code or techniques
Pure Rust standard library only (`xtask`).
