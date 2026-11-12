# Audit batch C: the open OpenSpec changes against main (read only)

Run 2026-11-12 by the spikes lane on the bulldozer's order (owner via architect; due 2026-11-13). Main e129414d (GEN-176 merged). Read only: nothing archived, synced, moved or deleted. The architect syncs the specs from this table; nobody archives before the architect says so.

## 1. What was counted and how

- Open changes: every folder under `openspec/changes/` except `archive/`: **73** (the order said 72). `design.md` files: **42**; `tasks.md` files: **15**; together the **57** files of the order (45 changes hold at least one of them).
- Requirement IDs of a change: the `### Requirement:` headings of its `specs/*/spec.md` deltas (126 IDs in 51 changes; 22 changes have no spec delta).
- **In main's specs:** the same ID as a `### Requirement:` heading under `openspec/specs/**/spec.md`. A delta that is not yet there is "not synced".
- **Tests cite the ID:** a `spec: ID` comment in a `.rs` file under `crates/`, `xtask/` or `tests/`. This shows the test exists, not that it passes or is not `#[ignore]`d: 21 test files hold `#[ignore]`d tests; the only ID whose citing files are all among them is REND-046 (`crates/frame/tests/clipmap.rs`; not read whether its own test is the ignored one). Tier C and D requirements (measurement, goldens, owner looks) have no citing test by their nature.
- **Merged in:** the PR number in the subject of the first commit on main whose message names the ID (or the change); where no commit names the ID, the PR of the change's other IDs.
- Status in the table: `planned` and `active` are the `Status:` lines of the delta; for a synced ID the second word is the status in main.

## 2. Summary

| Class | Changes | Reading |
|---|---|---|
| A. Merged, tests cite every ID, delta not in main's specs | 37 | finished as code and tests; the architect syncs the delta; their `design.md` and `tasks.md` are archivable after the sync (5 of them have an open task, see the notes under section 3) |
| B. Delta fully in main's specs | 7 | synced; archivable (`rm-horizon-render` has the task "Archive after the merge" open; `preview-level-1`: TEST-099 is `planned`, Verify D, in main too) |
| C. Partly in main, or some IDs without a citing test, or tasks open | 7 | unfinished or only partly done (section 5) |
| D. No spec delta | 22 | 18 finished (tooling, docs, measurement changes with merged PRs), 4 not (section 7) |

- Open draft PRs (the owner's, listed only): #1223 `change/pass1-deposition` (design notes, no code, 2026-11-06) and #1137 `change/c1-default` (draft, 2026-11-03). No other PR is open.

## 3. Class A: finished, delta not yet synced into `openspec/specs/`

The IDs are the architect's sync list. Every ID has at least one citing test; every change has a merged PR.

| Change | IDs | Delta status | IDs with a citing test | Merged in |
|---|---|---|---|---|
| breach-base-level | GEN-162 | 1 planned | 1/1 | #1307 |
| burn-width | GEN-168, GEN-169, TEST-110, TEST-112, TEST-113 | 5 planned | 5/5 | #1362 #1376 |
| c1-base-read | GEN-107 | 1 active | 1/1 | #1136 |
| carve-k-u | GEN-127 | 1 active | 1/1 | #1216 |
| carve-runoff | GEN-129 | 1 active | 1/1 | #1219 |
| cell-metric | GEN-138 | 1 planned | 1/1 | #1235 |
| channel-graph | GEN-170, STRM-084, TEST-114 | 3 planned | 3/3 | #1367 |
| cube-landmass | GEN-144 | 1 planned | 1/1 | #1243 |
| edit-cases | GEN-146 | 1 planned | 1/1 | #1251 |
| exit-window-cut | GEN-172, TEST-118 | 2 planned | 2/2 | #1379 |
| field-bake-class-source | GEN-115, STRM-073 | 2 planned | 2/2 | #1161 |
| flats-routing | GEN-173, TEST-119 | 2 planned | 2/2 | #1385 |
| g-surf | GEN-148 | 1 planned | 1/1 | #1253 |
| gsed-partition | GEN-155 | 1 planned | 1/1 | #1290 |
| h0-cross-edge | GEN-139 | 1 planned | 1/1 | #1236 |
| h0-stored-read | GEN-164 | 1 planned | 1/1 | #1317 |
| hydrology-h1 | GEN-145 | 1 planned | 1/1 | #1244 |
| k7-beds | GEN-176, TEST-124 | 2 planned | 2/2 | #1405 |
| k7-canonical | GEN-160 | 1 planned | 1/1 | #1283 |
| keep-pits | GEN-120, STRM-076 | 2 planned | 2/2 | #1192 |
| keep-pits-seams | GEN-121, STRM-077 | 2 planned | 2/2 | #1197 #1199 |
| landmass-rounds | GEN-147 | 1 planned | 1/1 | #1252 |
| layer-manifest | STRM-074 | 1 planned | 1/1 | #1164 |
| layer-source-l | STRM-083, TEST-111 | 2 planned | 2/2 | #1357 |
| m9-world-preview | GEN-103, TEST-086 | 2 planned | 2/2 | #1063 |
| partition-compact | GEN-149 | 1 planned | 1/1 | #1254 |
| partitioned-route | GEN-143 | 1 planned | 1/1 | #1241 |
| phase-plan | GEN-158 | 1 planned | 1/1 | #1287 |
| pre-breach | GEN-174, TEST-121 | 2 planned | 2/2 | #1399 (to be switched off in the default plan) |
| round-f32 | GEN-166, TEST-120 | 2 planned | 2/2 | #1394 (PR 1 of 2) |
| round-kernels | GEN-141 | 1 planned | 1/1 | #1238 |
| rounds-prototype | GEN-142 | 1 planned | 1/1 | #1240 |
| sed-catchment | GEN-152 | 1 planned | 1/1 | #1257 |
| sediment-cube | GEN-150 | 1 planned | 1/1 | #1255 |
| sediment-flood | GEN-159 | 1 planned | 1/1 | #1277 |
| sediment-wiring | GEN-151 | 1 planned | 1/1 | #1256 |
| viewer-class-store | VIEW-065 | 1 planned | 1/1 | #1167 |

Notes on class A:

- Open tasks in their `tasks.md`: `c1-base-read` 4.1 (the owner's decision on the default, the moved goldens, the archive; the default switch is the draft PR #1137); `field-bake-class-source` 1.4 (reviews, prepush, PR) and 1.5 (the layer manifest records the source id; `layer-manifest`); `layer-manifest` 1.3 (verifier, prepush, PR); `m9-world-preview` 3.3 (reviews, archive); `viewer-class-store` 1.3 (reviews, PR) and 1.4 (a tier D still for the owner). Their PRs are merged; the boxes look stale except `viewer-class-store` 1.4 and `c1-base-read` 4.1.
- `round-f32` is PR 1 of 2 (GEN-166: the narrowing; PR 2 stores the carried heights as f32): the change continues. `pre-breach` (GEN-174, TEST-121) is merged and is being switched off in the default plan (explorer): sync as written or hold it, the architect decides.
- `keep-pits` and `keep-pits-seams` (GEN-120/121, STRM-076/077) belong to a line whose later change `ocean-outlets` has no delta of its own (section 7).
- IDs without a commit naming them but with citing tests and a merged PR of the same change: TEST-111 (`layer-source-l`), TEST-119 (`flats-routing`), TEST-120 (`round-f32`), TEST-121 (`pre-breach`), TEST-124 (`k7-beds`).

## 4. Class B: delta fully in main's specs

| Change | IDs | Status (delta and main) | IDs with a citing test | Merged in |
|---|---|---|---|---|
| cache-eviction | GEN-153 | 1 active | 1/1 | #1274 |
| golden-review-anchor | TEST-016 | 1 active | 1/1 | #48 |
| preview-level-1 | AUTH-007, AUTH-008, AUTH-009, AUTH-010, GEN-157, TEST-098, TEST-099, VIEW-067 | 1 planned, 7 active | 8/8 | #1270 #1272 #1276 |
| process-checks | BUILD-040, BUILD-041, BUILD-042, BUILD-043, BUILD-044 | 5 active | 5/5 | #1260 #1262 #1265 #1266 #1267 |
| rm-horizon-render | REND-044, TEST-048, TEST-050, VIEW-054 | 4 active | 4/4 | #814 #639 #669 |
| stage-manifest | BUILD-047, TEST-103, TEST-104 | 3 active | 3/3 | #1285 |
| stage-probe | BUILD-046, GEN-161 | 2 active | 2/2 | #1282 |

Notes on class B: `rm-horizon-render` has its task 3.2 ("Archive after the merge") open and is a later PR (PR 6) of the `shadow-clipmap-march` plan; `preview-level-1` has TEST-099 (Verify D, `planned`) in main as `planned` too.

## 5. Class C: unfinished, or only partly in main's specs

| Change | IDs | Delta status | IDs with a citing test | Merged in |
|---|---|---|---|---|
| g-store | STRM-078 | 1 planned | 0/1 | #1259 |
| g-store-2 | STRM-079 | 1 planned | 0/1 | #1294 |
| l-store | GEN-156, GEN-154 | 2 planned | 1/2 | #1273 |
| m9-bake | GEN-093, GEN-094, GEN-095, GEN-096, GEN-097, GEN-098, GEN-099, GEN-100, GEN-101, GEN-110, GEN-111, GEN-102, GEN-112, GEN-113, GEN-114, STRM-065, STRM-066, STRM-067, STRM-068, STRM-072, TEST-081, TEST-082, TEST-083, TEST-084 | 24 planned | 20/24 | #1018 #1145 #1118 #1157 #1153 #1158 #1019 |
| m9-generator-round-2 | GEN-104, GEN-105, GEN-106, GEN-021, TEST-087, TEST-088 | 5 planned, 1 active | 6/6 | 083de557 (#1078) |
| shadow-clipmap-march | BUDG-002, REND-046, REND-047, REND-048, REND-049, REND-050, REND-051, REND-052, REND-053, STRM-060, TEST-054 | 9 planned, 2 active | 10/11 | #985 #1014 #1017 #1024 #1041 #1084 #1100 #786 #748 |
| viewer-erosion | GEN-109, VIEW-062, VIEW-063, VIEW-064 | 3 planned, 1 active | 4/4 | #1143 #1158 #1167 |

What is missing in each:

- `g-store` (STRM-078), `g-store-2` (STRM-079), `l-store` (GEN-154 has a citing test; GEN-156 is Verify D): merged measurement items (the new chain's store measured as real pack records); no citing test (Verify D); the gate G-STORE is `partial` in `docs/gates/ledger.md`. Not finished while G-STORE is open.
- `m9-bake`: 27 requirement headings in the delta, 3 of them in main already (modified requirements), **24 planned and not in main**; 20 of the 24 have a citing test, **GEN-093, GEN-096, TEST-081 and TEST-083 have none**; `tasks.md` 3 done, 11 open (2.3 to 2.9, 2.x, 2.y, 2.z: PRs 2 to 7, the overnight bake, the perf window, `verifier`, `spec-guardian`); merged PRs #1018, #1019, #1118, #1145, #1153, #1157, #1158. The oldest and widest change in the list; it needs the architect's read before any sync.
- `m9-generator-round-2`: GEN-104 to GEN-106, TEST-087, TEST-088 planned and not in main (all have citing tests); GEN-021 is in main (active); merged #1078; open tasks 3.3 (re-pin the known answers of GLOBAL_VERSION 6) and 3.4 (reviews).
- `shadow-clipmap-march`: 11 IDs; STRM-060 and TEST-054 active in main; BUDG-002 `planned` in main (Verify C, no citing test); **REND-046 to REND-053 planned and not in main** (citing tests: 1, 1, 1, 4, 5, 5, 3, 3 files; REND-046's only citing file holds `#[ignore]`d tests); `tasks.md` 4 done, 16 open (PRs 4 to 9, the 2-degree perf window, the owner's look review and `/bless` of the first lit goldens, the glossary changes of 4.1 to 4.4); merged #748, #786, #985, #1014, #1017, #1024, #1041, #1084, #1100.
- `viewer-erosion`: GEN-109 active in main; **VIEW-062, VIEW-063, VIEW-064 planned and not in main** (each has citing tests); merged #1143, #1158, #1167; open task 1.4b (verifier, prepush, PR; probably stale).

## 6. The 57 `design.md` and `tasks.md` files

One row per change that holds either file (45). "Archivable" means: the change is finished (class A, B or finished D) and the file can go when the change is archived (CLAUDE.md: delete `design.md` and `tasks.md` with the archive); "after the open task" means the change is finished but one task box is open (sections 3, 4 and 7 say which); nothing is to be archived before the architect says so.

| Change | design.md | tasks.md (done/open) | Class | Archivable | Why |
|---|---|---|---|---|---|
| c1-base-read | design | 10/1 | A | after the open task | merged, tests cite the IDs; spec delta not synced; open task in tasks.md |
| capture-erosion | - | 1/1 | D | after the open task | task 1.2 (the first sheet, tier D, for the owner) is open |
| carve-k-u | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| carve-runoff | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| cell-metric | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| cube-landmass | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| edit-cases | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| field-bake-class-source | - | 3/2 | A | after the open task | merged, tests cite the IDs; spec delta not synced; open task in tasks.md |
| g-store | design | - | C | no | see section 5 |
| g-store-2 | design | - | C | no | see section 5 |
| g-surf | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| ground-channels-2b | design | 1/3 | D | no | design notes only; the three PRs of tasks 2.1 to 2.3 are not started |
| h0-cross-edge | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| hydrology-h1 | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| k7-canonical | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| l-store | design | - | C | no | see section 5 |
| landmass-rounds | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| layer-manifest | - | 2/1 | A | after the open task | merged, tests cite the IDs; spec delta not synced; open task in tasks.md |
| m9-bake | design | 3/11 | C | no | see section 5 |
| m9-generator-round-1 | design | 4/1 | D | after the open task | merged; the open box is a stale review box |
| m9-generator-round-1b | design | 3/1 | D | after the open task | merged; the open box is a stale review box |
| m9-generator-round-2 | design | 5/2 | C | no | see section 5 |
| m9-generator-round-3 | design | 5/1 | D | after the open task | merged; the open box is a stale review box |
| m9-world-preview | design | 5/1 | A | after the open task | merged, tests cite the IDs; spec delta not synced; open task in tasks.md |
| partition-compact | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| partitioned-route | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| phase-plan | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| preview-latency | design | - | D | yes | no spec delta |
| preview-level-1 | design | - | B | yes | delta synced into main specs |
| process-checks | design | - | B | yes | delta synced into main specs |
| region-12288 | design | - | D | yes | no spec delta |
| region-mode1 | design | - | D | yes | no spec delta |
| rm-horizon-render | design | 7/1 | B | after the open task | delta synced into main specs; open task in tasks.md |
| round-f32 | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| round-kernels | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| rounds-prototype | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| runtime-composition | design | - | D | no | living notes of A3 section 18; the owner/architect decide |
| sed-catchment | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| sediment-cube | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| sediment-flood | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| sediment-wiring | design | - | A | yes | merged, tests cite the IDs; spec delta not synced |
| shadow-clipmap-march | design | 4/16 | C | no | see section 5 |
| stage-manifest | design | - | B | yes | delta synced into main specs |
| viewer-class-store | design | 2/2 | A | after the open task | merged, tests cite the IDs; spec delta not synced; open task in tasks.md |
| viewer-erosion | design | 6/1 | C | no | see section 5 |

Counts: of the 42 `design.md` files, **26 are archivable now** (20 of class A, 3 of B, 3 of finished D), **7 after the open task** (3 of A, 1 of B, 3 of finished D with a stale review box) and **9 are not** (the seven class C changes, `ground-channels-2b`, `runtime-composition`). Of the 15 `tasks.md` files, **none is clean**: 10 belong to finished changes with one open box (5 of A, 1 of B, 3 of finished D, `capture-erosion`) and 5 to unfinished changes (the four class C changes with tasks, `ground-channels-2b`).

## 7. Class D: changes without a spec delta (22)

Tooling, docs and measurement changes carry no requirement of their own. "Finished" here means a merged PR named the change and the open boxes of `tasks.md` are reviews of that PR.

| Change | Files | Last commit touching it | Assessment |
|---|---|---|---|
| breach-and-ridged-band | proposal | 89ab4f11 2026-11-05 (#1180) | finished: the proposal's change (class source 6, bake kernel 4) merged; no spec delta (its proposal says GEN-098 and GEN-100 are planned in m9-bake) |
| capture-erosion | proposal, tasks 1/1 | 1d1f9242 2026-11-04 (#1171) | partly: the capture-stills change merged (VIEW-062 of viewer-erosion); task 1.2 (the first sheet over the m9c14 store, tier D, for the owner) is open |
| cost-patch-share | proposal | faddc31f 2026-11-04 (#1172) | finished: merged; no spec delta (REND-044 sits in rm-horizon-render) |
| deposit-depth-code | proposal | d4eff020 2026-11-05 (#1203) | finished: merged; no spec delta (GEN-111, STRM-072 sit in m9-bake) |
| ground-channels-2b | proposal, design, tasks 1/3 | 42fd6f76 2026-11-05 (#1187) | not finished: design notes only ("D1 an open branch"); tasks 2.1 to 2.3 (the three PRs) are not started |
| isostasy-rebound | proposal | a7c0814c 2026-11-05 (#1206) | abandoned by decision: its last commit records "isostasy dropped (candidate note)"; no code, no tasks |
| k7-manifest | proposal | bab40841 2026-11-10 (#1289) | finished: merged (TEST-105 line k7-f1-t3-4); no spec delta |
| k7-probe-emit | proposal | 79e8e5f9 2026-11-10 (#1286) | finished: merged (GEN-161 sits in stage-probe, synced) |
| m9-generator-round-1 | proposal, design, tasks 4/1 | 6078eef8 2026-11-02 (#1068) | finished: merged; task 2.3 (reviews) is the only unchecked box and is stale; no spec delta (GEN-103 to GEN-106 sit in m9-world-preview and m9-generator-round-2) |
| m9-generator-round-1b | proposal, design, tasks 3/1 | 8aee9cec 2026-11-02 (#1071) | finished: merged; task 2.2 (reviews) unchecked and stale; no spec delta |
| m9-generator-round-3 | proposal, design, tasks 5/1 | 36a4beef 2026-11-03 (#1086) | finished: merged; task 3.2 (verifier) unchecked and stale; no spec delta |
| node-buffer-delta | proposal | 9f45ae7e 2026-11-03 (#1113) | finished: PR A merged; its subject records that a node delta is not possible and that the Tracked line was rewritten |
| ocean-outlets | proposal | c31a1260 2026-11-05 (#1205) | finished: merged; no spec delta (STRM-077 is amended in the keep-pits-seams delta) |
| preview-latency | proposal, design | d575fd4d 2026-11-09 (#1272) | finished: merged (TEST-099 untimed latency scene; the requirement sits in preview-level-1, synced) |
| region-12288 | proposal, design | 0cfc5589 2026-11-08 (#1246) | finished: merged (h1-corrections, #1246); no spec delta |
| region-mode1 | proposal, design | df1562e3 2026-10-20 (#427) | finished: merged long ago (region-items, #427, 2026-10-20); the oldest open change in the list |
| region-plains-block | proposal | 563c0e60 2026-11-10 (#1293) | finished: merged; no spec delta |
| region-scale-window | proposal | 5ccc0b88 2026-11-10 (#1291) | finished: merged; no spec delta |
| region-sed-land | proposal | 9ca0aa0c 2026-11-10 (#1296) | finished: merged; no spec delta |
| runtime-composition | proposal, design | 203c8ba2 2026-11-11 (#1380) | not finished: living design notes of A3 section 18 (the river drawing); the IDs it reserved (REND-059, REND-060, STRM-085, TEST-116, TEST-117) are in main's specs by other changes; last commit 2026-11-11 |
| stage-review-agent | proposal | 056c334b 2026-11-10 (#1288) | finished: merged (BUILD-048); no spec delta |
| stream-look-marks | proposal | b8f8edc2 2026-11-10 (#1304) | finished: merged; no spec delta |

## 8. Not done

- The tests that cite an ID were not run; `#[ignore]` was checked at file level only. Whether a `Status: planned` in a delta is stale is not decided: the 37 class A deltas say `planned` while their tests exist.
- The spec deltas were not compared with the code line by line; the evidence is the IDs, the citing tests and the merged PRs.
- Nothing was synced, archived or edited; the architect syncs from sections 3 to 5.
- Source: `openspec/changes/`, `openspec/specs/`, `git log origin/main`, `gh pr list`; the scripts are in the lane's scratch area, not in the repo.
