## ADDED Requirements

### Requirement: BUILD-062 Companion agent guardrails and unified ID allocator
`cargo xtask agent-guard` SHALL read a `PreToolUse` JSON payload on stdin and emit a JSON decision (`allow`, `deny`, or `force_ask`) enforcing read-only sibling workspaces, zoned writes under `SeriousPlanetData` (`tmp/heavy/**`, `tmp/ag2/**`, `lanes/ag2-*.md`), protected repository files, zero script files (`BUILD-002`), and yielding cargo runs while `quiet` or an exclusive machine lock is active. `cargo xtask next-id <PREFIX|ADR|SPIKE>` SHALL scan `openspec/`, `docs/`, and optional lane files to print the next contiguous `max + 1` identifier. `.agents/**` SHALL match `.claude/**` in `DOCS_PATTERNS` across both GitHub workflows and `prbody`.

Verify: A
Status: active
Source: ADR 0128 (proposed), CON-25, BUILD-002, BUILD-026, BUILD-044

#### Scenario: Companion guardrails and sequential ID allocation
- **WHEN** `agent-guard` receives writes targeting a read-only sibling repo, an unapproved `SeriousPlanetData` path, or `cargo xtask check` while `quiet` is active, and `next-id` scans specs, lanes, and open PRs
- **THEN** read-only paths, unapproved data paths, and `cargo` under `quiet` or `exclusive` are denied, owned paths allow, and `next-id` prints `max + 1`
