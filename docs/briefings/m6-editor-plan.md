# M6 editor plan (opus-planner, 2026-10-19)

Scope: the owner's editor MVP requirements (`docs/status.md`, "Editor MVP requirements"), the M6 exit criteria (edit visible within 1 s; undo/redo over 1,000 operations; item files merge cleanly in git), SPIKE-13 (edit-to-visible) and SPIKE-14 (egui in the frame). Out of scope: global-scale authoring (continental edits, global re-bake, generator editor UI).

## Lanes
- **Main lane (sonnet-explorer):** generation, streaming and render changes; the only lane that moves goldens.
- **Support lane (sonnet-crusader):** UI, input, tooling; moves no goldens (the editor UI is drawn only in the editor mode).
- Never two open changes that move the same goldens. A change with a part that does not depend on its predecessor starts that part at once.

## Steps

| # | Change | Lane | Depends on | Content | Done when |
|---|---|---|---|---|---|
| E0 | `authored-items-adr` | main | - | One ADR deciding ADR 0021 points 2 (reach), 5 (region summaries), 6 (authored stages) and 8 (selection-cache bounds), plus the item file schema (TOML, 128-bit ULID ids, ADR 0007) and whether the command log is persisted (ADR 0007 deferred it to M6). The item file format is hard to reverse: the owner confirms it. | ADR accepted |
| E1 | `edit-invalidation` | main | E0 | SPIKE-13 variant D: footprint find of affected tiles, refresh with the old tile drawn until the new one is resident (stale while revalidating), an edit event that forces a plan, a request class for edited tiles. M3 gate condition 2 (per-tile node set id, the declared-versus-recorded reads contract test, a tile-level GEN-017 test) lands here. | exact <= 300 ms up to 150 m stamps, <= 1 s up to 500 m or at 1 m altitude (perf window) |
| E2 | `stamp-stage` | main | E0 (E1 in parallel) | The first authored stage: stamps (crater, hill, plateau) as relative deltas with a footprint, keyed per ADR 0006 and E0; numerics-reviewer. | stamps render, deterministic, keys tested |
| E3 | `edit-preview` | main | E1, E2 | The stamp's displacement in the vertex stage over resident tiles until the exact tiles arrive. | preview <= 16 ms |
| E4 | `edit-commands` | main | E0 | The command layer over the world definition: put, change, delete item; undo and redo over 1,000 operations; items written as files. | exit criterion 2; files merge (tier A) |
| U1 | `editor-ui-shell` | support | - | egui `=0.36.2` (SPIKE-14): one `ui` node after `tonemap`, egui first with the pointer latched at press, fonts under an allowed licence; drawn only in the editor mode. | no golden moves; UI smoke |
| U2 | `surface-picking` | support | - | Mouse ray to the planet surface (CPU, f64, against the height field), hover and selection of an item by its footprint. | tier A ray tests |
| U3 | `editor-panels` | support | U1, U2, E4 | Map first: hover outline and name, selection with handles, inspector bound to E4 commands, search, the list of edits near the camera; layer list and graph as secondary views. | the mock's flows work on stamps |
| U4 | `globe-view` | support | U1 | The globe overview with item markers; click flies there. | |
| E5 | `lines-and-regions` | main | E2, E4 | Splines (ridge, carved channel) and regions (mask, surface type stored, name); the debug overlay tints regions by surface type. | |
| E6 | `brush` | main | E5 | Raise, lower, smooth, flatten as brush items. | |
| G | M6 gate | - | all | Exit criteria measured; the owner's review of the editor. | |

## Start
- E0 now, in parallel with the end of phase 0 (#304, the water-motion-gap fix, the removal perf window, M5 housekeeping 10b with the first process review).
- U1 and U2 now in the support lane.
