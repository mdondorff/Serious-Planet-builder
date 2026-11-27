# Parking the river graphs: the merged plan (lead, 2026-11-26)

Lead (opus-architect), 2026-11-26 23:00. Status: **in execution** (owner, 2026-11-26: "Please go"; no-regret parts only).
Inputs: Fable's plan `fable-rg-park-plan-2026-11-26.md` and Carbon's plan `c:/dev/ag_serious-planet-builder/review/rg-parking-plan-2026-11-26.md`. Both reach the same direction: park RG, keep what helps B, write the record, and close P3. Where they differ, §5 lists the difference for the owner.

## 1. Rules for the park
- No RG research, no new switch rows, no RG stage on main.
- On main only:
  - bit-identical code that helps B's bake (CON-14);
  - docs;
  - tooling that is candidate-neutral.
- Nothing irreversible without the owner: no data deletion, no worktree pruning, no branch deletion (§4).
- At most 3 PRs plus the P3 closure. Each PR goes through verifier and CI, as usual.

## 2. Steps
| # | Step | Who | Form | Size | Blocks on the owner? |
|---|---|---|---|---|---|
| 1 | Lane freeze; every lane tip pushed to `spike/*` | lanes | done 22:20-22:45 | - | no |
| 2 | Annotated archive tags: `archive/rg-g2-record-2026-11-22` at faa923f04 (the G2 record); `archive/rg-parked-2026-11-26` at the spike tip after the last pushes; `archive/rg-k5p-sills-2026-11-17` at `spike/k5p-sills` | lead | tags on origin | 0.1 | no |
| 3 | **PR A (code):** wave 1's memory levers on B's bake: K5's in-flight catchment cap, K6 levers (a)-(h), base1's parallel decide with the 1, 2 and N thread test, and the `landmass_scale wave1` mode with the counting allocator. Bit-identical; GEN-192, TEST-157. Time box 2 lane-days: whatever isn't green and bit-identical by then stays on the tag | crusader | code PR | 1.5-2 | no |
| 4 | **PR B (tooling, the owner's persist item):** stage-by-stage sheets with diffs between consecutive stages, as `cargo xtask look-sheet --steps` (BUILD-071, TEST-158), from spikes' `step_row.py` logic; candidate-neutral, so it runs on B's stages and on any chain run's crops | spikes | code PR | 1-1.5 | no |
| 5 | **PR C (docs, the record):** see §3 | lead | docs PR | 0.5 | ADR 0129 merges as **proposed**; the owner accepts it |
| 6 | Flight plan and screenshot playbook for the owner's flight | lead | digest doc | 0.2 | the flight is the owner's |
| 7 | **P3 closure:** `accept` and `housekeeping`, including a cleanup and a proposal to simplify and delete working files | lead, then a coder pass | docs PR(s) | 1 | the deletion proposal is the owner's |
| 8 | Lanes idle after PRs A and B, with heartbeats only, until the owner names the next topic | lanes | - | - | the next topic is the owner's |

## 3. PR C content (the record)
1. **ADR 0129 "River graphs parked"** (proposed, one page):
   - Context: P3 E6, G2 (2026-11-22, five plan criteria unmet), G3 (2026-11-23, nine rows, none passed in full by 2026-11-26), and two independent reviews.
   - Decision:
     1. RG stops, frozen at the tag.
     2. B stays the production chain, with its defects named as B's.
     3. The component record is archived.
     4. Restart conditions are listed.
     5. What lands on main.
     6. The dispositions of the proposed ADRs.
2. **Restart conditions** (Fable's four triggers combined with Carbon's five-point charter):
   - the base is signed off with rivers off (S0, K5's hillslope and planform, K7's read, and the world parameters that feed hydrology: precipitation, runoff, sea level, relief scale);
   - the editor's edit model is decided by an ADR amending 0112/0113, bounded under BUDG-003/005;
   - one lane and about an hour a day of owner time;
   - a pre-set H1-on-sills test passes on the new K5 before any L stage is written;
   - gates are calibrated (TEST-155) and set before any row runs, with the owner's flight at every look gate;
   - rivers and lakes are gated on discharge, not on a dry-ground D8 raster.
3. **Component record** (Fable §2.2), in `docs/archive/P3/river-graph/`, with the G0 contracts, the G2 report, the G3 gate and its risk register, the snapshot, both feasibility reviews, the attribution notes, the cross-seam breach note, the R23 design note, and both park plans.
4. **Dispositions:**
   - PR #1499 (ADR 0123) is closed as superseded;
   - ADR 0124 stays proposed, as B's path;
   - ADRs 0125-0127 are marked parked in `docs/adr/README.md`;
   - E4 and E5 close with them;
   - E6 closes as "parked with record".
5. **`docs/parking.md`:** one line each for RG (the tag, ADR 0129), K5's washboard (O5), K7's C2 read (O12), the calibrated look bars (O13, O14), the world-parameter lift for S0 and hydrology (ADR 0108, GEN-184), and the editor edit model.
6. **`docs/routing.md`:** rules 11-13 from the lead rules of 2026-11-25:
   - at most 2 unreviewed switch-row generations per look item;
   - no geometric bar without a passing TEST-155 calibration;
   - lanes do infrastructure work only when 3 or more owner decisions block a track.
7. **Learnings:** Fable's eight, condensed to fit the 20 KB cap after the 2026-11-14 block moves to `docs/archive/P3/learnings.md`. Neither the 2026-11-26 entry nor rule 10 is repeated.
8. **SPIKE-143:** a closing line, plus the attribution scripts in its results folder (the method rule 10 needs).
9. **Status and log:** `docs/status.md` updated; a log fragment `docs/log.d/2026-11-26-rg-parked.md`.

## 4. Held for the owner
| Item | Both plans propose | Why held |
|---|---|---|
| Delete about 100 GB of field dumps in `C:/DEV/SeriousPlanetData/tmp` (g1f6, g1f15, g203h/j, g196, g1f14*, g19x, g20x, rd, ks-g1b2) | yes | it can't be undone; keep every `.md`, `.txt`, `.csv` and `.png` and the six fly1126 stores either way |
| Prune six lane worktrees (`spikes-gate`, `start-879e4b`, `sonnet-spikes-36fc88`, `crusader-spike`, `spikes-161c`, `spikes-bake`) | yes | after the tags; it removes local state |
| ADR 0129's acceptance | - | a decision record of the owner's |
| The base work (D) | Carbon: park indefinitely; Fable: don't start now, restart by trigger 1 | the next topic is the owner's |
| The flight | Carbon: optional; Fable: keep it | the owner asked for a flight plan; the playbook is ready (step 6) |

## 5. Where the two plans differ
| Topic | Fable | Carbon | Merged |
|---|---|---|---|
| Tag names | `rg/...` | `archive/rg-...` | `archive/rg-...` (one namespace for archives) |
| Base work after the park | not now; restart form F2 needs it first | parked in `docs/parking.md` with everything else | parked now; the owner decides when, with the restart conditions |
| Flight | kept (the only look evidence of RG against B) | optional | kept as an owner option with a playbook |
| Unpark rules | 4 triggers, 4 restart forms (graph-only overlay first) | 5-point charter (editor model, S0 and world parameters first, discharge-gated water, 3-stage CubePatch architecture, single lane) | combined into ADR 0129's restart conditions; the restart forms go in the archive |
| Extra main items | attribution method folder, routing rules 11-13, eight learnings, the cross-seam join as a doc | none beyond the wave-1 PR | taken: docs only, small |
| The owner's persist item (sheets) | not covered | not covered | added as PR B |
