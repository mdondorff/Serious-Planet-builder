# Where to find what

| I want | Look in |
|---|---|
| What the project is, how to run it, every key | `README.md` (repository root) |
| The rules that never bend | `docs/constitution.md` |
| What a term means (tile, patch, field, stage...) and the pictures of the model | `docs/domain.md` |
| What is happening now, what waits for the owner | `docs/status.md` |
| The current milestone's plan and this week's start brief | `docs/briefings/` |
| Planned against actual cost of briefed items | `docs/briefings/estimates.md` |
| Where the milestones may go after the current one (non-binding) | `docs/roadmap.md` |
| What was done, newest first | `cargo xtask log` (current milestone: `docs/log.md` and `docs/log.d/`) |
| Why something was decided | `docs/adr/` (index: `docs/adr/README.md`) |
| The current rules, numbers and budgets per subsystem | `openspec/specs/<subsystem>/spec.md` |
| Work in progress on a subsystem | `openspec/changes/<id>/` |
| A milestone's verdict, exit criteria and process review | `docs/milestones/<M>-report.md` |
| A finished milestone's log, learnings, spikes, status snapshot and briefings | `docs/archive/<M>/` |
| Every experiment, one line each | `docs/spikes.md` (register) |
| An experiment's report, numbers and pictures | `spikes/<name>/results/` |
| Pictures of SPIKE-03 to 07 that accepted ADRs link to | `docs/spikes/` (frozen) |
| Performance windows and CI history | `docs/perf/` (`window-<N>.md`, raw data in `window-<N>/`) |
| Surprises and corrected assumptions still in force | `docs/learnings.md` |
| Progress screenshots | `docs/dev_screenshots/` |
| The original research and set-up documents | `docs/background/` (frozen, non-normative) |
| Reference images for golden tests | `tests/goldens/<adapter>/` |
| Agent roles, skills and their memory | `.claude/` and `CLAUDE.md` |

## Folders by kind
| Kind | Folders | Changes |
|---|---|---|
| Rules | `docs/constitution.md`, `openspec/specs/`, `docs/adr/` | owner, ADRs, archived changes |
| Working files (small, current) | `docs/status.md`, `docs/log.md`, `docs/log.d/`, `docs/learnings.md`, `docs/spikes.md`, `docs/briefings/` | every session; size caps (ADR 0075) |
| Evidence | `docs/milestones/`, `docs/perf/`, `spikes/`, `tests/goldens/` | written once per report or run |
| History | `docs/archive/`, `docs/background/`, `docs/spikes/` | frozen |
