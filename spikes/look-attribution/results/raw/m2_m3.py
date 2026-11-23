"""SPIKE-143, M2 and M3 (pre-registered in results.md), on a chain's final graph (graph.txt) and its stage surfaces.
M2: the diagonal-step share of the L reaches' vertex steps; the bank saw-tooth s(k) = |b(k) - (b(k-1) + b(k+1)) / 2| per bank, b the bilinear sample at +-2 cells along
the normal of the chord k-1 -> k+1, over interior vertices of reaches with at least 5 vertices inside the box; p50, p90, mean per stage, all vertices and split by the
vertex's step (diagonal or axis).
M3: the share of KG-trunk image length (parent set) in runs of one repeated step direction longer than 2 km; sinuosity (path / chord) of reaches over 2 km; and the
coincidence ratio: the share of high saw-tooth vertices (s above the final surface's p90) that lie in straight runs, over the share of all vertices in straight runs.
Usage: m2_m3.py <chain dir> <x0> <y0> <stage>..."""
import sys, numpy as np
from scipy import ndimage
D, x0, y0 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
stages = sys.argv[4:]
n, m, dx = 8160, 1600, 101.802
reaches = []
with open(f'{D}/graph.txt', encoding='utf-8') as f:
    for line in f:
        if not line.startswith('reach ') or ' level=l ' not in line:
            continue
        parts = dict(p.split('=', 1) for p in line.split()[2:] if '=' in p)
        v = np.array([int(t) for t in parts['vertices'].split(',')], dtype=np.int64)
        gx, gy = (v & 0xFFFFFFFF) - x0, (v >> 32) - y0
        reaches.append((gx, gy, parts.get('parent', '-') != '-'))
inbox = lambda x, y: (x >= m) & (x < n - m) & (y >= m) & (y < n - m)
# M2 diagonal-step share, M3 straight runs and sinuosity
steps = diag = 0
trunk_len = straight_len = 0.0
sin, sin_w = [], []
vx, vy, vdiag, vstraight = [], [], [], []
for gx, gy, trunk in reaches:
    if len(gx) < 2:
        continue
    ddx, ddy = np.diff(gx), np.diff(gy)
    ok = inbox(gx[1:], gy[1:]) & inbox(gx[:-1], gy[:-1])
    isd = (ddx != 0) & (ddy != 0)
    steps += int(ok.sum()); diag += int((isd & ok).sum())
    seg = np.where(isd, dx * np.sqrt(2), dx)
    # runs of one repeated direction
    code = (ddx + 1) * 3 + (ddy + 1)
    run_id = np.concatenate([[0], np.cumsum(code[1:] != code[:-1])])
    run_len = np.bincount(run_id, weights=seg)
    st = run_len[run_id] > 2000.0
    if trunk:
        trunk_len += float(seg[ok].sum()); straight_len += float(seg[ok & st].sum())
    L = seg.sum()
    if L > 2000.0 and ok.all():
        chord = np.hypot(gx[-1] - gx[0], gy[-1] - gy[0]) * dx
        if chord > 0:
            sin.append(L / chord); sin_w.append(L)
    if len(gx) >= 5:
        k = np.arange(1, len(gx) - 1)
        good = inbox(gx[k], gy[k])
        vx.append(np.stack([gx[k - 1], gx[k], gx[k + 1]], 1)[good]); vy.append(np.stack([gy[k - 1], gy[k], gy[k + 1]], 1)[good])
        # a vertex's step: the step into it
        vdiag.append(isd[k - 1][good]); vstraight.append((st[k - 1] | st[k])[good])
sin, sin_w = np.array(sin), np.array(sin_w)
print(f'reaches {len(reaches)}; vertex steps in the box {steps}, diagonal {diag / max(steps, 1) * 100:.1f} %')
print(f'KG-trunk image length in the box {trunk_len / 1000:.0f} km, in straight runs over 2 km {straight_len / max(trunk_len, 1) * 100:.1f} %')
print(f'sinuosity of reaches over 2 km ({len(sin)}): p50 {np.median(sin):.3f}, length-weighted mean {np.sum(sin * sin_w) / np.sum(sin_w):.3f}')
vx = np.concatenate(vx); vy = np.concatenate(vy); vdiag = np.concatenate(vdiag); vstraight = np.concatenate(vstraight)
# chord normals at the middle vertex
tx, ty = (vx[:, 2] - vx[:, 0]).astype(float), (vy[:, 2] - vy[:, 0]).astype(float)
tl = np.hypot(tx, ty); tl[tl == 0] = 1
nx, ny = -ty / tl, tx / tl
res = {}
for stg in stages:
    z = np.fromfile(f'{D}/{stg}.f32', '<f4').reshape(n, n)
    out = []
    for side in (1, -1):
        b = np.stack([ndimage.map_coordinates(z, [vy[:, j] + side * 2 * ny, vx[:, j] + side * 2 * nx], order=1) for j in range(3)], 1)
        out.append(np.abs(b[:, 1] - (b[:, 0] + b[:, 2]) / 2))
    s = np.maximum(out[0], out[1])  # the larger of the two banks
    res[stg] = s
    q = lambda a: f'p50 {np.percentile(a, 50):5.2f} p90 {np.percentile(a, 90):6.2f} mean {a.mean():5.2f}'
    print(f'{stg:7s} saw-tooth (m): all {q(s)} | diagonal-step vertices {q(s[vdiag])} | axis-step vertices {q(s[~vdiag])}')
if 'final' in res:
    hi = res['final'] > np.percentile(res['final'], 90)
    share_all = vstraight.mean(); share_hi = vstraight[hi].mean()
    print(f'coincidence: high saw-tooth vertices in straight runs {share_hi * 100:.1f} % against all vertices {share_all * 100:.1f} % (ratio {share_hi / max(share_all, 1e-9):.2f}); '
          f'high saw-tooth on diagonal steps {vdiag[hi].mean() * 100:.1f} % against all {vdiag.mean() * 100:.1f} %')
np.savez_compressed('m2-vertices.npz', vx=vx[:, 1], vy=vy[:, 1], vdiag=vdiag, vstraight=vstraight, **{f's_{k}': v for k, v in res.items()})
