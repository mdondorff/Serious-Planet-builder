"""SPIKE-119: the GLO-30 benchmark's preparation (provisional benchmark, not a gate).

A 1-degree DEM tile becomes a square grid of L cells (101.8 m), the same method as the river review (#1518, crusader's glo30_dump):
- Square grid: the pixel's ground size is scale_lat * 111,320 m north-south and scale_lon * 111,320 * cos(lat_c) m east-west, lat_c the tile's centre latitude.
  An L cell is the box mean of the whole pixels whose row lies in [floor(k * 101.8 / py), floor((k + 1) * 101.8 / py)) and whose column lies in the same span for px.
  Cells hold 3-4 pixels a side, alternating; the east-west scale is exact at lat_c and off by up to +/- tan(lat_c) * 0.0087 at the tile's north and south edge.
- No-data pixels are left out of the mean; a cell without any finite pixel becomes -100 m (sea, an outlet).
- Sea: GLO-30 sets the open sea to 0 m, so a cell at or below the tile's sea level (0 m on coastal tiles) is an outlet; inland tiles below 0 m (Death Valley) take a sea level under their lowest land.
- Pits are NOT filled or breached here: the gate measures the raw surface (its basins are the DEM's own pits, real or artefact), and it routes river cells on its own plain flood.
- 3DEP DTM tiles (USGS 1 arc-second, NAVD88) are cropped to the same 1-degree box (the 6-pixel overlap removed) and prepared the same way; the vertical datum (NAVD88 against GLO-30's EGM2008) differs by under about 1 m, noted not corrected.

Usage: dem_prep.py <tile.tif> <out.f64> [--box lat_s lat_n lon_w lon_e]
Prints PREP <out> <w> <h> and the counts: pixels, pixel spacing, no-data pixels, cells, pixels per cell, cells without data, cells at or below 0 m.
"""
import sys, math
import numpy as np
import rasterio

CELL = 101.8
M_PER_DEG = 111_320.0


def main():
    src, out = sys.argv[1], sys.argv[2]
    box = None
    if '--box' in sys.argv:
        k = sys.argv.index('--box')
        box = tuple(float(v) for v in sys.argv[k + 1:k + 5])
    with rasterio.open(src) as d:
        z = d.read(1).astype(np.float64)
        t = d.transform
        nod = d.nodata
        lon0, lat0, slon, slat = t.c, t.f, t.a, -t.e
    if nod is not None:
        z[z == nod] = np.nan
    z[z < -1000] = np.nan
    if box:
        lat_s, lat_n, lon_w, lon_e = box
        r0 = int(round((lat0 - lat_n) / slat))
        r1 = int(round((lat0 - lat_s) / slat))
        c0 = int(round((lon_w - lon0) / slon))
        c1 = int(round((lon_e - lon0) / slon))
        z = z[r0:r1, c0:c1]
        lat0 = lat_n
    hgt, wid = z.shape
    nodata_px = int(np.isnan(z).sum())
    latc = lat0 - 0.5 * hgt * slat
    py = slat * M_PER_DEG
    px = slon * M_PER_DEG * math.cos(math.radians(latc))
    h = int(hgt * py / CELL)
    w = int(wid * px / CELL)
    rows = [(int(k * CELL / py), int((k + 1) * CELL / py)) for k in range(h)]
    cols = [(int(k * CELL / px), int((k + 1) * CELL / px)) for k in range(w)]
    # Box means by cumulative sums over finite pixels.
    fin = np.isfinite(z)
    zz = np.where(fin, z, 0.0)
    S = np.zeros((hgt + 1, wid + 1)); S[1:, 1:] = zz.cumsum(0).cumsum(1)
    K = np.zeros((hgt + 1, wid + 1)); K[1:, 1:] = fin.cumsum(0).cumsum(1)
    r0 = np.array([r[0] for r in rows]); r1 = np.minimum(np.array([r[1] for r in rows]), hgt)
    c0 = np.array([c[0] for c in cols]); c1 = np.minimum(np.array([c[1] for c in cols]), wid)
    s = S[r1][:, c1] - S[r0][:, c1] - S[r1][:, c0] + S[r0][:, c0]
    k = K[r1][:, c1] - K[r0][:, c1] - K[r1][:, c0] + K[r0][:, c0]
    out_z = np.where(k > 0, s / np.maximum(k, 1), -100.0)
    out_z.astype('<f8').tofile(out)
    ppc = (r1 - r0).mean(), (c1 - c0).mean()
    print(f'PREP {out} {w} {h}')
    print(f'COUNTS src {src}: {wid} x {hgt} pixels ({slon * 3600:.3f}" x {slat * 3600:.3f}", {px:.2f} x {py:.2f} m at lat {latc:.3f}), '
          f'no-data pixels {nodata_px}; cells {w} x {h} of {CELL} m, pixels a cell {ppc[1]:.2f} x {ppc[0]:.2f}; '
          f'cells without data {int((k == 0).sum())}; cells at or below 0 m {int((out_z <= 0).sum())}; land z p5/p50/p95 '
          f'{np.percentile(out_z[out_z > 0], 5) if (out_z > 0).any() else float("nan"):.0f}/'
          f'{np.percentile(out_z[out_z > 0], 50) if (out_z > 0).any() else float("nan"):.0f}/'
          f'{np.percentile(out_z[out_z > 0], 95) if (out_z > 0).any() else float("nan"):.0f} m; min {out_z.min():.1f} m')


if __name__ == '__main__':
    main()
