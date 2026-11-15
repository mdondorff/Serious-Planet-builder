## Why
- Short jobs (a 10-25 min prepush) waited behind 30-60 min probes; timing and window runs still need the machine alone (lead, 2026-11-14, explorer #148; owner: "Please do").

## What changes
- BUILD-026: ordinary heavy jobs take one of `PLANET_HEAVY_SLOTS` slots (default 2): `heavy.lock` (slot 1, the single lock's name: an older xtask shares slot 1), `heavy.lock.1`, ...; a holder note per slot names its slot.
- `perf`, `gate` (but `--small`), `repro --window` and `heavy --exclusive <cmd>` wait until no slot is held and hold every slot; their notes say "exclusive".
- The ticket queue stays: the oldest live ticket tries (an ordinary one the first free slot, an exclusive one every slot), so a queued exclusive run blocks later ordinary ones and is not starved.
- The overdue warning names each overdue holder.

## The prepush GPU-tier trigger (checked, unchanged)
| Triggers local tier C today | Why it can move a golden |
|---|---|
| `crates/{render,frame,generators,core,streaming,world-def,cache,app,testkit}/` (not examples, benches, non-`gpu_` tests) | the renderer and what it draws from |
| `*.wgsl`, `tests/goldens/`, `Cargo.toml`, `Cargo.lock`, `.cargo/` | shaders, goldens, toolchain |

- A bake-kernel diff (`generators/src/bake/flood.rs`) can move a golden: `test-render` (crates/app/src/lib.rs, the river and lake scenes) runs K7, the lakes, the tiles and H1, whose floods and routes are `flood.rs`; `gpu_rivers` reads K7's types. Dropping `generators` or `bake/` from the trigger is not safe.
- Proposal (not built): run local tier C once per branch state: skip it when the last prepush of this branch ran tier C green and the diff since that commit touches no golden path. CI runs tier C on every push either way.

## Impact
xtask (heavy.rs, main.rs). Lock files, quiet flag, tickets and the child pass are compatible with older worktrees (they take slot 1 only).
