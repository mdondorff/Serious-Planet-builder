# Architecture and process review before M8 (opus-planner, 2026-10-23)

Owner's questions (2026-10-23): are features well compartmentalised, or do changes cross components often enough to hint at a weak layout? What should change in the development process? Data: SPIKE-36 (`spikes/architecture-review/results/co-change.md`), SPIKE-37 (`.../cohesion.md`), SPIKE-38 (`spikes/process-review/results/process-data.md`).

## 1. Architecture verdict
**The layering is sound; the weak spots sit inside `app` and `frame`, not between crates. No re-layout; six targeted moves before and during M8's first week.**

| Finding | Evidence | Reading |
|---|---|---|
| No upward dependency; GPU-free crates have no GPU crate in their tree | SPIKE-36 §5, SPIKE-37 §1 | CON-02 holds as built |
| 72 % of code PRs touch one or two crates; 14 % touch four or more | SPIKE-36 | normal for a vertical engine |
| `app` is in 58 % of code PRs; `app/window.rs` is the most-changed file (47) | SPIKE-36 | `app` is a catch-all, not only wiring |
| The terrain field is assembled in two crates (`generators::build_field`, `app/worldfield.rs:661-700`); the world's sources are an 843-line `app` struct | SPIKE-37 §2 | domain logic in the binary; M8's block field would add a third home |
| Item reach has five homes; angle, lat/lon, tangent basis and the radius are written 7 to 19 times; two inverse lat/lon functions disagree at the poles | SPIKE-37 §2 | missing core vocabulary; a latent bug at the poles |
| `HeightField`: one trait, 15+ methods, 13 implementations | SPIKE-37 §5 | every new capability (the level-aware height) touches 13 places |
| `app` + `frame` + `render` change together in 23 PRs | SPIKE-36 | partly expected (decide in frame, draw in render, wire in app); partly the catch-all above |
| `app` carries 5,457 lines of perf scenes and an 850-line editor state machine; `render` 649 lines of spike code | SPIKE-37 §3 | harness and editor logic in the product crate |

## 2. Refactors (from SPIKE-37's ranked list)
| # | Move | When | Lane |
|---|---|---|---|
| R1 | One GPU-free home for the field chain (base, block, stamps, water bound) and the world's sources, out of `app` | before M8-2 lands block fields | crusader |
| R2 | Split `HeightField` into value (with the level-aware height), key and bounds | first part of M8-2 (same code) | explorer |
| R3 | One function for "which tiles or blocks does an edit change", used by `frame/lod.rs` and `editor/feed.rs` | before M8-7 | crusader |
| R4 | Core vocabulary: angle, lat/lon and its pole-safe inverse, tangent basis, radius, smoothstep; replace the copies | first week | crusader |
| R5 | A channel abstraction in `cache/global_store.rs` | first part of M8-1 (same code) | crusader |
| R6 | XS cleanup: remove the spike code from `render`, drop `editor`'s unused `planet-render` dependency, refresh five stale crate docs, one prefill-depth constant | first day | crusader |
Later, with a trigger: `EditRuntime` into `editor` (when M8-7 edits blocks), perf scenes out of `app` (when `app` blocks a change), splitting `frame/lod.rs` and `render/terrain.rs`, streaming's three homes.

## 3. Process review
| Finding | Data (SPIKE-38) | Change | Cost |
|---|---|---|---|
| 40 % of code PRs need more than one CI run; most failures are `check` (fmt, clippy, layering) on push (49 runs) and tier-C goldens (106) | §2c, §2d, §4c | a new `cargo xtask hooks` installs `cargo xtask prepush` as each worktree's pre-push hook, so `check` cannot be skipped; tier-C lavapipe differences stay CI's (owner, 2026-10-19) | XS |
| The verifier is named in 66 % of code PR bodies in M4, 16 % in M6, 0 % in M7 | §4a | the PR body carries `Verifier: pass` and `Lane: <session>`; a new CI step fails a code PR without them | S |
| The lane is unknown for 313 of 421 change PRs | §1d | the `Lane:` line above | with the above |
| 45 % of PRs are docs-only, but they merge in about a minute (median 0.02 h); `docs/status.md` is touched by 250 PRs | §5b, §1c | none for docs PRs; status edits stay with the planner, coders change only their queue row | none |
| Spikes changed or set a plan in 8 of 27 cases and diagnosed 9 bugs; they cost hours | §6a | keep a dedicated spike lane | none |
| Code PR lead time median 0.31 h, p90 0.84 h; a typical green run 13 min | §1c, §4c | none | none |
| Idle sessions did not read their inbox; two sessions vanished from the owner's list | M7 report | the loop line naming only `docs/status.md`; Remote Control on in every session (done 2026-10-23) | done |

## 4. Lanes for M8 (owner's proposal, confirmed by the data)
| Lane | Role | First week |
|---|---|---|
| sonnet-explorer | main: the critical path | R2 inside M8-2 block field infrastructure, then M8-3 |
| sonnet-crusader | support: refactors and off-path steps | R6, R4, R1, then R5 inside M8-1 global coupled stage; R3 before M8-7 |
| sonnet-spikes | spikes only, on the planner's go | SPIKE-35 (A-net open items), then idle |
| opus-planner | plan, reviews, perf windows, owner decisions | — |
