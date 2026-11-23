"""#174a scoping: band6's trenches (ks1c -> band6) whose S_KH crest sits at an H1 corner; for the 2 x 2 H1 block around that corner: a saddle block on z_s (one
diagonal pair above the other), its low pair both H1 channel cells (h1-rivers.u8 > 0), and a D8 step of z_s between the low pair (steepest descent, 8 neighbours).
Usage: corner174b.py <chain dir> <x0> <y0>"""
import sys, numpy as np
from scipy import ndimage
D, x0, y0 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
n, m, nh = 8160, 1600, 1020
ld = lambda k: np.fromfile(f'{D}/{k}.f32', '<f4').reshape(n, n).astype(np.float64)
kh = ld('kh')
box = np.zeros((n, n), bool); box[m:n-m, m:n-m] = True
cut = ld('ks1c') - ld('band6')
low = (cut > 0.25) & box
lab, k = ndimage.label(low, structure=np.ones((3, 3)))
nb = ndimage.convolve(low.astype(np.int32), np.ones((3, 3), np.int32), mode='constant') - low
ends = low & (nb <= 1)
idx = np.arange(1, k + 1)
mc = ndimage.maximum(cut, lab, idx)
top = ndimage.maximum(kh, lab, idx)
e = ndimage.maximum(np.where(ends, kh, -np.inf), lab, idx)
ne = ~np.isfinite(e)
e[ne] = ndimage.minimum(kh, lab, idx)[ne]
ti = idx[(mc > 10) & (top - e > 10)]
c = np.array(ndimage.maximum_position(kh, lab, ti))
gx, gy = c[:, 1] + x0, c[:, 0] + y0
cor = np.isin(gx % 8, (0, 7)) & np.isin(gy % 8, (0, 7))
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(nh, nh).astype(np.float64)
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
# D8 steepest descent on z_s (distance-weighted drop)
pad = np.pad(zs, 1, constant_values=np.inf)
best = np.full(zs.shape, -1); bestd = np.zeros(zs.shape)
offs = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
for i, (dx, dy) in enumerate(offs):
    nbz = pad[1 + dy:1 + dy + nh, 1 + dx:1 + dx + nh]
    drop = (zs - nbz) / (np.sqrt(2) if dx and dy else 1.0)
    better = drop > bestd
    best[better] = i; bestd[better] = drop[better]
def recv_is(a, b):  # D8 receiver of H1 cell a is b
    (ay, ax), (by, bx) = a, b
    i = best[ay, ax]
    return i >= 0 and (ax + offs[i][0], ay + offs[i][1]) == (bx, by)
cls = {'saddle': 0, 'saddle, low pair both channel': 0, 'saddle, D8 step on the low pair': 0, 'saddle, both': 0, 'no saddle': 0}
for i in np.where(cor)[0]:
    r, col = c[i]
    hx, hy = (col + 4) // 8, (r + 4) // 8
    a, b, cq, dq = (hy - 1, hx - 1), (hy - 1, hx), (hy, hx - 1), (hy, hx)
    za, zb, zc, zd = zs[a], zs[b], zs[cq], zs[dq]
    if min(za, zd) > max(zb, zc):
        lo = (b, cq)
    elif min(zb, zc) > max(za, zd):
        lo = (a, dq)
    else:
        cls['no saddle'] += 1
        continue
    cls['saddle'] += 1
    ch = riv[lo[0]] and riv[lo[1]]
    st = recv_is(lo[0], lo[1]) or recv_is(lo[1], lo[0])
    cls['saddle, low pair both channel'] += ch
    cls['saddle, D8 step on the low pair'] += st
    cls['saddle, both'] += ch and st
print(f'band6: {len(ti)} trenches, {int(cor.sum())} with the S_KH crest at an H1 corner; their 2 x 2 H1 blocks: {cls}')
