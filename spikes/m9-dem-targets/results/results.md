Opened: 2026-10-27
Question: Which open DEM should M9's hill-terrain realism be checked against, which landform statistics does the literature give, and which of them can the generator be compared on today?
Why it matters: M9 (hill terrain, fine tier) needs measurable targets; SPIKE-40 and SPIKE-45 showed that unchecked numbers become wrong specs.
Time box: half a day of agent work (desk research; no downloads, no heavy runs)
Success criteria: a source table with one recommendation; a per-landform target table with every number marked fetched (**v**) or recalled (**r**); a list of what the store can be compared on and with which tool
Fallback if it fails: the source table and the method only; the target numbers are computed from sample tiles after the owner's go
Code location: none (desk research)
Closed: 2026-10-27

## Method
- Web pages fetched on 2026-10-27 with the fetch tool (AWS registry, Copernicus readme, GEE catalog pages, NASA Earthdata, JAXA, ESSD discussion, ESurf). A PDF of the Copernicus handbook and the ESSD paper could not be read (size, binary); their numbers are not used.
- **v**: read on a fetched page or in a search result snippet on 2026-10-27 (snippet-only claims say so). **r**: recalled, not fetched; check before any number enters a spec.
- Ours: read from the repo (specs, `spikes/hydrology-targets/results/results.md`, `crates/app/src/worldfield.rs`); nothing was run.

## 1. Candidate DEMs
| DEM | Resolution | Coverage | Surface | Licence and attribution | Access | Size |
|---|---|---|---|---|---|---|
| Copernicus GLO-30 Public | 1 arcsec (about 30 m N-S; E-W spacing grows with latitude: 1.5x from 50, 2x from 60, 3x from 70) **v** | global land, ocean has no tiles; some countries' tiles withheld from the public set **v** | DSM (canopy, buildings) **v**; absolute error under 4 m LE90, relative under 2 m on slopes up to 20 %, under 4 m above (snippet) **v** | free for the general public under the Copernicus licence **v**; a notice is required on distribution and a different one on modified data **v**; the grant clauses themselves were not read | AWS `copernicus-dem-30m`, eu-central-1, anonymous (`aws s3 ... --no-sign-request`), STAC endpoint **v** | COG tiles of 1 degree, 3600 x 3600 (source 3601), 1024 blocks **v**; about 52 MB per tile as raw f32 (computed); compressed size not found |
| Copernicus GLO-90 | 3 arcsec | global **v** | DSM | same licence **v** | AWS `copernicus-dem-90m` **v** | 1200 x 1200 (source 1201) **v**; about 5.8 MB raw f32 (computed) |
| FABDEM V1-2 | 1 arcsec | global **v** | GLO-30 with buildings and trees removed by machine learning **v** | CC BY-NC-SA 4.0 per the GEE catalog (non-commercial, share-alike) plus the WorldDEM-30 notice **v**; the Bristol portal names a different licence in a search snippet **v**: unresolved | Bristol repository, GeoTIFF 1 degree tiles in 10 x 10 degree zips; GEE community catalog **v** | not found |
| NASADEM | 1 arcsec | 60 N to 56 S, about 80 % of land **v** | SRTM reprocessed, geoid heights, void-filled **v** | "openly shared, without restriction" under EOSDIS data use and citation guidance, citation required (DOI 10.5067/MEASURES/NASADEM/NASADEM_HGT.001) **v** | Earthdata, cloud-enabled; whether login is required was not stated **v** | not found |
| ALOS AW3D30 | 1 arcsec | global land | DSM | free under JAXA "Terms for Use"; JAXA asks for reports of published results **v**; redistribution and derived-data terms were not found | JAXA download, registration (not checked) **r** | not found |
| SRTM 1 and 3 arcsec | 30 m (US only) and 90 m | 60 N to 56 S | DSM, radar | public domain, no restriction **r** | USGS EarthExplorer, mirrors **r** | 1 degree tiles of about 26 MB (3601 x 3601 i16) **r** |
| ETOPO 2022 and GEBCO 2024 | 15 to 60 arcsec (about 0.5 to 1.9 km) | global including bathymetry | mixed, bare earth over land **r** | NOAA public domain; GEBCO free with attribution **r** | NOAA and GEBCO downloads, one file of a few hundred MB to GB **r** | whole planet in one file **r** |

Findings:
- Only the Copernicus pair has a fetched, free, anonymous, cloud-optimised path (no account, per-tile reads over HTTP range requests).
- GLO-30 is a surface model: forest adds 5 to 30 m of canopy to the heights. Over forested hills it overstates roughness at 30 m (the slope between a canopy top and a clearing). It is the right reference for open ground (grass, scrub, desert, bare rock) and a biased one under forest.
- FABDEM removes that bias but its licence is non-commercial and share-alike: usable for internal statistics, not as a committed fixture or as the basis of anything shipped. The licence conflict is open.
- Statistics (numbers per landform) are not covered by the data licence in any reading; copying tile crops into the repo is. Commit statistics and tile identifiers, never tiles.

**Recommendation:** Copernicus GLO-30 for sample tiles (and GLO-90 for the 90 m numbers, rather than resampling: the product's own 90 m is an average of the same radar, so check both once); NASADEM as the independent second DEM for a one-tile cross-check of the DSM bias; FABDEM only for a forested-hills tile, locally, nothing committed.

Selecting tiles: pick by the same relief metric as the classes below (not by name), 5 to 10 tiles per class, open ground first. Candidate regions are recalled, not checked: plains (Great Plains USA, Pampas, Hungarian Plain), rolling hills (Palouse, Flint Hills, Swabian Alb), hill country (Scottish Southern Uplands, Appalachian Plateau, Massif Central), low mountains (Vosges, Black Forest, Appalachians), high mountains (Alps, Southern Alps NZ, Sierra Nevada).

## 2. Target statistics per landform
Window and class definitions found (**v**):
- Esri World Local Relief (a 6 km radius focal range): 0 to 30 m plains, 301 to 900 m low mountains, over 900 m high mountains; the classes between 30 and 300 m were not in the fetched text (snippet) **v**.
- Meybeck et al. 2001: plains are relief roughness (relief over window length) under 5 per mille (snippet) **v**.
- Engelen and Wen (via a LEDDRIS page): level land dominant slope 0 to 8 % with relief under 50 m; sloping land 8 to 30 % with relief usually over 50 m; steep land over 30 % **v** (snippet).
- Yang et al. 2025 (ESSD 17, 4331): a global three-level typology of 26 landform classes from a 1 arcsec DEM, thresholds called empirical and subjective by a reviewer; data on Zenodo in 10 degree latitude bands, CC BY 4.0 **v** (class thresholds and per-class statistics not read; the final PDF exceeded the fetch size).

Published numbers found (**v**):
| Quantity | Value | Source |
|---|---|---|
| Tile-mean slope over 236 DEMIX tiles, Copernicus DEM | median 10.4 %; median relief 356.1 m (per tile, relief window not read) | arXiv 2302.08425 (snippet) |
| Scale effect | slope rises as the DEM spacing falls (qualitative; factor not found) | USGS-related study (snippet) |
| Hilltop span and average gradient, three Oregon sites, 0.91 m lidar | 149 to 274 m and 0.11 to 0.23 | Struble and Roering 2021 |
| Hillslope gradients, Oregon Coast Range | many hillslopes 0.8 to 1.1 (tan), planarity rises toward 1.2 | Roering et al. 1999, second-hand |
| Mean hillslope length | 1 / (2 Dd), derived for an idealised valley, widely used | EGU 2025 abstract (snippet) |
| Drainage density | humid 1 to 5 per km, arid 0.1 to 1; depends on the head threshold; correlated positively with slope where overland flow dominates, negatively where landsliding does | SPIKE-40/41 (**v** in part), Talling and Sowter 1999 (snippet) |
| Channel-head area against slope | steeper humid ground has smaller source areas; landslide-initiated heads follow a slope-area law | Montgomery and Dietrich 1988 (abstract) |

**Not found:** no fetched source gives a per-landform table of slope median and p90 at 30 m and 90 m, nor local relief per 1 and 5 km window per class, nor hypsometric integrals per class. These are the numbers the task wants; they are not published in a form a page fetch returns, and they depend on the window and the DEM. They must be computed from the tiles of section 1.

Expected ranges, **r** (recalled, to be replaced by the computed table; use only to size the tool's histogram bins):
| Landform | Median slope 30 m | p90 slope 30 m | Relief 1 km window | Relief 5 km window | Dd per km (head 0.1 to 1 km2) | Hypsometric integral of a 10 km basin |
|---|---|---|---|---|---|---|
| Plains | 0.3 to 1 deg | 2 to 4 deg | under 30 m | under 60 m | 0.5 to 2 (if channelled at all) | not meaningful |
| Rolling hills | 3 to 6 deg | 10 to 14 deg | 30 to 80 m | 100 to 250 m | 1.5 to 4 | 0.4 to 0.6 |
| Hill country | 8 to 12 deg | 18 to 25 deg | 80 to 200 m | 250 to 600 m | 2 to 5 | 0.35 to 0.55 |
| Low mountains | 12 to 18 deg | 25 to 32 deg | 200 to 500 m | 600 to 1,200 m | 2 to 5 | 0.3 to 0.5 |
| High mountains | 20 to 28 deg | 35 to 42 deg | 500 to 1,500 m | over 1,200 m | 2 to 6 | 0.3 to 0.6 |
- 90 m slopes recalled at 0.6 to 0.8 of the 30 m values in hills (the direction is **v**, the factor is **r**); hilltop gradients of 0.11 to 0.23 (6 to 13 deg) at 1 m fit the hill-country lower half and say the true slope of a hillslope is steeper than any 30 m DEM shows.
- The Strahler hypsometric classes (over 0.6 young, 0.35 to 0.6 mature, under 0.35 monadnock) are **r**.

## 3. What the generator can be compared on today
| Measure | Where ours lives | Comparable now? | Tool |
|---|---|---|---|
| Slope distribution at 30 and 90 m | chunk stage, 8 m cells, 608 x 608 core (4.9 km), 3 km halo | yes, after aggregating 8 m cells to 32 m and 88 m (4 x 4 and 11 x 11 means) and taking slope with the 8-neighbour true-distance rule of `generators::drivers`; the DEM side uses the same windows after projection to a local metric grid | none; a new read-only example over `WorldTerrain` chunk rasters (as `chunk_incision.rs` does) and a small GeoTIFF reader for the tiles |
| Local relief at 1 km | the same chunk core | yes (4.9 km core holds a grid of 1 km windows) | the same example |
| Local relief at 5 km | 16384 bake, 611 m cells (40,000 km / (4 x 16384)), an 8 x 8 window | yes, but the bake's heights are the input before the coupled run's compression, and the DEM must be aggregated to 611 m to match | the same example on the bake reader; DEM side: GLO-90 averaged 7 x 7 |
| Slope distribution at 611 m | 16384 bake | yes against the DEM aggregated to 611 m (not against the 30 m numbers) | same |
| Drainage density | chunk threshold network (SPIKE-43: 1.4 to 2.0 km/km2 at head area 0.3 km2) | yes, with the head threshold stated on both sides | SPIKE-43's threshold network on the DEM side (priority flood and D8 on the tile) |
| Hypsometry | 16384 bake per basin or region | yes; basin outlines need the same drainage rule | same |
| Receivers, areas, Hack, Horton | stored coupled run, 2048 lattice | done in SPIKE-40; its heights are compressed against the base (SPIKE-40 `run_vs_base`; SPIKE-45), so slopes from it are the run's, not the drawn ground's | none new |

Landform assignment in our world: classify each chunk or bake cell by the DEM's own relief metric (Esri 6 km radius range), then compare within a class. A site-by-name comparison (a Scottish hill against our "hills") has no meaning on a synthetic planet.

Caveats:
- The generator makes its own world; real terrain statistics are a range, not a target to hit. The check is "inside the real range per class", not "equal".
- 8 m slopes are far steeper than 30 m slopes on the same ground. Only aggregated numbers are comparable; the aggregation recipe is part of the comparison.
- A 3 km halo makes the chunk's 4.9 km core too small for 5 km windows; use the bake for those.
- DSM canopy raises DEM roughness under forest; compare against open-ground tiles or FABDEM first.

## Recommendation and next step
1. Source: Copernicus GLO-30 (AWS, anonymous), GLO-90 for the 90 m numbers, NASADEM for one cross-check tile. Downloads need the owner's go: about 6 classes x 5 tiles x (about 20 to 30 MB compressed, **r**) = under 1 GB.
2. Targets: take the per-landform table of section 2 from computation, not from the recalled ranges above.
3. Tool (one change, M9 prep): a read-only `terrain_stats` example or xtask that takes a raster (a chunk, the bake, or a GeoTIFF tile) and a window recipe and prints slope percentiles at 30 and 90 m, relief at 1 and 5 km, and hypsometry; it needs a GeoTIFF reader dependency (`tiff` crate) in the example only. Size: about 400 lines.

## Visuals
Nothing to look at: tables of sources and statistics only, no renders.
