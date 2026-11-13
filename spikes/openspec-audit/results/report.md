# OpenSpec audit: the open changes against main (read only)

Run 2026-11-14 by the spikes lane on the lead's order #109 (owner: P3's first item, started early). Main 482c1f05. Read only: nothing archived, synced, moved or deleted; archiving is P3's work for the owning lanes.

## Method
- Open changes: every folder under `openspec/changes/` except `archive/`: **72**.
- **Synced:** each `### Requirement:` heading of the change's spec deltas is a `### Requirement:` heading in `openspec/specs/**/spec.md` (the criterion of audit batch C, `spikes/audit-batch-c/results/open-changes-2026-11-12.md`). "Unsynced" lists the headings missing from the main specs.
- **Merged:** PR numbers from `git log origin/main` (subjects naming the change or its IDs) and the change's own commits.
- Where a note says an ID "is in the spec", the spec's text names it; the Unsynced column (headings) decides the class.
- Four read-only agents read each change's proposal, deltas, tasks and PRs against the specs, the ADRs, `docs/milestones/P2.md` and `docs/parking.md`; the lane recomputed the sync column by script and checked the classes against audit batch C.

## Classes
| Class | Meaning |
|---|---|
| done | merged, and every requirement of the delta is in the main specs (or the change has no requirement: a tool, an example, a measurement, notes) |
| done, sync pending | merged, but some requirement headings of the delta are not in the main specs: archivable after the sync |
| superseded | replaced or dropped by a later change, ADR or decision |
| still wanted | not done and still needed; its home is the P2 exit item, a parking line, or "no home" |

## Counts
| Class | Changes |
|---|---|
| done | 23 |
| done, sync pending | 42 |
| superseded | 3 |
| still wanted | 4 |

- **done:** `breach-and-ridged-band`, `cache-eviction`, `capture-erosion`, `cost-patch-share`, `deposit-depth-code`, `golden-review-anchor`, `k7-manifest`, `k7-probe-emit`, `m9-generator-round-1`, `m9-generator-round-1b`, `m9-generator-round-3`, `ocean-outlets`, `process-checks`, `region-12288`, `region-plains-block`, `region-scale-window`, `region-sed-land`, `rm-horizon-render`, `runtime-composition`, `stage-manifest`, `stage-probe`, `stage-review-agent`, `stream-look-marks`.
- **done, sync pending:** `breach-base-level`, `build-env-pin`, `c1-base-read`, `carve-k-u`, `carve-runoff`, `cell-metric`, `channel-graph`, `cube-landmass`, `determinism-threads`, `edit-cases`, `field-bake-class-source`, `g-store`, `g-store-2`, `g-surf`, `gsed-partition`, `h0-cross-edge`, `h0-stored-read`, `hydrology-h1`, `k7-canonical`, `k7-layout`, `keep-pits`, `keep-pits-seams`, `l-store`, `landmass-rounds`, `layer-manifest`, `layer-source-l`, `m9-bake`, `m9-generator-round-2`, `m9-world-preview`, `partition-compact`, `partitioned-route`, `phase-plan`, `pre-breach`, `round-kernels`, `rounds-prototype`, `sed-catchment`, `sediment-cube`, `sediment-flood`, `sediment-wiring`, `viewer-class-store`, `viewer-erosion`, `world-datum`.
- **superseded:** `coast-tilt`, `isostasy-rebound`, `node-buffer-delta`.
- **still wanted:** `band-slope-cap`, `ground-channels-2b`, `region-mode1`, `shadow-clipmap-march`.

## Findings
- No open change belongs to a P2 exit item: the P2 work (E1-E5) runs in changes opened since and archived on merge.
- The sync backlog is the bulk: most "done, sync pending" changes were merged in M8/M9 with their deltas left in the change folder (the parking line "Audit batch C: sync specs" holds them). `m9-bake` alone carries 24 unsynced headings, part of them overtaken by the K7 chain (ADR 0119): its sync is a decision per ID, not a copy.
- Three of the four "still wanted" changes have no home in P2 or the parking list (`ground-channels-2b`, `region-mode1`, `shadow-clipmap-march`): the owner either parks them or drops them (then they become superseded and are archived as such).
- `design.md` and `tasks.md` remain in many done changes; ADR 0055 deletes them at the archive.
- GEN-112 (the layer read, in `m9-bake`'s delta) still names the Catmull-Rom read that GEN-188 (2026-11-13, synced) replaced: the m9-bake sync takes GEN-188's wording.

## Table
"Unsynced": the delta's requirement headings not in the main specs. "Files": `design.md` / `tasks.md` still in the change folder.

| Change | Class | Evidence | Unsynced | Files | Note |
|---|---|---|---|---|---|
| band-slope-cap | still wanted | #1430; GEN-179 not in specs | GEN-179, TEST-127 | - | Band slope cap, merged but off. Delta not synced. Parking "Starting surface" candidate; the owner decides on a picture sheet. Home: parking: Starting surface (candidate GEN-179, merged, off) |
| breach-and-ridged-band | done | #1180 | - | - | Mountain band x4.5 ridged and centred, pass-1 breach 500 m. Proposal-only change; the hill-band text is in the generator spec. |
| breach-base-level | done, sync pending | #1307; GEN-162 | GEN-162 | - | A breach never cuts below its chain exit. GEN-162 is in the generator spec. |
| build-env-pin | done, sync pending | #1417; BUILD-005 | BUILD-050 | - | Exact Rust toolchain pin 1.99.0 with tests (BUILD-050). BUILD-050 itself is not in the spec text. |
| c1-base-read | done, sync pending | #1136; GEN-107 | GEN-107 | design tasks | C1 read of the bake lattice and field cpu-erosion-v2. Open task 4.1 is only the owner's default decision and the archive. |
| cache-eviction | done | #1274; GEN-143, GEN-146, GEN-153 | - | - | Byte budget and LRU eviction for the edit-path memo caches. All three IDs are in the spec. |
| capture-erosion | done | #1171; VIEW-062 | - | tasks | capture-stills draws the erosion layer; class/legacy source choice. Open task 1.2 is a one-off owner sheet, not a requirement. |
| carve-k-u | done, sync pending | #1216; GEN-116, GEN-123 | GEN-127 | design | Both carves read lithology K and the uplift field. GEN-127 is not in specs; the behaviour is. |
| carve-runoff | done, sync pending | #1219; GEN-117, GEN-119 | GEN-129 | design | Runoff-weighted discharge and vegetation factor in both carves. GEN-129 is not in specs; the behaviour is. |
| cell-metric | done, sync pending | #1235, #1236, #1324; GEN-138 not in specs | GEN-138 | design | Physical cube-face cell metric behind a switch. Delta not synced into specs; archive after a spec check. |
| channel-graph | done, sync pending | #1367; GEN-170, STRM-084 | GEN-170, STRM-084, TEST-114 | - | Compact channel graph store, Kind 20 per L tile, cube-edge resolver. Both IDs are in specs. |
| coast-tilt | superseded | #1435, #1441; GEN-180 not in specs | GEN-180, TEST-131 | - | Coast tilt dropped 2026-11-12: it does not drain the plains (docs/parking.md, ADR 0120). Code merged, off. |
| cost-patch-share | done | #1172; REND-044 | - | - | shadow-cost logs the patch break-up's share per pose. REND-044 is in the renderer and compute-budgets specs. |
| cube-landmass | done, sync pending | #1243, #1244, #1245, #1251, #1252; GEN-139 | GEN-144 | design | Six cube faces as one grid, landmass routed whole. GEN-139 is in the spec. |
| deposit-depth-code | done | #1203; STRM-072 | - | - | Deposited depth (kind 15) as a square-root code. STRM-072 is in the streaming spec. |
| determinism-threads | done, sync pending | #1428; GEN-003, GEN-177 | BUILD-051 | - | Region bake fixture at 1 and 8 threads joined the cross-OS determinism manifest. BUILD-051 is not in specs. |
| edit-cases | done, sync pending | #1251, #1252, #1253; GEN-146 | GEN-146 | design | Memoised re-solve, Q_w and E_open, edit cases E1-E7, G-EQ and G-INC. GEN-146 is in the spec. |
| field-bake-class-source | done, sync pending | #1161, #1162; GEN-013, STRM-011 | GEN-115, STRM-073 | tasks | Field bake from the class source and the store's source identity. GEN-115 and STRM-073 are not in specs. Tasks 1.4 and 1.5 are open. |
| g-store | done, sync pending | #1259, #1316; STRM-078 not in main specs | STRM-078 | design | Measured the new chain's store items as real pack records. Delta unsynced; G-STORE ledger row still partial (parking: G-STORE extension). |
| g-store-2 | done, sync pending | #1294, #1297; STRM-079 not in main specs | STRM-079 | design | Measured field bake, replacement and compaction space, edit items. Delta unsynced. |
| g-surf | done, sync pending | #1253, #1273; GEN-148 not in main specs | GEN-148 | design | K7 on the base read with channels burned in; G-SURF reported. Delta unsynced (audit batch C). |
| golden-review-anchor | done | ADR 0096 (#1112); TEST-016, TEST-022 in specs | - | - | Rounding auto-bless on lit, sky and water views is checked against the owner's last bless. |
| ground-channels-2b | still wanted | #1187 design notes only; tasks 1/3; no GEN/STRM IDs | - | design tasks | Ground materials reading baked channels (per-tile carrier); PRs 2.1 to 2.3 never started. No home. Home: no home (closest: SPIKE-100 D1, materials from the channels) |
| gsed-partition | done, sync pending | #1290; GEN-155 not in main specs | GEN-155 | - | G-SED partition test (same totals under different partitions). Delta unsynced. |
| h0-cross-edge | done, sync pending | #1236; GEN-139 in specs | GEN-139 | design | One drainage graph over the six cube faces. |
| h0-stored-read | done, sync pending | #1317; GEN-107, GEN-139 in specs; GEN-164 not | GEN-164 | - | H0 from a stored bake equals the in-memory H0 at cube edges and corners. GEN-164 unsynced. |
| hydrology-h1 | done, sync pending | #1244, #1251, #1252; GEN-146 in specs; GEN-145 not | GEN-145 | design | Channel graph and basins from a routing, G-CON exact. Delta partly unsynced. |
| isostasy-rebound | superseded | #1204, #1206; GEN-124 absent | - | - | Flexural rebound measured then dropped from M9 by decision; parking "Isostatic rebound (GEN-124)", flexural version as candidate. |
| k7-canonical | done, sync pending | #1283; GEN-160 not in main specs | GEN-160 | design | K7 floods with the canonical flood by a parameter. Delta unsynced. |
| k7-layout | done, sync pending | #1445, #1448; GEN-182 not in main specs; GEN-169, GEN-176 in | GEN-182 | - | K7 layout (ADR 0119), merged #1445, #1448; GEN-182 not synced; its TEST-130 is on the parking list. |
| k7-manifest | done | #1289; TEST-105 in specs | - | - | Stage manifest fixture runs K7 on one tile (line k7-f1-t3-4). |
| k7-probe-emit | done | #1286; GEN-161 in specs | - | - | Stage probe's K7 hook. |
| keep-pits | done, sync pending | #1192, #1201; GEN-120, STRM-072, STRM-076 in specs | GEN-120, STRM-076 | - | Pass 1 and pass 2 keep their pits and store the fill depth. |
| keep-pits-seams | done, sync pending | #1197, #1199, #1201, #1205; GEN-120, STRM-075, STRM-076 in specs | GEN-121, STRM-077 | - | Planet-wide spill level, fill depth by post-pass (STRM-077), exact base. STRM-077 unsynced. |
| l-store | done, sync pending | #1273; GEN-156 in specs; GEN-154 not | GEN-154, GEN-156 | design | Stored change L of the K7 chain measured as kind 13 streams. G-STORE still partial (parking). |
| landmass-rounds | done, sync pending | #1252; GEN-147 not in main specs | GEN-147 | design | Prototype landmass rounds at the erosion grid, peak 7.7 GiB against the 12 GB budget. Delta unsynced. |
| layer-manifest | done, sync pending | STRM-072 in spec; 3adedfc2 (PR number not found) | STRM-074 | tasks | Layer manifest (STRM-074) built and merged. The ID is not in the main spec, so a spec sync is pending (parking: audit batch C). |
| layer-source-l | done, sync pending | #1357, ae9403f2; STRM-083, TEST-111 | STRM-083, TEST-111 | - | CommitLayer and PairedLayer read the new chain's L. The IDs are not in the main spec, so a spec sync is pending (audit batch C). |
| m9-bake | done, sync pending | #1104, #1107, #1108, #1111, #1118, #1119, #1132; GEN-020/061/062/099/104/109/120 in spec | GEN-093, GEN-094, GEN-095, GEN-096, GEN-097, GEN-098, GEN-099, GEN-100, GEN-101, GEN-102, GEN-110, GEN-111, GEN-112, GEN-113, GEN-114, STRM-065, STRM-066, STRM-067, STRM-068, STRM-072, TEST-081, TEST-082, TEST-083, TEST-084 | design tasks | Pass 1/2 bake, layer store, relief switch merged; tasks 2.7-2.9 (overnight bake, perf) overtaken by the K7 chain (ADR 0119); the sync decides which of the 24 IDs still hold (4 have no citing test: GEN-093, GEN-096, TEST-081, TEST-083, audit batch C). |
| m9-generator-round-1 | done | #1068, #1071 | - | design tasks | Segmented belts and a continuous mask in the preview. The only open task is the review agents. |
| m9-generator-round-1b | done | #1071 | - | design tasks | Bent, tapered, segmented belts, wider mountains, seeds 1 and 2. The only open task is the review agents. |
| m9-generator-round-2 | done, sync pending | #1078; GEN-021/025/064/104 in spec | GEN-104, GEN-105, GEN-106, TEST-087, TEST-088 | design tasks | Continent stage from SPIKE-92 H2r3. GEN-106 and TEST-087/088 are absent from the spec, so a sync is pending. |
| m9-generator-round-3 | done | #1086 | - | design tasks | Relief on the continents (SPIKE-93 bands, p99 step limit). The only open task is the verifier. |
| m9-world-preview | done, sync pending | #1063; GEN-093..098/103, TEST-082/086 | GEN-103, TEST-086 | design tasks | Default class source and whole-planet preview iteration 0. These IDs are not in the main spec; ADRs 0088/0089 name them. |
| node-buffer-delta | superseded | #1113 | - | - | The node-delta idea was shown impossible. PR A (frame-log CPU phase columns) merged. PR B was never built; the Tracked line was rewritten. |
| ocean-outlets | done | #1205, c31a1260 | - | - | Only ocean-connected cells are outlets of the fill post-pass. The proposal has no requirement IDs. |
| partition-compact | done, sync pending | #1254, 6807ad71; GEN-149 | GEN-149 | design | Partitioned route on the compact grid. GEN-149 is not in the main spec, so a spec sync is pending. |
| partitioned-route | done, sync pending | GEN-143 in spec; 4fa17ae5, 2e5453ea, 9b5a3e65 (PR numbers not found) | GEN-143 | design | Partitioned route behind a switch, equal to route_rules. Merged; GEN-141/144 are absent from the spec. |
| phase-plan | done, sync pending | #1287, 1094d82a; GEN-158 | GEN-158 | design | Resident build as one phase plan. GEN-158 is not in the main spec (ADR 0115 mentions it), so a spec sync is pending. |
| pre-breach | done, sync pending | #1399, #1407; GEN-162/173 in spec; GEN-174 absent | GEN-174, TEST-121 | - | Round-0 breach of closed basins, merged #1399, #1407, then set off by default (architect, 2026-11-12); a redesign has no named home. |
| process-checks | done | BUILD-040..044 all in spec; #1312, #1469 | - | design | Process checks are specified and implemented. |
| region-12288 | done | #1252, #1253, 2e5453ea | - | design | Prototype region measured at the 12,288-cell erosion grid (gates, channel density, memory). Measurement only, no requirements. |
| region-mode1 | still wanted | #427 (clip-inside only); #443 deferred (owner) | - | design | "Clip outside" region mask is design only and deferred by the owner. WDEF-012 still refuses mode 1. No home: absent from P2 and parking. Home: no home (owner deferred the "clip outside" mode; WDEF-012 refuses it) |
| region-plains-block | done | #1293, 563c0e60 | - | - | region_scale runs with the stage probe on and prints the plains block (item 13). An example only. |
| region-scale-window | done | #1291 | - | - | Example-only proposal: region_scale takes PLANET_REGION window; merged, no requirement IDs. Archive only. |
| region-sed-land | done | #1296 | - | - | Example-only: region_scale prints G-LAND sinks and G-SED balance. Merged, no requirement IDs. Archive only. |
| rm-horizon-render | done | #1109; REND-038/041/043/044, VIEW-042/054, TEST-027/048/050 in specs | - | design tasks | PR 6 of ADR 0085: horizon removed from renderer, frame and viewer; deltas synced. Archive. |
| round-kernels | done, sync pending | #1238; GEN-141 not in specs | GEN-141 | design | MFD clipped to catchment and canonical flood behind switches. Code merged; delta never synced to main specs (audit batch C sync). |
| rounds-prototype | done, sync pending | #1240, #1244-#1253; GEN-162 in specs; GEN-141/142 not | GEN-142 | design | Round solver, G-ROUND/G-DET exact on prototype region; GEN-141/142 never synced into specs. |
| runtime-composition | done | #1305, #1323, #1350, #1353, #1363, #1368, #1380; GEN-167, STRM-082/085, REND-059/060 in specs | - | design | Design notes for sampler composition, lakes, rivers; code PRs (bake writer, channel graph store) landed. Archive. |
| sed-catchment | done, sync pending | #1257; GEN-152 not in specs | GEN-152 | design | Deposition balanced per catchment and round. Merged; delta never synced to main specs. |
| sediment-cube | done, sync pending | #1255; GEN-150 not in specs | GEN-150 | design | Deposition, pit spreading, thermal on a cube grid. Merged; delta never synced. |
| sediment-flood | done, sync pending | #1277, #1385; GEN-159, GEN-173 in specs | GEN-159 | design | Sediment pass floods with canonical flood; flats graded by pour-point distance. Archive. |
| sediment-wiring | done, sync pending | #1256; GEN-148/151 not in specs | GEN-151 | design | Sediment steps after the rounds on compact landmass, G-SED measured. Merged; delta never synced. |
| shadow-clipmap-march | still wanted | #941, #1024, #1041, #1084, #1085, #1109; REND-044/045 in specs; REND-046..053 not | REND-046, REND-047, REND-048, REND-049, REND-050, REND-051, REND-052, REND-053 | design tasks | ADR 0085 PRs 1-6 merged; PR 7-9 (horizon jobs, generators, archive) unmerged. No home in P2 or parking: "no home". Home: no home (ADR 0085 PRs 7-9: horizon jobs, generators, archive) |
| stage-manifest | done | #1285; BUILD-046/047, TEST-103/104 in specs | - | design | Committed stage manifest, staleness check, cargo xtask stages. Archive. |
| stage-probe | done | #1282, #1299; BUILD-046, GEN-161 in specs | - | - | Read-only stage hook API and sheet probe with hillshade. Archive. |
| stage-review-agent | done | #1288 | - | - | Paper-only: stage-reviewer agent and sheet size check (BUILD-048). Merged; archive. |
| stream-look-marks | done | #1304 | - | - | Streaming perf smoke lines carry run time and phases; no requirement. Archive. |
| viewer-class-store | done, sync pending | #1167; TEST-012 in specs; VIEW-065, STRM-073/074 not | VIEW-065 | design tasks | Viewer shows the world a store records; erosion layer read by manifest. Merged; VIEW-065 delta unsynced. |
| viewer-erosion | done, sync pending | #1158, #1171; GEN-099/109, VIEW-062 in specs; VIEW-063/064 not | VIEW-062, VIEW-063, VIEW-064 | design tasks | `--erosion on` draws erosion layer over a stored world. Merged; part of delta unsynced. |
| world-datum | done, sync pending | #1416, #1422, #1424, #1433; GEN-166, GEN-175 in specs | GEN-175, TEST-122 | - | Per-world flood epsilon for f32 rounds; TEST-122 not in specs. Archive. |
