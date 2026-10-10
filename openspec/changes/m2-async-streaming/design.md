## Context

Report §4 (scheduling), §10 (injected time and I/O), §15 (M2 minimal async, M4 hardening); CON-04, CON-22.

## Decisions

- **One synchronous wait only:** the six level-0 tiles load at start-up (`load_roots_blocking`) so every wanted tile always has an ancestor; everything else is asynchronous and `begin_frame` never waits.
- **Parent-first by construction:** every wanted tile requests its whole ancestor chain, with priority `p - depth * 1e12`, so ancestors sort before descendants; a test asserts first-dispatch order over a dive.
- **Cancellation:** queued requests nobody needs are removed; in-flight ones get a token that skips work not yet started (a started job finishes and its valid result is kept). A cancelled job is not a failure.
- **Determinism:** `ManualSpawner` runs queued jobs in submission order only when the test says so; the dispatch log is identical across runs (tested).
- **Priorities** are the caller's numbers (node distance for terrain); mapping screen-space error and prefetch into priorities is M4.
- **Limits:** capacity counts tiles, not bytes; no disk or network source, no velocity prefetch; the dive test uses an integer tile source (the real mesh source is exercised by the app test).

## Risks

- Re-dispatch after cancellation shows up as duplicate dispatches (9 in the dive); acceptable, but M4 should avoid cancelling work that is nearly done.
