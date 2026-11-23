"""SPIKE-142 (#173), part 4: the H1 corner test. For each trench (lattice173.py's definition), the crest on S_KH and on S0 at an H1 cell corner (x and y within
one cell of an H1 edge; uniform 6.25 %); for S0's corner crests, whether the 2 x 2 H1 block of z_s around that corner is a saddle block (one diagonal pair above the
other); and the humps over 50 m with the share Z (z_s bilinear) carries. Reads r1h-layers.npz (lattice173c.py) for the S0 crests and the hump split.
Usage: lattice173d.py <chain dir> <x0> <y0> <r1h-layers.npz>"""
import sys, numpy as np
from scipy import ndimage
D, x0, y0, layers = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
n, m, nh = 8160, 1600, 1020
ld = lambda k: np.fromfile(f'{D}/{k}.f32', '<f4').reshape(n, n).astype(np.float64)
kh = ld('kh')
cut = kh - ld('final')
box = np.zeros((n, n), bool); box[m:n-m, m:n-m] = True
low = (cut > 0.25) & box
lab, k = ndimage.label(low, structure=np.ones((3, 3)))
nb = ndimage.convolve(low.astype(np.int32), np.ones((3, 3), np.int32), mode='constant') - low
ends = low & (nb <= 1)
idx = np.arange(1, k + 1)
mc = ndimage.maximum(cut, lab, idx)
del cut
top = ndimage.maximum(kh, lab, idx)
e = ndimage.maximum(np.where(ends, kh, -np.inf), lab, idx)
ne = ~np.isfinite(e)
e[ne] = ndimage.minimum(kh, lab, idx)[ne]
ti = idx[(mc > 10) & (top - e > 10)]
print(f'{len(ti)} trenches')
corner = lambda gx, gy: np.isin(gx % 8, (0, 7)) & np.isin(gy % 8, (0, 7))
c = np.array(ndimage.maximum_position(kh, lab, ti))
print(f'S_KH crests at an H1 corner: {np.mean(corner(c[:, 1] + x0, c[:, 0] + y0)) * 100:.1f} % (uniform 6.25 %)')
d = np.load(layers)
c, dS0, dZ = d['c'], d['dS0'], d['dZ']
gx, gy = c[:, 1] + x0, c[:, 0] + y0
ex, ey = np.isin(gx % 8, (0, 7)), np.isin(gy % 8, (0, 7))
print(f'S0 crests: at an x edge only {np.mean(ex & ~ey) * 100:.1f} % (uniform 18.75 %), y edge only {np.mean(ey & ~ex) * 100:.1f} % (18.75 %), '
      f'at a corner {np.mean(ex & ey) * 100:.1f} % (6.25 %), interior {np.mean(~ex & ~ey) * 100:.1f} % (56.25 %)')
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(nh, nh).astype(np.float64)
sad = tot = 0
for i in np.where(ex & ey)[0]:
    r, col = c[i]
    hx, hy = (col + 4) // 8, (r + 4) // 8  # the H1 corner nearest the crest: between H1 cells hx - 1, hx and hy - 1, hy
    if hx < 1 or hy < 1 or hx >= nh or hy >= nh:
        continue
    a, b, cq, dq = zs[hy - 1, hx - 1], zs[hy - 1, hx], zs[hy, hx - 1], zs[hy, hx]
    tot += 1
    if min(a, dq) > max(b, cq) or min(b, cq) > max(a, dq):
        sad += 1
print(f'S0 corner crests: {tot}; on an H1 saddle block (one diagonal pair of z_s above the other) {sad} ({sad / max(tot, 1) * 100:.0f} %)')
big = dS0 > 50
print(f'humps over 50 m on S0: {big.sum()} ({big.mean() * 100:.0f} %); Z carries over half of {np.mean(dZ[big] / dS0[big] > 0.5) * 100:.0f} % of them')
