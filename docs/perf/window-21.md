# Perf window 21 (the atmosphere camera march), 2026-10-20

Rasierklinge, RTX 3080 Ti Laptop, release builds, 2560x1440. Coordinator opus-planner, run by sonnet-explorer. Two binaries: `change/atmosphere-camera-march` at 29982b2 + the blend band (6 to 10 km; 8 steps by day, 12 in twilight) and main at 80e1418 (the volume only). The lit perf scene (`test-render --perf --scene lit`, views ground-1m, aerial-20km, orbit) with GPU timestamps, the 16384-cell store, the harness directly (the binaries differ); the twilight runs use `--look docs/perf/window-21/twilight.look.toml` (the sun at a zenith cosine of 0.03 at the aerial view's camera). Order: branch day, main day, branch twilight, main twilight, branch day, main day.

## Verdict

- **Part 2, the half-resolution haze (the pass `atmosphere-haze`, a compute pass before the composite inside its timed span; a depth-aware upsample): within the 1 ms of ADR 0042.** Aerial 20 km, atmosphere median GPU ms: 0.629 and 0.600 (day), 0.761 (twilight), against 0.246 with the volume (main, same session). The composite span: 0.520 and 0.496 by day, 0.644 in twilight, against 0.142. Ground 1 m 0.256 ms (main 0.214: the dispatch of the haze pass when the camera is below 6 km), orbit 0.71 ms (main 0.69). The runs held 1,252 to 1,335 MHz at the measurement (the floor is 1,400), so the harness marks them 'ok at a low clock': the time is overstated, the verdict `ok` stands.
- Part 1 below (the per-pixel march) was over the budget and is kept for the record.

- Preflight OK on the first run (single 46.3 to 50.0 ms, limit 52 ms; parallel median 88.2 to 98.7 ms at 18 threads, limit 112 ms); the readiness gate was ok in every run; no `--machine-ready`.
- **Over the 1 ms of ADR 0042 on the aerial 20 km view:** the atmosphere is 1.12 and 1.15 ms by day and 1.65 ms in twilight with the march, against 0.25 and 0.26 ms with the volume. The composite pass carries it: 1.01 to 1.04 ms by day and 1.53 ms in twilight against 0.14 to 0.15 ms.
- The branch runs of the aerial view held a clock below the harness floor of 1,400 MHz (1,245 to 1,387 MHz after the 30 s warm-up cap; 71 to 72 C in twilight), so the harness marks its GPU verdicts not valid (it overstates the time; by the clock ratio the day figure is about 0.95 ms and the twilight figure about 1.4 ms at 1,500 MHz, an estimate). The main runs held the floor.
- The other views are unchanged: ground 1 m 0.21 to 0.23 ms and orbit 0.68 to 0.69 ms in every run (the march is not used there).

## Part 2: aerial 20 km with the half-resolution haze, GPU ms per pass (median)

| Run | Atmosphere | Sky composite (the haze pass + the composite) | Aerial perspective | Sky view | Frame (waited) median | Clock at the measurement |
|---|---|---|---|---|---|---|
| haze, day | 0.629 | 0.520 | 0.070 | 0.039 | 2.711 | 1,282 MHz |
| haze, day (repeat) | 0.600 | 0.496 | 0.067 | 0.038 | 2.784 | 1,335 MHz |
| haze, twilight | 0.761 | 0.644 | 0.076 | 0.042 | 2.907 | 1,252 MHz |
| main, day (control) | 0.246 | 0.142 | 0.067 | 0.038 | 2.423 | 1,417 MHz |

The cost of the march over the volume: about 0.35 to 0.38 ms by day and 0.5 ms in twilight. The second preflight (`preflight2.md`) was OK first time. The files `haze-day.txt`, `haze-twilight.txt`, `haze-day2.txt` and `main-day3.txt` hold the complete output.

## Part 1: aerial 20 km with the per-pixel march, GPU ms per pass (median)

| Run | Atmosphere | Sky composite | Aerial perspective | Sky view | Frame (waited) median | Clock at the measurement |
|---|---|---|---|---|---|---|
| branch, day | 1.122 | 1.013 | 0.070 | 0.039 | 3.355 | 1,282 MHz (below the floor) |
| branch, day (repeat) | 1.147 | 1.036 | 0.071 | 0.040 | 3.309 | 1,275 MHz (below the floor) |
| branch, twilight | 1.654 | 1.531 | 0.080 | 0.044 | 4.001 | 1,252 MHz (below the floor) |
| main, day | 0.251 | 0.146 | 0.066 | 0.037 | 2.286 | 1,440 MHz |
| main, day (repeat) | 0.249 | 0.143 | 0.067 | 0.038 | 2.269 | 1,402 MHz |
| main, twilight | 0.261 | 0.145 | 0.074 | 0.041 | 2.346 | 1,327 MHz |

The cost of the march over the volume lookup, measured: about 0.87 to 0.89 ms by day, 1.39 ms in twilight (both at a low clock).

## Not measured

- The default resolution of the viewer (the lit scene runs at 1440p only; the 1440p numbers are the ones against the budget).
- A valid GPU verdict of the branch: the harness refuses it below the clock floor; a cooler run or the half-resolution haze below decides.

## Files

`docs/perf/window-21/*.txt` hold the complete output of the six runs, `preflight.md` the machine report and `twilight.look.toml` the twilight look.
