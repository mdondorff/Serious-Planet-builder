"""SPIKE-142 (#173), part 2: the trenches of lattice173.py split by path size; the crest phase modulo 8 per layer (the argmax of S_KH, S0, B and r on the path);
the phase of every trench path cell (the path's own bias) and of S_KH's local maxima; and the crest's 3 x 3 form on S0 (peak, saddle, slope).
Usage: lattice173b.py <chain dir> <x0> <y0>"""
import sys, numpy as np
from scipy import ndimage
D, x0, y0 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
n, m = 8160, 1600
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
size = ndimage.sum(np.ones((n, n)), lab, idx)


def hump(z):
    top = ndimage.maximum(z, lab, idx)
    e = ndimage.maximum(np.where(ends, z, -np.inf), lab, idx)
    noend = ~np.isfinite(e)
    e[noend] = ndimage.minimum(z, lab, idx)[noend]
    return top - e


tr = (maxcut > 10) & (hump(kh) > 10)
ti = idx[tr]
sz = size[tr]
print(f'{tr.sum()} trenches; path size p10 {np.percentile(sz, 10):.0f} p50 {np.percentile(sz, 50):.0f} p90 {np.percentile(sz, 90):.0f} max {sz.max():.0f} cells; '
      f'under 50 cells {np.sum(sz < 50)}, 50-500 {np.sum((sz >= 50) & (sz < 500))}, 500+ {np.sum(sz >= 500)}')
s0 = ld('s0'); b = ld('base')
layers = {'S_KH': kh, 'S0': s0, 'B': b, 'r': s0 - b}


def share(v, P=8):
    return np.bincount(v % P, minlength=P) / max(len(v), 1) * 100


def fmt(a):
    return ' '.join(f'{x:5.1f}' for x in a)


edge = lambda gx, gy: np.mean(((gx % 8) in (7, 0)) if np.isscalar(gx) else np.isin(gx % 8, (0, 7)) | np.isin(gy % 8, (0, 7))) * 100
print('crest phase x mod 8 / y mod 8 per layer, and the share of crests within one cell of an H1 cell edge (uniform 43.8 %):')
for name, z in layers.items():
    c = np.array(ndimage.maximum_position(z, lab, ti))
    gx, gy = c[:, 1] + x0, c[:, 0] + y0
    print(f'  {name:4s}: x {fmt(share(gx))} | y {fmt(share(gy))} | at an edge {edge(gx, gy):.1f} %')
    for lo, hi in ((0, 50), (50, 500), (500, 1e12)):
        s = (sz >= lo) & (sz < hi)
        print(f'        size {lo:.0f}-{hi:.0g}: {s.sum():5d} trenches, at an edge {edge(gx[s], gy[s]):.1f} %')
# the path cells' own phase (every cell of the trenches), and S_KH's local maxima
on = np.isin(lab, ti)
py, px = np.nonzero(on)
print(f'every trench path cell ({len(px)}): x {fmt(share(px + x0))} | y {fmt(share(py + y0))} | at an edge {edge(px + x0, py + y0):.1f} %')
lm = box & (kh == ndimage.maximum_filter(kh, size=3)) & (s0 > 0)
my, mx = np.nonzero(lm)
print(f'S_KH local maxima ({len(mx)}): x {fmt(share(mx + x0))} | y {fmt(share(my + y0))} | at an edge {edge(mx + x0, my + y0):.1f} %')
lm0 = box & (s0 == ndimage.maximum_filter(s0, size=3)) & (s0 > 0)
my, mx = np.nonzero(lm0)
print(f'S0 local maxima ({len(mx)}): x {fmt(share(mx + x0))} | y {fmt(share(my + y0))} | at an edge {edge(mx + x0, my + y0):.1f} %')
# the crest's form on S0: higher neighbours across the path (saddle) or none (peak)
c = np.array(ndimage.maximum_position(kh, lab, ti))
nbmax = ndimage.maximum_filter(s0, size=3)
nbmin = ndimage.minimum_filter(s0, size=3)
z = s0[c[:, 0], c[:, 1]]
peak = z >= nbmax[c[:, 0], c[:, 1]] - 1e-9
pit = z <= nbmin[c[:, 0], c[:, 1]] + 1e-9
print(f'crest form on S0 (3 x 3): peak {peak.mean() * 100:.1f} %, lowest {pit.mean() * 100:.1f} %, between (saddle or slope) {(~peak & ~pit).mean() * 100:.1f} %')
