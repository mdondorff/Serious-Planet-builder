"""Lead #184c: the kept-size depressions on each stage surface of a chain (basins-2's default rule: at least 0.25 km2 and 2 m deep; no 10 m cap), in the box. The flood:
priority flood from the window's edge and the sea (S0 at or below 0 m), epsilon 0; a depression is an 8-connected component of cells below the filled level.
Reported per stage: their count and area (km2), and the area of S0's kept-size depressions still below their own S0 spill level on the stage (the lakes S0 holds that
the stage keeps). Usage: kept_stages.py <chain dir> [stage ...] (default s0 kh ks0 ks1c band6c kv final)"""
import sys, heapq, numpy as np, numba
from scipy import ndimage
D = sys.argv[1]
stages = sys.argv[2:] or ['s0', 'kh', 'ks0', 'ks1c', 'band6c', 'kv', 'final']
n, m, cell = 8160, 1600, 101.802


@numba.njit(cache=True)
def fill(z, sea):
    H, W = z.shape
    lev = z.copy()
    done = np.zeros((H, W), np.bool_)
    hp = [(0.0, 0, 0)]
    hp.pop()
    for y in range(H):
        for x in range(W):
            if y == 0 or x == 0 or y == H - 1 or x == W - 1 or sea[y, x]:
                heapq.heappush(hp, (z[y, x], y, x))
                done[y, x] = True
    while len(hp) > 0:
        e, y, x = heapq.heappop(hp)
        for oy in (-1, 0, 1):
            for ox in (-1, 0, 1):
                ny, nx = y + oy, x + ox
                if 0 <= ny < H and 0 <= nx < W and not done[ny, nx]:
                    done[ny, nx] = True
                    v = max(z[ny, nx], e)
                    lev[ny, nx] = v
                    heapq.heappush(hp, (v, ny, nx))
    return lev


s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n).astype(np.float64)
sea = s0 <= 0.0
eight = np.ones((3, 3), bool)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True


def kept(z):
    lev = fill(z, sea)
    dep = lev - z
    lab, k = ndimage.label(dep > 0, structure=eight)
    if k == 0:
        return lab, np.zeros(0, bool), lev
    area = ndimage.sum(np.ones_like(z), lab, index=np.arange(1, k + 1)) * cell * cell
    depth = ndimage.maximum(dep, lab, index=np.arange(1, k + 1))
    ok = (area >= 0.25e6) & (depth >= 2.0)
    return lab, np.concatenate([[False], ok]), lev


lab0, ok0, lev0 = kept(s0)
s0kept = ok0[lab0] & box
print(f'{D.rstrip("/").split("/")[-1]}: S0 kept-size depressions in the box: {len(np.unique(lab0[s0kept]))}, {s0kept.sum() * cell * cell / 1e6:.1f} km2')
for st in stages:
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n).astype(np.float64)
    lab, ok, lev = kept(z)
    k = ok[lab] & box
    # S0's kept lakes the stage holds: their cells still below the S0 spill level on the stage and below the stage's own fill
    held = s0kept & (z < lev0) & (lev > z)
    print(f'  {st:7s} kept-size depressions {len(np.unique(lab[k])):5d}, {k.sum() * cell * cell / 1e6:8.1f} km2; S0 lakes still held {held.sum() * cell * cell / 1e6:8.1f} km2')
