# B6: the dark spot of frame-102263 is the coarse horizon map (owner test 2026-10-27)

Poses from the F9 bundles (the sun time is not in a bundle: 17.85 h is 2.0 degrees of elevation at frame-102263's site on the equinox, 17.93 h is 0.9).
`poses-on.toml` / `poses-off.toml` (frame-102263: 34.5 km above ground, lat -27.8368 lon -13.0795, heading 206.8, pitch 41 down), `poses2-*.toml` (frame-197238:
50 km, lat -40.8635 lon 9.9876, heading 325.6, pitch 24.1 down); `coarse_horizon = false` draws without the coarse map (the chunk horizons stay).

| Frame, sun | Pixels differing by more than 20 (coarse on against off) |
|---|---|
| 102263, 0.9 degrees | 8.0 % (mean abs difference 10.3) |
| 102263, 2.0 degrees | 0.1 % |
| 197238, 1.7 degrees | 0.0 % |

- With the map on, a smooth dark blob 20 to 30 km wide covers lit crests on the left of the frame; off, the same crests are lit (`f102263-on/f102263-sun1.png`, `f102263-off/f102263-sun1.png`).
- The map is not stale: it loads for the current run, and along its march the run's heights and the base field the screen draws differ by at most about 190 m (probe below).
- The cause is the cell: one stored horizon per 19.5 km observer cell, the highest elevation angle of the run's heights (4.9 km steps, 50 km range) measured from the cell's mean height, applied to every point of the cell (GEN-045, REND-038, REND-043). The cells under the blob read 1.4 to 1.8 degrees toward the sun (a plateau 15 to 50 km sunward stands 3.9 to 4.0 km high over mean ground): a sun under that angle blanks the whole cell, lit crests included.
- Frame-197238 is not affected: its missing plateau shadow was the chunk horizon (the window ran no horizon job; #828).

The probe: `cargo run --release -p planet --example coarse_probe -- target/accept/global16384 -27.8368 -13.0795 206.8 40 270` prints the stored angle toward the sun for the 7 x 7 observer cells around the point 40 km ahead and, for the highest cell, the run's heights against the base field's along the march.
