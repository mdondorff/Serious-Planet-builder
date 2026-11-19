"""SPIKE-140 item 3: structure of mountain terrain at 100 m, GLO-30 mountain tiles against the M1 base (and S0), in 100 km squares.
Per square (1000 x 1000 cells at ~100 m; the tile resampled by block means, its x spacing scaled by cos(latitude)):
- relief: p50 and p95 of max-min in 1 km, 5 km and 25 km discs (squares) around sampled cells;
- the share of spectral power at wavelengths 10-50 km (the range scale) over 1-50 km;
- range coherence: mean local anisotropy of the gradient's structure tensor in 10 km blocks after a 2 km smoothing (1 = every block's ridges run one way, 0 = isotropic), and the
  consistency of the block directions over the square (length of the mean doubled-angle vector);
- main-valley spacing: the median distance between local minima of a 5 km smoothed profile along the square's rows and columns (a proxy for the spacing of the larger valleys).
Usage: python -I mtn167.py <out.txt> ; reads the tiles and the window bases named below."""
import sys
import numpy as np
import rasterio
from scipy.ndimage import uniform_filter, maximum_filter, minimum_filter, gaussian_filter, label as cc
from skimage.morphology import reconstruction

out = open(sys.argv[1], "w")
DEM = "C:/DEV/SeriousPlanetData/dem/glo30-samples/"
TILES = [("Alps N47E011", "Copernicus_DSM_COG_10_N47_00_E011_00_DEM.tif", 47.5), ("Himalaya N27E086", "Copernicus_DSM_COG_10_N27_00_E086_00_DEM.tif", 27.5),
         ("Appalachian N40W078", "Copernicus_DSM_COG_10_N40_00_W078_00_DEM.tif", 40.5), ("Scottish Highlands N57W005", "Copernicus_DSM_COG_10_N57_00_W005_00_DEM.tif", 57.5),
         ("Black Forest N48E008", "Copernicus_DSM_COG_10_N48_00_E008_00_DEM.tif", 48.5)]
DX = 100.0


def tile_square(path, lat):
    with rasterio.open(DEM + path) as src:
        z = src.read(1).astype(np.float64)
    # GLO-30: 1 arc second rows; columns at 1 arc second below 50 deg, 1.5 at 50-60 deg (the file's width says which)
    ny, nx = z.shape
    dy = 111320.0 / ny
    dx = 111320.0 * np.cos(np.radians(lat)) / nx
    fy, fx = max(1, int(round(DX / dy))), max(1, int(round(DX / dx)))
    h, w = (ny // fy) * fy, (nx // fx) * fx
    zb = z[:h, :w].reshape(h // fy, fy, w // fx, fx).mean(axis=(1, 3))
    return zb, (dx * fx, dy * fy)


def measures(z, cell, name):
    n = min(z.shape[0], z.shape[1], 1000)
    z = z[:n, :n]
    land = z > 0.5
    r = []
    cv5 = 0.0
    for km in (1, 5, 25):
        k = max(3, int(round(km * 1000 / cell)))
        rel = maximum_filter(z, size=k) - minimum_filter(z, size=k)
        s = rel[k // 2:n - k // 2:7, k // 2:n - k // 2:7]
        r.append((np.percentile(s, 50), np.percentile(s, 95)))
        if km == 5:
            # the 5 km relief averaged over 25 km blocks: how much it varies across the square (0 = the same everywhere)
            b25 = int(25e3 / cell)
            nb = n // b25
            blocks = rel[: nb * b25, : nb * b25].reshape(nb, b25, nb, b25).mean(axis=(1, 3))
            cv5 = float(blocks.std() / blocks.mean())
    # closed basins (the gate's rule at 100 m: fill raise > 0.5 m, >= 16 cells; outlets: the square's border and the sea)
    seed = np.full_like(z, z.max() + 1.0)
    edge = np.zeros(z.shape, bool)
    edge[0, :] = edge[-1, :] = edge[:, 0] = edge[:, -1] = True
    o = edge | ~land
    seed[o] = z[o]
    wet = (reconstruction(seed, z, method="erosion") - z) > 0.5
    lab, _ = cc(wet, structure=np.ones((3, 3)))
    big = np.bincount(lab.ravel()) >= 16
    big[0] = False
    closed = 100 * (big[lab] & land).sum() / max(land.sum(), 1)
    zz = z - z.mean()
    yy, xx = np.mgrid[0:n, 0:n]
    A = np.c_[xx.ravel(), yy.ravel(), np.ones(n * n)]
    c = np.linalg.lstsq(A[::13], zz.ravel()[::13], rcond=None)[0]
    zz = zz - (c[0] * xx + c[1] * yy + c[2])
    p = np.abs(np.fft.rfft2(zz * np.hanning(n)[:, None] * np.hanning(n)[None, :])) ** 2
    k = np.hypot(np.fft.fftfreq(n)[:, None], np.fft.rfftfreq(n)[None, :]) / cell  # cycles per metre
    lam = np.where(k > 0, 1 / np.maximum(k, 1e-12), np.inf)
    rng = p[(lam >= 10e3) & (lam <= 50e3)].sum() / p[(lam >= 1e3) & (lam <= 50e3)].sum()
    zs = gaussian_filter(z, 2000 / cell / 2.355)
    gy, gx = np.gradient(zs)
    b = int(10e3 / cell)
    nb = n // b
    a_loc, vecs = [], []
    for i in range(nb):
        for j in range(nb):
            sx, sy = gx[i * b:(i + 1) * b, j * b:(j + 1) * b], gy[i * b:(i + 1) * b, j * b:(j + 1) * b]
            jxx, jyy, jxy = (sx * sx).mean(), (sy * sy).mean(), (sx * sy).mean()
            tr = jxx + jyy
            if tr <= 0:
                continue
            l1 = tr / 2 + np.sqrt(max(tr * tr / 4 - (jxx * jyy - jxy * jxy), 0))
            a_loc.append((2 * l1 - tr) / tr)
            th = np.arctan2(2 * jxy, jxx - jyy)
            vecs.append((np.cos(th), np.sin(th)))
    coh = float(np.mean(a_loc)) if a_loc else 0.0
    cons = float(np.hypot(*np.mean(vecs, axis=0))) if vecs else 0.0
    z5 = gaussian_filter(z, 5000 / cell / 2.355)
    gaps = []
    for prof in list(z5[::50]) + list(z5.T[::50]):
        mins = np.where((prof[1:-1] < prof[:-2]) & (prof[1:-1] < prof[2:]))[0]
        if len(mins) > 1:
            gaps.extend(np.diff(mins) * cell / 1000)
    sp = float(np.median(gaps)) if gaps else float("nan")
    line = (f"| {name} | {r[0][0]:.0f} / {r[0][1]:.0f} | {r[1][0]:.0f} / {r[1][1]:.0f} | {r[2][0]:.0f} / {r[2][1]:.0f} | {100 * rng:.1f} % | {coh:.2f} | {cons:.2f} | {sp:.1f} km | {cv5:.2f} | {closed:.1f} % | {100 * land.mean():.0f} % |")
    print(line)
    out.write(line + "\n")
    out.flush()


hdr = "| Terrain (100 km square, 100 m cells) | relief 1 km p50 / p95 (m) | relief 5 km | relief 25 km | power at 10-50 km of 1-50 km | local range coherence | direction consistency | main-valley spacing | 5 km relief's variation over 25 km blocks (CV) | closed basins | land |\n|---|---|---|---|---|---|---|---|---|---|---|"
print(hdr)
out.write(hdr + "\n")
for name, f, lat in TILES:
    z, (cx, cy) = tile_square(f, lat)
    measures(z, 0.5 * (cx + cy), name)
N = 8160
for label, path in [("M1 base (GEN-140 off)", "C:/DEV/SeriousPlanetData/tmp/crusader-166/cap/m1-chain/base.f32"), ("M1 base (GEN-140 on)", "C:/DEV/SeriousPlanetData/tmp/crusader-167/turned/m1-chain/base.f32"),
                    ("M1 S0 (GEN-140 off)", "C:/DEV/SeriousPlanetData/tmp/crusader-166/cap/m1-chain/s0.f32")]:
    z = np.fromfile(path, dtype="<f4").reshape(N, N).astype(np.float64)
    # three 100 km squares of 983 L cells (101.8 m) inside the counted box: centre, upper left, lower right
    for tag, (y0, x0) in [("centre", (3588, 3588)), ("upper left", (1600, 1600)), ("lower right", (5577, 5577))]:
        measures(z[y0:y0 + 983, x0:x0 + 983], 101.8, f"{label}, {tag}")
    del z
out.close()
