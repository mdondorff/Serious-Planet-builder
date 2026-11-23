"""SPIKE-142 (#173), part 3: which layer makes each trench's hump. At the trench's S0 crest c and its highest end e (on S0), the hump on S0 splits as
S0(c) - S0(e) = [Z(c) - Z(e)] + [rho(c) - rho(e)], with Z the K5 surface z_s read bilinearly at L (H1 centres at L cells 8i + 3.5) and rho = S0 - Z the sub-H1 detail
(the base's own relief below 814 m, with the C1 read of its 611 m lattice), and also as B and r = S0 - B. Also: z_s's checkerboard (the share of H1 cells that are a strict
local maximum or minimum of their 4 neighbours, against a smooth field's few). Usage: lattice173c.py <chain dir>"""
import sys, numpy as np
from scipy import ndimage
D = sys.argv[1]
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
maxcut = ndimage.maximum(cut, lab, idx)
del cut


def hump(z):
    top = ndimage.maximum(z, lab, idx)
    e = ndimage.maximum(np.where(ends, z, -np.inf), lab, idx)
    noend = ~np.isfinite(e)
    e[noend] = ndimage.minimum(z, lab, idx)[noend]
    return top - e


tr = (maxcut > 10) & (hump(kh) > 10)
ti = idx[tr]
s0 = ld('s0'); b = ld('base')
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(nh, nh).astype(np.float64)
yy, xx = np.mgrid[0:n, 0:n]
Z = ndimage.map_coordinates(zs, [(yy - 3.5) / 8.0, (xx - 3.5) / 8.0], order=1, mode='nearest')
del yy, xx
c = np.array(ndimage.maximum_position(s0, lab, ti))
endz = np.where(ends, s0, -np.inf)
e = np.array(ndimage.maximum_position(endz, lab, ti))
# a path without a degree-1 end: its lowest S0 cell
bad = ~np.isfinite(endz[e[:, 0], e[:, 1]])
if bad.any():
    e[bad] = np.array(ndimage.minimum_position(s0, lab, ti[bad]))
d = lambda z: z[c[:, 0], c[:, 1]] - z[e[:, 0], e[:, 1]]
dS0, dZ, dB = d(s0), d(Z), d(b)
dRho, dR = dS0 - dZ, dS0 - dB
q = lambda v: f'p10 {np.percentile(v, 10):6.1f} p50 {np.percentile(v, 50):6.1f} p90 {np.percentile(v, 90):6.1f} m'
print(f'{tr.sum()} trenches; S0 hump at the S0 crest against the highest end: {q(dS0)}')
print(f'  K5 surface Z (z_s bilinear): {q(dZ)}; sub-814 m detail rho = S0 - Z: {q(dRho)}')
print(f'  base B: {q(dB)}; K5 delta r = S0 - B: {q(dR)}')
for name, v in (('Z', dZ), ('rho', dRho)):
    print(f'  share of the hump carried by {name}: p50 {np.median(v / dS0) * 100:.0f} %; trenches where {name} carries over half {np.mean(v / dS0 > 0.5) * 100:.1f} %, '
          f'where {name} alone is over 10 m {np.mean(v > 10) * 100:.1f} %')
small = np.abs(dRho) < 0.5 * dS0
print(f'trenches whose hump is mostly K5 relief (|rho| under half): {small.mean() * 100:.1f} %; mostly sub-814 m detail (|Z| under half): {(np.abs(dZ) < 0.5 * dS0).mean() * 100:.1f} %')
# z_s's checkerboard inside the box: strict 4-neighbour extrema
zb = zs[200:820, 200:820]
up = np.stack([np.roll(zb, s, a) for s, a in ((1, 0), (-1, 0), (1, 1), (-1, 1))])
inner = np.zeros_like(zb, bool); inner[1:-1, 1:-1] = True
mx = (zb > up.max(0)) & inner; mn = (zb < up.min(0)) & inner
print(f'z_s (H1, inside the box): strict 4-neighbour maxima {mx.mean() * 100:.2f} %, minima {mn.mean() * 100:.2f} % of H1 cells (a checkerboard would be 50 %)')
lap = 4 * zb - up.sum(0)
print(f'z_s Laplacian (4 z - neighbours) p50 |.| {np.median(np.abs(lap[inner])):.2f} m, p90 {np.percentile(np.abs(lap[inner]), 90):.2f} m; '
      f'neighbour sign alternation (lap and right neighbour of opposite sign over 1 m): {np.mean((lap[1:-1, 1:-2] * lap[1:-1, 2:-1] < 0) & (np.abs(lap[1:-1, 1:-2]) > 1)) * 100:.1f} %')
np.savez_compressed('r1h-layers.npz', dS0=dS0, dZ=dZ, dB=dB, dRho=dRho, c=c, e=e)
