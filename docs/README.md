# Where to find what

The documentation strategy (owner, 2026-11-27): a fixed set of live entries at the top of `docs/`, every other document in its type's home, and a finished milestone's documents in `docs/archive/<M>/`.

## Live at the top of `docs/`
| Entry | What it holds | Type |
|---|---|---|
| `constitution.md` | the rules that never bend | rule |
| `routing.md` | who does what, who merges, where a lane reports (read at session start) | rule |
| `domain.md` | what a term means (tile, patch, field, stage...) and the pictures of the model | rule |
| `status.md` | what is happening now, what waits for the owner | state |
| `parking.md` | what is outside the current milestone, one line each | state |
| `learnings.md` | surprises and corrected assumptions still in force | state |
| `spikes.md` | every open or planned experiment, one line each (register) | state |
| `log.md`, `log.d/` | what was done, newest first: `cargo xtask log` | state |
| `roadmap.md` | where the milestones may go after the current one (non-binding) | plan |
| `adr/` | why something was decided (index: `adr/README.md`) | decision |
| `milestones/` | the current milestone's plan, reports and evidence | milestone |
| `design/` | proposed designs and reviews until built or superseded | design |
| `tools/` | files tools own: the gate ledger (`tools/gates/`), the stage manifest (`tools/stages/`), the CI history (`tools/perf/`), the lanes (`tools/lanes.txt`) | tool |
| `archive/<M>/` | a finished milestone's log, spikes, learnings, briefings, designs and perf verdicts | history |

Kept where they are (the owner's guardrails, 2026-11-27):

| Entry | Why |
|---|---|
| `background/` | the original research and set-up documents (frozen, non-normative; index `background/README.md`); the constitution names it |
| `spikes/` | the pictures of SPIKE-03 to 07 that accepted ADRs link to (frozen) |
| `dev_screenshots/` | the owner's progress screenshots (`<date>-<desc>.png`) |
| `milestones/M8/` images | the M8 owner test and look review pictures |

## Elsewhere
| I want | Look in |
|---|---|
| What the project is, how to run it, every key | `README.md` (repository root) |
| The target architecture and its gate table; the gate contract changes | `docs/design/terrain-target.md` (A3; section 14) |
| A proposed later river project (assisted rivers after erosion) and the seams it needs | `docs/design/rivers-after-erosion.md` (proposal, non-normative) |
| Downstream (`M10`–`M13` / game) producer seams for `G3` and `P4` | `docs/design/hydrology-downstream-seams.md` (parked) |
| The driver regions (ADR 0117, proposed) | `docs/design/driver-regions.md` |
| The current rules, numbers and budgets per subsystem | `openspec/specs/<subsystem>/spec.md` |
| Work in progress on a subsystem | `openspec/changes/<id>/` |
| Requirement IDs that must never be issued again | `openspec/retired-ids.txt` |
| A milestone's verdict, exit criteria and process review | `docs/milestones/<M>-report.md` |
| The terrain prototype's plan (P1 to P3: items, phases, the A/B gate) | `docs/archive/P3/briefings/terrain-prototype.md` |
| The perf windows up to M7 | `docs/archive/M5/perf/`, `docs/archive/M6/perf/`, `docs/archive/M7/perf/` (the raw dumps are in git history at a179884d1) |
| An experiment's report, numbers and pictures; the catalogue of every spike | `spikes/<name>/results/`; `spikes/README.md` |
| The offline pipeline DAG, step previews and version pins | `cargo xtask cockpit --help` (brief: `docs/design/pipeline-cockpit.md`) |
| The sample ground look on the baked channels | `crates/frame/tests/data/ground-channels-d1.look.toml` |
| Reference images for golden tests | `tests/goldens/<adapter>/` |
| Agent roles, skills and their memory | `.claude/` and `CLAUDE.md` |
| Evidence of a later milestone too heavy for git (images, dumps) | `C:/DEV/SeriousPlanetData/evidence/<M>/` |
