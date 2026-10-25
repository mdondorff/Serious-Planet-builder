Opened: 2026-10-25
Question: Which numbers marked **r** (recalled) in the SPIKE-40 table survive a check against sources that were fetched?
Why it matters: the proposed M8 hydrology targets (`spikes/hydrology-targets/results/results.md`) drive the channel and valley laws that replace GEN-057's ad hoc depth law.
Time box: about 1 h, web sources only (opus-planner, 2026-10-25)
Success criteria: each **r** number confirmed, corrected or marked unconfirmed, each with a citation whose text was fetched
Fallback if it fails: mark the number unconfirmed and say what would settle it
Code location: none
Closed: 2026-10-25

## Result per number
"Fetched" means the PDF or page text was read in this session. "Secondary" means the fetched text quotes another paper.

| Number (SPIKE-40) | Status | Evidence (fetched) |
|---|---|---|
| Moody and Troutman 2002: w = 7.2 Q^0.5, d = 0.27 Q^0.3 | **confirmed, secondary**: w = 7.2 Q^(0.5 ± 0.02), d = 0.27 Q^(0.3 ± 0.01), bankfull top width and depth, Q in m3/s | Modaresi Rad, Johnson, Ghahremani, Coll, Frazier 2024, JGR Machine Learning and Computation 10.1029/2024JH000173, Eqs. 1 and 2, [NOAA copy](https://repository.library.noaa.gov/view/noaa/68731/noaa_68731_DS1.pdf). The original paper (ESPL 27, 1251, [USGS page](https://pubs.usgs.gov/publication/70024149)) was not read, its page gives the abstract only. |
| Width exponent about 0.5, depth exponent 0.3 to 0.5 | **corrected for depth**: a gauge fit on the Mississippi gives a depth exponent of 0.47 | Miller, Pavelsky, Allen 2014, HESS 18, 4883, [PDF](https://hess.copernicus.org/articles/18/4883/2014/hess-18-4883-2014.pdf): w = 16.0 Q^0.43 (r2 0.62), w = 13.4 Q^0.46 (r2 0.64), d = 0.18 Q^0.47 (r2 0.73); the paper cites exponents of about 0.5 for width and 0.4 for depth from Leopold and Maddock 1953 and Moody and Troutman 2002 |
| Bankfull depth 1 m at 1e4 km2, 4 m at 1e6, 9 m at 9.5e6 (Q = 0.01 A) | **range widened**: Moody and Troutman 1.1 m at Q = 100, 4.3 m at 1e4, 8.5 m at 1e5; Miller et al. 1.6 m at 100, 14 m at 1e4 (their data end near 2e4 m3/s) | computed from the two fits above; depth at large Q differs by a factor 3 between them |
| Bankfull width 70 m at 1e4 km2, 2.2 km at 9.5e6 km2 | **confirmed within a factor 1.6**: Moody and Troutman 72 m at Q = 100 and 2.3 km at 1e5; Miller et al. 111 to 116 m and 2.3 to 2.7 km | same sources |
| Runoff Q = 0.01 A m3/s | **unconfirmed** (not fetched) | no source read |
| Valley width as a function of drainage area | **confirmed in form, exponent corrected**: W = k A^w, w typically 0.4 to 0.5, possible range 0.03 to 0.9 | Turowski, Bufe, Tofelde 2024, ESurf 12, 493, "A physics-based model for fluvial valley width", [page](https://esurf.copernicus.org/articles/12/493/2024/) |
| Valley floor 5 to 20 bankfull widths | **not supported, corrected to about 1 to 5**: the same model gives an unconfined channel-belt width W0 = k0 h + Wc with k0 "a few hundred" (321 in the experiments); with w/d = 27 Q^0.2 from the two fits above, 300 depths is 4.5 channel widths at Q = 100 and 1.1 at Q = 1e5. Fitted field values: Himalaya W0 = 266 m, Wc = 63 m (ratio 4.2); Tian Shan north 495 and 243 m (ratio 2.0); Tian Shan south 971 and 22 m (ratio 44) | same paper; these are bedrock valleys of mountain ranges, not the wide floodplains of large lowland rivers (no source read for those) |
| Valley incision 5 to 50 m lowland, 100 m to over 1 km mountain | **unconfirmed**: no fetched source states a depth range; Turowski et al. report no incision depths | the search snippets (Mississippi valley fill 5 to over 60 m to bedrock, thalweg 13 to 20 m under the floodplain; Yarlung Tsangpo buried canyon under over 500 m of fill) were not read at page level |
| ks 20 to 300 m^0.9, steep above 150 | **upper end confirmed, lower end unconfirmed**: "ks > 150" is called steep, and ksn against erosion rate fits reach about 350 m^0.9 at 1500 m/Ma (San Gabriel Mountains) | Whipple, DiBiase, Rossi, Adams, "Topography, Climate, Erosion Rates and Tectonics" slides, [PDF](https://geodynamics.org/resources/836/download/topo_erosion_matter_whipple.pdf) (S = ksn A^-0.45 is the reference form there; the units of ksn are not printed in the extracted text) |
| Concavity 0.4 to 0.7, reference 0.45 | **confirmed for the reference and one case**: best-fit 0.45 (0.4 to 0.5 across methods) in the Loess Plateau, the most uniform landscape the authors examined; reference concavities 0.4 to 0.5 give an accurate relative steepness there | Mudd, Clubb, Gailleton, Hurst 2018, ESurf 6, 505, "How concave are river channels?", [PDF](https://esurf.copernicus.org/articles/6/505/2018/esurf-6-505-2018.pdf) |
| Horton Rb 3 to 5, Rl 1.5 to 3.5; Dd 1 to 5 /km; Hack c 1.4 | **unconfirmed** (not fetched; the earlier search confirmed Hack's form and h 0.57 to 0.6 only) | none |

## Consequences for the SPIKE-40 table
- Channel law: use w = 7.2 Q^0.5 and a depth between 0.27 Q^0.3 and 0.18 Q^0.47; the depth target at large Q is 8 to 40 m, not 9 m. Width target stays within a factor 2.
- Valley width: the factor is 1 to 5 bankfull widths for the models and field fits found, not 5 to 20. The change to the target is that a valley of a few channel widths is the lower bound; the missing piece is a source for wide lowland floodplains.
- At the lowland site (Q about 9.5e4 m3/s at 0.01 A) the channel alone is about 2.2 km wide, so the 64 m wide slot is wrong by the channel width, whatever the valley factor.
- Still open, needs a fetched source: valley incision depth by setting, the low end of ks, runoff per area, Horton ratios, drainage density.

## Visuals
Nothing to look at: a source check, no renders.
