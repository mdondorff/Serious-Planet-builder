## MODIFIED Requirements
### Requirement: BUILD-005 CI on both operating systems
Continuous integration SHALL run check, spec-lint, licence check, tiers A to C and the run-mode smoke starts on Windows (software adapter WARP) and on Linux (software Vulkan lavapipe) for every pull request and every push to `main`. A change that touches only `docs/`, `openspec/`, `.claude/`, root `*.md` files, `spikes/**/results/**` or `spikes/README.md` (owner, 2026-10-16; the spike catalogue, 2026-11-04) SHALL run the light `docs` workflow (spec-lint on Linux) instead; both workflows SHALL name the same list of those paths, and any other file in a change runs the full workflow.

Verify: A
Status: active
Source: CON-26, report section 9

#### Scenario: Workflow content
- **WHEN** the workflow file is parsed by the repository check test
- **THEN** it names both a Windows and a Linux runner and invokes `cargo xtask check`, `spec-lint`, `test` and `test gpu`

#### Scenario: Docs-only paths agree
- **WHEN** the repository check test reads both workflows
- **THEN** the paths the full workflow ignores equal the paths the docs workflow runs on, and include `spikes/**/results/**` and `spikes/README.md`
