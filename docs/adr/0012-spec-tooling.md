# 0012 Spec tooling
Status: proposed
Date: 2026-10-10
Source: background report §12 (Spec tooling), starter doc §4; constitution CON-16, CON-23, CON-24

## Context
Specs must stay current while decisions arrive late and implementation teaches things. The tool must work on Windows with Claude Code and stay light.

## Options (with trade-offs)
1. GitHub Spec Kit. Strong phase gates, per-feature folders, heavier process, Python/uv, PowerShell scripts by default.
2. **OpenSpec.** Evergreen `openspec/specs/` plus per-change deltas merged on archive; Node-based; light.
3. Plain Markdown. No tooling, no deltas.

## Decision
Option 2 for subsystem specs and changes; constitution, ADRs, spike register, learnings and status log stay plain Markdown in `docs/`. Conventions on top of OpenSpec:
- Requirement headings carry a trace ID: `### Requirement: COORD-003 Geo round-trip`, followed by body lines `Verify: A|B|C|D`, `Status: active|planned|deprecated`, optional `Source:` (constitution rule or ADR) and, for tier D, `Review:` naming the artefact.
- `cargo xtask spec-lint` enforces IDs, tiers, statuses, citations (`// spec: ID` in tests) and coverage of active A/B/C requirements.
- Constitution rules are referenced by ID in `Source:`, never copied.
- Milestones are one or more OpenSpec changes (`m1-coordinates`, ...); specs are created and changed through changes and archived into `openspec/specs/`.

## Consequences
- OpenSpec 1.x CLI (`openspec`) is a prerequisite for spec work, not for building.
- Switching later is cheap: both tools are Markdown plus slash commands.

## Licences of adopted code or techniques
OpenSpec: MIT (tooling only).
