# Perf window 12 (removal of the vertex render path), 2026-10-19

Rasierklinge, main 6f585b5 (#304 merged: compact is the only tile format, `lit-formats` gone), release build of that tree, `cargo xtask perf m5 --store <16384 store> --out target/perf-w12` once, no `--machine-ready`. Run by sonnet-m4 on opus-planner's go; the other sessions were paused. A regression check against window 11, not a gate.

## Verdict

Readiness gate ok. No session counts: every lit run and every streaming scene is "no verdict" under the ADR 0065 CPU calibration (single-thread job over 52 ms before or 56 ms after: 52.6 to 76.5 ms before, 56.0 to 65.6 ms after). The machine was 8 to 40 % slower on one thread than the calibration limit allows. GPU verdicts are also off: ground-1m 1395, aerial-20km 1264, coast-2km 1237, coast-3m 1252, orbit 1334 MHz, all below the 1,400 MHz floor (window 11: 1590, 1424, 1312, 1327, 1462). The numbers below are reported, not judged.

## Lit views, window 11 to window 12 (ms median)

| View | terrain | atmosphere | water | frame median / p99 | clock w11 -> w12 (MHz) |
|---|---|---|---|---|---|
| ground-1m | 1.083 -> 1.129 | 0.211 -> 0.223 | not in view | 2.908 -> 2.982 / 3.428 -> 4.881 | 1590 -> 1395 |
| aerial-20km | 0.776 -> 0.863 | 0.235 -> 0.267 | 0.108 -> 0.047 | 2.332 -> 2.266 / 3.071 -> 2.911 | 1424 -> 1264 |
| coast-2km | 1.024 -> 1.150 | 0.253 -> 0.284 | 1.647 -> 1.768 | 4.523 -> 4.781 / 4.921 -> 5.253 | 1312 -> 1237 |
| coast-3m | 1.312 -> 1.418 | 0.233 -> 0.249 | 1.938 -> 2.024 | 5.586 -> 5.785 / 6.083 -> 6.682 | 1327 -> 1252 |
| orbit | 0.053 -> 0.145 | 0.684 -> 0.748 | 0.169 -> 0.215 | 1.485 -> 1.799 / 2.078 -> 2.135 | 1462 -> 1334 |

Node counts equal window 11 (water nodes 844 and 840 on the coast views). First run of each view shown (the three repeats agree within 3 %).

## Compact terrain +17 to 26 % over vertex

The vertex form no longer exists, so the ratio cannot be measured again. Compact terrain against its own window 11 value: +4 % (ground-1m), +11 % (aerial-20km), +12 % (coast-2km), +8 % (coast-3m), at clocks 6 to 12 % lower. The change is within the clock difference, so the removal is not shown to move compact terrain; this window cannot show it does not.

## Open

- orbit terrain 0.053 -> 0.145 ms (+0.09 ms) and orbit frame +21 % are larger than the clock difference (-9 %) explains: not attributed (candidate: the floor-3 start or the reference-free draw path; needs a window with a valid session).
- aerial-20km water 0.108 -> 0.047 ms: not attributed.
- dive: median 5.33 -> 7.70 ms, p99 13.57 -> 16.24, max 15.73 -> 85.41; descent lag mean 0.846 (budget 1), worst frame 3.13 (budget 3, over). dive-fast 8.96 -> 8.90, dive-cold 5.46 -> 6.43, teleport-1m 6.65 -> 5.67, teleport-2km 4.07 -> 4.56, look-1m 3.77, look-2km 2.00. These do not count (calibration).
- Terrain slots 219,585 of 219,585 on every scene; RAM at most 1.03 GiB (dive-fast); driver VRAM at most 1.35 GiB.
- A window with a calibration inside the limits is needed before the removal is judged; the logs are in `target/perf-w12/` (not committed).
