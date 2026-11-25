"""Lead #184r: a dry estimate of what row 3 under A5 (a breach over 10 m becomes a kept lake or an around) would make of step 1's cols. No chain run.
- Cols: runs of a curve's raster cells (in the box) that KH cuts deeper than max(the defaults' KH lowering, 10 m), S0 - S_KH (col_list.py's rule). A col's crossing
  height is the highest S0 on its run.
- Around: a bottleneck (minimax) search on S0 from the curve cell 3 cells upstream of the run to the cell 2 cells downstream, inside a corridor of R L cells round the
  reach's raster (R = 8, the curve's own corridor, and R = 24, three H1 cells). The col is "around" when that path's highest point lies at least 10 m under the crossing.
- Spill: every other col. The lake it would hold: the final surface with every spill col's cut zone (cells within 8 L cells of its raster cells) set back to
  max(final, S0), priority-flood filled from the window's edge and the sea. The lake behind a col is the filled depression that holds the cell just upstream of the run.
  Kept-size: basins-2's default rule, at least 0.25 km2 and 2 m deep.
- Lake chains: trunk reaches with two or more spill cols in series.
With a fourth argument T (m), only cols cut deeper than T on the final surface are held (spill); the shallower ones count as breached (A5 with its cap at T).
With a fifth argument `partial`, every col without a way round is breached by up to T and holds a lake only above that: its cut zone is set to max(final, S0 - T).
Usage: row3_estimate.py <chain dir> <defaults chain dir> <trunk-curves.bin> [T] [partial]"""
import sys, struct, heapq, numpy as np, numba
from scipy import ndimage
D, DD, CB = sys.argv[1:4]
T = float(sys.argv[4]) if len(sys.argv) > 4 else 10.0
PARTIAL = len(sys.argv) > 5 and sys.argv[5] == 'partial'
n, m, cell = 8160, 1600, 101.802
ld = lambda d, s: np.fromfile(f'{d}/{s}.f32', '<f4').reshape(n, n)
s0 = ld(D, 's0').astype(np.float64); fin = ld(D, 'final').astype(np.float64)
khm = s0 - ld(D, 'kh'); khd = s0 - ld(DD, 'kh')
sea = s0 <= 0.0


@numba.njit
def bottleneck(z, ok, sy, sx, ty, tx):
    H, W = z.shape
    best = np.full((H, W), np.inf)
    best[sy, sx] = z[sy, sx]
    hp = [(z[sy, sx], sy, sx)]
    while len(hp) > 0:
        b, y, x = heapq.heappop(hp)
        if y == ty and x == tx:
            return b
        if b > best[y, x]:
            continue
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                ny, nx = y + dy, x + dx
                if (dy or dx) and 0 <= ny < H and 0 <= nx < W and ok[ny, nx]:
                    v = max(b, z[ny, nx])
                    if v < best[ny, nx]:
                        best[ny, nx] = v
                        heapq.heappush(hp, (v, ny, nx))
    return np.inf


@numba.njit
def fill(z, out):
    H, W = z.shape
    lev = z.copy()
    done = np.zeros((H, W), np.bool_)
    hp = [(0.0, 0, 0)]
    hp.pop()
    for y in range(H):
        for x in range(W):
            if y == 0 or x == 0 or y == H - 1 or x == W - 1 or out[y, x]:
                heapq.heappush(hp, (z[y, x], y, x)); done[y, x] = True
    while len(hp) > 0:
        e, y, x = heapq.heappop(hp)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                ny, nx = y + dy, x + dx
                if 0 <= ny < H and 0 <= nx < W and not done[ny, nx]:
                    done[ny, nx] = True
                    v = max(z[ny, nx], e)
                    lev[ny, nx] = v
                    heapq.heappush(hp, (v, ny, nx))
    return lev


# the cols, per reach (raster cells in order)
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]
cols = []  # (reach, run cells (y, x arrays), up cell, down cell, crossing, reach raster)
for _ in range(cnt):
    r, npt = struct.unpack_from('<II', b, k); k += 8 + 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    c = np.frombuffer(b, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
    x, y = c % n, c // n
    inb = (x >= m) & (x < n - m) & (y >= m) & (y < n - m)
    h = inb & (khm[y, x] > np.maximum(khd[y, x], 10.0) + 1e-6)
    i = 0
    while i < len(h):
        if h[i]:
            j = i
            while j < len(h) and h[j]:
                j += 1
            u, dn = max(i - 3, 0), min(j + 1, len(h) - 1)
            cols.append((r, y[i:j], x[i:j], (y[u], x[u]), (y[dn], x[dn]), float(s0[y[i:j], x[i:j]].max()), (y, x), float((s0 - fin)[y[i:j], x[i:j]].max())))
            i = j
        else:
            i += 1
print(f'{D.rstrip("/").split("/")[-1]}: {len(cols)} cols over 10 m on {len(set(c[0] for c in cols))} reaches')
# around, at R = 8 and 24
around = {8: np.zeros(len(cols), bool), 24: np.zeros(len(cols), bool)}
for ci, (r, ys, xs, up, dn, cross, (ry, rx), _dc) in enumerate(cols):
    for R in (8, 24):
        y0, y1 = max(min(ys.min(), up[0], dn[0]) - R - 2, 0), min(max(ys.max(), up[0], dn[0]) + R + 3, n)
        x0, x1 = max(min(xs.min(), up[1], dn[1]) - R - 2, 0), min(max(xs.max(), up[1], dn[1]) + R + 3, n)
        sub = np.ones((y1 - y0, x1 - x0), bool)
        sel = (ry >= y0) & (ry < y1) & (rx >= x0) & (rx < x1)
        sub[ry[sel] - y0, rx[sel] - x0] = False
        ok = ndimage.distance_transform_edt(sub) <= R
        bb = bottleneck(s0[y0:y1, x0:x1], ok, up[0] - y0, up[1] - x0, dn[0] - y0, dn[1] - x0)
        around[R][ci] = bb <= cross - 10.0
for R in (8, 24):
    print(f'  around within {R:2d} L cells of the reach (a way round at least 10 m under the crossing): {around[R].sum()} cols; spill {len(cols) - around[R].sum()}')
spill = ~around[24] & ((np.array([c[7] for c in cols]) > T) | PARTIAL)
print(f'  held (spill) cols at the cap T = {T:.0f} m: {spill.sum()}')
# the lakes the spill cols would hold
mask = np.zeros((n, n), bool)
for ci in np.nonzero(spill)[0]:
    mask[cols[ci][1], cols[ci][2]] = True
zone = ndimage.distance_transform_edt(~mask) <= 8
z2 = np.where(zone, np.maximum(fin, s0 - T if PARTIAL else s0), fin)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True


def kept(z):
    lev = fill(z, sea)
    dep = lev - z
    lab, kk = ndimage.label(dep > 1e-6, structure=np.ones((3, 3), bool))
    area = ndimage.sum(np.ones_like(z), lab, index=np.arange(1, kk + 1)) * cell * cell
    depth = ndimage.maximum(dep, lab, index=np.arange(1, kk + 1))
    okk = np.concatenate([[False], (area >= 0.25e6) & (depth >= 2.0)])
    return lab, okk, lev, area, depth


lab0, ok0, lev0, _, _ = kept(fin)
lab2, ok2, lev2, area2, depth2 = kept(z2)
k0 = ok0[lab0] & box; k2 = ok2[lab2] & box
print(f'  kept-size depressions in the box (>= 0.25 km2, >= 2 m): final {len(np.unique(lab0[k0]))} ({k0.sum() * cell * cell / 1e6:.1f} km2); '
      f'with the spill cols held {len(np.unique(lab2[k2]))} ({k2.sum() * cell * cell / 1e6:.1f} km2)')
new = k2 & ~k0
print(f'  added kept-size lake area {new.sum() * cell * cell / 1e6:.1f} km2')
# per spill col: the lake behind it
per = []
for ci in np.nonzero(spill)[0]:
    up = cols[ci][3]
    L = lab2[up]
    if L > 0 and ok2[L]:
        per.append((ci, L, area2[L - 1] / 1e6, depth2[L - 1], lev2[up]))
lakes = {}
for ci, L, a, dpt, spl in per:
    lakes.setdefault(L, []).append(ci)
if per:
    A = np.array([p[2] for p in per]); Dp = np.array([p[3] for p in per])
    ul = np.unique([p[1] for p in per])
    ua = np.array([area2[L - 1] / 1e6 for L in ul]); ud = np.array([depth2[L - 1] for L in ul])
    print(f'  spill cols with a kept-size lake behind them: {len(per)} of {spill.sum()}; distinct lakes {len(ul)}')
    print(f'    those lakes: area p50 {np.median(ua):.2f}, p90 {np.percentile(ua, 90):.2f}, max {ua.max():.1f} km2, sum {ua.sum():.0f} km2; depth p50 {np.median(ud):.1f}, p90 {np.percentile(ud, 90):.1f}, max {ud.max():.0f} m')
    for amin in (0.25, 1.0, 5.0):
        s = ua >= amin
        print(f'    lakes of at least {amin} km2: {s.sum()} ({ua[s].sum():.0f} km2)')
    rr = {}
    for ci, L, *_ in per:
        rr.setdefault(cols[ci][0], set()).add(L)
    chains = np.array([len(v) for v in rr.values()])
    print(f'  trunk reaches with lakes behind spill cols: {len(rr)}; with 1 lake {np.sum(chains == 1)}, 2 {np.sum(chains == 2)}, 3 or more {np.sum(chains >= 3)} (lake chains: {np.sum(chains >= 2)})')
nl = spill.sum() - len(per)
print(f'  spill cols with no kept-size lake behind them (a pond under 0.25 km2 or 2 m, or the held col drains round inside its zone): {nl}')
