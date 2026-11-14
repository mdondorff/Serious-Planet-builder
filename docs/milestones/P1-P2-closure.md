# P1 and P2 closure report (P3 E5, read only)

Status: report (explorer, 2026-11-14; lead's order #143 item 2). Nothing here is removed, edited or decided; each row proposes.
Sources: `cargo xtask worktrees --no-size` (2026-11-14 11:00; a snapshot), `docs/learnings.md`, `docs/status.md`, `docs/briefings/terrain-prototype.md` (closure checklist), spike results, lane outboxes and digests of 2026-11-10 to 2026-11-14.

## 1. Worktree inventory (38 worktrees; `--prune` would remove 19)
| Worktree (under `.claude/worktrees/`) | Branch | Merged | Last commit | Uncommitted | Proposal |
|---|---|---|---|---|---|
| (root) `C:/DEV/Serious-Planet-Builder` | main | in origin/main | 2026-11-07 | 1 | keep (root) |
| ab-main | detached acf57400 | in origin/main | 2026-10-31 | 12 | owner of the 12 files checks them, then remove |
| agent-af86013e3572ee49d | docs/no-decision-refs | PR 1370 merged | 2026-11-11 | 0 | remove |
| archive-p0 | change/catchment-key-rules | PR 1355 merged | 2026-11-11 | 0 | remove |
| archive-sweep | change/m8-envelope-chunks-code | PR 748 merged | 2026-10-26 | 25 | owner checks the 25 files, then remove |
| bisect | detached 59a7deaa | in origin/main | 2026-11-10 | 0 | remove |
| bulldozer-role-fc3a48 | detached e581e4c1 | no merged PR | 2026-11-12 | 0 | ask bulldozer (BUILD-051 is on main: likely remove) |
| bulldozer-status-b1 | detached df8c3cf7 | in origin/main | 2026-11-12 | 0 | remove |
| burn | change/burn-width | PR 1362 merged | 2026-11-11 | 0 | remove |
| coast-tilt-id | change/river-gate-names (#1505 merged) | open (#1506, explorer) | 2026-11-14 | 0 | keep until #1506 merges |
| crusader-main | detached 845c901b | in origin/main | 2026-11-14 | 0 | keep (crusader's base) |
| exitbase | change/exit-base-valley | PR 1379 merged | 2026-11-11 | 0 | remove |
| f32 | change/round-f32 | PR 1394 merged | 2026-11-12 | 0 | remove |
| flats | change/flats-routing | PR 1385 merged | 2026-11-12 | 0 | remove |
| k7beds | change/k7-beds | PR 1405 merged | 2026-11-12 | 0 | remove |
| mono | change/burn-monotone | PR 1376 merged | 2026-11-11 | 0 | remove |
| opus-architect-b6fadb | docs/adr-0123-river-architecture | open (#1499) | 2026-11-14 | 0 | keep |
| opus-planner | docs/routing-architect-name | PR 1264 merged | 2026-11-09 | 0 | remove |
| pboff | change/pre-breach-off | PR 1407 merged | 2026-11-12 | 0 | remove |
| pd | change/pass1-deposition | no merged PR | 2026-11-07 | 0 | ask its lane (probe branch, M9 era): likely remove |
| prebreach | change/pre-breach | PR 1399 merged | 2026-11-12 | 0 | remove |
| pt | detached ee8347b7 | in origin/main | 2026-11-10 | 0 | remove |
| qunit | change/q-unit | PR 1345 merged | 2026-11-11 | 0 | remove |
| sonnet-crusader-arch | change/archive-crusader-1 | PR 1330 merged | 2026-11-10 | 0 | remove |
| sonnet-crusader-c1a4f2 | spike/k7-flat | no merged PR | 2026-11-14 | 2 | keep (crusader, SPIKE-117; holds the r3a stores) |
| sonnet-spikes-36fc88 | spike/schott-fair | no merged PR | 2026-11-14 | 2 | keep (spikes, SPIKE-116/118) |
| spike-m9-g1 | spike/m9-g1-octave-cost | in origin/main | 2026-10-28 | 2 | check the 2 files, then remove |
| start-879e4b | exp/stair-remedies | no merged PR | 2026-11-03 | 11 | explorer's old session: check the 11 files (diagnosis branch), then remove with its nested worktrees |
| start-879e4b/.claude/worktrees/band | change/band-slope-cap | PR 1430 merged | 2026-11-12 | 0 | remove (nested: against the worktree rule) |
| start-879e4b/.claude/worktrees/base4 | detached c4ffeded | in origin/main | 2026-11-12 | 1 | check, then remove (nested) |
| start-879e4b/.claude/worktrees/f32s | change/round-f32-storage | PR 1424 merged | 2026-11-12 | 1 | check, then remove (nested) |
| start-879e4b/.claude/worktrees/sweep | detached df8c3cf7 | in origin/main | 2026-11-12 | 3 | check, then remove (nested) |
| start-879e4b/.claude/worktrees/tilt | change/coast-tilt | PR 1435 merged | 2026-11-12 | 0 | remove (nested) |
| start-879e4b/target/wt-depprobe | exp/deposit-probe | in origin/main | 2026-11-05 | 3 | check, then remove (inside target/: against the worktree rule) |
| start-879e4b/target/wt-iso | exp/isostasy-probe | in origin/main | 2026-11-05 | 2 | check, then remove (inside target/) |
| start-879e4b/target/wt-pitprobe | exp/keep-pits-probe | no merged PR | 2026-11-05 | 1 | check, then remove (inside target/) |
| start-879e4b/target/wt-proposals | change/archetype-driver-proposals | no merged PR | 2026-11-05 | 0 | remove (proposals dropped, owner D2, ADR 0102) |
| worktrees/bulldozer-status-b1 | status/start-surface-rows | in origin/main | 2026-11-12 | 0 | remove (path `.claude/worktrees/worktrees/`: a nesting slip) |

- Seven worktrees sit inside another worktree or inside `target/` (CLAUDE.md: worktrees only under `.claude/worktrees/`): five nested in start-879e4b, four in its `target/`, one under `.claude/worktrees/worktrees/`.
- Remove with `cargo xtask worktrees --prune` (clean and merged ones only, never `--force`); the rows with uncommitted files need their lane's look first.

## 2. Learnings of P1 and P2 not in `docs/learnings.md`
`docs/learnings.md` holds four entries dated P1/P2 (carve amplification, the f32 flat grade, the K7 halo, the push gate). Not recorded:

| # | Finding | Source | Scope |
|---|---|---|---|
| 1 | The L basins come from the 814 m to L hand-over, not from B: the 814 m surface drains at its grid (5.9 % of river length on closed basins), read at L 74.5 % (a D8 diagonal between two higher cells is a saddle in any bilinear or C1 read); five B-stage variants leave 61-70 % against 71 %. P3.md's "the base's lattice relief" names the wrong stage | spikes/drain-base/results/results.md; #1498 | hand-over 814 m to L |
| 2 | The basin metric decides the verdict: "any pit over 5 cm" fails SPIKE-115's flat windows (29.1 %, 13.7 %); basins of 0.5 m and 16 cells (fill 1e-7) give 0-4.6 % on the same kind of surface | spikes/combo-117/results/results.md; erosion-at-l results | river gate (BUILD-055) |
| 3 | K7 beds are not monotone: the burn only lowers, so line heights rise downstream (r3a-e1: 63,167 of 219,390 line cells rise over 0.25 m, max 44.7 m); deep cuts (10-13 % over 50 m) come from H1 lines crossing L ridges | gprofile-read-2026-11-11.md; crusader-outbox | K7 burn (GEN-169, GEN-176, GEN-185) |
| 4 | The stored channel level bounds the bed from above, it is no water surface: the drawn ground is below it at 83 % of channel points (median -3.8 m); on r3a-p2 the line is a raised one-cell line beside the carved low | spikes-outbox; SPIKE-113 | K7 levels, river drawing |
| 5 | 80 % of E1's drawn "lakes" were coastal-band sea on inland ground K7 put below 0 m (4,764 cells, 3.5-42 m, 1-23 km inland); the 4,317 km2 margin lake is a window-edge outlet never lowered | lanes/digest/2026-11-13.md; SPIKE-112 | K7 sea floor, window outlets, coastal band |
| 6 | Window names collide: SPIKE-114/115 call face 1 (8056-8675 x 760-1379, seed 14) "R3a", the spikes lane calls it R1h and uses "R3a" for face 4 seed 15 | drain-base and erosion-at-l results | spike and gate reports; settled: canonical names R1h, R3a, R3b (lead, 2026-11-14, #1506) |
| 7 | The 4-neighbour hand-over aligns rivers to the grid: 66.5-68.6 % of channel steps along an axis, against 47 % | combo-117 results | hand-over (ADR 0123 draft) |
| 8 | The river network cannot be rebuilt from B: 8.0 % of channel cells agree with routing on B + regional; store the L geometry (about 0.9 MB a region) | rivers-at-l results | store format |
| 9 | One regional-kernel round at L is the optimum: three rounds raise relief to 156/143 %, ten to 521/358 %; Schott 2024 multi-scale with unchanged parameters over-erodes (relief about 3x); K7's canonical flood refuses flats of an eroded surface | erosion-at-l results | regional kernel at L, K7 flood |
| 10 | Rounds and routing at L are not tile-bounded: largest catchment 5.07 M cells against 0.59 M a tile | erosion-at-l, rivers-at-l results | partitioning (CON-30) |
| 11 | The layer read's clamp to the four surrounding cells made steps at L cell-centre lines (max 12.5 m); the bake's twin reads clamp the same way; SPIKE-114's "41 % step" count measured curvature too | spikes-outbox; digest/2026-11-14-todo-lead.md | layer read (GEN-188) |
| 12 | The drawn ground was 1-7 m off the stored B + L, all of it the crease detail term (seen as water flowing uphill); pinning it cost 31 % of frames before the memo | ADR 0122; log.d/2026-11-13-commit-base-exact.md | runtime detail (GEN-187, VIEW-072) |
| 13 | The flight stalls were viewer residency growing to 66,114 tiles with no eviction, not machine contention; with the cap a 26-min flight had 0 stalls | digest/2026-11-13-preview-round.md; digest/2026-11-14.md | viewer residency (VIEW-070) |
| 14 | No solver path holds an inland lake: the breach cuts every basin; a 400 m dome keeps about 188 m after the rounds | SPIKE-112 | edits and lakes (ADR 0113) |
| 15 | Memoised rounds cost 2x the resident ones: the serial area sweep (about 80 %) is recomputed whole; a per-catchment parallel sweep takes 69 s against 159 s, bit-equal | spikes-outbox | regional solver, edit path |
| 16 | Later-stage start-surface fixes failed: the coast tilt made R3a (face 4, seed 15) worse at every setting (12.0-15.6 % against 9.9 %); the band's crest rings dam a basin each | bulldozer-outbox; SPIKE-111 | start surface |
| 17 | A probe that copies library code goes stale: G-LOCAL's L1 mismatch (437,249 cells) was the probe missing GEN-173's graded flood | glocal-f32-2026-11-12.md | gates and probes |
| 18 | Under both K7 tilings the burn and breach are bit-identical; the seam difference opens only in the carve (at most 0.012 m), deposition adds the 0.087 m maximum | k7-seam-2026-11-11.md (partly in the K7 halo entry) | K7 (ADR 0119) |
| 19 | `gpu_skeleton` timeouts come only from the two pipelined loops under CPU oversubscription on WARP (2.8 s alone, 28-69 s loaded) | spikes/gpu-skeleton/results/results.md | tier C, prepush |
| 20 | The CPU latch recurred on 2026-11-13 (about 15 %), cleared by an EC reset, and voided SPIKE-113's timings (the 2026-10-29 entry has the first case only) | spikes-outbox; crusader-outbox | Rasierklinge perf |
| 21 | After 10 sends without an owner message the app pauses a lane's messages, and an order file does not wake an idle lane (explorer idle about 5 h on 2026-11-14) | digest/2026-11-14.md; ADR 0118 | lane process |
| 22 | A default-on stage switched after one measurement made its defect worse (the round-0 breach needed a second PR to switch off) | ADR 0118, ADR 0120 | generators, process |
| 23 | The width law never reaches a visible size on the prototype region (widest trench 1.9 cells); the drawn width's one-cell floor makes tributaries as wide as the main stem | gsurfi-reread-2026-11-11.md; preview-round F6 | K7 width, river drawing |

Proposal: the owning lanes record rows 1-17 and 19-23 with `/learning` (one entry each, with its scope); row 18 extends the K7 halo entry.

## 3. `docs/status.md` lines that name finished work
| Line | What is finished | Proposal |
|---|---|---|
| Waiting for the owner: "ADR 0110 acceptance ... ADRs 0110-0115: architect's text pass in P0, then owner acceptance" | ADRs 0110-0115 accepted for the prototype (owner, 2026-11-11, `docs/adr/README.md`) | drop the acceptance part; keep only what still gates item 15 PR 5b, if anything |
| Queue E1: "SPIKE-116: a base whose 100 m relief drains by construction" | SPIKE-116 closed 2026-11-14 (no B-stage variant drains, #1498) | replace by the E1 step now open (ADR 0123, SPIKE-117 closed #1501, SPIKE-118) |
| Queue E1: "SPIKE-115: erosion rounds at L ..." | results merged (#1495); open only until the owner's pick | name the pick as the open step |
| Queue E3: "GEN-187 (pinned detail, with the memo) merged" | merged (#1490) | keep the owner's two flights only |
| Waiting for the owner: "Smooth base as default (GEN-107) ... draft #1137" | not finished (#1137 open), but the M9 erosion preview it waits on is replaced by the P3 chain | owner: drop or park |
| Tracked: "Local `test gpu` worse ... `gpu_skeleton` timeouts (diagnosed #1442 ...)" | the diagnosis is done | keep the symptom, drop "diagnosed" detail into learnings row 19 |

## 4. Closure checklist (briefing) items that look unmet
- Specs synced and changes archived: 43 change folders unarchived; P3 E5 in progress.
- Briefing current: header "Re-estimate: 2026-11-11 00:30"; items 15 and 16 "in progress" with the old CP1 order; P2's phase exits have no state.
- Independent review pass at CP2: not done (P3 E5).
- G-RT: `docs/gates/ledger.md` "no measurement on main | not run"; waits on the owner's flights (P3 E3).
- Thermal-dam options brief: not written (P3 E4).
- Worktrees of merged branches pruned: section 1.
- Learnings recorded: section 2.
- One-page close and sign-off: none for P1 or P2; P2.md "Sign-off: (none yet)"; ADR 0117 proposed; ADR 0123 a draft (#1499).
