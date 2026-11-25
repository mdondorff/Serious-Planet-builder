"""Lead #184c: where the corner-only links of a chain's final surface sit. A corner-only link (corner157.py's form, 5 m) is a 2 x 2 block whose diagonal pair lies more
than 5 m below both other cells. Each block is classed by the graph's L reaches (graph.txt): a block whose low pair is a diagonal step of a reach (two consecutive
vertices), by the reach's kind (trunk: an L image with a parent H1 reach; tributary: without); a block touching a trunk vertex and a tributary vertex (a junction);
touching one kind's vertices only; or touching no vertex. Usage: corner184.py <chain dir> [stage]"""
import sys, numpy as np
D = sys.argv[1]; st = sys.argv[2] if len(sys.argv) > 2 else 'final'
n, m, T = 8160, 1600, 5.0
x0 = y0 = None
for line in open(f'{D}/chain.txt'):
    if line.startswith('window '):
        _, a, b, _w, _h = line.split()[:5]; x0, y0 = int(a), int(b)
z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n).astype(np.float64)
trunk = np.zeros((n, n), np.bool_); trib = np.zeros((n, n), np.bool_)
diag_trunk = set(); diag_trib = set()
for line in open(f'{D}/graph.txt'):
    if not line.startswith('reach ') or ' level=l ' not in line:
        continue
    f = line.split()
    par = next(t for t in f if t.startswith('parent=')).split('=', 1)[1]
    vs = next(t for t in f if t.startswith('vertices=')).split('=', 1)[1].split(',')
    xy = [((int(v) & 0xFFFFFFFF) - x0, (int(v) >> 32) - y0) for v in vs]
    is_trunk = par != '-'
    tgt, dset = (trunk, diag_trunk) if is_trunk else (trib, diag_trib)
    for (x, y) in xy:
        if 0 <= x < n and 0 <= y < n:
            tgt[y, x] = True
    for (ax, ay), (bx, by) in zip(xy, xy[1:]):
        if abs(ax - bx) == 1 and abs(ay - by) == 1:
            dset.add((min(ax, bx), min(ay, by)))
zz = z[m:n - m, m:n - m]
a, b, c, d = zz[:-1, :-1], zz[:-1, 1:], zz[1:, :-1], zz[1:, 1:]
blk = ((np.minimum(b, c) - np.maximum(a, d)) > T) | ((np.minimum(a, d) - np.maximum(b, c)) > T)
ys, xs = np.nonzero(blk)
ys, xs = ys + m, xs + m
cls = {'trunk diagonal step': 0, 'tributary diagonal step': 0, 'trunk and tributary vertices (junction)': 0, 'trunk vertices only': 0, 'tributary vertices only': 0, 'no vertex': 0}
for y, x in zip(ys, xs):
    if (x, y) in diag_trunk:
        cls['trunk diagonal step'] += 1; continue
    if (x, y) in diag_trib:
        cls['tributary diagonal step'] += 1; continue
    tr = trunk[y:y + 2, x:x + 2].any(); tb = trib[y:y + 2, x:x + 2].any()
    k = 'trunk and tributary vertices (junction)' if tr and tb else 'trunk vertices only' if tr else 'tributary vertices only' if tb else 'no vertex'
    cls[k] += 1
from scipy import ndimage
dt = ndimage.distance_transform_edt(~trunk)
dtb = ndimage.distance_transform_edt(~(trunk | trib))
nov = [(y, x) for y, x in zip(ys, xs) if not (trunk[y:y + 2, x:x + 2].any() or trib[y:y + 2, x:x + 2].any())]
dd = np.array([dt[y, x] for y, x in nov]) if nov else np.zeros(0)
db = np.array([dtb[y, x] for y, x in nov]) if nov else np.zeros(0)
print(f'{D.rstrip("/").split("/")[-1]} {st}: {len(ys)} corner-only blocks over {T:.0f} m in the box')
if len(dd):
    print(f'  no-vertex blocks by distance to the nearest trunk vertex (L cells): <= 2 {int((dd <= 2).sum())}, 2-8 {int(((dd > 2) & (dd <= 8)).sum())}, 8-30 {int(((dd > 8) & (dd <= 30)).sum())}, > 30 {int((dd > 30).sum())}; to any line vertex: <= 2 {int((db <= 2).sum())}, > 2 {int((db > 2).sum())}')
for k, v in cls.items():
    print(f'  {k:42s} {v:6d}')
