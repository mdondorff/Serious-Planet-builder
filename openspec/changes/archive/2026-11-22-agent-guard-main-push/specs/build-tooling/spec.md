## MODIFIED Requirements

### Requirement: BUILD-062 Companion agent guardrails and unified ID allocator
`cargo xtask agent-guard` SHALL read a `PreToolUse` JSON payload on stdin and emit a JSON decision (`allow`, `deny`, or `force_ask`) enforcing read-only sibling workspaces, zoned writes under `SeriousPlanetData` (`tmp/heavy/**`, `tmp/ag2/**`, `lanes/ag2-*.md`), protected repository files, zero script files (`BUILD-002`), yielding cargo runs while `quiet` or an exclusive machine lock is active, and blocking direct `git push` to `main`/`master`, bare `git push`, force-pushes, and repository deletion while allowing explicit feature-branch pushes and commit messages. `cargo xtask next-id <PREFIX|ADR|SPIKE>` SHALL scan `openspec/`, `docs/`, and optional lane files to print the next contiguous `max + 1` identifier. `.agents/**` SHALL match `.claude/**` in `DOCS_PATTERNS` across both GitHub workflows and `prbody`.

Verify: A
Status: active
Source: ADR 0128 (proposed), CON-25, BUILD-002, BUILD-026, BUILD-044

#### Scenario: Companion guardrails and sequential ID allocation
- **WHEN** `agent-guard` receives writes targeting a read-only sibling repo, an unapproved `SeriousPlanetData` path, `cargo xtask check` while `quiet` is active, a direct `git push` to `main`/`master`, a bare `git push`, or a force-push, and `next-id` scans specs, lanes, and open PRs
- **THEN** read-only paths, unapproved data paths, direct/bare/force pushes, and `cargo` under `quiet` or `exclusive` are denied, explicit feature-branch pushes, commit messages, and owned paths allow, and `next-id` prints `max + 1`
