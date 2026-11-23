# SPIKE-144: `sel-post` ablated on R1h; `base1` on the H1 cube grid and the hooks as keyed stage configs

Opened: 2026-11-23 (lead #178, #178a; G3 gate risks R16 and R24)
Question: Does RG need stage 3, `sel-post` (the selective drain of z_s after K5)? And what must `base1` (the selective drain of z_b before K5) and the process-global hooks become on the H1 cube grid?
Why it matters: wave 1 of the port (G3-5, G3-C) misses `base1` and `sel-post`, and the hooks sit outside every cache key (R24).
Time box: half a day of agent work.
Success criteria: one R1h ablation row judged by the ablation rule (review-response section 3, with the 1 % floor for failing bars); a design note with what each hook becomes.
Fallback if it fails: `sel-post` stays on as today.
Code location: `spike/rg-prototype-spikes` 36bc4523 (`rg_chain --base-stages 1`, `install_s138("sel-pre")`, spike only, off by default).
Status: closed 2026-11-23. The row is open under O3: it is rerun and judged again once the owner's net budget validator is in (lead #178a).

## Answer
- `sel-post` off improves R1h on almost every column, but fails rule 1 on `bar_violations`: 102 -> 110. That is +7.8 %, over the 1 % floor for a failing bar.
  - The extra violations are KV validator records (KV overruns 99 -> 108). The one spill mismatch disappears.
  - Lead's reading (#178a): strict, so `sel-post` stays on, and R3a and M1 are not run.
- `base1` on the cube needs CubePatch adjacency, the quadrature cell area, the global sea-mask outlets, a global processing order, and a breach bound or the seam-pair spill engine.
- The hooks become `RegionSpec` configs inside the cache key.

## The row (R1h, today's tip 7fe4e73d; gate main cd84f36a; `raw/`)
| R1h | defaults (stages 1 + 3) | `sel-post` off (stage 1) |
|---|---|---|
| bars passing | 10/11 | 10/11 |
| bar_violations (fails on both) | 102 (spill mismatch 1 + validator 101) | 110 (spill mismatch 0 + validator 110) |
| KV validator overruns | 99 | 108 |
| ponds | 0.68 | 0.01 |
| uphill (bar) | 0.31 | 0.03 |
| agreement 1 / 75 km2 | 92.73 / 98.26 | 93.04 / 98.62 |
| corridor | 99.78 | 99.89 |
| kept river / basin share | 3.01 / 3.68 | 3.05 / 3.06 |
| kept basins / kept lakes | 1,491 / 778 km2 | 1,491 / 777 km2 |
| saddles1 against S_KH / S0 | 340 / 1,950 | 351 / 1,946 |
| corner-only links at 5 m (final) | 260 | 272 |
| b_overrun | 0 | 0 |

- The default's stage 3 meets 10 depressions on R1h's z_s (7 filled, 3 breached): `raw/r1h-def-chain-lines.txt`. Without it, ponds and uphill steps nearly vanish.

## Design note: `base1` on the H1 cube grid (no build)
- Today: `install_s138`'s `BASE_HOOK` (`region_bake.rs` 216-258) runs `drain_selective` on z_b over the region's flat H1 window before K5's rounds.
  - Rules: breach or fill each depression whose cut or fill depth is within 50 m; at most 12 rounds and a search of 2^20 cells.
  - Outlets: the window grid's (sea cells and the window edge). Neighbours: x/y arithmetic on `w`, `h` (`drain.rs` 77-84, `fn neighbours`; `drain_selective` itself at 178).
  - The regional field stays `z_s - z_b` of the undrained base.
- On the cube (G3-5, wave 1):
  1. Adjacency: `drain_selective`'s neighbour loop over `CubePatch` (GlobalCell neighbours across face edges, the folded halo). Corner cells have 7 neighbours.
  2. Area: the quadrature cell area (`a_geo`) for the volumes; the 50 m depth rules are unchanged.
  3. Outlets: the global sea mask (the connected component from sea outlets over cube adjacency), no window edge.
  4. Determinism and partitioning (CON-14, CON-30):
     - depressions in a global order (spill height, then GlobalCell id);
     - a breach path either bounded to a tile's halo or resolved through the seam-pair spill engine (G3-B). Today's breach is unbounded (`--breach-max-len 0`). The bound is D9's (G3-C); its value needs a row.
- Size: 9.1e8 H1 cells on the planet. Per tile with the spill engine across seams, not one array.
- `sel-post`: retired if the O3 rerun accepts the ablation, otherwise the same four points apply on z_s.

## Design note: the hooks as keyed stage configs (R24)
- Set once per process today:
  - `BASE_HOOK` and `POST_HOOK` (`region_bake.rs` 208-211, `OnceLock<Box<dyn Fn>>`);
  - `LEVEL_FLOOR` (`round.rs` 40, `OnceLock<Vec<f64>>`);
  - K5's atomics `K5_LTD`, `K5_KDT_SCALE`, `K5_UOK_SCALE` and `K5_DINF` (`region_bake.rs` 180-186).
- A second `install_s138` in one process is ignored without notice, because `OnceLock::set`'s error is dropped.
- They become values in `RegionSpec`:
  - `base_drain: Drain` and `post_drain: Drain`, with `Drain = Off | Uncapped | Capped { cap_m } | Selective { cut_cap_m, fill_cap_m, rounds, search } | SelectiveClamp { ... }`;
  - `k5: K5Knobs { route: D8 | Ltd | Dinf, kdt_scale, uok_scale }`;
  - the clamp's level floor computed inside the solve and passed to the rounds as an argument.
- Each config's canonical bytes enter the region's cache key, and the store manifest names them, so a store written under one config is refused under another.
- Two regions in one process, the 1-vs-2-region test (CON-30) and incremental against fresh (CON-29) then see one config.
- Tests:
  - two specs that differ only in the drain config give different keys;
  - one config on 1 and 2 regions is bit-equal;
  - no set-once global remains (a grep test).

## Addendum: #182, the graph's attributes (G3 risk R6; lead #182, #182a)
- Code: `spike/rg-prototype-spikes` 6ebc286f (three switches, off) and 993709cc (`--trunk-head-once` and `--strahler-junction` on by default, readings 45 and 46). Spike only.
- Rows: today's tip 7fe4e73d plus each switch; gate main cd84f36a; R1h's defaults row is the one above. `raw/s182-*`.

| Row | bars; failing | violations | agree 1 / 75 km2 | saddles1 vs S_KH / S0 | corner links 5 m | g_rb (orders) | stage time, s |
|---|---|---|---|---|---|---|---|
| R1h defaults | 10/11; violations | 102 | 92.73 / 98.26 | 340 / 1,950 | 260 | 7.66 (5) | - |
| R1h `--trunk-head-once` | 10/11 | 101 | 92.76 / 98.39 | 325 / 1,948 | 262 | 7.66 (5) | 853 |
| R1h `--strahler-junction` | 10/11 | 102 | 92.73 / 98.26 | 340 / 1,950 | 260 | **5.24 (7)** | 874 |
| R1h `--runoff-cell` | 10/11 | **40,969** | 92.78 / 98.53 | 322 / 1,945 | 278 | 7.66 (5) | 918 |
| R3a defaults | 10/11; agree 1 km2 | 0 | 88.89 / 99.21 | 0 / 28 | 0 | 5.36 (4) | 736 |
| R3a `--trunk-head-once` | 10/11 | 0 | 88.90 / 99.32 | 0 / 28 | 0 | 5.36 (4) | 774 |
| R3a `--strahler-junction` | 10/11 | 0 | 88.89 / 99.21 | 0 / 28 | 0 | **4.78 (6)** | 688 |
| M1 defaults | 9/11; violations, spectrum_steep | 1,306 | 92.19 / 95.99 | 3,215 / 9,398 | 4,009 | 7.01 (5) | 1,285 |
| M1 `--trunk-head-once` | 9/11 | 1,300 | 92.20 / 96.03 | 3,191 / 9,387 | 4,011 | 7.01 (5) | 1,242 |
| M1 `--strahler-junction` | 9/11 | 1,306 | 92.19 / 95.99 | 3,215 / 9,398 | 4,009 | **4.34 (7)** | 1,956 |

- Stage times are loaded runs under the heavy lock, not solo pairs: no saving is claimed. Peak memory 11.6-12.3 GiB on every row.
- `--trunk-head-once`: a head trunk reach starts from its own L inflow, not the larger of that and its H1 seed.
  - It passes the ablation rule on all three windows. Kept shares move by 0.01 points at most.
  - Trunk reach areas fall where they change (`raw/s182-head-areas.txt`, `head182.py`):

    | Window | reaches changed | last vertex, ratio p10 / p50 / p90 | first vertex, ratio p50 |
    |---|---|---|---|
    | R1h | 7,457 | 0.732 / 0.870 / 0.956 | 0.817 |
    | R3a | 16,574 | 0.698 / 0.859 / 0.959 | 0.798 |
    | M1 | 17,246 | 0.746 / 0.865 / 0.958 | 0.825 |

  - `catchment_p90` does not move (R1h 3.626, R3a 6.960, M1 1.441): the gate builds it from the routed network, not from the graph's attributes.
- `--strahler-junction`: the trunks' orders are updated at every `trib_junction` (a tributary of equal order raises the order by one; a higher one sets it).
  - Attributes only: every surface column is bit-identical to the defaults.
  - g_rb against the GLO-30 R_b 1 km2 bar (`spikes/river-suite/results/network-bar.md`):

    | Window | bar | defaults | `--strahler-junction` | routed network |
    |---|---|---|---|---|
    | R1h | 4.14 to 4.42 | 7.66, above | 5.24, above | 4.52 |
    | R3a | 4.22 to 5.97 | 5.36, inside | 4.78, inside | 5.38 |
    | M1 | 4.11 to 5.37 | 7.01, above | 4.34, inside | 4.18 |

- `--runoff-cell`: no runoff field exists at L; the form reads the H1 field at each L cell's parent.
  - The form as built scales `a_run` per vertex by the catchment's mean runoff from a separate L routing. The per-vertex ratios do not add up at nodes: 40,866 conservation breaks on R1h, and rule 1 fails.
  - The form that conserves is a runoff-weighted accumulation through the graph's own topology (KG-trib's `extract` and `accumulate_trunks`). It goes to crusader after #187 (lead #182a). R3a and M1 were not run.

## Visuals
- None: gate rows, attribute tables and a design note. The chain's surfaces were not rendered (#182's switches are judged on gate columns).
