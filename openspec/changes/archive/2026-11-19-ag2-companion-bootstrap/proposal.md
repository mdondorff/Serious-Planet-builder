## Why
- Enable `ag2-antigravity` to develop isolated UI/editor features, spec cleanups, and ahead-of-milestone groundwork directly from its own clone (`c:/dev/ag2_serious-planet-builder`) alongside Claude Code without conflicting on locks, worktrees, or requirement IDs.
- Keep `CON-25` intact by implementing hook guardrails in pure Rust (`xtask`) rather than PowerShell or shell scripts.

## What changes
- `BUILD-062`: `cargo xtask agent-guard` (pure-Rust `PreToolUse` hook guardrail) and `cargo xtask next-id` (contiguous `max + 1` ID allocator across specs, ADRs, spikes, and `lanes/`).
- `.agents/**` parity in `DOCS_PATTERNS` across `.github/workflows/{ci,docs}.yml`, `xtask/src/prbody.rs`, `xtask/src/repocheck.rs`, and `xtask/src/testcmd.rs`.
- In-repo companion files: `GEMINI.md`, `.agents/hooks.json`, `.agents/skills.json`, proposed `docs/adr/0128-multi-harness-companion-lane.md`, and companion row in `docs/routing.md`.

## Impact
`xtask` (`agentguard`, `nextid`, `heavy`, `main`, `prbody`, `repocheck`, `testcmd`), `.github/workflows/`, `docs/adr/`, `docs/routing.md`, `.agents/`, `GEMINI.md`. No engine or runtime crate changes.
